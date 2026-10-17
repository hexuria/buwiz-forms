//! The assistant's seam into every form: one entry point, keyed by form
//! code, for what each form page runs on its draft.
//!
//! - [`validate`] is the page's Validate: recompute, then the form's own
//!   `validate()` (plus the page's entry checks, see
//!   [`super::form_2551q::Form2551QEntries`]).
//! - [`AgentFormKind::definition`] lists every box of the official form
//!   (`rules/forms/<package>/fields.json`) and how an official box key is
//!   written into the draft. That map is derived by [`probe`], never typed by
//!   hand, and pinned per form in `data/agent-forms/<package>.json`
//!   (`UPDATE_AGENT_FORMS=1 cargo test -p bir-core --test sample_payloads_test agent_forms`).
//! - [`fill`] writes one official box, [`needs_you`] lists the required boxes
//!   that are still empty, [`card_boxes`] shapes both for a chat form.
//!
//! Drafts travel as JSON (`serde_json::Value`), the way they are stored.
//! Nothing here queues, submits or changes a return's filing status.

pub mod probe;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::form_1601c::Form1601CDraft;
use super::form_2551q::{Form2551QDraft, Form2551QEntries};
use super::queueable::{QueueableForm, QueueableKind};
use super::{FilingPeriod, FilingStatus, FormValidator};

/// Every form the assistant can open: the generic queue's drafts plus the
/// two hand-built ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentFormKind {
    Form1601C,
    Form2551Q,
    Queueable(QueueableKind),
}

impl AgentFormKind {
    /// All 43 forms.
    pub fn all() -> Vec<Self> {
        [Self::Form1601C, Self::Form2551Q]
            .into_iter()
            .chain(QueueableKind::ALL.iter().copied().map(Self::Queueable))
            .collect()
    }

    /// From a form code as people write it: `1601C`, `1601-C`, `form-1601c`.
    pub fn from_code(code: &str) -> Option<Self> {
        let wanted = normalize_code(code);
        Self::all()
            .into_iter()
            .find(|kind| normalize_code(kind.form_code()) == wanted)
    }

    /// `form_drafts.form_code`, e.g. `"1601C"`.
    pub fn form_code(self) -> &'static str {
        match self {
            Self::Form1601C => "1601C",
            Self::Form2551Q => "2551Q",
            Self::Queueable(kind) => kind.form_code(),
        }
    }

    /// Official layout id, e.g. `"1601c-v2018"` or `"1600pt-v2018-government"`.
    pub fn layout_id(self) -> &'static str {
        match self {
            Self::Form1601C => "1601c-v2018",
            Self::Form2551Q => "2551q-v2018",
            Self::Queueable(kind) => {
                crate::with_queueable_kind!(kind, F => <F as QueueableForm>::LAYOUT_ID)
            }
        }
    }

    /// Rule package under `rules/forms/`, e.g. `"1600pt-v2018"`.
    pub fn package(self) -> &'static str {
        let layout = self.layout_id();
        PACKAGES
            .iter()
            .map(|(package, _)| *package)
            .filter(|package| layout.starts_with(package))
            .max_by_key(|package| package.len())
            .unwrap_or(layout)
    }

    /// The form's boxes and how each official key is written (pinned data).
    pub fn definition(self) -> &'static AgentFormDef {
        static PARSED: OnceLock<BTreeMap<&'static str, AgentFormDef>> = OnceLock::new();
        let parsed = PARSED.get_or_init(|| {
            PACKAGES
                .iter()
                .map(|(package, raw)| {
                    let def: AgentFormDef = serde_json::from_str(raw).unwrap_or_else(|error| {
                        panic!("invalid data/agent-forms/{package}.json: {error}")
                    });
                    (*package, def)
                })
                .collect()
        });
        parsed
            .get(self.package())
            .expect("every agent form kind has a pinned data/agent-forms file")
    }
}

fn normalize_code(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    let upper = upper.strip_prefix("FORM").unwrap_or(&upper);
    upper.chars().filter(char::is_ascii_alphanumeric).collect()
}

macro_rules! packages {
    ($($id:literal),* $(,)?) => {
        &[$(($id, include_str!(concat!("../../../data/agent-forms/", $id, ".json")))),*]
    };
}

/// Every rule package with its pinned agent data.
pub const PACKAGES: &[(&str, &str)] = packages![
    "0605-v2003",
    "0619e-v2018",
    "0619f-v2018",
    "1600pt-v2018",
    "1600vt-v2018",
    "1600wp-v2010",
    "1601c-v2018",
    "1601eq-v2018",
    "1601fq-v2018",
    "1602q-v2018",
    "1603q-v2018",
    "1604c-v2018",
    "1604e-v2018",
    "1604f-v2018",
    "1606-v2018",
    "1700-v2013",
    "1701-v2018",
    "1701a-v2018",
    "1701ms-v2024",
    "1701q-v2018",
    "1702ex-v2018c",
    "1702mx-v2018c",
    "1702q-v2018c",
    "1702rt-v2018c",
    "1706-v2018",
    "1707-v2021",
    "1707a-v2021",
    "1800-v2018",
    "1801-v2018",
    "2000-v2018",
    "2000ot-v2018",
    "2200a-v2020",
    "2200an-v2018",
    "2200c-v2018",
    "2200m-v2018",
    "2200p-v2020",
    "2200s-v2018",
    "2200t-v2020",
    "2550m-v2007",
    "2550q-v2024",
    "2551q-v2018",
    "2552-v2018",
    "2553-v1999",
];

// ------------------------------------------------------------------ seam

/// Who a draft belongs to and which period it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftIdentity {
    pub tin: String,
    pub taxable_year: u16,
    pub period: FilingPeriod,
    pub status: FilingStatus,
}

/// What every draft offers the assistant, the way its page uses it.
trait Seam: Serialize + DeserializeOwned {
    /// Recompute every derived amount (the page does this on each edit).
    fn seam_compute(&mut self);
    /// The page's Validate, after [`Self::seam_compute`].
    fn seam_validate(&self) -> Vec<(String, String)>;
    /// Official control id -> value.
    fn seam_field_map(&self) -> BTreeMap<String, String>;
    fn seam_identity(&self) -> DraftIdentity;
}

impl<F: QueueableForm> Seam for F {
    fn seam_compute(&mut self) {
        self.compute();
    }
    fn seam_validate(&self) -> Vec<(String, String)> {
        QueueableForm::validate(self)
    }
    fn seam_field_map(&self) -> BTreeMap<String, String> {
        self.field_map()
    }
    fn seam_identity(&self) -> DraftIdentity {
        DraftIdentity {
            tin: self.tin().to_string(),
            taxable_year: self.taxable_year(),
            period: self.filing_period(),
            status: self.lifecycle().status.clone(),
        }
    }
}

impl Seam for Form1601CDraft {
    fn seam_compute(&mut self) {
        self.compute();
    }
    fn seam_validate(&self) -> Vec<(String, String)> {
        FormValidator::validate(self)
    }
    fn seam_field_map(&self) -> BTreeMap<String, String> {
        self.to_bir_field_map()
    }
    fn seam_identity(&self) -> DraftIdentity {
        DraftIdentity {
            tin: self.tin.clone(),
            taxable_year: self.taxable_year,
            period: FilingPeriod::Monthly(self.month),
            status: self.status.clone(),
        }
    }
}

impl Seam for Form2551QDraft {
    fn seam_compute(&mut self) {
        self.recompute(None);
    }
    fn seam_validate(&self) -> Vec<(String, String)> {
        let mut errors = FormValidator::validate(self);
        let entry_errors = self.entry_errors(&Form2551QEntries::from_draft(self), &errors);
        errors.extend(entry_errors);
        errors
    }
    fn seam_field_map(&self) -> BTreeMap<String, String> {
        self.to_bir_field_map()
    }
    fn seam_identity(&self) -> DraftIdentity {
        DraftIdentity {
            tin: self.tin.clone(),
            taxable_year: self.taxable_year,
            period: FilingPeriod::Quarterly(self.quarter),
            status: self.status.clone(),
        }
    }
}

/// Run `$body` with `$ty` bound to the draft type of an [`AgentFormKind`].
macro_rules! with_agent_kind {
    ($kind:expr, $ty:ident => $body:expr) => {
        match $kind {
            AgentFormKind::Form1601C => {
                type $ty = Form1601CDraft;
                $body
            }
            AgentFormKind::Form2551Q => {
                type $ty = Form2551QDraft;
                $body
            }
            AgentFormKind::Queueable(kind) => crate::with_queueable_kind!(kind, $ty => $body),
        }
    };
}

fn parse<F: DeserializeOwned>(kind: AgentFormKind, draft: &Value) -> Result<F, String> {
    serde_json::from_value(draft.clone())
        .map_err(|error| format!("not a {} draft: {error}", kind.form_code()))
}

fn to_json<F: Serialize>(draft: &F) -> Result<Value, String> {
    serde_json::to_value(draft).map_err(|error| error.to_string())
}

/// Run form code that may panic on a value it never expected (the assistant
/// can write combinations the page's widgets never produce).
fn guarded<T>(what: &str, run: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(run)).map_err(|_| format!("{what} failed on this draft"))
}

/// Recompute `draft` in place, as the page does after every edit.
pub fn compute(kind: AgentFormKind, draft: &mut Value) -> Result<(), String> {
    with_agent_kind!(kind, F => {
        let mut typed: F = parse(kind, draft)?;
        guarded("recompute", || typed.seam_compute())?;
        *draft = to_json(&typed)?;
        Ok(())
    })
}

/// The page's Validate: recompute `draft` in place, then run the form's
/// checks. Errors are `(field, message)` exactly as the page shows them.
pub fn validate(kind: AgentFormKind, draft: &mut Value) -> Result<Vec<(String, String)>, String> {
    with_agent_kind!(kind, F => {
        let mut typed: F = parse(kind, draft)?;
        guarded("recompute", || typed.seam_compute())?;
        let errors = guarded("validate", || typed.seam_validate())?;
        *draft = to_json(&typed)?;
        Ok(errors)
    })
}

/// Official control id -> value for `draft` as it stands.
pub fn field_map(kind: AgentFormKind, draft: &Value) -> Result<BTreeMap<String, String>, String> {
    with_agent_kind!(kind, F => {
        let typed: F = parse(kind, draft)?;
        guarded("field map", || typed.seam_field_map())
    })
}

/// TIN, year, period and filing status of `draft`.
pub fn identity(kind: AgentFormKind, draft: &Value) -> Result<DraftIdentity, String> {
    with_agent_kind!(kind, F => {
        let typed: F = parse(kind, draft)?;
        Ok(typed.seam_identity())
    })
}

/// Save an editable Draft the way its page's "Save draft" does, recomputed
/// first; returns the `form_drafts` row id (also written into `draft`).
/// Refuses anything that is not a Draft. Never queues.
pub fn save_draft(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &mut Value,
) -> Result<i64, String> {
    compute(kind, draft)?;
    if identity(kind, draft)?.status != FilingStatus::Draft {
        return Err("this return is no longer a draft".to_string());
    }
    let id = match kind {
        AgentFormKind::Form1601C => {
            let typed: Form1601CDraft = parse(kind, draft)?;
            db.save_1601c_draft(&typed)
                .map_err(|error| error.to_string())?
        }
        AgentFormKind::Form2551Q => {
            let typed: Form2551QDraft = parse(kind, draft)?;
            db.save_2551q_draft(&typed)
                .map_err(|error| error.to_string())?
        }
        // The 2550Q page keeps its own row format (`save_form_draft_v2`).
        AgentFormKind::Queueable(QueueableKind::Form2550Q) => {
            let typed: super::form_2550q::Form2550QDraft = parse(kind, draft)?;
            let quarter = typed.quarter_number().unwrap_or(0);
            db.save_form_draft_v2(
                &typed.tin,
                "2550Q",
                typed.taxable_year,
                &FilingPeriod::Quarterly(quarter),
                &typed.lifecycle.status,
                &typed,
            )
            .map_err(|error| error.to_string())?
        }
        AgentFormKind::Queueable(queueable) => crate::with_queueable_kind!(queueable, F => {
            let typed: F = parse(kind, draft)?;
            db.save_queueable_draft(&typed).map_err(|error| error.to_string())?
        }),
    };
    if let Value::Object(object) = draft {
        object.insert("id".to_string(), Value::from(id));
    }
    Ok(id)
}

/// Parse `draft` as `kind`'s draft type, recompute it and return the field
/// map. Used by [`probe`]; `None` when the value is not a valid draft or the
/// form code panics on it.
fn probe_eval(kind: AgentFormKind, draft: &Value) -> Option<(Value, BTreeMap<String, String>)> {
    with_agent_kind!(kind, F => {
        let mut typed: F = serde_json::from_value(draft.clone()).ok()?;
        catch_unwind(AssertUnwindSafe(|| {
            typed.seam_compute();
            let map = typed.seam_field_map();
            (serde_json::to_value(&typed).ok(), map)
        }))
        .ok()
        .and_then(|(json, map)| json.map(|json| (json, map)))
    })
}

/// The deserialization error for `draft`, if any (used by [`probe`] to read
/// an enum's variants).
fn parse_error(kind: AgentFormKind, draft: &Value) -> Option<String> {
    with_agent_kind!(kind, F => {
        serde_json::from_value::<F>(draft.clone()).err().map(|error| error.to_string())
    })
}

/// The identity of a probed draft, for [`probe`]'s lock detection.
fn probe_identity(kind: AgentFormKind, draft: &Value) -> Option<DraftIdentity> {
    identity(kind, draft).ok()
}

// ------------------------------------------------------------ definition

/// One box of the official form (projected from `fields.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxDef {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub item: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub computed: bool,
    /// `required`, `conditional`, `optional`, `computed` or `hidden`.
    pub required: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<BoxOption>,
    /// Not in `fields.json`: written by the app's field map for a revision
    /// the rule package does not describe (see `probe::supplement_from_field_map`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub layout_only: bool,
    /// One radio of an item: the key of the item's group box. Still fillable
    /// by its own key (`true`), but listed only through the group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The group box of a radio item (not in `fields.json`): one box whose
    /// options are its radios. See `probe::group_radios`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub radio_group: bool,
    /// A group box's radios, in the order of its `options`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    /// For one radio: what choosing it means and the page's control value
    /// (`fields.json` `enum_values`, e.g. `Single Proprietor` / `Single`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub choice: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub choice_value: String,
    /// One row of a row family (`txtAmt{N>=0}`): the family's key. The
    /// official page builds these rows in script, so `fields.json` lists
    /// only the family (see `probe::supplement_row_families`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
}

impl BoxDef {
    /// A box the user can type or pick on the official page.
    pub fn is_user_box(&self) -> bool {
        !self.computed && !self.kind.contains("hidden") && self.required != "hidden"
    }

    pub fn is_radio(&self) -> bool {
        self.kind.contains("radio")
    }

    /// A box the user sees as one question: not one radio of a group, not a
    /// row family pattern (`...{N>=0}`).
    pub fn is_question(&self) -> bool {
        self.is_user_box() && self.group.is_none() && !self.key.contains('{')
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

/// Why the assistant may not write a box that does map to the draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lock {
    /// TIN, year or period: `form.open` picks the return.
    Identity,
    /// Comes from the taxpayer profile; change it there.
    Profile,
    /// An income-tax election: never inferred, only the user answers it.
    Election,
}

impl Lock {
    pub fn reason(self) -> &'static str {
        match self {
            Self::Identity => {
                "picks which return this is; reopen the form for another TIN or period"
            }
            Self::Profile => "comes from the taxpayer profile; change it in the profile",
            Self::Election => "an income-tax election; only the user answers it",
        }
    }
}

/// Date text format of a string draft field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    /// `YYYY-MM-DD`
    Iso,
    /// `MM/DD/YYYY`
    Mdy,
    /// `MM/YYYY`
    MonthYear,
}

/// One part of a date that a box shows on its own: 1604C's Item 11
/// refund date is one `MM/DD/YYYY` field written to a month select and
/// day and year boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePart {
    Month,
    Day,
    Year,
}

impl DatePart {
    fn name(self) -> &'static str {
        match self {
            Self::Month => "month (01-12)",
            Self::Day => "day (01-31)",
            Self::Year => "four-digit year",
        }
    }
}

/// How an official box is written into the draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Binding {
    /// The box shows a string field as typed (or reformatted as a date).
    Text {
        path: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        nullable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        date: Option<DateFormat>,
        /// The box shows only this part of the date in `path` (`date` is
        /// then that field's format); filling it rewrites that part.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<DatePart>,
    },
    /// The box shows an amount field.
    Amount {
        path: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        nullable: bool,
    },
    /// The box shows a whole-number field.
    Whole {
        path: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        nullable: bool,
    },
    /// A radio, checkbox or select: each official value sets these fields.
    Choice { options: Vec<ChoiceOption> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    /// The official box value, e.g. `"true"` or `"WC160"`.
    pub value: String,
    /// Draft field (JSON pointer) -> value that produces it.
    pub set: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundBox {
    #[serde(flatten)]
    pub binding: Binding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<Lock>,
}

/// One form's boxes and their bindings (`data/agent-forms/<package>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentFormDef {
    pub package: String,
    pub form_code: String,
    pub boxes: Vec<BoxDef>,
    /// Official key -> how to write it.
    pub bindings: BTreeMap<String, BoundBox>,
    /// Row list (JSON pointer, indices as `*`) -> a blank row to append.
    #[serde(default)]
    pub rows: BTreeMap<String, Value>,
    /// User box (not computed) the assistant cannot write -> why.
    #[serde(default)]
    pub unmapped: BTreeMap<String, String>,
}

impl AgentFormDef {
    pub fn box_def(&self, key: &str) -> Option<&BoxDef> {
        self.boxes.iter().find(|entry| entry.key == key)
    }

    /// Boxes the assistant may fill: user boxes with a binding and no lock,
    /// a radio item as its one group box.
    pub fn fillable(&self) -> impl Iterator<Item = &BoxDef> {
        self.boxes
            .iter()
            .filter(|entry| entry.is_question() && self.refusal(&entry.key).is_none())
    }

    /// The value a box shows in `map`: for a radio group, the option value
    /// of its chosen radio (`""` while none is chosen).
    pub fn box_value(&self, key: &str, map: &BTreeMap<String, String>) -> String {
        match self.box_def(key) {
            Some(entry) if entry.radio_group => entry
                .options
                .iter()
                .zip(&entry.members)
                .find(|(_, member)| map.get(*member).is_some_and(|value| value == "true"))
                .map(|(option, _)| option.value.clone())
                .unwrap_or_default(),
            _ => map.get(key).cloned().unwrap_or_default(),
        }
    }

    /// Why `key` cannot be filled, or `None` when it can.
    pub fn refusal(&self, key: &str) -> Option<String> {
        let Some(entry) = self.box_def(key) else {
            return Some(format!(
                "`{key}` is not a box on Form {}; use the keys bir_form_fields lists",
                self.form_code
            ));
        };
        if entry.computed || entry.required == "computed" {
            return Some(format!("`{key}` is computed by BIR's own formulas"));
        }
        if !entry.is_user_box() {
            return Some(format!("`{key}` is a hidden control, not a box"));
        }
        match self.bindings.get(key) {
            Some(BoundBox {
                lock: Some(lock), ..
            }) => Some(format!("`{key}` {}", lock.reason())),
            Some(_) => None,
            None => Some(format!(
                "`{key}` cannot be filled from here: {}",
                self.unmapped
                    .get(key)
                    .map(String::as_str)
                    .unwrap_or("it has no field in this app's draft")
            )),
        }
    }

    /// The box a user sees for `key`: its radio group, or itself.
    pub fn question_of(&self, key: &str) -> String {
        self.box_def(key)
            .and_then(|entry| entry.group.clone())
            .unwrap_or_else(|| key.to_string())
    }
}

// ------------------------------------------------------------------ fill

/// Write official box `key` of `draft` to `value` (no recompute; call
/// [`compute`] after the last box). Refuses computed, locked and unknown
/// boxes with the reason.
pub fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[])
}

/// [`fill`] for the user's own form page. The one difference: an income-tax
/// election ([`Lock::Election`]) is the user's to answer, so it is written;
/// the assistant is still refused it through [`fill`]. Identity, profile,
/// computed, hidden and unbound boxes are refused exactly as in [`fill`].
pub fn fill_as_user(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[Lock::Election])
}

/// [`fill_as_user`] for a box the user chose to edit on their own form page
/// after it was filled from the profile ([`Lock::Profile`]): the edit stays
/// on this return's draft and the profile is never written. The assistant is
/// still refused profile boxes through [`fill`]; identity, computed, hidden
/// and unbound boxes are refused exactly as in [`fill`].
pub fn fill_profile_box_as_user(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[Lock::Election, Lock::Profile])
}

/// [`fill_as_user`] for a period box ([`Lock::Identity`]: year, month,
/// quarter, day) of a form shown as a sample that is never saved, so the
/// filer can type the period to see the form work. The caller must hold such
/// a sample; a stored return's period stays locked through every other
/// writer. Profile, computed, hidden and unbound boxes are refused as in
/// [`fill_as_user`].
pub fn fill_sample_period_box(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[Lock::Election, Lock::Identity])
}

fn fill_box(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    allowed: &[Lock],
) -> Result<(), String> {
    let def = kind.definition();
    let lock = def.bindings.get(key).and_then(|bound| bound.lock);
    let let_through = match lock {
        Some(Lock::Election) => allowed.contains(&Lock::Election),
        // Only the lock itself is lifted: a computed or hidden box stays refused.
        Some(lock @ (Lock::Profile | Lock::Identity)) => {
            allowed.contains(&lock)
                && def
                    .box_def(key)
                    .is_some_and(|entry| entry.is_user_box() && entry.required != "computed")
        }
        _ => false,
    };
    if let Some(reason) = def.refusal(key)
        && !let_through
    {
        return Err(reason);
    }
    let bound = def
        .bindings
        .get(key)
        .expect("refusal() is None only for bound boxes");
    let text = value_text(value);
    match &bound.binding {
        Binding::Text {
            path,
            date: Some(format),
            part: Some(part),
            ..
        } => {
            let current = draft.pointer(path).and_then(Value::as_str).unwrap_or("");
            let leaf = set_date_part(current, *format, *part, &text).ok_or_else(|| {
                format!(
                    "`{key}` must be the {} of a date (got `{text}`)",
                    part.name()
                )
            })?;
            set_path(draft, path, Value::String(leaf), &def.rows)
        }
        Binding::Text {
            path,
            nullable,
            date,
            ..
        } => {
            let leaf = if text.trim().is_empty() && *nullable {
                Value::Null
            } else if let Some(format) = date {
                Value::String(reformat_date(&text, *format).ok_or_else(|| {
                    format!("`{key}` must be a date as MM/DD/YYYY (got `{text}`)")
                })?)
            } else {
                Value::String(text.trim().to_string())
            };
            set_path(draft, path, leaf, &def.rows)
        }
        Binding::Amount { path, nullable } => {
            let leaf = if text.trim().is_empty() && *nullable {
                Value::Null
            } else {
                Value::from(parse_amount(&text).ok_or_else(|| {
                    format!("`{key}` must be an amount such as 1234.50 (got `{text}`)")
                })?)
            };
            set_path(draft, path, leaf, &def.rows)
        }
        Binding::Whole { path, nullable } => {
            let leaf = if text.trim().is_empty() && *nullable {
                Value::Null
            } else if text.trim().is_empty()
                && def
                    .box_def(key)
                    .is_some_and(|entry| entry.kind.contains("select"))
            {
                // A month or day select's blank official choice (`00`): the
                // draft keeps 0 for "not chosen" and its validation says so.
                Value::from(0)
            } else {
                let number = text
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| format!("`{key}` must be a whole number (got `{text}`)"))?;
                Value::from(number)
            };
            set_path(draft, path, leaf, &def.rows)
        }
        // A select's blank official choice ("Select Tax Rate") when the
        // binding lists none: every option sets the same one field, so blank
        // clears it. The typed draft decides whether that field may be empty.
        Binding::Choice { options }
            if text.trim().is_empty()
                && !options.iter().any(|option| option.value.is_empty())
                && def
                    .box_def(key)
                    .is_some_and(|entry| entry.kind.contains("select"))
                && single_choice_path(options).is_some() =>
        {
            let path = single_choice_path(options).expect("checked in the guard");
            set_path(draft, path, Value::Null, &def.rows)
        }
        Binding::Choice { options } => {
            let option = choose(def, key, options, &text).ok_or_else(|| {
                let allowed: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
                format!(
                    "`{key}` must be one of {} (got `{text}`)",
                    allowed.join(", ")
                )
            })?;
            for (path, leaf) in &option.set {
                set_path(draft, path, leaf.clone(), &def.rows)?;
            }
            // 1601C keeps Item 3's "unanswered" mark beside the answer; an
            // answer clears it, as `set_any_taxes_withheld` does.
            if kind == AgentFormKind::Form1601C
                && option.set.contains_key("/any_taxes_withheld")
                && let Value::Object(object) = draft
            {
                object.insert("any_taxes_withheld_unanswered".into(), Value::Bool(false));
            }
            Ok(())
        }
    }?;
    // A page that keeps the raw text of a box (2550Q) shows that text over
    // the typed value when it reopens: keep the two in step.
    let raw = format!(
        "/raw_editor_state/singleton_fields/{}",
        key.replace('~', "~0").replace('/', "~1")
    );
    if let Some(Value::Object(slot)) = draft.pointer_mut(&raw)
        && slot.get("state").and_then(Value::as_str) == Some("text")
    {
        slot.insert("text".to_string(), Value::String(text.trim().to_string()));
    }
    // Typed values must still make a draft (an integer field refuses 1.5).
    with_agent_kind!(kind, F => parse::<F>(kind, draft).map(|_| ()))
        .map_err(|error| format!("`{key}` = `{text}` does not fit this box: {error}"))
}

/// The one draft field every option of a choice sets, if they all set
/// exactly that field.
fn single_choice_path(options: &[ChoiceOption]) -> Option<&str> {
    let mut paths = options.iter().map(|option| {
        let mut keys = option.set.keys();
        match (keys.next(), keys.next()) {
            (Some(path), None) => Some(path.as_str()),
            _ => None,
        }
    });
    let first = paths.next()??;
    paths.all(|path| path == Some(first)).then_some(first)
}

fn choose<'a>(
    def: &AgentFormDef,
    key: &str,
    options: &'a [ChoiceOption],
    text: &str,
) -> Option<&'a ChoiceOption> {
    let wanted = text.trim();
    let alias = match wanted.to_ascii_lowercase().as_str() {
        "yes" | "y" | "1" | "checked" | "on" => Some("true"),
        "no" | "n" | "0" | "unchecked" | "off" => Some("false"),
        _ => None,
    };
    // A label (`Yes`, `Private`), or for a radio group one of its radio keys.
    let by_label = def.box_def(key).and_then(|entry| {
        entry
            .options
            .iter()
            .zip(
                entry
                    .members
                    .iter()
                    .map(Some)
                    .chain(std::iter::repeat(None)),
            )
            .find(|(option, member)| {
                option.label.eq_ignore_ascii_case(wanted) || *member == Some(&wanted.to_string())
            })
            .map(|(option, _)| option.value.as_str())
    });
    options.iter().find(|option| {
        option.value.eq_ignore_ascii_case(wanted)
            || alias.is_some_and(|alias| option.value == alias)
            || by_label.is_some_and(|value| option.value == value)
    })
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `1,234.50`, `₱1234.5` or `(100.00)` -> number.
pub fn parse_amount(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let (negative, body) = match trimmed
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        Some(inner) => (true, inner),
        None => (false, trimmed),
    };
    let cleaned: String = body
        .chars()
        .filter(|c| !matches!(c, ',' | '₱' | ' '))
        .collect();
    let amount = cleaned
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite())?;
    Some(if negative { -amount } else { amount })
}

/// Accept MM/DD/YYYY (the official box format) or the field's own format.
/// `current` (a date in `format`, possibly partial or empty) with one part
/// replaced by `text`: `01` or `1 - January` for a month, `5` for a day, a
/// four-digit year. Blank text clears the part; all parts blank is `""`.
/// The other parts are kept as they are, so a date can be entered one box
/// at a time; the form's own validation judges the finished date.
fn set_date_part(current: &str, format: DateFormat, part: DatePart, text: &str) -> Option<String> {
    let digits: String = text
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() && !text.trim().is_empty() {
        return None;
    }
    let value = if digits.is_empty() {
        String::new()
    } else {
        let number: u32 = digits.parse().ok()?;
        match part {
            DatePart::Month if (1..=12).contains(&number) => format!("{number:02}"),
            DatePart::Day if (1..=31).contains(&number) => format!("{number:02}"),
            DatePart::Year if digits.len() == 4 => digits,
            _ => return None,
        }
    };
    let (separator, order): (char, &[DatePart]) = match format {
        DateFormat::Mdy => ('/', &[DatePart::Month, DatePart::Day, DatePart::Year]),
        DateFormat::Iso => ('-', &[DatePart::Year, DatePart::Month, DatePart::Day]),
        DateFormat::MonthYear => ('/', &[DatePart::Month, DatePart::Year]),
    };
    let at = order.iter().position(|p| *p == part)?;
    let mut parts: Vec<String> = current
        .trim()
        .split(separator)
        .map(|piece| piece.trim().to_string())
        .collect();
    parts.resize(order.len(), String::new());
    parts.truncate(order.len());
    parts[at] = value;
    Some(if parts.iter().all(String::is_empty) {
        String::new()
    } else {
        parts.join(&separator.to_string())
    })
}

fn reformat_date(text: &str, format: DateFormat) -> Option<String> {
    let digits: Vec<&str> = text
        .trim()
        .split(['/', '-', '.'])
        .filter(|part| !part.is_empty())
        .collect();
    let (year, month, day) = match digits.as_slice() {
        [y, m, d] if y.len() == 4 => (*y, *m, Some(*d)),
        [m, d, y] if y.len() == 4 => (*y, *m, Some(*d)),
        [m, y] if y.len() == 4 => (*y, *m, None),
        _ => return None,
    };
    let year: u16 = year.parse().ok()?;
    let month: u8 = month.parse().ok().filter(|m| (1..=12).contains(m))?;
    let day: Option<u8> = match day {
        Some(day) => Some(day.parse().ok().filter(|d| (1..=31).contains(d))?),
        None => None,
    };
    Some(match (format, day) {
        (DateFormat::Iso, Some(day)) => format!("{year:04}-{month:02}-{day:02}"),
        (DateFormat::Mdy, Some(day)) => format!("{month:02}/{day:02}/{year:04}"),
        (DateFormat::MonthYear, _) => format!("{month:02}/{year:04}"),
        _ => return None,
    })
}

/// Row list pattern of a pointer: `/schedule/3/amount` -> `/schedule/*`.
fn row_pattern(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| {
            if segment.parse::<usize>().is_ok() {
                "*"
            } else {
                segment
            }
        })
        .fold(String::new(), |mut out, segment| {
            out.push('/');
            out.push_str(segment);
            out
        })
}

/// Set the leaf at JSON pointer `path`, appending blank rows (from `rows`)
/// when the pointer names a row past the end of its list.
pub(crate) fn set_path(
    root: &mut Value,
    path: &str,
    leaf: Value,
    rows: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let segments: Vec<&str> = path.split('/').skip(1).collect();
    let mut node = root;
    for (depth, segment) in segments.iter().enumerate() {
        let last = depth + 1 == segments.len();
        node = match node {
            Value::Object(object) => {
                if last {
                    object.insert((*segment).to_string(), leaf);
                    return Ok(());
                }
                object
                    .get_mut(*segment)
                    .ok_or_else(|| format!("draft has no `{path}`"))?
            }
            Value::Array(items) => {
                let index: usize = segment
                    .parse()
                    .map_err(|_| format!("draft has no `{path}`"))?;
                if index >= items.len() {
                    let pattern = row_pattern(&segments[..depth]);
                    let blank = rows
                        .get(&pattern)
                        .ok_or_else(|| format!("rows of `{pattern}` cannot be added here"))?;
                    if index >= ROW_LIMIT {
                        return Err(format!(
                            "row {} of `{pattern}` is past the last row",
                            index + 1
                        ));
                    }
                    items.resize(index + 1, blank.clone());
                }
                if last {
                    items[index] = leaf;
                    return Ok(());
                }
                &mut items[index]
            }
            _ => return Err(format!("draft has no `{path}`")),
        };
    }
    Err(format!("draft has no `{path}`"))
}

/// No official form has more rows than this in one list.
const ROW_LIMIT: usize = 500;

// ------------------------------------------------------------- needs you

/// One box the user still has to answer, or one with a validation error.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CardBox {
    pub key: String,
    pub item: String,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<BoxOption>,
    pub value: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CardBox {
    pub fn new(def: &AgentFormDef, entry: &BoxDef, map: &BTreeMap<String, String>) -> Self {
        let options = match def.bindings.get(&entry.key).map(|bound| &bound.binding) {
            Some(Binding::Choice { options }) if entry.options.is_empty() => options
                .iter()
                .map(|option| BoxOption {
                    value: option.value.clone(),
                    label: String::new(),
                })
                .collect(),
            _ => entry.options.clone(),
        };
        Self {
            key: entry.key.clone(),
            item: entry.item.clone(),
            label: entry.label.clone(),
            kind: entry.kind.clone(),
            options,
            value: def.box_value(&entry.key, map),
            required: entry.required == "required",
            error: None,
        }
    }
}

/// A box value that counts as unanswered.
fn is_blank(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value == "false"
}

/// Official keys a validation error's field names: the key itself, or the
/// keys bound to the draft field it names (`schedule_1.0.atc` or `atc`).
/// A choice counts only when it decides that field (its value differs
/// between the options); of those, the boxes deciding the fewest other
/// fields win (Item 7's radios, not the ATC radios that also set the type).
pub fn error_keys(kind: AgentFormKind, field: &str) -> Vec<String> {
    let def = kind.definition();
    if def.box_def(field).is_some() {
        return vec![field.to_string()];
    }
    let pointer = error_pointer(field);
    let direct = bound_error_keys(def, field, &pointer);
    if !direct.is_empty() {
        return direct;
    }
    let related = related_error_keys(def, &pointer);
    if !related.is_empty() {
        return related;
    }
    aliased_error_keys(kind, def, field)
}

/// Error names that are neither a draft field nor its parent, with the
/// fields the form's own check reads (reviewed against its `validate`).
const ERROR_ALIASES: &[(&str, &str, &[&str])] = &[(
    // form_1701ms.rs Item 31: "Item 31 is mandatory because Item 30 is has
    // negative value." / "The total amount in Item 31 must equal ...".
    "1701MS",
    "overpayment",
    &[
        "/to_be_refunded",
        "/to_be_issued_tcc",
        "/to_be_carried_over",
        "/refund_amount",
        "/tcc_amount",
        "/carry_over_amount",
    ],
)];

fn aliased_error_keys(kind: AgentFormKind, def: &AgentFormDef, field: &str) -> Vec<String> {
    let Some((_, _, paths)) = ERROR_ALIASES
        .iter()
        .find(|(form, name, _)| *form == kind.form_code() && *name == field)
    else {
        return Vec::new();
    };
    def.bindings
        .iter()
        .filter(|(_, bound)| match &bound.binding {
            Binding::Text { path, .. }
            | Binding::Amount { path, .. }
            | Binding::Whole { path, .. } => paths.contains(&path.as_str()),
            Binding::Choice { options } => options
                .iter()
                .flat_map(|option| option.set.keys())
                .any(|path| paths.contains(&path.as_str())),
        })
        .map(|(key, _)| key.clone())
        .collect()
}

/// `atc_rows[0].atc_code` / `schedules[0][0].reference` -> a JSON pointer.
fn error_pointer(field: &str) -> String {
    let mut pointer = String::from("/");
    for c in field.chars() {
        match c {
            '.' | '[' => {
                if !pointer.ends_with('/') {
                    pointer.push('/');
                }
            }
            ']' => {}
            c => pointer.push(c),
        }
    }
    pointer
}

/// When no box is bound to the named field itself: the boxes bound under it
/// (an object or a row: `place_of_production`, `sellers[0]`), the month,
/// day and year boxes of a split date (`donation_date` or `year_end` ->
/// `donation_month`/`year_end_month`, `_day`, `_year` in the same object),
/// else the boxes the
/// probe saw the field change without binding them (a TIN written over
/// four boxes: "derived by the app: changes with /seller_tin").
fn related_error_keys(def: &AgentFormDef, pointer: &str) -> Vec<String> {
    let under = format!("{pointer}/");
    // `donation_date` -> `donation_month`/...; `year_end` -> `year_end_month`/...
    let date_parts: Vec<String> = [Some(pointer), pointer.strip_suffix("_date")]
        .into_iter()
        .flatten()
        .flat_map(|stem| ["month", "day", "year"].map(|part| format!("{stem}_{part}")))
        .collect();
    let names = |path: &str| path.starts_with(&under) || date_parts.iter().any(|p| p == path);
    let mut keys: Vec<String> = def
        .bindings
        .iter()
        .filter(|(_, bound)| match &bound.binding {
            Binding::Text { path, .. }
            | Binding::Amount { path, .. }
            | Binding::Whole { path, .. } => names(path),
            Binding::Choice { options } => options
                .iter()
                .flat_map(|option| option.set.keys())
                .any(|path| names(path)),
        })
        .map(|(key, _)| key.clone())
        .collect();
    if keys.is_empty() {
        let mentions = |reason: &str| {
            reason
                .strip_prefix("derived by the app: changes with ")
                .map(|rest| rest.split(" but ").next().unwrap_or(""))
                .is_some_and(|paths| {
                    paths
                        .split(", ")
                        .any(|path| path == pointer || path.starts_with(&under))
                })
        };
        keys = def
            .unmapped
            .iter()
            .filter(|(_, reason)| mentions(reason))
            .map(|(key, _)| key.clone())
            .collect();
    }
    keys
}

/// The keys bound to the draft field a validation error names (see
/// [`error_keys`]).
fn bound_error_keys(def: &AgentFormDef, field: &str, pointer: &str) -> Vec<String> {
    let pointer = pointer.to_string();
    let leaf = |path: &str| path == pointer || path.rsplit('/').next() == Some(field);
    let decided = |bound: &BoundBox| -> Option<usize> {
        match &bound.binding {
            Binding::Text { path, .. }
            | Binding::Amount { path, .. }
            | Binding::Whole { path, .. } => leaf(path).then_some(1),
            Binding::Choice { options } => {
                let varying: BTreeSet<&String> = options
                    .iter()
                    .flat_map(|option| option.set.keys())
                    .filter(|path| {
                        let values: Vec<Option<&Value>> =
                            options.iter().map(|option| option.set.get(*path)).collect();
                        values.iter().any(|value| *value != values[0])
                    })
                    .collect();
                varying
                    .iter()
                    .any(|path| leaf(path))
                    .then_some(varying.len())
            }
        }
    };
    let found: Vec<(usize, &String)> = def
        .bindings
        .iter()
        .filter_map(|(key, bound)| decided(bound).map(|count| (count, key)))
        .collect();
    let fewest = found.iter().map(|(count, _)| *count).min();
    found
        .into_iter()
        .filter(|(count, _)| Some(*count) == fewest)
        .map(|(_, key)| key.clone())
        .collect()
}

/// A validation error placed on the boxes it names: the fillable question
/// boxes (a radio as its group), else none.
fn error_questions(kind: AgentFormKind, field: &str) -> Vec<String> {
    let def = kind.definition();
    let mut keys: Vec<String> = error_keys(kind, field)
        .iter()
        .map(|key| def.question_of(key))
        .filter(|key| {
            def.box_def(key).is_some_and(BoxDef::is_question) && def.refusal(key).is_none()
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// What the user still has to answer on the open form, and the validation
/// errors that name no box the assistant or the user can fill here.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NeedsYou {
    pub boxes: Vec<CardBox>,
    pub other_errors: Vec<(String, String)>,
}

/// Needs-you: every fillable box a validation error names (with BIR's
/// message), then, while the form does not validate yet, the required
/// fillable boxes that are still empty. A radio item is one box (its group)
/// and counts as answered when any of its radios is chosen. Boxes the app
/// writes itself (computed, the profile's, the period's, or with no draft
/// field) are never listed: nobody can answer them here.
///
/// Once the form validates, nothing is listed: a required box the rules
/// accept empty is not needed.
pub fn needs_you(
    kind: AgentFormKind,
    map: &BTreeMap<String, String>,
    errors: &[(String, String)],
) -> NeedsYou {
    let def = kind.definition();
    let mut boxes: Vec<CardBox> = Vec::new();
    let mut other_errors = Vec::new();
    for (field, message) in errors {
        let keys = error_questions(kind, field);
        if keys.is_empty() {
            other_errors.push((field.clone(), message.clone()));
        }
        for key in keys {
            match boxes.iter_mut().find(|card| card.key == key) {
                Some(card) => {
                    if card.error.is_none() {
                        card.error = Some(message.clone());
                    }
                }
                None => {
                    let entry = def.box_def(&key).expect("error_questions checks the box");
                    let mut card = CardBox::new(def, entry, map);
                    card.error = Some(message.clone());
                    boxes.push(card);
                }
            }
        }
    }
    if !errors.is_empty() {
        for entry in def.fillable() {
            if entry.required == "required"
                && is_blank(&def.box_value(&entry.key, map))
                && !boxes.iter().any(|card| card.key == entry.key)
            {
                boxes.push(CardBox::new(def, entry, map));
            }
        }
    }
    NeedsYou {
        boxes,
        other_errors,
    }
}

/// What a chat form card shows: the needs-you boxes, each with BIR's error
/// text where validation named it, and the errors that name no such box.
pub fn card_boxes(
    kind: AgentFormKind,
    map: &BTreeMap<String, String>,
    errors: &[(String, String)],
) -> (Vec<CardBox>, Vec<(String, String)>) {
    let needs = needs_you(kind, map, errors);
    (needs.boxes, needs.other_errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_answers_an_election_the_assistant_may_not() {
        let kind = AgentFormKind::Form2551Q;
        let profile: crate::profile::TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Example Store",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "store@example.test",
            "default_form_type": "2551Qv2018"
        }))
        .unwrap();
        let mut draft =
            serde_json::to_value(Form2551QDraft::new_from_profile(&profile, 2026, 3)).unwrap();
        let refused = fill(kind, &mut draft, "frm2551Qv2018:taxRate", &Value::from("2"));
        assert!(refused.unwrap_err().contains("election"));
        fill_as_user(kind, &mut draft, "frm2551Qv2018:taxRate", &Value::from("2")).unwrap();
        assert_eq!(draft["item_13_election"], "eight_percent");
        // Profile and identity boxes stay refused for the user too.
        let def = kind.definition();
        for (key, bound) in &def.bindings {
            if matches!(bound.lock, Some(Lock::Profile | Lock::Identity)) {
                assert!(
                    fill_as_user(kind, &mut draft, key, &Value::from("x")).is_err(),
                    "{key}"
                );
            }
        }
    }

    #[test]
    fn only_the_users_page_edit_lifts_a_profile_lock() {
        let mut lifted = 0;
        for kind in AgentFormKind::all() {
            let def = kind.definition();
            for (key, bound) in &def.bindings {
                let Some(lock @ (Lock::Profile | Lock::Identity)) = bound.lock else {
                    continue;
                };
                let entry = def.box_def(key).expect("a bound key is a box");
                // The refusal each writer must give, checked before any value.
                let computed = entry.computed || entry.required == "computed";
                let expected = if computed {
                    "is computed by BIR's own formulas"
                } else if !entry.is_user_box() {
                    "is a hidden control"
                } else {
                    lock.reason()
                };
                let mut draft = Value::Null;
                let value = Value::from("x");
                for (writer, result) in [
                    ("fill", fill(kind, &mut draft, key, &value)),
                    ("fill_as_user", fill_as_user(kind, &mut draft, key, &value)),
                ] {
                    let refused = result.expect_err(key);
                    assert!(refused.contains(expected), "{writer} {key}: {refused}");
                }
                let page = fill_profile_box_as_user(kind, &mut draft, key, &value);
                if lock == Lock::Profile && !computed && entry.is_user_box() {
                    // Past every refusal: what fails now is the empty draft.
                    let error = page.expect_err("a null draft takes no value");
                    for reason in [
                        Lock::Profile.reason(),
                        Lock::Identity.reason(),
                        "is computed by BIR's own formulas",
                        "is a hidden control",
                    ] {
                        assert!(!error.contains(reason), "{key}: {error}");
                    }
                    lifted += 1;
                } else {
                    let refused = page.expect_err(key);
                    assert!(refused.contains(expected), "page {key}: {refused}");
                }
            }
        }
        assert!(lifted > 100, "{lifted} profile boxes");
    }

    #[test]
    fn every_form_has_pinned_data_for_its_package() {
        let kinds = AgentFormKind::all();
        assert_eq!(kinds.len(), 43);
        let mut packages: Vec<&str> = kinds.iter().map(|kind| kind.package()).collect();
        packages.sort_unstable();
        packages.dedup();
        assert_eq!(packages.len(), 43, "{packages:?}");
        for kind in kinds {
            let def = kind.definition();
            assert_eq!(def.package, kind.package());
            assert_eq!(def.form_code, kind.form_code());
            assert_eq!(AgentFormKind::from_code(kind.form_code()), Some(kind));
        }
        assert_eq!(
            AgentFormKind::from_code("form-1601-c"),
            Some(AgentFormKind::Form1601C)
        );
        assert_eq!(AgentFormKind::from_code("1601X"), None);
    }

    #[test]
    fn amounts_and_dates_parse_like_the_official_boxes() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount("(100.00)"), Some(-100.0));
        assert_eq!(parse_amount("₱ 5"), Some(5.0));
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(
            reformat_date("07/19/2031", DateFormat::Iso).as_deref(),
            Some("2031-07-19")
        );
        assert_eq!(
            reformat_date("2031-07-19", DateFormat::Mdy).as_deref(),
            Some("07/19/2031")
        );
        assert_eq!(
            reformat_date("07/2031", DateFormat::MonthYear).as_deref(),
            Some("07/2031")
        );
        assert_eq!(reformat_date("13/40/2031", DateFormat::Mdy), None);
    }

    #[test]
    fn a_date_part_box_rewrites_only_its_part() {
        use DatePart::{Day, Month, Year};
        let mdy = DateFormat::Mdy;
        // Entered one box at a time, from empty.
        assert_eq!(set_date_part("", mdy, Month, "3").as_deref(), Some("03//"));
        assert_eq!(
            set_date_part("03//", mdy, Day, "15").as_deref(),
            Some("03/15/")
        );
        assert_eq!(
            set_date_part("03/15/", mdy, Year, "2026").as_deref(),
            Some("03/15/2026")
        );
        // A select's label carries its code first.
        assert_eq!(
            set_date_part("03/15/2026", mdy, Month, "12 - December").as_deref(),
            Some("12/15/2026")
        );
        // Clearing a part keeps the others; clearing all leaves nothing.
        assert_eq!(
            set_date_part("03/15/2026", mdy, Day, "").as_deref(),
            Some("03//2026")
        );
        assert_eq!(set_date_part("03//", mdy, Month, "").as_deref(), Some(""));
        // Out of range or not a number is refused.
        assert_eq!(set_date_part("", mdy, Month, "13"), None);
        assert_eq!(set_date_part("", mdy, Month, "00"), None);
        assert_eq!(set_date_part("", mdy, Day, "32"), None);
        assert_eq!(set_date_part("", mdy, Year, "26"), None);
        assert_eq!(set_date_part("", mdy, Month, "March"), None);
        // Other formats keep their own order.
        assert_eq!(
            set_date_part("2031-07-19", DateFormat::Iso, Month, "2").as_deref(),
            Some("2031-02-19")
        );
        assert_eq!(
            set_date_part("07/2031", DateFormat::MonthYear, Year, "2032").as_deref(),
            Some("07/2032")
        );
    }

    #[test]
    fn rows_past_the_end_are_appended_from_the_blank_row() {
        let mut draft = serde_json::json!({"rows": [{"a": 1.0, "b": "x"}]});
        let rows = BTreeMap::from([("/rows".to_string(), serde_json::json!({"a": 0.0, "b": ""}))]);
        set_path(&mut draft, "/rows/2/a", Value::from(5.0), &rows).unwrap();
        assert_eq!(
            draft,
            serde_json::json!({"rows": [{"a": 1.0, "b": "x"}, {"a": 0.0, "b": ""}, {"a": 5.0, "b": ""}]})
        );
        assert!(set_path(&mut draft, "/other/0/a", Value::from(1), &rows).is_err());
    }
}
