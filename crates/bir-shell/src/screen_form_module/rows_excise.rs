//! PR 11: the excise returns 2200A, 2200AN, 2200C, 2200M, 2200P, 2200S and
//! 2200T. Their page layout lives in app/forms/schema-overrides.json; this
//! module holds what the host does for them alone.
//!
//! - **Penalties from the typed date (decision D8, 2026-10-18).** The model
//!   measures their penalties from Item 1 (`PenaltyItems::typed_due_date`).
//!   A draft stored before that (penalty_model_version 1) keeps what it
//!   holds: its penalty boxes are "kept as entered before penalties were
//!   computed" until the filer chooses Edit penalties, and blanking them
//!   ("Use the computed amounts") puts the return on the typed due date.
//!   Nothing changes on open, so a stored draft saves back byte for byte.
//! - **Printed item numbers (decision D6).** Where BIR's message names its
//!   program's item number and the sheet prints another, the page shows the
//!   printed one ([`PRINTED_MESSAGES`]); the checks and the filed data are
//!   unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::queueable::{QueueableForm, QueueableKind};
use bir_core::forms::{FilingStatus, form_2200a, form_2200an, form_2200c, form_2200m};
use bir_core::forms::{form_2200p, form_2200s, form_2200t};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

/// The lock of an excise return's penalty box while the draft predates
/// decision D8: the amounts are the ones it was stored with.
pub(super) const KEPT_AS_ENTERED: &str =
    "kept as entered before penalties were computed; choose Edit penalties to change them.";

/// [`KEPT_AS_ENTERED`] as the filer reads it under the item.
pub(super) const NOTE_KEPT: &str = "Kept as entered before penalties were computed.";

/// The excise return `kind` is, if it is one.
fn excise(kind: AgentFormKind) -> Option<QueueableKind> {
    match kind {
        AgentFormKind::Queueable(
            queueable @ (QueueableKind::Form2200A
            | QueueableKind::Form2200AN
            | QueueableKind::Form2200C
            | QueueableKind::Form2200M
            | QueueableKind::Form2200P
            | QueueableKind::Form2200S
            | QueueableKind::Form2200T),
        ) => Some(queueable),
        _ => None,
    }
}

/// Whether `draft` is a Draft whose penalties are measured from a typed
/// date it is not yet on (stored before D8).
#[inline(never)]
fn stored_before_typed_due_date<F: QueueableForm + DeserializeOwned>(draft: &Value) -> bool {
    let Ok(form) = serde_json::from_value::<F>(draft.clone()) else {
        return false;
    };
    matches!(form.lifecycle().status, FilingStatus::Draft)
        && F::penalty_items()
            .iter()
            .any(|items| items.typed_due_date.is_some() && !items.on_typed_due_date(&form))
}

/// Put the draft on its typed due date (`PenaltyItems::join_typed_due_date`).
#[inline(never)]
fn join_typed_due_date<F: QueueableForm + DeserializeOwned>(draft: &mut Value) {
    let Ok(mut form) = serde_json::from_value::<F>(draft.clone()) else {
        return;
    };
    for items in F::penalty_items() {
        items.join_typed_due_date(&mut form);
    }
    if let Ok(value) = serde_json::to_value(&form) {
        *draft = value;
    }
}

/// The penalty controls of the form (`PenaltyItems::controls`).
#[inline(never)]
fn penalty_controls<F: QueueableForm>() -> Vec<&'static str> {
    F::penalty_items()
        .iter()
        .flat_map(|items| items.controls)
        .collect()
}

fn before_d8(kind: QueueableKind, draft: &Value) -> bool {
    match kind {
        QueueableKind::Form2200A => {
            stored_before_typed_due_date::<form_2200a::Form2200ADraft>(draft)
        }
        QueueableKind::Form2200AN => {
            stored_before_typed_due_date::<form_2200an::Form2200ANDraft>(draft)
        }
        QueueableKind::Form2200C => {
            stored_before_typed_due_date::<form_2200c::Form2200CDraft>(draft)
        }
        QueueableKind::Form2200M => {
            stored_before_typed_due_date::<form_2200m::Form2200MDraft>(draft)
        }
        QueueableKind::Form2200P => {
            stored_before_typed_due_date::<form_2200p::Form2200PDraft>(draft)
        }
        QueueableKind::Form2200S => {
            stored_before_typed_due_date::<form_2200s::Form2200SDraft>(draft)
        }
        QueueableKind::Form2200T => {
            stored_before_typed_due_date::<form_2200t::Form2200TDraft>(draft)
        }
        _ => false,
    }
}

fn controls(kind: QueueableKind) -> Vec<&'static str> {
    match kind {
        QueueableKind::Form2200A => penalty_controls::<form_2200a::Form2200ADraft>(),
        QueueableKind::Form2200AN => penalty_controls::<form_2200an::Form2200ANDraft>(),
        QueueableKind::Form2200C => penalty_controls::<form_2200c::Form2200CDraft>(),
        QueueableKind::Form2200M => penalty_controls::<form_2200m::Form2200MDraft>(),
        QueueableKind::Form2200P => penalty_controls::<form_2200p::Form2200PDraft>(),
        QueueableKind::Form2200S => penalty_controls::<form_2200s::Form2200SDraft>(),
        QueueableKind::Form2200T => penalty_controls::<form_2200t::Form2200TDraft>(),
        _ => Vec::new(),
    }
}

/// [`KEPT_AS_ENTERED`] for a penalty box of an excise return stored before
/// D8, `None` for every other box and form.
pub(super) fn penalty_lock(kind: AgentFormKind, draft: &Value, key: &str) -> Option<&'static str> {
    let kind = excise(kind)?;
    (controls(kind).contains(&key) && before_d8(kind, draft)).then_some(KEPT_AS_ENTERED)
}

/// Whether a penalty group's boxes are kept as entered: the page offers
/// "Use the computed amounts" as soon as Edit opens them.
pub(super) fn kept_as_entered(kind: AgentFormKind, keys: &[String], draft: &Value) -> bool {
    keys.iter()
        .any(|key| penalty_lock(kind, draft, key).is_some())
}

/// A blank written to a penalty box of an excise return stored before D8
/// ("Use the computed amounts") puts it on the typed due date, so the
/// amounts computed from Item 1 file.
pub(super) fn join_model(kind: AgentFormKind, draft: &mut Value, key: &str) {
    let Some(kind) = excise(kind) else {
        return;
    };
    if !controls(kind).contains(&key) || !before_d8(kind, draft) {
        return;
    }
    match kind {
        QueueableKind::Form2200A => join_typed_due_date::<form_2200a::Form2200ADraft>(draft),
        QueueableKind::Form2200AN => join_typed_due_date::<form_2200an::Form2200ANDraft>(draft),
        QueueableKind::Form2200C => join_typed_due_date::<form_2200c::Form2200CDraft>(draft),
        QueueableKind::Form2200M => join_typed_due_date::<form_2200m::Form2200MDraft>(draft),
        QueueableKind::Form2200P => join_typed_due_date::<form_2200p::Form2200PDraft>(draft),
        QueueableKind::Form2200S => join_typed_due_date::<form_2200s::Form2200SDraft>(draft),
        QueueableKind::Form2200T => join_typed_due_date::<form_2200t::Form2200TDraft>(draft),
        _ => {}
    }
}

/// The penalty locks of an excise return as this fill leaves them, for the
/// page to apply at once (`relocks`): a group still computed is locked as
/// computed, one kept as entered says so, an edited one is open (`null`).
/// Empty for every other form.
pub(super) fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let mut locks = Map::new();
    if excise(kind).is_none() {
        return locks;
    }
    for group in super::penalties::groups(kind) {
        let edited = super::penalties::edited(&group, draft);
        for key in &group.keys {
            let lock = if edited {
                Value::Null
            } else if !super::penalties::applies(kind, &group, draft) {
                json!(super::penalties::NOT_ON_RETURN)
            } else {
                json!(penalty_lock(kind, draft, key).unwrap_or(super::penalties::LOCK))
            };
            locks.insert(key.clone(), lock);
        }
    }
    locks
}

/// The 2200S penalty items the draft holds as entered (an override set).
fn entered_2200s_penalties(draft: &Value) -> usize {
    ["surcharge", "interest", "compromise"]
        .iter()
        .filter(|component| draft["penalty_overrides"][**component].is_number())
        .count()
}

/// What an edit removed that the filer typed, for the page to ask first and
/// offer Undo (the `removes` of the fill response). 2200S Items 23A-23C are
/// open on a prepayment only (form_2200s.rs, disablePenalties): an answer
/// at Part II that leaves Item 16 takes the penalties entered there off the
/// return.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    if kind.form_code() != "2200S"
        || !edits
            .keys()
            .any(|key| key.starts_with("frm2200S:chkPymntManner"))
    {
        return None;
    }
    let entered = entered_2200s_penalties(before);
    (entered > 0 && entered_2200s_penalties(after) == 0).then(|| {
        json!({
            "message": "Items 23A-23C are on a prepayment only (item 16): this answer removes the penalties you entered there. Continue?",
            "done": "Removed the penalties entered at items 23A-23C",
            "rows": entered,
        })
    })
}

/// Decision D6: `(form, BIR's message, the same message naming the printed
/// item)` where the official program numbers an item differently from the
/// sheet (html-frozen/<slug>/writer-cells.json captions). 2200AN's checks
/// still use the 2013 layout's numbers; 2200M's and 2200P's are 2200C's.
pub(super) const PRINTED_MESSAGES: &[(&str, &str, &str)] = &[
    (
        "2200AN",
        "Please enter a valid TIN number on Item 5.",
        "Please enter a valid TIN number on Item 4.",
    ),
    (
        "2200AN",
        "Please enter a valid RDO Code on Item 6.",
        "Please enter a valid RDO Code on Item 5.",
    ),
    (
        "2200AN",
        "Please enter a valid Taxpayer Name on Item 8.",
        "Please enter a valid Taxpayer Name on Item 6.",
    ),
    (
        "2200AN",
        "Please enter Taxpayer's Registered Address on Item 10.",
        "Please enter Taxpayer's Registered Address on Item 7.",
    ),
    (
        "2200AN",
        "Please enter Taxpayer's Zip Code on Item 11.",
        "Please enter Taxpayer's Zip Code on Item 7A.",
    ),
    (
        "2200AN",
        "Please enter a valid Telephone Number on Item 9.",
        "Please enter a valid Telephone Number on Item 8.",
    ),
    (
        "2200M",
        "Please enter a valid RDO Code on Page 1 Item 6.",
        "Please enter a valid RDO Code on Page 1 Item 5.",
    ),
    (
        "2200M",
        "Name field on Page 1 Item 7 is required.",
        "Name field on Page 1 Item 6 is required.",
    ),
    (
        "2200M",
        "Registered Address field on Page 1 Item 8 is required.",
        "Registered Address field on Page 1 Item 7 is required.",
    ),
    (
        "2200M",
        "Zip Code field on Page 1 Item 8A is required.",
        "Zip Code field on Page 1 Item 7A is required.",
    ),
    (
        "2200M",
        "Contact Number field on Page 1 Item 9 is required.",
        "Contact Number field on Page 1 Item 8 is required.",
    ),
    (
        "2200M",
        "E-mail address on page 1 item 10 is required.",
        "E-mail address on page 1 item 9 is required.",
    ),
    (
        "2200M",
        "Please enter a valid e-mail address on page 1 item 10",
        "Please enter a valid e-mail address on page 1 item 9",
    ),
    (
        "2200P",
        "Please enter a valid RDO Code on Page 1 Item 6.",
        "Please enter a valid RDO Code on Page 1 Item 5.",
    ),
    (
        "2200P",
        "Name field on Page 1 Item 7 is required.",
        "Name field on Page 1 Item 6 is required.",
    ),
    (
        "2200P",
        "Registered Address field on Page 1 Item 8 is required.",
        "Registered Address field on Page 1 Item 7 is required.",
    ),
    (
        "2200P",
        "Zip Code field on Page 1 Item 8A is required.",
        "Zip Code field on Page 1 Item 7A is required.",
    ),
    (
        "2200P",
        "Contact Number field on Page 1 Item 9 is required.",
        "Contact Number field on Page 1 Item 8 is required.",
    ),
    (
        "2200P",
        "E-mail address on page 1 item 10 is required.",
        "E-mail address on page 1 item 9 is required.",
    ),
    (
        "2200P",
        "Please enter a valid e-mail address on page 1 item 10",
        "Please enter a valid e-mail address on page 1 item 9",
    ),
];

/// The printed-number wording of BIR's `message` on `kind`, if D6
/// rewrites it.
pub(super) fn printed_message(kind: AgentFormKind, message: &str) -> Option<&'static str> {
    let code = kind.form_code();
    PRINTED_MESSAGES
        .iter()
        .find(|(form, official, _)| *form == code && *official == message)
        .map(|(_, _, printed)| *printed)
}
