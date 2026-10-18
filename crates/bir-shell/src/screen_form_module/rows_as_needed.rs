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
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_0605::Form0605Draft;
use serde_json::{Map, Value};

/// A box this module writes or gates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// 0605 Item 3, quarter 1-4.
    Quarter0605(u8),
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    match kind.form_code() {
        "0605" => {
            let n: u8 = key.strip_prefix("itemQuarter_")?.parse().ok()?;
            (1..=4).contains(&n).then_some(Cell::Quarter0605(n))
        }
        _ => None,
    }
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page).
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    cell(kind, key).is_some()
}

/// Edits of one batch that must run in order: none so far.
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    cell(kind, key).map(|_| 0)
}

fn ticked(value: &Value) -> bool {
    matches!(value, Value::Bool(true)) || value.as_str().is_some_and(|s| s.trim() == "true")
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
    let _ = &ordinary;
    Some(match cell(kind, key)? {
        Cell::Quarter0605(quarter) => fill_0605_quarter(draft, quarter, value),
    })
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

/// What each box the form fills from another box waits for while it shows
/// blank (the page shows it in the box).
pub(super) fn waits(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let _ = (kind, draft);
    Map::new()
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
