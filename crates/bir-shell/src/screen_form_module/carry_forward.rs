//! A new return starts with the answers of the previous period's return of
//! its form for the branch (owner's decision 9): `open_screen_form` calls
//! [`carry_into`] for a brand-new draft, before the draft follows the
//! profile, so profile boxes still win. The branch's template wins over the
//! previous return for each box it pins. Which boxes are copied is
//! bir-core's one rule (`agent::carry`); every value is replayed through the
//! page's own writer ([`super::apply_screen_edits`]), so catalog-written and
//! unmapped boxes (2551Q's ATC selects) come across like a user's pick.
//!
//! The draft records what was filled and from where (`kept`) and the
//! automatic stops (`auto_stops`: the amounts and dates the user filled in
//! the previous return), with its other lifecycle fields: both survive save
//! and reopen. A carry never makes a draft that fails to open: a value the
//! writer refuses is skipped, and a carry that leaves a draft the page cannot
//! read is dropped whole.

use std::collections::{BTreeMap, BTreeSet};

use bir_core::db::Database;
use bir_core::forms::agent::carry;
use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::forms::queueable::QueueableKind;
use serde_json::{Map, Value, json};

use super::{apply_screen_edits, lifecycle_pointer, set_draft_path, values_json};

/// Where a carried answer came from (`kept`'s values).
pub(super) const FROM_LAST: &str = "last";
pub(super) const FROM_TEMPLATE: &str = "template";

/// Each box's value as the page shows it: a radio item as its group box's
/// option value.
fn shown(kind: AgentFormKind, values: &Value) -> BTreeMap<String, String> {
    let map: BTreeMap<String, String> = values
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_string())))
        .collect();
    let def = kind.definition();
    def.boxes
        .iter()
        .map(|entry| (entry.key.clone(), def.box_value(&entry.key, &map)))
        .collect()
}

/// The page values of a draft, computed as the page shows them.
fn page_values(kind: AgentFormKind, draft: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut draft = draft.clone();
    agent::compute(kind, &mut draft)?;
    Ok(shown(kind, &values_json(kind, &draft)?))
}

/// The edit the page sends to set `key` to `value`: a radio item ticks the
/// radio whose option it is.
fn edit_of(kind: AgentFormKind, key: &str, value: &str) -> Option<(String, Value)> {
    let entry = kind.definition().box_def(key)?;
    if entry.radio_group {
        let member = entry
            .options
            .iter()
            .zip(&entry.members)
            .find(|(option, _)| option.value == value)
            .map(|(_, member)| member.clone())?;
        return Some((member, json!("true")));
    }
    Some((key.to_string(), json!(value)))
}

/// Write `edits` to `draft` as the page's fill does, skipping what the
/// writer refuses. A batch the writer fails on is retried box by box.
fn replay(kind: AgentFormKind, draft: &mut Value, edits: &Map<String, Value>) {
    let none = BTreeSet::new();
    let mut batch = draft.clone();
    if apply_screen_edits(kind, &mut batch, edits, true, &none).is_ok() {
        *draft = batch;
        return;
    }
    for (key, value) in edits {
        let mut one_box = draft.clone();
        let one = Map::from_iter([(key.clone(), value.clone())]);
        if let Ok(rejected) = apply_screen_edits(kind, &mut one_box, &one, true, &none)
            && rejected.is_empty()
        {
            *draft = one_box;
        }
    }
}

/// Whether the page can open `draft` (what `opened_json` reads first).
fn opens(kind: AgentFormKind, draft: &Value) -> bool {
    let mut draft = draft.clone();
    agent::compute(kind, &mut draft).is_ok() && values_json(kind, &draft).is_ok()
}

/// An added 2550Q row gets its stable row id, as a page fill's does; `None`
/// when the draft cannot take one.
fn with_row_ids(kind: AgentFormKind, draft: Value) -> Option<Value> {
    if kind != AgentFormKind::Queueable(QueueableKind::Form2550Q) {
        return Some(draft);
    }
    let mut typed: bir_core::forms::form_2550q::Form2550QDraft =
        serde_json::from_value(draft).ok()?;
    bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut typed).ok()?;
    serde_json::to_value(typed).ok()
}

/// Fill the brand-new `draft` of `kind` for branch `tin` and (`year`,
/// `period`) from the branch's template and its previous return, and record
/// `kept` and `auto_stops` on it. Reads only; never fails the open.
pub(super) fn carry_into(
    db: &Database,
    kind: AgentFormKind,
    tin: &str,
    year: u16,
    period: u8,
    draft: &mut Value,
) {
    let code = kind.form_code();
    let previous = match db.previous_return(tin, code, year, u32::from(period)) {
        Ok(previous) => previous,
        Err(error) => {
            tracing::warn!("{code}: reading the previous return failed: {error}");
            None
        }
    };
    let last = previous.as_ref().and_then(|previous| {
        page_values(kind, &previous.draft)
            .inspect_err(|error| {
                tracing::warn!("{code}: reading the previous return's answers failed: {error}")
            })
            .ok()
    });
    let pinned = match db.get_form_template(tin, code) {
        Ok(template) => template
            .map(|template| template.pinned())
            .unwrap_or_default(),
        Err(error) => {
            tracing::warn!("{code}: reading the template failed: {error}");
            BTreeMap::new()
        }
    };
    let Ok(now) = page_values(kind, draft) else {
        return;
    };
    // What each carried box takes: the template's pin, else last period's.
    let mut wanted: BTreeMap<&str, (String, &str)> = BTreeMap::new();
    for key in carry::carried_boxes(kind) {
        let value = pinned
            .get(key)
            .map(|value| (value.clone(), FROM_TEMPLATE))
            .or_else(|| {
                last.as_ref()
                    .and_then(|last| last.get(key))
                    .map(|value| (value.clone(), FROM_LAST))
            });
        if let Some((value, from)) = value
            && !carry::is_blank_value(&value)
        {
            wanted.insert(key, (value, from));
        }
    }
    // Only what the new return does not show already is written.
    let mut edits = Map::new();
    for (key, (value, _)) in &wanted {
        if now.get(*key).map(|shown| shown.trim()) == Some(value.trim()) {
            continue;
        }
        if let Some((edit, value)) = edit_of(kind, key, value) {
            edits.insert(edit, value);
        }
    }
    let mut next = draft.clone();
    if !edits.is_empty() {
        replay(kind, &mut next, &edits);
        next = match with_row_ids(kind, next) {
            Some(prepared) if opens(kind, &prepared) => prepared,
            _ => {
                tracing::warn!("{code}: the carried answers left a draft that does not open");
                draft.clone()
            }
        };
    }
    let after = page_values(kind, &next).unwrap_or_default();
    let kept: BTreeMap<String, String> = wanted
        .iter()
        .filter(|(key, (value, _))| {
            let before = now.get(**key).map(|shown| shown.trim()).unwrap_or("");
            after
                .get(**key)
                .map(|shown| shown.trim())
                .is_some_and(|shown| {
                    !carry::is_blank_value(shown) && (shown == value.trim() || shown != before)
                })
        })
        .map(|(key, (_, from))| (key.to_string(), from.to_string()))
        .collect();
    // The amounts and dates the user filled last period.
    let auto_stops: Vec<String> = match &last {
        Some(last) => carry::stoppable(kind)
            .into_iter()
            .filter(|key| carry::carry_of(kind, key).is_auto_stop())
            .filter(|key| {
                last.get(*key)
                    .is_some_and(|value| !carry::is_blank_value(value))
            })
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    };
    *draft = next;
    if !kept.is_empty() {
        let pointer = lifecycle_pointer(draft, "kept");
        set_draft_path(draft, &pointer, json!(kept));
    }
    if !auto_stops.is_empty() {
        let pointer = lifecycle_pointer(draft, "auto_stops");
        set_draft_path(draft, &pointer, json!(auto_stops));
    }
}

/// The record a draft keeps of its carry: `kept` and `auto_stops`.
pub(super) fn recorded(draft: &Value) -> (Value, Value) {
    let field = |name: &str| draft.pointer(&lifecycle_pointer(draft, name)).cloned();
    let kept = field("kept")
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let auto_stops = field("auto_stops")
        .filter(Value::is_array)
        .unwrap_or_else(|| json!([]));
    (kept, auto_stops)
}

/// Take the carry record off a draft (an amendment is no carry).
pub(super) fn forget(draft: &mut Value) {
    for name in ["kept", "auto_stops"] {
        let pointer = lifecycle_pointer(draft, name);
        if let Some((parent, leaf)) = pointer.rsplit_once('/')
            && let Some(Value::Object(object)) = draft.pointer_mut(parent)
        {
            object.remove(leaf);
        }
    }
}

/// Interface I-2's fields of `screen_form_open`: `kept` and `auto_stops`
/// from the draft, the branch's `stop_edits` for the form and the boxes
/// that may be stops.
pub(super) fn open_fields(
    db: &Database,
    kind: AgentFormKind,
    tin: &str,
    draft: &Value,
) -> [(&'static str, Value); 4] {
    let (kept, auto_stops) = recorded(draft);
    let stop_edits = db
        .get_form_template(tin, kind.form_code())
        .inspect_err(|error| {
            tracing::warn!("{}: reading the template failed: {error}", kind.form_code())
        })
        .ok()
        .flatten()
        .map(|template| template.stop_edits())
        .unwrap_or_default();
    [
        ("kept", kept),
        ("auto_stops", auto_stops),
        ("stop_edits", json!(stop_edits)),
        ("stoppable", json!(carry::stoppable(kind))),
    ]
}
