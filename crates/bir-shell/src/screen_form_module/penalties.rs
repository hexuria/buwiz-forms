//! Penalties (rule 8): computed, but the filer can override them.
//!
//! A form on the shared penalty model (`bir_core::forms::penalty_items`)
//! binds its Surcharge, Interest and Compromise boxes to the draft's
//! per-component overrides (`<party>/penalty_overrides/surcharge`, ...):
//! with no override the box shows what the model computed from the due
//! date, and an override files instead. This is the one hook every such
//! form shares, read from the bindings:
//!
//! - The three boxes of a party are one group. While no override is set the
//!   group is locked and shows the computed amounts; the page's one Edit
//!   opens all three (it sends them as `unlocked`).
//! - Once an override is set the group stays open, and its Total Penalties
//!   and amount payable recompute from what was entered (the model does
//!   that). A blank box clears its override: "Use the computed amounts"
//!   blanks all three.
//!
//! - 0605 (decided 2026-10-17) has no due date to compute a penalty from,
//!   so its three boxes (20A-20C) are plain open boxes: no lock and no
//!   Edit. They still write the overrides, and 20D Total Penalties still
//!   adds them up. Every other form keeps its locked group and Edit.
//!
//! Validation, the assistant's `form.fill` and the XML are untouched; the
//! XML carries whatever the filer entered, which is the point of the rule.

use bir_core::forms::agent::{AgentFormKind, Binding};
use serde_json::{Value, json};

/// Why a computed penalty box is locked on the page.
pub(super) const LOCK: &str =
    "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to";

/// Why a penalty group that is not part of the return (a spouse's column
/// with no spouse) is locked: amounts entered there would not file.
pub(super) const NOT_ON_RETURN: &str = "computed; not part of this return as it stands";

/// Why 2551Q's Items 20-22 are locked on an 8% return: the official page
/// zeroes and disables them (`taxRateOption()`), so they take no amount.
pub(super) const EIGHT_PERCENT: &str = "nil on this return: Item 13 is the 8% income tax rate, which the official form files with no penalties";

/// Why a 2551Q penalty box on an 8% return still shows an amount: a draft
/// saved with the old manual switch keeps what it filed (bir-core
/// `adopt_legacy_manual_penalties`), and a blank clears it.
pub(super) const KEPT_ON_EIGHT_PERCENT: &str = "Kept from an earlier version of this return. On the 8% rate the official form files no penalties; clear it to follow the official form.";

/// Whether `draft` is a 2551Q return on Item 13 = 8%, whose Items 20-22
/// take no amount.
pub(super) fn eight_percent(kind: AgentFormKind, draft: &Value) -> bool {
    kind == AgentFormKind::Form2551Q && draft["item_13_election"] == "eight_percent"
}

/// On a 2551Q 8% return saved with the old manual switch, the draft path of
/// the non-zero amount `key`'s component still files (`/surcharge`, ...).
pub(super) fn kept_amount_path(kind: AgentFormKind, draft: &Value, key: &str) -> Option<String> {
    if !eight_percent(kind, draft) || draft["auto_compute_penalties"] != false {
        return None;
    }
    let (_, component) = override_of(kind, key)?;
    let path = format!("/{component}");
    draft
        .pointer(&path)
        .and_then(Value::as_f64)
        .filter(|amount| *amount != 0.0)
        .map(|_| path)
}

/// Why a penalty group that is not part of the return is locked.
pub(super) fn off_reason(kind: AgentFormKind, draft: &Value) -> &'static str {
    if eight_percent(kind, draft) {
        EIGHT_PERCENT
    } else {
        NOT_ON_RETURN
    }
}

/// Why `key` of a group is locked while no amount was entered there: a
/// kept 8% amount, the computed amount, or not on the return.
pub(super) fn lock_reason(
    kind: AgentFormKind,
    group: &Group,
    draft: &Value,
    key: &str,
) -> &'static str {
    if kept_amount_path(kind, draft, key).is_some() {
        KEPT_ON_EIGHT_PERCENT
    } else if applies(kind, group, draft) {
        LOCK
    } else {
        off_reason(kind, draft)
    }
}

const COMPONENTS: [&str; 3] = ["surcharge", "interest", "compromise"];

/// One party's Surcharge, Interest and Compromise boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    /// The draft path of the party's penalty state (`""` or `/spouse`).
    pub prefix: String,
    /// Its boxes on the page (a component may repeat on another page).
    pub keys: Vec<String>,
}

/// The override a box writes: `(party prefix, component)`.
fn override_of(kind: AgentFormKind, key: &str) -> Option<(String, &'static str)> {
    let Some(Binding::Amount { path, .. }) = kind
        .definition()
        .bindings
        .get(key)
        .map(|bound| &bound.binding)
    else {
        return None;
    };
    let (prefix, component) = path.rsplit_once("/penalty_overrides/")?;
    let component = COMPONENTS.into_iter().find(|c| *c == component)?;
    Some((prefix.to_string(), component))
}

/// The form's penalty groups on the page: a party whose three overrides
/// all have a box the filer types. A computed box bound to an override too
/// (1700's spouse Total Penalties) is not one of them: it shows the total.
pub(super) fn groups(kind: AgentFormKind) -> Vec<Group> {
    let def = kind.definition();
    let mut groups: Vec<(Group, Vec<&str>)> = Vec::new();
    for key in super::schema_keys(kind.form_code()) {
        let Some((prefix, component)) = override_of(kind, key) else {
            continue;
        };
        if def.box_def(key).is_some_and(|entry| entry.computed)
            || super::schema_computed(kind.form_code(), key)
        {
            continue;
        }
        match groups.iter_mut().find(|(group, _)| group.prefix == prefix) {
            Some((group, components)) => {
                group.keys.push(key.clone());
                components.push(component);
            }
            None => groups.push((
                Group {
                    prefix,
                    keys: vec![key.clone()],
                },
                vec![component],
            )),
        }
    }
    let mut groups: Vec<Group> = groups
        .into_iter()
        .filter(|(_, components)| COMPONENTS.iter().all(|c| components.contains(c)))
        .map(|(group, _)| group)
        .collect();
    // The filer's own items first, as the forms declare them.
    groups.sort_by_key(|group| group.prefix.contains("spouse"));
    groups
}

/// The boxes a model's penalty message names (`item_17a_surcharge`): the
/// control the form declares for that item.
pub(super) fn error_keys(kind: AgentFormKind, field: &str) -> Vec<String> {
    use bir_core::forms::queueable::QueueableForm;
    let items: Vec<([&'static str; 3], [&'static str; 3])> = match kind {
        AgentFormKind::Queueable(kind) => bir_core::with_queueable_kind!(kind, F => {
            <F as QueueableForm>::penalty_items()
                .iter()
                .map(|items| (items.fields, items.controls))
                .collect()
        }),
        AgentFormKind::Form1601C => {
            let items = bir_core::forms::form_1601c::Form1601CDraft::penalty_items();
            vec![(items.fields, items.controls)]
        }
        AgentFormKind::Form2551Q => Vec::new(),
    };
    let named = |name: &str| {
        field == name
            || field.ends_with(&format!(".{name}"))
            || field.ends_with(&format!("/{name}"))
    };
    items
        .iter()
        .flat_map(|(fields, controls)| fields.iter().zip(controls))
        .filter(|(name, _)| named(name))
        .map(|(_, control)| control.to_string())
        .collect()
}

/// Forms whose penalties the filer types, with no computed amount to
/// lock: 0605 only (decided 2026-10-17; it has no due date to compute
/// from).
const OPEN_PENALTY_FORMS: [&str; 1] = ["0605"];

/// Whether `kind`'s penalty groups are computed, locked until Edit opens
/// them; false for 0605, whose penalty boxes are open.
pub(super) fn computes(kind: AgentFormKind) -> bool {
    !OPEN_PENALTY_FORMS.contains(&kind.form_code())
}

/// The groups locked until Edit opens them: every group, or none on 0605.
pub(super) fn computed_groups(kind: AgentFormKind) -> Vec<Group> {
    if computes(kind) {
        groups(kind)
    } else {
        Vec::new()
    }
}

/// Whether the filer has entered any of the group's amounts.
pub(super) fn edited(group: &Group, draft: &Value) -> bool {
    COMPONENTS.iter().any(|component| {
        draft
            .pointer(&format!("{}/penalty_overrides/{component}", group.prefix))
            .is_some_and(|value| !value.is_null())
    })
}

/// The draft path of a penalty box's override.
pub(super) fn path_of(kind: AgentFormKind, key: &str) -> Option<String> {
    override_of(kind, key)
        .map(|(prefix, component)| format!("{prefix}/penalty_overrides/{component}"))
}

/// Whether the group is part of the return as it stands: an amount entered
/// there survives the form's computation. A spouse's column on a return
/// with no spouse does not (the model leaves it to the form's own rules,
/// which clear it), so it gets no Edit.
pub(super) fn applies(kind: AgentFormKind, group: &Group, draft: &Value) -> bool {
    if edited(group, draft) {
        return true;
    }
    let mut copy = draft.clone();
    // A 1701 or 1701Q column stored before the penalty model has no state
    // for the trial's overrides to land in; give the copy one first
    // (rows_annual_individual.rs make_room), or the trial reads a kept
    // column as off the return. The draft itself is not touched.
    if let Some(key) = group.keys.first() {
        super::rows_annual_individual::make_room(kind, &mut copy, key);
    }
    for component in COMPONENTS {
        let path = format!("{}/penalty_overrides/{component}", group.prefix);
        super::set_draft_path(&mut copy, &path, json!(1.0));
    }
    bir_core::forms::agent::compute(kind, &mut copy).is_ok() && edited(group, &copy)
}

/// What the page needs: each computed group's boxes, whether the filer
/// entered its amounts, and whether it is part of the return (Edit is
/// offered). A group the model does not compute is not listed: its boxes
/// are plain open boxes.
pub(super) fn json(kind: AgentFormKind, draft: &Value) -> Value {
    Value::Array(
        computed_groups(kind)
            .iter()
            .map(|group| {
                json!({
                    "keys": group.keys,
                    "edited": edited(group, draft),
                    // A kept 8% amount opens with Edit so a blank can clear it.
                    "applies": applies(kind, group, draft)
                        || group.keys.iter().any(|key| kept_amount_path(kind, draft, key).is_some()),
                    "kept": kept(kind, group, draft),
                })
            })
            .collect(),
    )
}

/// Whether the group's boxes are still on the amounts the draft was stored
/// with, before its penalties were computed: a 1701Q (PR 5) or 1701 (PR 7)
/// column, an excise return stored before maintainer decision (excise penalties
/// run from the typed date) (PR 11), or a 1800 or
/// 1801 return stored before maintainer decision (1800/1801 penalties run from the due date).
pub(super) fn kept(kind: AgentFormKind, group: &Group, draft: &Value) -> bool {
    super::rows_income_q::kept_as_entered(kind, &group.keys, draft)
        || super::rows_annual_individual::kept_as_entered(kind, &group.keys, draft)
        || super::rows_excise::kept_as_entered(kind, &group.keys, draft)
}
