//! PR 5: the quarterly income tax returns (1701Q, 1702Q). What the host
//! writes or closes on these pages, beyond the probed bindings:
//!
//! - 1701Q Civil Status (maintainer decision 11). The January 2018 sheet has
//!   no such item; the page asks it at the head of Part II
//!   ([`CIVIL_STATUS`], a page box with no official key) and
//!   `Form1701QDraft::set_civil_status` keeps the answer: Married opens
//!   Part II and the spouse's column B, another answer closes them and
//!   clears what was entered there. While they are closed (not Married, or
//!   an estate or trust at item 7, the official `disableSpouse`) a write
//!   there is refused in the filer's words and rule 6 fades the boxes with
//!   what opens them ([`gated`]). A draft stored before the answer existed
//!   shows Married when it reports a spouse and is never changed on open.
//! - 1701Q item 17, the spouse's TIN: one draft field shown over four boxes,
//!   written box by box the way 1700's item 18 is (`rows_1700`).
//! - 1701Q items 43 and 48's "(specify)" boxes, shared by both columns,
//!   close while no party files their schedule (the compute keeps the text).
//! - 1702Q's schedules need item 5's ATC: the form's own `recompute` clears
//!   them otherwise, so rule 6 fades them through the conditions the corpus
//!   leaves out (`conditions.rs` CORPUS_GAPS_PR5); nothing here writes them.
//! - An edit that removes what the filer typed elsewhere (Civil Status off
//!   Married, an item 7 estate or trust, an ATC, rate or deduction method
//!   that closes a schedule, Amended Return No, 1702Q's item 5 cleared) is
//!   reported ([`removes`]) so the page asks first and offers Undo.
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged; the civil status is not filed.

use std::collections::BTreeMap;

use bir_core::forms::agent::{self, AgentFormKind, Binding};
use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QCivilStatus, Form1701QDraft, Form1701QFilerType, Form1701QTaxRate,
};
use serde_json::{Map, Value, json};

/// The page box of 1701Q's Civil Status. Not an official control: the
/// upload, the print and the field map never see it.
pub(crate) const CIVIL_STATUS: &str = "page:1701Q:civilStatus";

/// What a closed spouse part says it needs (decision 11's words).
pub(crate) const NEEDS_MARRIED: &str = "Needs Civil Status = Married";
/// What it needs on an estate's or trust's return.
pub(crate) const NEEDS_INDIVIDUAL: &str = "Needs item 7 = Single Proprietor or Professional";

/// The refusal of a write to a closed spouse part, in the filer's words.
pub(crate) const MARRIED_FIRST: &str =
    "Part II and column B are for a married filer: choose Married at Civil Status first.";
/// The refusal on an estate's or trust's return.
pub(crate) const INDIVIDUAL_FIRST: &str = "An estate or trust reports no spouse: Part II and column B are for a Single Proprietor or Professional (item 7).";

/// What item 43's and item 48's "(specify)" boxes say they need: the tags
/// of their conditions (app/forms/condition-labels.json).
pub(crate) const NEEDS_GRADUATED: &str = "Needs the graduated income tax rate";
pub(crate) const NEEDS_EIGHT: &str = "Needs the 8% income tax rate";
/// Their refusals.
pub(crate) const GRADUATED_FIRST: &str =
    "Item 43 is Schedule I's: choose Graduated Rates at item 16 (or item 25 for the spouse) first.";
pub(crate) const EIGHT_FIRST: &str =
    "Item 48 is Schedule II's: choose the 8% rate at item 16 (or item 25 for the spouse) first.";

/// A compensation earner only (item 20 II011) files no Schedule IV: what
/// the spouse's penalty boxes say then, and their refusal.
pub(crate) const NEEDS_NOT_II011: &str = "Needs item 20 other than II011 Compensation Income";
pub(crate) const NOT_II011_FIRST: &str = "A spouse who is a compensation earner only (item 20 II011) files no penalties here: choose the spouse's other ATC at item 20 first.";

/// Whether `reason` is this module's refusal of a box its answer closes.
#[cfg(test)]
pub(super) fn is_gate_refusal(reason: &str) -> bool {
    [
        MARRIED_FIRST,
        INDIVIDUAL_FIRST,
        GRADUATED_FIRST,
        EIGHT_FIRST,
        NOT_II011_FIRST,
    ]
    .contains(&reason)
}

/// Part II's boxes (items 17-25A).
pub(crate) const SPOUSE_PART: [&str; 24] = [
    "frm1701q:txtSpouseTIN1",
    "frm1701q:txtSpouseTIN2",
    "frm1701q:txtSpouseTIN3",
    "frm1701q:txtSpouseBranchCode",
    "frm1701q:txtSpouseRDOCode",
    "frm1701q:optSpouseType_1",
    "frm1701q:optSpouseType_2",
    "frm1701q:optSpouseType_3",
    "frm1701q:optSpouseATC_1",
    "frm1701q:optSpouseATC_2",
    "frm1701q:optSpouseATC_3",
    "frm1701q:optSpouseATC_4",
    "frm1701q:optSpouseATC_5",
    "frm1701q:optSpouseATC_6",
    "frm1701q:optSpouseATC_7",
    "frm1701q:txtSpouseName",
    "frm1701q:txtSpouseCitizenship",
    "frm1701q:txtSpouseForeignTaxNum",
    "frm1701q:optSpouseForeignTaxCred_1",
    "frm1701q:optSpouseForeignTaxCred_2",
    "frm1701q:optSpouseTaxRate_1",
    "frm1701q:optSpouseTaxRate_2",
    "frm1701q:optSpouseMethod:_1",
    "frm1701q:optSpouseMethod:_2",
];

/// The spouse's column B items the filer types (Schedules I-IV).
pub(crate) const SPOUSE_COLUMN: [u8; 20] = [
    36, 37, 39, 42, 43, 44, 47, 48, 50, 52, 55, 56, 57, 58, 59, 60, 61, 64, 65, 66,
];

/// A box this module writes or closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// 1701Q's Civil Status (a page box).
    CivilStatus,
    /// 1701Q item 17: TIN box 0-2 or 3 (the branch code).
    SpouseTin(usize),
    /// Any other box of Part II or column B: written by the ordinary fill
    /// while the spouse part is open.
    SpousePart,
    /// Items 64B-66B: as `SpousePart`, and closed too for a compensation
    /// earner only, whose column files no penalties (the shared model's
    /// spouse items do not apply), so the page's penalty lock never offers
    /// an Edit that cannot file.
    SpousePenalty,
    /// Item 43's (`true`) or item 48's "(specify)" box, shared by both
    /// columns: open while either party files that schedule (the official
    /// enableSchedule1/2; the compute keeps the text, so the host closes it).
    Specify(bool),
}

fn is_1701q(kind: AgentFormKind) -> bool {
    kind.form_code() == "1701Q"
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    if !is_1701q(kind) {
        return None;
    }
    if key == CIVIL_STATUS {
        return Some(Cell::CivilStatus);
    }
    match key {
        "frm1701q:txtSpouseTIN1" => return Some(Cell::SpouseTin(0)),
        "frm1701q:txtSpouseTIN2" => return Some(Cell::SpouseTin(1)),
        "frm1701q:txtSpouseTIN3" => return Some(Cell::SpouseTin(2)),
        "frm1701q:txtSpouseBranchCode" => return Some(Cell::SpouseTin(3)),
        _ => {}
    }
    if SPOUSE_PART.contains(&key) {
        return Some(Cell::SpousePart);
    }
    match key {
        "frm1701q:txt43Desc" => return Some(Cell::Specify(true)),
        "frm1701q:txt48Desc" => return Some(Cell::Specify(false)),
        _ => {}
    }
    let item: u8 = key
        .strip_prefix("frm1701q:txt")?
        .strip_suffix('B')?
        .parse()
        .ok()?;
    if (64..=66).contains(&item) {
        return Some(Cell::SpousePenalty);
    }
    SPOUSE_COLUMN.contains(&item).then_some(Cell::SpousePart)
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page): the Civil Status and the spouse's TIN.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    matches!(
        cell(kind, key),
        Some(Cell::CivilStatus | Cell::SpouseTin(_))
    )
}

/// Civil Status first, then the spouse's TIN boxes in order, then the rest
/// of the spouse part, so one batch can answer Married and fill Part II.
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    match cell(kind, key)? {
        Cell::CivilStatus | Cell::Specify(_) => None,
        Cell::SpouseTin(part) => Some(1 + part),
        Cell::SpousePart | Cell::SpousePenalty => Some(5),
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::Bool(flag) => flag.to_string(),
        other => other.as_str().unwrap_or_default().trim().to_string(),
    }
}

fn typed(draft: &Value) -> Result<Form1701QDraft, String> {
    serde_json::from_value(draft.clone()).map_err(|e| e.to_string())
}

fn store(draft: &mut Value, form: &Form1701QDraft) -> Result<(), String> {
    *draft = serde_json::to_value(form).map_err(|e| e.to_string())?;
    Ok(())
}

/// A write that says nothing (blank, zero, unticked), which a closed box
/// takes: the page clears what it shows.
fn says_nothing(value: &Value) -> bool {
    let shown = text(value);
    shown.is_empty()
        || shown == "false"
        || shown.chars().all(|c| matches!(c, '0' | '.' | ',' | '-'))
}

/// Why the spouse part is closed on this draft, or `None` while it is open.
fn spouse_closed(form: &Form1701QDraft) -> Option<(&'static str, &'static str)> {
    if matches!(
        form.filer_type,
        Some(Form1701QFilerType::Estate | Form1701QFilerType::Trust)
    ) {
        Some((INDIVIDUAL_FIRST, NEEDS_INDIVIDUAL))
    } else if !form.has_spouse {
        Some((MARRIED_FIRST, NEEDS_MARRIED))
    } else {
        None
    }
}

/// Why item 43's (`graduated`) or item 48's "(specify)" box is closed: no
/// party files that schedule (the taxpayer at item 16, a married spouse who
/// is not a compensation earner only at item 25), or `None` while one does.
fn specify_closed(form: &Form1701QDraft, graduated: bool) -> Option<(&'static str, &'static str)> {
    let rate = if graduated {
        Form1701QTaxRate::Graduated
    } else {
        Form1701QTaxRate::EightPercent
    };
    let spouse = spouse_closed(form).is_none()
        && form.spouse_atc != Some(Form1701QAtc::Ii011)
        && form.spouse_tax_rate == Some(rate);
    if form.tax_rate == Some(rate) || spouse {
        None
    } else if graduated {
        Some((GRADUATED_FIRST, NEEDS_GRADUATED))
    } else {
        Some((EIGHT_FIRST, NEEDS_EIGHT))
    }
}

/// Why a box this module closes is closed on this draft: `(refusal, tag)`,
/// or `None` while it is open (and for every other box).
fn closed(cell: Cell, form: &Form1701QDraft) -> Option<(&'static str, &'static str)> {
    match cell {
        Cell::SpouseTin(_) | Cell::SpousePart => spouse_closed(form),
        Cell::SpousePenalty => spouse_closed(form).or_else(|| {
            (form.spouse_atc == Some(Form1701QAtc::Ii011))
                .then_some((NOT_II011_FIRST, NEEDS_NOT_II011))
        }),
        Cell::Specify(graduated) => specify_closed(form, graduated),
        Cell::CivilStatus => None,
    }
}

/// `Some(tag)` for a box the host closes while its answer does not hold
/// (1701Q's spouse part while the filer is not married, or files for an
/// estate or trust; items 43 and 48's "(specify)" while no party files
/// their schedule), `None` for every other box. Rule 6 fades it with the
/// tag whatever it shows: a legacy draft's value there is faded, not hidden.
pub(super) fn gated(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
    let cell = cell(kind, key)?;
    let form = typed(draft).ok()?;
    closed(cell, &form).map(|(_, tag)| tag)
}

/// The refusal of a write to a closed spouse part (`None` for every other
/// write): the page's fill asks it before any other lock of the box, so a
/// spouse penalty says what opens it rather than "Edit penalties".
pub(super) fn closed_refusal(
    kind: AgentFormKind,
    draft: &Value,
    key: &str,
    value: &Value,
) -> Option<&'static str> {
    let cell = cell(kind, key)?;
    if says_nothing(value) {
        return None;
    }
    closed(cell, &typed(draft).ok()?).map(|(refusal, _)| refusal)
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
    Some(fill_cell(draft, cell, value, ordinary))
}

fn fill_cell(
    draft: &mut Value,
    cell: Cell,
    value: &Value,
    ordinary: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let mut form = typed(draft)?;
    if cell == Cell::CivilStatus {
        let answer = text(value);
        let status = if answer.is_empty() {
            None
        } else {
            Some(Form1701QCivilStatus::from_label(&answer).ok_or_else(|| {
                format!(
                    "`{answer}` is not a civil status: choose Single, Married, Legally Separated or Widow/er."
                )
            })?)
        };
        form.set_civil_status(status);
        return store(draft, &form);
    }
    if let Some((refusal, _)) = closed(cell, &form) {
        // Clearing a closed box is taken (it files nothing either way).
        if !says_nothing(value) {
            return Err(refusal.to_string());
        }
    }
    match cell {
        Cell::SpouseTin(part) => {
            form.spouse_tin =
                super::rows_1700::with_tin_part(&form.spouse_tin, part, &text(value))?;
            store(draft, &form)
        }
        _ => ordinary(draft),
    }
}

/// What each page box of this module shows: the Civil Status answer, and
/// item 17's TIN boxes as the draft holds the digits (the field map writes
/// a box only once it is whole, so a box typed digit by digit would show
/// nothing until then). Display only: the field map, upload and print are
/// unchanged.
pub(super) fn values(kind: AgentFormKind, draft: &Value, map: &mut BTreeMap<String, String>) {
    if !is_1701q(kind) {
        return;
    }
    let Ok(form) = typed(draft) else {
        return;
    };
    let shown = form
        .shown_civil_status()
        .map_or("", Form1701QCivilStatus::label);
    map.insert(CIVIL_STATUS.to_string(), shown.to_string());
    if spouse_closed(&form).is_none() {
        let digits: String = form
            .spouse_tin
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        let slice = |from: usize, to: usize| {
            digits
                .get(from.min(digits.len())..to.min(digits.len()))
                .unwrap_or("")
                .to_string()
        };
        for (key, (from, to)) in [
            ("frm1701q:txtSpouseTIN1", (0, 3)),
            ("frm1701q:txtSpouseTIN2", (3, 6)),
            ("frm1701q:txtSpouseTIN3", (6, 9)),
            ("frm1701q:txtSpouseBranchCode", (9, digits.len())),
        ] {
            map.insert(key.to_string(), slice(from, to));
        }
    }
}

/// The spouse's penalty boxes (64B-66B) while their column is not part of
/// the return: rule 6 fades them with what opens them (`gated` or the
/// form's own clearing), so no penalty lock stands beside the tag.
pub(super) fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
    if !is_1701q(kind) {
        return;
    }
    let Ok(form) = typed(draft) else {
        return;
    };
    let spouse = &Form1701QDraft::penalty_declarations()[1];
    if !(spouse.applies)(&form) {
        for key in spouse.controls {
            locks.remove(key);
        }
    }
}

/// A typed entry's printed item for a message: `36A`, `21`, or the
/// corpus's item (`Schedule 2 Item 1` -> `Schedule 2 item 1`).
fn item_name(kind: AgentFormKind, key: &str) -> String {
    let tail = key.rsplit(':').next().unwrap_or(key);
    if is_1701q(kind)
        && let Some(item) = tail
            .strip_prefix("txt")
            .filter(|rest| rest.ends_with(['A', 'B']))
            .filter(|rest| rest[..rest.len() - 1].chars().all(|c| c.is_ascii_digit()))
    {
        return item.to_string();
    }
    kind.definition()
        .box_def(key)
        .map(|entry| entry.item.replace(" Item ", " item "))
        .filter(|item| !item.is_empty())
        .unwrap_or_else(|| tail.to_string())
}

/// The amounts and texts the filer typed, as the page shows them: bound
/// boxes the form does not compute, plus the spouse's TIN boxes.
fn typed_entries(kind: AgentFormKind, draft: &Value) -> BTreeMap<String, String> {
    let def = kind.definition();
    let Ok(map) = agent::field_map(kind, draft) else {
        return BTreeMap::new();
    };
    let typed_box = |key: &str| {
        let binding = def.bindings.get(key).map(|bound| &bound.binding);
        // A penalty box shows the computed amount until the filer types
        // one (its override, rule 8): only that is typed.
        if let Some(Binding::Amount { path, .. }) = binding
            && path.contains("/penalty_overrides/")
        {
            return draft.pointer(path).is_some_and(|value| !value.is_null());
        }
        let bound = matches!(
            binding,
            Some(Binding::Amount { .. } | Binding::Whole { .. } | Binding::Text { .. })
        ) && def.box_def(key).is_some_and(|entry| !entry.computed);
        bound || matches!(cell(kind, key), Some(Cell::SpouseTin(_)))
    };
    map.into_iter()
        .filter(|(key, _)| typed_box(key))
        .filter(|(_, shown)| !says_nothing(&json!(shown)))
        .collect()
}

/// When `edits` turned `before` into `after` by removing what the filer
/// typed in other boxes, what the page asks first (`message`), what the
/// Undo toast says (`done`) and how many boxes went; `None` when nothing
/// typed went. The host applies the edit either way; the page keeps its
/// old draft until the filer chooses Continue.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    if !matches!(kind.form_code(), "1701Q" | "1702Q") {
        return None;
    }
    let had = typed_entries(kind, before);
    let has = typed_entries(kind, after);
    let lost: Vec<&String> = had
        .keys()
        .filter(|key| !edits.contains_key(*key) && !has.contains_key(*key))
        .collect();
    if lost.is_empty() {
        return None;
    }
    let n = lost.len();
    let done = format!("Removed {n} entr{}", if n == 1 { "y" } else { "ies" });
    if edits.contains_key(CIVIL_STATUS) {
        return Some(json!({
            "message": format!(
                "Without Married at Civil Status, the spouse's part goes: this removes {n} entr{} you made in Part II and column B. Continue?",
                if n == 1 { "y" } else { "ies" }
            ),
            "done": "Cleared the spouse's part",
            "rows": n,
        }));
    }
    let mut items: Vec<String> = Vec::new();
    for key in &lost {
        let item = item_name(kind, key);
        if !items.contains(&item) {
            items.push(item);
        }
    }
    let shown = if items.len() > 4 {
        format!("{} and {} more", items[..3].join(", "), items.len() - 3)
    } else if items.len() > 1 {
        format!(
            "{} and {}",
            items[..items.len() - 1].join(", "),
            items[items.len() - 1]
        )
    } else {
        items[0].clone()
    };
    Some(json!({
        "message": format!(
            "This answer clears what you typed in item{} {shown}. Continue?",
            if items.len() == 1 { "" } else { "s" }
        ),
        "done": done,
        "rows": n,
    }))
}

/// 1701Q item 11's month, day and year boxes are one date on the page
/// (`one_date`: the page shows "May 17, 1980" and writes the three boxes
/// together); the draft and the upload keep the three.
pub(super) fn mark_one_date(form: &mut Value, code: &str) {
    if code != "1701Q" {
        return;
    }
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let keys: Vec<&str> = item["f"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|field| field["k"].as_str())
            .collect();
        if keys
            == [
                "frm1701q:txtBirthMonth",
                "frm1701q:txtBirthDay",
                "frm1701q:txtBirthYear",
            ]
        {
            item["one_date"] = Value::Bool(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_1701q_spouse_boxes_only() {
        let q = AgentFormKind::from_code("1701Q").unwrap();
        assert_eq!(cell(q, CIVIL_STATUS), Some(Cell::CivilStatus));
        assert_eq!(
            cell(q, "frm1701q:txtSpouseBranchCode"),
            Some(Cell::SpouseTin(3))
        );
        assert_eq!(cell(q, "frm1701q:txt36B"), Some(Cell::SpousePart));
        assert_eq!(cell(q, "frm1701q:txt66B"), Some(Cell::SpousePenalty));
        assert_eq!(cell(q, "frm1701q:txtSpouseName"), Some(Cell::SpousePart));
        // Computed B lines and the taxpayer's column are not.
        assert_eq!(cell(q, "frm1701q:txt38B"), None);
        assert_eq!(cell(q, "frm1701q:txt36A"), None);
        let other = AgentFormKind::from_code("1700").unwrap();
        assert_eq!(cell(other, "frm1701q:txt36B"), None);
    }

    #[test]
    fn item_names_are_the_printed_ones() {
        let q = AgentFormKind::from_code("1701Q").unwrap();
        assert_eq!(item_name(q, "frm1701q:txt36B"), "36B");
        assert_eq!(item_name(q, "frm1701q:txt43Desc"), "43");
        let c = AgentFormKind::from_code("1702Q").unwrap();
        assert_eq!(item_name(c, "frm1702q:Sched2:txtTax1"), "Schedule 2 item 1");
    }
}
