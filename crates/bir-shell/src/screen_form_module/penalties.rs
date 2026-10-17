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
                    "applies": applies(kind, group, draft),
                })
            })
            .collect(),
    )
}
