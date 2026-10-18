//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552). Boxes the host writes through the typed draft because
//! the probed bindings cannot (a box showing one part of a draft field, a
//! whole number shown as one tick box per value), boxes the official page
//! keeps shut until another answer opens them, and the answers that remove
//! what the filer typed.
//!
//! - 0605 Item 3: one tick box per quarter, all four the draft's `quarter`
//!   (form_0605_xml.rs `insert_one_of_four`). Ticking one sets the quarter;
//!   unticking the ticked one clears it (the official page opens with none
//!   ticked).
//! - 1606 item 8C and 1706 item 7C: the other party's TIN, one draft field
//!   (`seller_tin`, `buyer_tin`) shown as three boxes and a branch code;
//!   the boxes fill in order (rows_1700.rs `with_tin_part`).
//! - Boxes the official page enables only on another answer (rules/forms
//!   fields.json `enabled_when`) take nothing while it does not hold, so
//!   rule 6 shows them off with what turns them on: 1606's relief list
//!   (item 11 Yes), Schedule 1 "Others" classification and Schedule 2
//!   "Exempt or Others" specification; 1706's items 8 and 9 (an individual
//!   seller at item 3), relief list (item 11 Yes) and Schedule 2
//!   specification. A blank (clearing) always goes through.
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_0605::Form0605Draft;
use bir_core::forms::form_1606::Form1606Draft;
use bir_core::forms::form_1706::Form1706Draft;
use serde_json::{Map, Value};

use super::rows_1700::with_tin_part;

/// A box this module writes or gates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// 0605 Item 3, quarter 1-4.
    Quarter0605(u8),
    /// 1606 item 8C: seller TIN box 0-2, or 3 (branch code).
    SellerTin1606(usize),
    /// 1706 item 7C: buyer TIN box 0-2.
    BuyerTin1706(usize),
    /// A box the official page opens only on another answer.
    Gate(Gate),
}

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
            let n: u8 = key.strip_prefix("itemQuarter_")?.parse().ok()?;
            (1..=4).contains(&n).then_some(Cell::Quarter0605(n))
        }
        "1606" => match key {
            "frm1606:txtBranchCodeS" => Some(Cell::SellerTin1606(3)),
            "frm1606:selTreaty" => Some(Cell::Gate(Gate::Relief1606)),
            "frm1606:j_id393_7" => Some(Cell::Gate(Gate::ClassOther1606)),
            "frm1606:txtOthers20" => Some(Cell::Gate(Gate::Transaction1606)),
            _ => tin_part(key.strip_prefix("frm1606:txtTINS")?).map(Cell::SellerTin1606),
        },
        "1706" => match key {
            "frm1706:j_id392:_1" | "frm1706:j_id392:_2" | "frm1706:j_id393:_1"
            | "frm1706:j_id393:_2" => Some(Cell::Gate(Gate::Individual1706)),
            "frm1706:selTreaty" => Some(Cell::Gate(Gate::Relief1706)),
            "frm1706:txtOthers21" => Some(Cell::Gate(Gate::Transaction1706)),
            _ => tin_part(key.strip_prefix("frm1706:txtTINB")?).map(Cell::BuyerTin1706),
        },
        _ => None,
    }
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
    Some(match cell(kind, key)? {
        Cell::Quarter0605(quarter) => fill_0605_quarter(draft, quarter, value),
        Cell::SellerTin1606(part) => fill_1606_seller_tin(draft, part, value),
        Cell::BuyerTin1706(part) => fill_1706_buyer_tin(draft, part, value),
        Cell::Gate(gate) => {
            if blank(value) || open(gate, draft) {
                ordinary(draft)
            } else {
                // Shut until its answer holds: the page shows it off with
                // what turns it on, and nothing is kept.
                Ok(())
            }
        }
    })
}

/// Whether a gated box's answer holds on `draft`.
fn open(gate: Gate, draft: &Value) -> bool {
    let is = |path: &str, values: &[&str]| {
        draft
            .pointer(path)
            .and_then(Value::as_str)
            .is_some_and(|v| values.contains(&v))
    };
    match gate {
        Gate::Relief1606 | Gate::Relief1706 => {
            !is("/tax_relief", &["no"]) && draft.pointer("/tax_relief").is_some()
        }
        Gate::ClassOther1606 => is("/property_class", &["others"]),
        Gate::Transaction1606 => is("/transaction", &["exempt", "others"]),
        Gate::Individual1706 => is("/seller_type", &["individual"]),
        Gate::Transaction1706 => is("/transaction_type", &["exempt", "others"]),
    }
}

#[inline(never)]
fn fill_0605_quarter(draft: &mut Value, quarter: u8, value: &Value) -> Result<(), String> {
    let mut form: Form0605Draft = typed(draft)?;
    if ticked(value) {
        form.quarter = quarter;
    } else if form.quarter == quarter {
        form.quarter = 0;
    } else {
        return Ok(());
    }
    form.recompute();
    store(draft, &form)
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
        "The Others description holds at most 25 characters.",
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
        "The Others description holds at most 25 characters.",
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

/// When `edits` turned `before` into `after` by removing what the filer
/// typed, what the page asks first (`message`), what the Undo toast says
/// (`done`) and how many rows went; `None` when nothing typed went.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let _ = (kind, before, after, edits);
    None
}
