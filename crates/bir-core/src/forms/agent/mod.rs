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
//! - [`locks`] and [`draft_locks`] are the form page's lock map, and what
//!   [`fill`] refuses: one computation for both.
//!
//! Drafts travel as JSON (`serde_json::Value`), the way they are stored.
//! Nothing here queues, submits or changes a return's filing status.

pub mod carry;
pub mod draft_locks;
pub mod gated_rows;
pub mod probe;
pub mod recipe;

pub use draft_locks::{draft_locks, live_locks};

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::form_1601c::Form1601CDraft;
use super::form_2551q::Form2551QDraft;
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
        // The page's Validate. Queue and the pre-upload revalidate call this
        // same list (`filing_errors`), so the assistant is not checked by a
        // second copy of the rules.
        self.filing_errors()
    }
    fn seam_field_map(&self) -> BTreeMap<String, String> {
        let mut map = self.to_bir_field_map();
        // Display only. Not an official upload key: `to_bir_field_map` stays
        // the two radios the XML writer knows.
        map.insert(
            "frm2551Qv2018:taxRateNA".to_string(),
            if matches!(
                self.item_13_election,
                crate::forms::form_2551q::Item13Election::NotApplicable
            ) {
                "true".to_string()
            } else {
                "false".to_string()
            },
        );
        map
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

// Each operation's work on the typed draft lives in one generic function
// per draft type, kept out of line: the dispatcher (a match over every
// form) then holds only the small result in its frame, never every form's
// typed draft at once, which in a debug build made each dispatcher's frame
// hundreds of kilobytes.

#[inline(never)]
fn compute_as<F: Seam>(kind: AgentFormKind, draft: &mut Value) -> Result<(), String> {
    let mut typed: F = parse(kind, draft)?;
    guarded("recompute", || typed.seam_compute())?;
    *draft = to_json(&typed)?;
    Ok(())
}

#[inline(never)]
fn validate_as<F: Seam>(
    kind: AgentFormKind,
    draft: &mut Value,
) -> Result<Vec<(String, String)>, String> {
    let mut typed: F = parse(kind, draft)?;
    guarded("recompute", || typed.seam_compute())?;
    let errors = guarded("validate", || typed.seam_validate())?;
    *draft = to_json(&typed)?;
    Ok(errors)
}

#[inline(never)]
fn field_map_as<F: Seam>(
    kind: AgentFormKind,
    draft: &Value,
) -> Result<BTreeMap<String, String>, String> {
    let typed: F = parse(kind, draft)?;
    guarded("field map", || typed.seam_field_map())
}

#[inline(never)]
fn identity_as<F: Seam>(kind: AgentFormKind, draft: &Value) -> Result<DraftIdentity, String> {
    let typed: F = parse(kind, draft)?;
    Ok(typed.seam_identity())
}

/// Recompute `draft` in place, as the page does after every edit.
pub fn compute(kind: AgentFormKind, draft: &mut Value) -> Result<(), String> {
    with_agent_kind!(kind, F => compute_as::<F>(kind, draft))
}

/// Resolve `draft`'s penalty inputs from the profile as its page's open
/// does (`QueueableForm::apply_penalty_profile`): each item's due date as
/// the Calendar resolves it (global plus profile deadline overrides,
/// decision P5-9) and the profile's EOPT class. Only a Draft is touched.
/// A form without penalty items — and 1601C and 2551Q, which keep their
/// own penalty state — is unchanged.
pub fn apply_penalty_profile(
    kind: AgentFormKind,
    draft: &mut Value,
    profile: &crate::profile::TaxpayerProfile,
    global_overrides: &[crate::calendar_rules::DeadlineOverride],
) -> Result<(), String> {
    let AgentFormKind::Queueable(queueable) = kind else {
        return Ok(());
    };
    crate::with_queueable_kind!(queueable, F => {
        let mut typed: F = parse(kind, draft)?;
        if typed.lifecycle().is_editable() {
            typed.apply_penalty_profile(profile, global_overrides);
            *draft = to_json(&typed)?;
        }
        Ok(())
    })
}

/// Bring a stored draft to the current model before any box is written, so
/// a write is never undone by a migration that runs later in the compute:
/// a 2551Q draft saved before Items 20-22 took overrides adopts its manual
/// amounts once ([`Form2551QDraft::adopt_legacy_manual_penalties`]). Every
/// other form is unchanged.
pub fn prepare_for_writes(kind: AgentFormKind, draft: &mut Value) -> Result<(), String> {
    if kind != AgentFormKind::Form2551Q {
        return Ok(());
    }
    // A draft that does not parse yet is left to the write's own checks.
    let Ok(mut typed) = parse::<Form2551QDraft>(kind, draft) else {
        return Ok(());
    };
    if typed.adopt_legacy_manual_penalties() {
        *draft = to_json(&typed)?;
    }
    Ok(())
}

/// The page's Validate: recompute `draft` in place, then run the form's
/// checks. Errors are `(field, message)` exactly as the page shows them.
pub fn validate(kind: AgentFormKind, draft: &mut Value) -> Result<Vec<(String, String)>, String> {
    with_agent_kind!(kind, F => validate_as::<F>(kind, draft))
}

/// Official control id -> value for `draft` as it stands.
pub fn field_map(kind: AgentFormKind, draft: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut fields = with_agent_kind!(kind, F => field_map_as::<F>(kind, draft))?;
    // A previous-payment amount belongs only to an amended return. While the
    // gate is closed the map writes the box's empty zero even if a corrupt
    // draft reaches this map without a recompute: the upload never carries
    // the dormant amount, and the assistant's form card still shows the box
    // at the zero the return holds.
    if let Some(spec) = draft_locks::previous_payment::spec(kind)
        && draft_locks::previous_payment::lock_reason(spec, draft).is_some()
    {
        fields.insert(spec.key.to_string(), "0.00".to_string());
    }
    Ok(fields)
}

/// TIN, year, period and filing status of `draft`.
pub fn identity(kind: AgentFormKind, draft: &Value) -> Result<DraftIdentity, String> {
    with_agent_kind!(kind, F => identity_as::<F>(kind, draft))
}

/// A new amended return copied from one that reached BIR (Submitted or
/// Confirmed), the way the user's "Revert to Draft" amends it
/// (`Database::revert_queueable_for_amendment` and its 1601C/2551Q twins),
/// but in memory: back to Draft, its "Amended Return?" answer set to Yes the
/// way its editor does, and no row id. Nothing is stored and the filed
/// original is not touched; saving it goes through that revert, which keeps
/// the original as an amendment snapshot. Refused for a return that never
/// left the app, a Paid one, and a form with no amended-return answer.
pub fn amended_copy(kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    let status = identity(kind, draft)?.status;
    if !matches!(status, FilingStatus::Submitted | FilingStatus::Confirmed) {
        return Err(format!(
            "only a submitted or confirmed {} return can be amended (this one is {status:?})",
            kind.form_code()
        ));
    }
    // A dry run reached nobody: there is no filed return to amend.
    if crate::forms::dry_run::is_dry_run(&status, crate::forms::dry_run::json_marks_dry_run(draft))
    {
        return Err(format!(
            "this {} return was a dry run: nothing reached the BIR, so it is sent again as the original, not amended",
            kind.form_code()
        ));
    }
    match kind {
        AgentFormKind::Form1601C => amended_1601c(kind, draft),
        AgentFormKind::Form2551Q => amended_2551q(kind, draft),
        AgentFormKind::Queueable(queueable) => {
            crate::with_queueable_kind!(queueable, F => amended_queueable::<F>(kind, draft))
        }
    }
}

#[inline(never)]
fn amended_1601c(kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    let mut copy: Form1601CDraft = parse(kind, draft)?;
    copy.revert_to_draft();
    copy.mark_amended();
    copy.id = None;
    to_json(&copy)
}

#[inline(never)]
fn amended_2551q(kind: AgentFormKind, draft: &Value) -> Result<Value, String> {
    let mut copy: Form2551QDraft = parse(kind, draft)?;
    copy.revert_to_draft();
    copy.mark_amended();
    copy.id = None;
    to_json(&copy)
}

#[inline(never)]
fn amended_queueable<F: QueueableForm + Serialize + DeserializeOwned>(
    kind: AgentFormKind,
    draft: &Value,
) -> Result<Value, String> {
    let mut copy: F = parse(kind, draft)?;
    copy.lifecycle_mut().revert_to_draft();
    if !copy.mark_amended() {
        return Err(format!("{} has no amended-return answer", kind.form_code()));
    }
    copy.set_draft_id(None);
    to_json(&copy)
}

/// Save an editable Draft the way its page's "Save draft" does, recomputed
/// first; returns the `form_drafts` row id (also written into `draft`).
/// Refuses anything that is not a Draft. Never queues.
pub fn save_draft(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &mut Value,
) -> Result<i64, String> {
    let incoming = draft.clone();
    save_draft_with_clearance_context(kind, db, draft, incoming, BTreeSet::new())
}

/// [`save_draft`] with the draft as it was before the fill being saved and
/// the gate keys that fill already showed inline. Hidden test/host plumbing:
/// callers normally use [`save_draft`].
#[doc(hidden)]
pub fn save_draft_with_clearance_context(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &mut Value,
    incoming: Value,
    reported: BTreeSet<String>,
) -> Result<i64, String> {
    // The caller's bytes are only the fallback before-image. When the row
    // already exists, the transactional save reads its stored bytes and those
    // decide whether this recompute clears a dormant gated value: an assistant
    // or page fill may already have recomputed its in-memory draft.
    compute(kind, draft)?;
    let after = draft.clone();
    if identity(kind, draft)?.status != FilingStatus::Draft {
        return Err("this return is no longer a draft".to_string());
    }
    let writer =
        move || pending_settings_writer(kind, incoming.clone(), after.clone(), reported.clone());
    let id = match kind {
        AgentFormKind::Form1601C => save_1601c(kind, db, draft, writer())?,
        AgentFormKind::Form2551Q => save_2551q(kind, db, draft, writer())?,
        // The 2550Q page keeps its own row format (`save_form_draft_v2`).
        AgentFormKind::Queueable(QueueableKind::Form2550Q) => {
            save_2550q(kind, db, draft, writer())?
        }
        AgentFormKind::Queueable(queueable) => crate::with_queueable_kind!(queueable, F => {
            save_queueable::<F>(kind, db, draft, writer())?
        }),
    };
    if let Value::Object(object) = draft {
        object.insert("id".to_string(), Value::from(id));
    }
    Ok(id)
}

#[doc(hidden)]
pub fn pending_settings_writer(
    kind: AgentFormKind,
    incoming: Value,
    after: Value,
    reported: BTreeSet<String>,
) -> crate::db::SettingsWriter<'static> {
    Some(Box::new(move |draft_id, stored_json| {
        let before = stored_json
            .and_then(|stored| serde_json::from_str::<Value>(stored).ok())
            .unwrap_or_else(|| incoming.clone());
        Ok(draft_locks::pending_notices::settings_writes(
            kind, &before, &incoming, &after, draft_id, &reported,
        ))
    }))
}

#[inline(never)]
fn save_1601c(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &Value,
    settings_writer: crate::db::SettingsWriter<'static>,
) -> Result<i64, String> {
    let typed: Form1601CDraft = parse(kind, draft)?;
    db.save_1601c_draft_with_settings(&typed, settings_writer)
        .map_err(|error| error.to_string())
}

#[inline(never)]
fn save_2551q(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &Value,
    settings_writer: crate::db::SettingsWriter<'static>,
) -> Result<i64, String> {
    let typed: Form2551QDraft = parse(kind, draft)?;
    db.save_2551q_draft_with_settings(&typed, settings_writer)
        .map_err(|error| error.to_string())
}

#[inline(never)]
fn save_queueable<F: QueueableForm + Serialize + DeserializeOwned>(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &Value,
    settings_writer: crate::db::SettingsWriter<'static>,
) -> Result<i64, String> {
    let typed: F = parse(kind, draft)?;
    db.save_queueable_draft_with_settings(&typed, settings_writer)
        .map_err(|error| error.to_string())
}

#[inline(never)]
fn save_2550q(
    kind: AgentFormKind,
    db: &crate::db::Database,
    draft: &Value,
    settings_writer: crate::db::SettingsWriter<'static>,
) -> Result<i64, String> {
    Ok({
        let typed: super::form_2550q::Form2550QDraft = parse(kind, draft)?;
        let quarter = typed.quarter_number().unwrap_or(0);
        // Its row path upserts by period: never over a return that left Draft.
        if let Some(stored) = db
            .get_form_draft_v2::<super::form_2550q::Form2550QDraft>(
                &typed.tin,
                "2550Q",
                typed.taxable_year,
                &FilingPeriod::Quarterly(quarter),
            )
            .map_err(|error| error.to_string())?
            && stored.lifecycle.status != FilingStatus::Draft
        {
            return Err(format!(
                "the 2550Q return for this quarter is {:?}; it can no longer change",
                stored.lifecycle.status
            ));
        }
        db.save_form_draft_v2_with_settings(
            &typed.tin,
            "2550Q",
            typed.taxable_year,
            &FilingPeriod::Quarterly(quarter),
            &typed.lifecycle.status,
            (&typed, settings_writer),
        )
        .map_err(|error| error.to_string())?
    })
}

/// Parse `draft` as `kind`'s draft type, recompute it and return the field
/// map. Used by [`probe`]; `None` when the value is not a valid draft or the
/// form code panics on it.
fn probe_eval(kind: AgentFormKind, draft: &Value) -> Option<(Value, BTreeMap<String, String>)> {
    with_agent_kind!(kind, F => probe_eval_as::<F>(kind.form_code(), draft))
}

#[inline(never)]
fn probe_eval_as<F: Seam>(code: &str, draft: &Value) -> Option<(Value, BTreeMap<String, String>)> {
    let mut typed: F = serde_json::from_value(draft.clone()).ok()?;
    catch_unwind(AssertUnwindSafe(|| {
        typed.seam_compute();
        let mut map = typed.seam_field_map();
        // A box keyed as the first occurrence of a control the field
        // map writes once shows that control's value.
        for (form, alias, _) in probe::FIRST_OCCURRENCE {
            if *form == code
                && let Some((official, _)) = alias.split_once("#occurrence-")
                && let Some(value) = map.get(official).cloned()
            {
                map.entry(alias.to_string()).or_insert(value);
            }
        }
        (serde_json::to_value(&typed).ok(), map)
    }))
    .ok()
    .and_then(|(json, map)| json.map(|json| (json, map)))
}

/// The deserialization error for `draft`, if any (used by [`probe`] to read
/// an enum's variants).
fn parse_error(kind: AgentFormKind, draft: &Value) -> Option<String> {
    with_agent_kind!(kind, F => parse_error_as::<F>(draft))
}

#[inline(never)]
fn parse_error_as<F: DeserializeOwned>(draft: &Value) -> Option<String> {
    serde_json::from_value::<F>(draft.clone())
        .err()
        .map(|error| error.to_string())
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

// ------------------------------------------------------------------ locks
//
// The effective lock every writer shares (decision P4-19): what the form
// page's `locks` map shows and what `fill` refuses are one computation.
// The page-specific parts — which keys the page's host writes through its
// own editors, which on-screen keys repeat an official control, and the
// penalty groups the page draws — are pinned data (`data/agent-locks.json`,
// generated from the page's own classifiers and schema by bir-app-api's
// `agent_locks` tests; regenerate with
// `UPDATE_AGENT_LOCKS=1 cargo test -p bir-app-api agent_locks_table`).
//
// Two layers, both here: the base map ([`locks`], PR 15) and the
// draft-dependent locks over it ([`draft_locks`], PR 17: the row families
// the page relocks from the draft as it stands, and each computed penalty
// group the filer has not edited). The page's open applies `draft_locks`
// on top of `locks` and its fill reports `live_locks`; `fill` (the
// assistant) refuses what either locks, judged on the draft as it stands
// when the key is written. The page's own writers (`fill_as_user` and the
// rest) do not consult the draft layer: the page's UX governs those boxes
// (its Edit penalties opens a group, sent as `unlocked`).

/// The draft the taxpayer profile alone makes for a return's period, and
/// its box values (official key -> string): what decides whether the
/// profile filled a box ([`profile_blank`]). The two writers build the
/// values differently:
///
/// - the form page (bir-shell `opened_json`) recomputes the profile's own
///   fresh draft (`load_draft(..., fresh = true)`) and reads it with
///   `values_json`: the [`field_map`] plus each duplicate occurrence the
///   page shows (`txtEmail#occurrence-1`) and the page's display-only
///   text (the row modules' additions, an unescaped line of business);
/// - the desktop assistant (bir-shell `profile_context`) builds the draft
///   for the open return's TIN, year and period and reads it with
///   [`field_map`] alone.
///
/// The two maps differ, but [`profile_blank`] reads a bound box's own
/// draft field before them, so the verdicts do not: bir-shell's
/// `the_page_and_the_desktop_lock_alike_for_a_full_profile` and
/// `..._for_a_sparse_profile` (`screen_form_module/tests/agent_locks.rs`)
/// pin [`locks`] and the [`fill`] verdicts as identical for both contexts
/// of one profile draft, on every form.
#[derive(Debug, Clone)]
pub struct FromProfile {
    pub draft: Value,
    pub values: Value,
}

/// A package's page-side lock data (`data/agent-locks.json`): keys the
/// page's host writes through its own editors (`catalogs::host_writes` and
/// the row modules', bound or not), the on-screen keys that repeat an
/// official control id (`#occurrence-`) without a binding of their own, and
/// the penalty groups the page's schema draws
/// ([`draft_locks::penalties::groups`]).
#[derive(Debug, Default, Deserialize)]
struct AgentLocks {
    #[serde(default)]
    host_written: BTreeSet<String>,
    #[serde(default)]
    occurrence_aliases: BTreeMap<String, String>,
    #[serde(default)]
    penalty_groups: Vec<draft_locks::penalties::Group>,
}

fn agent_locks(package: &str) -> &'static AgentLocks {
    static PARSED: OnceLock<BTreeMap<String, AgentLocks>> = OnceLock::new();
    static EMPTY: AgentLocks = AgentLocks {
        host_written: BTreeSet::new(),
        occurrence_aliases: BTreeMap::new(),
        penalty_groups: Vec::new(),
    };
    PARSED
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../../data/agent-locks.json"))
                .expect("data/agent-locks.json parses")
        })
        .get(package)
        .unwrap_or(&EMPTY)
}

/// Whether the page's host writes `key` through its own editors (a row
/// cell, a catalog control, a gate a row module owns), never through the
/// box's draft binding. Bound or not, `fill` refuses it: a write by box
/// key would bypass the host's own logic.
pub fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    agent_locks(kind.package()).host_written.contains(key)
}

/// On-screen keys that repeat an official control id
/// (`txtEmail#occurrence-2`) and have no binding of their own, each with
/// that official key. The draft holds one value per official key and the
/// field map, the XML and the print sheet all write it under the official
/// key, so every occurrence shows that one value. Read-only: `fill`
/// refuses these keys, so every write goes through a bound box.
pub fn occurrence_aliases(kind: AgentFormKind) -> Vec<(&'static String, &'static str)> {
    agent_locks(kind.package())
        .occurrence_aliases
        .iter()
        .map(|(key, official)| (key, official.as_str()))
        .collect()
}

/// The filer's own name or registered name. The open draft copies it from
/// the taxpayer profile, but the probe compares raw profile strings and
/// the forms print it in capitals, so it is not always detected as a
/// profile box. Spouse, payee, buyer and seller names stay typed.
pub fn is_profile_name(key: &str) -> bool {
    let tail = key.rsplit(':').next().unwrap_or(key).to_ascii_lowercase();
    let Some(rest) = tail.strip_prefix("txt") else {
        return false;
    };
    let stem = rest.trim_end_matches(|c: char| c.is_ascii_digit());
    ["taxpayername", "registeredname"].iter().any(|name| {
        stem.strip_suffix(name).is_some_and(|prefix| {
            prefix
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, 'p' | 'g' | 't' | 'i' | 'm'))
        })
    })
}

/// Whether `key`'s own binding carries [`Lock::Profile`].
pub fn has_profile_lock(def: &AgentFormDef, key: &str) -> bool {
    matches!(
        def.bindings.get(key),
        Some(BoundBox {
            lock: Some(Lock::Profile),
            ..
        })
    )
}

/// A box the taxpayer profile fills: bound (itself or through its item's
/// group box) with a profile lock, or the filer's own name.
pub fn is_profile_box(def: &AgentFormDef, entry: &BoxDef) -> bool {
    !entry.computed
        && entry.is_user_box()
        && def.bindings.contains_key(&entry.key)
        && (has_profile_lock(def, &entry.key)
            || is_profile_name(&entry.key)
            || entry
                .group
                .as_deref()
                .is_some_and(|group| has_profile_lock(def, group)))
}

/// Whether the profile left `key` blank. A bound box reads the draft field
/// it writes (a Page 2 header bound to `/taxpayer_name` is filled when the
/// name is, whatever its own field-map slot shows); a field the draft does
/// not have yet falls back to the field map, and a box missing there too
/// counts as filled. Blank: no text, an all-zero code (the official
/// runtime's empty RDO, `000`), zero, false, or no radio of its item
/// ticked.
pub fn profile_blank(def: &AgentFormDef, key: &str, from_profile: &FromProfile) -> bool {
    let zeros = |text: &str| text.trim().chars().all(|c| c == '0');
    let shown = |key: &str| {
        from_profile.values[key]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string()
    };
    if let Some(
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. },
    ) = def.bindings.get(key).map(|bound| &bound.binding)
    {
        return match from_profile.draft.pointer(path) {
            Some(Value::Null) => true,
            Some(Value::String(text)) => zeros(text),
            Some(Value::Number(number)) => number.as_f64() == Some(0.0),
            Some(Value::Bool(set)) => !set,
            Some(_) => false,
            None => from_profile
                .values
                .get(key)
                .is_some_and(|_| zeros(&shown(key))),
        };
    }
    let entry = def.box_def(key);
    let group = match entry {
        Some(entry) if entry.radio_group => Some(entry),
        Some(entry) => entry.group.as_deref().and_then(|group| def.box_def(group)),
        None => None,
    };
    match (group, entry) {
        (Some(group), _) => !group.members.iter().any(|member| shown(member) == "true"),
        (None, Some(entry)) if entry.is_radio() => shown(key) != "true",
        _ => zeros(&shown(key)),
    }
}

/// Why a box key refuses a write, the families the page's lock map and
/// [`fill_box`] read from one computation. `None` from [`gate`] means the
/// key is open for this writer.
enum Gate {
    /// A repeated official control (`#occurrence-`): the reason says where
    /// the one bound box is filled instead.
    Alias(String),
    /// Not a box on this form at all.
    NotABox,
    /// Computed by BIR's own formulas.
    Computed,
    /// A hidden control, not a box.
    Hidden,
    /// The page's host writes it through its own editors; a write by box
    /// key would bypass them.
    HostWritten,
    /// A bound box's effective lock: its own, the filer's own name read as
    /// the profile's, or its radio item's group lock. The `allowed` list
    /// of [`fill_box`] may lift it.
    Lock(Lock),
    /// A user box no binding writes: the `unmapped` reason, or no field in
    /// this app's draft.
    Unmapped,
}

/// What an occurrence alias's lock says: the alias repeats `official`
/// (never written by key itself), so the reason redirects there — or says
/// the page cannot write that official field at all.
fn alias_reason(def: &AgentFormDef, official: &str, from_profile: Option<&FromProfile>) -> String {
    match def.bindings.get(official) {
        Some(BoundBox {
            lock: Some(Lock::Profile),
            ..
        }) => {
            if from_profile.is_some_and(|fp| profile_blank(def, official, fp)) {
                format!("repeats `{official}`; fill it there")
            } else {
                format!("repeats `{official}` from the tax profile; use Edit on that box")
            }
        }
        Some(BoundBox {
            lock: Some(lock), ..
        }) => lock.reason().to_string(),
        Some(_) => format!("repeats `{official}`; fill it there"),
        None => format!("repeats the official field `{official}`, which this page cannot write"),
    }
}

/// The one effective verdict for `key`: the same families, in the same
/// order, as the page's lock map decides them — an occurrence alias first
/// (it overwrites whatever the box's own classification said), then the
/// box's own gate. `from_profile = None` cannot prove a profile box blank,
/// so the profile lock stands (the conservative verdict).
fn gate(def: &AgentFormDef, key: &str, from_profile: Option<&FromProfile>) -> Option<Gate> {
    if let Some(official) = agent_locks(&def.package).occurrence_aliases.get(key) {
        return Some(Gate::Alias(alias_reason(def, official, from_profile)));
    }
    let Some(entry) = def.box_def(key) else {
        return Some(Gate::NotABox);
    };
    if entry.computed || entry.required == "computed" {
        return Some(Gate::Computed);
    }
    if !entry.is_user_box() {
        return Some(Gate::Hidden);
    }
    if agent_locks(&def.package).host_written.contains(key) {
        return Some(Gate::HostWritten);
    }
    match def.bindings.get(key) {
        Some(bound) => {
            // The filer's own name reads as the profile's; a radio member
            // without a lock of its own takes its item's group lock (as the
            // page's bound_lock falls back — so an election item's every
            // radio is refused to the assistant, not only the pinned ones).
            let lock = if is_profile_name(key) {
                Some(Lock::Profile)
            } else {
                bound.lock.or_else(|| {
                    entry
                        .group
                        .as_deref()
                        .and_then(|group| def.bindings.get(group))
                        .and_then(|bound| bound.lock)
                })
            };
            match lock {
                Some(Lock::Profile)
                    if is_profile_box(def, entry)
                        && from_profile.is_some_and(|fp| profile_blank(def, key, fp)) =>
                {
                    None
                }
                Some(lock) => Some(Gate::Lock(lock)),
                None => None,
            }
        }
        None => Some(Gate::Unmapped),
    }
}

impl Gate {
    /// The reason this gate's key shows in the page's `locks` map, or
    /// `None` when the page draws no lock for it: a hidden control is
    /// never shown, an election is the user's own answer, and a box the
    /// host writes stays open on the page — all refused to `fill` alike.
    fn page_reason(&self, def: &AgentFormDef, key: &str) -> Option<String> {
        match self {
            Self::Alias(reason) => Some(reason.clone()),
            Self::Computed => Some("computed by BIR's own formulas".to_string()),
            Self::Lock(lock @ (Lock::Profile | Lock::Identity)) => Some(lock.reason().to_string()),
            Self::Unmapped => Some(format!(
                "cannot be filled here: {}",
                def.unmapped
                    .get(key)
                    .map(String::as_str)
                    .unwrap_or("it has no field in this app's draft")
            )),
            Self::NotABox | Self::Hidden | Self::HostWritten | Self::Lock(Lock::Election) => None,
        }
    }

    /// `fill`'s refusal text for this gate. The families that existed
    /// before read exactly as `def.refusal` gave them; the new ones name
    /// their redirect (an alias) or the host that owns the box.
    fn fill_reason(&self, def: &AgentFormDef, key: &str) -> String {
        match self {
            Self::Alias(reason) => reason.clone(),
            Self::Lock(lock) => format!("`{key}` {}", lock.reason()),
            Self::HostWritten => {
                // The probe's unmapped note says where the real edit is
                // ("the ATC popup's tick box…: chosen in the row"); keep it.
                let detail = def
                    .unmapped
                    .get(key)
                    .map(|note| format!(": {note}"))
                    .unwrap_or_default();
                format!(
                    "`{key}` cannot be filled from here: it is written through the form's own editors{detail}"
                )
            }
            Self::NotABox | Self::Computed | Self::Hidden | Self::Unmapped => def
                .refusal(key)
                .expect("each of these gates is a refusal()"),
        }
    }
}

/// Why each box is locked on the page: the base of the lock map
/// `screen_form_open` shows. A profile box is locked only where the profile
/// gave it a value ([`FromProfile`]; the page's Edit then opens it for this
/// return); one the profile left blank loses its profile lock and is an
/// ordinary box, whatever the filer has typed into it since.
///
/// The draft-dependent layer goes on top ([`draft_locks`], also core-side;
/// see the `locks` section's note): the page's open applies it over this
/// map, and [`fill`] refuses what it locks too. This is the shared base
/// every writer — page and assistant alike — reads.
pub fn locks(kind: AgentFormKind, from_profile: &FromProfile) -> Map<String, Value> {
    let def = kind.definition();
    let agent_locks = agent_locks(&def.package);
    let mut locks = Map::new();
    for key in def.boxes.iter().map(|entry| entry.key.as_str()) {
        // The page draws no lock for a box it writes through its own
        // editors — a computed one included: the host's edit is open to
        // the filer. `fill` still refuses it (its gate is unaffected).
        if agent_locks.host_written.contains(key) {
            continue;
        }
        if let Some(gate) = gate(def, key, Some(from_profile))
            && let Some(reason) = gate.page_reason(def, key)
        {
            locks.insert(key.to_string(), Value::String(reason));
        }
    }
    // A duplicate occurrence's reason is inserted after the boxes' pass:
    // it overwrites whatever the alias key's own box classification said.
    for key in agent_locks.occurrence_aliases.keys().map(String::as_str) {
        if let Some(gate) = gate(def, key, Some(from_profile))
            && let Some(reason) = gate.page_reason(def, key)
        {
            locks.insert(key.to_string(), Value::String(reason));
        }
    }
    locks
}

/// Boxes the taxpayer profile filled (`from_profile`), plus repeated
/// occurrences of one; a box the profile left blank is not listed. The
/// extracted schema's auto flag is a guess; this is what the app does.
pub fn profile_keys(kind: AgentFormKind, from_profile: &FromProfile) -> Vec<String> {
    let def = kind.definition();
    let mut keys: Vec<String> = def
        .boxes
        .iter()
        .filter(|entry| is_profile_box(def, entry) && !profile_blank(def, &entry.key, from_profile))
        .map(|entry| entry.key.clone())
        .collect();
    for (key, official) in occurrence_aliases(kind) {
        if has_profile_lock(def, official) && !profile_blank(def, official, from_profile) {
            keys.push(key.clone());
        }
    }
    keys
}

/// The filled profile boxes the page may open with Edit: the edit stays on
/// this return (`screen_form_fill`'s `unlocked`) and never reaches the
/// profile. Repeated occurrences are not here; they follow their box.
pub fn profile_edit_keys(kind: AgentFormKind, from_profile: &FromProfile) -> Vec<String> {
    let def = kind.definition();
    profile_keys(kind, from_profile)
        .into_iter()
        .filter(|key| {
            def.bindings.contains_key(key)
                && def
                    .box_def(key)
                    .is_some_and(|entry| entry.required != "computed")
        })
        .collect()
}

// ------------------------------------------------------------------ fill

/// Write official box `key` of `draft` to `value` (no recompute; call
/// [`compute`] after the last box). Refuses exactly what the page locks
/// against the assistant: computed, locked, hidden, unmapped, host-written
/// and occurrence-alias keys, each with the page's reason.
///
/// It also refuses what the page's draft-dependent layer locks
/// ([`draft_locks`]), judged on `draft` as it stands when this key is
/// written — so in a batch, on the boxes written before it — with
/// `` `{key}` {reason} `` and the page's reason: an unedited computed
/// penalty group (the assistant has no Edit penalties; once the group holds
/// a typed amount it is open), 2000's 15B and 15C, 2553's classification
/// and quarter boxes, 1702RT's shut item 42, and the rest of that layer.
///
/// `from_profile` is the draft the profile alone makes for this return's
/// period and its field map ([`FromProfile`], as [`locks`] reads): a
/// profile box the profile left blank is an ordinary box and takes the
/// write. `None` cannot prove the blank, so every profile box keeps its
/// lock — the conservative verdict.
pub fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    from_profile: Option<&FromProfile>,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[], from_profile, false)
}

/// The option of choice box `key` that a fill of `value` selects, spelled as
/// the binding spells it: the same matching of labels, aliases and radio
/// keys the write uses. `None` when `key` is not a choice or nothing matches.
pub fn chosen_option(kind: AgentFormKind, key: &str, value: &Value) -> Option<String> {
    let def = kind.definition();
    let Some(Binding::Choice { options }) = def.bindings.get(key).map(|bound| &bound.binding)
    else {
        return None;
    };
    choose(def, key, options, &value_text(value)).map(|option| option.value.clone())
}

/// Drop the answer of choice box `key`, so the form asks for it again where
/// it applies: a stopped box's stored answer is not reused. A box that is
/// not one choice over one draft field is left as it is. So is a refused
/// one (a choice the return cannot hold unanswered): `draft` is unchanged
/// and the caller may go on with it.
pub fn forget(kind: AgentFormKind, draft: &mut Value, key: &str) -> Result<(), String> {
    let def = kind.definition();
    let Some(Binding::Choice { options }) = def.bindings.get(key).map(|bound| &bound.binding)
    else {
        return Ok(());
    };
    let Some(path) = single_choice_path(options) else {
        return Ok(());
    };
    if draft.pointer(path).is_none_or(Value::is_null) {
        return Ok(());
    }
    let mut dropped = draft.clone();
    set_path(&mut dropped, path, Value::Null, &def.rows)?;
    with_agent_kind!(kind, F => parse::<F>(kind, &dropped).map(|_| ()))
        .map_err(|error| format!("`{key}` cannot be left unanswered: {error}"))?;
    *draft = dropped;
    Ok(())
}

/// [`fill`] for the user's own form page. Two differences: an income-tax
/// election ([`Lock::Election`]) is the user's to answer, and a bound box
/// the host writes through its own editors writes — the page's edit
/// pipeline is that host. Identity, profile, computed, hidden and unbound
/// boxes are refused exactly as in [`fill`]. Like every page writer it does
/// not consult the draft-dependent layer ([`draft_locks`]): the page's own
/// UX governs those boxes (its Edit penalties opens a group).
pub fn fill_as_user(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(kind, draft, key, value, &[Lock::Election], None, true)
}

/// [`fill_as_user`] for a box the user chose to edit on their own form page
/// after it was filled from the profile ([`Lock::Profile`]): the edit stays
/// on this return's draft and the profile is never written. The assistant is
/// still refused profile boxes through [`fill`]; identity, computed, hidden
/// and unbound boxes are refused exactly as in [`fill`], and a bound
/// host-written box writes as in [`fill_as_user`].
pub fn fill_profile_box_as_user(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(
        kind,
        draft,
        key,
        value,
        &[Lock::Election, Lock::Profile],
        None,
        true,
    )
}

/// Draft fields that hold a return's own period. Exact names: a field that
/// only mentions a period (`/estimated_life_months`, the Calendar answer of
/// "For the") is an answer, not the period.
pub const PERIOD_FIELDS: &[&str] = &[
    "year",
    "taxable_year",
    "month",
    "quarter",
    "day",
    "transaction_year",
    "transaction_month",
    "transaction_day",
    "transaction_date",
    "year_end_month",
    "year_end_day",
    "year_end_year",
    "is_short_period",
    "due_day",
];

/// The draft paths a binding writes.
pub fn binding_paths(binding: &Binding) -> Vec<&str> {
    match binding {
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. } => {
            vec![path.as_str()]
        }
        Binding::Choice { options } => options
            .iter()
            .flat_map(|option| option.set.keys().map(String::as_str))
            .collect(),
    }
}

/// Whether every path a binding writes is one of [`PERIOD_FIELDS`].
pub fn writes_only_the_period(binding: &Binding) -> bool {
    let paths = binding_paths(binding);
    !paths.is_empty()
        && paths.iter().all(|path| {
            path.rsplit('/')
                .next()
                .is_some_and(|leaf| PERIOD_FIELDS.contains(&leaf))
        })
}

/// Build a new draft's period: write a box whose binding writes only
/// [`PERIOD_FIELDS`] (year, month, day, a date), whatever its
/// [`Lock::Identity`]. For a host constructing a draft for a period it chose
/// (a library sample rebuilt for the period its filer completed); never for
/// an edit a page sends, which keeps the period of a return locked through
/// every other writer. The TIN, profile, computed, hidden and unbound boxes
/// are refused as in [`fill_as_user`].
pub fn write_period_box(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    fill_box(
        kind,
        draft,
        key,
        value,
        &[Lock::Election, Lock::Identity],
        None,
        true,
    )
}

pub use super::{DYNAMIC_SCHEDULE_ROW_LIMIT, DYNAMIC_SCHEDULE_ROW_LIMIT_MESSAGE};

fn dynamic_write_has_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Bool(on) => *on,
        Value::Number(_) | Value::Array(_) | Value::Object(_) => true,
    }
}

fn digits_suffix(text: &str) -> Option<usize> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())?
}

fn schedule_1601c_row(key: &str) -> Option<usize> {
    for stem in [
        "frm1601c:sched1:txtMonthYear",
        "frm1601c:sched1:txtDatePaid",
        "frm1601c:sched1:txtBankCode",
        "frm1601c:sched1:txtNumber",
        "frm1601c:sched1:txtTaxPaid",
        "frm1601c:sched1:txtShouldTaxDue",
        "frm1601c:sched1:txtAdjustments",
    ] {
        if let Some(suffix) = key.strip_prefix(stem) {
            return digits_suffix(suffix);
        }
    }
    None
}

fn others_2200a_row(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("frm2200Av2020:txtSched1_xa_others")?;
    let (digits, column) = rest.split_once('_')?;
    if !matches!(
        column,
        "Check" | "Atc" | "Desc" | "Bracket" | "Rate" | "Export" | "Taxable" | "Due"
    ) {
        return None;
    }
    digits_suffix(digits)
}

fn incomplete_2200a_row(row: &super::form_2200a::Form2200AOtherRow, n: usize) -> Option<String> {
    let atc = row.atc_digits.trim();
    if atc.is_empty() {
        Some(format!("ATC is required in row #{n}"))
    } else if atc.len() != 3 || !atc.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(format!("Please supply valid ATC Code for row #{n}"))
    } else if row.description.trim().is_empty() {
        Some(format!("Description is required in row #{n}"))
    } else if row.bracket.trim().is_empty() {
        Some(format!(
            "Tax Bracket/Unit of Measure is required in row #{n}"
        ))
    } else if row.rate.trim().is_empty() {
        Some(format!("Applicable Tax Rate is required in row #{n}"))
    } else {
        None
    }
}

/// The page/assistant write refusal of the two runtime-row schedules. It is
/// evaluated before box binding so row 13, deliberately absent from the
/// writable bindings, still receives the cap sentence. 2200A follows the
/// official `isValidDataOnSched1` (`BIR-Form2200Av2020.hta` lines 7426-7462):
/// the Add button may not start a new row past the printed three while an
/// earlier row is incomplete. 1601C's inventoried `addSchedule` evidence has no completeness guard
/// (`official-hta-runtime#addSchedule:L3349-L3385`), so only its cap applies.
pub fn dynamic_schedule_write_refusal(
    kind: AgentFormKind,
    draft: &Value,
    key: &str,
    value: &Value,
) -> Option<String> {
    if !dynamic_write_has_value(value) {
        return None;
    }
    if kind == AgentFormKind::Form1601C
        && let Some(row) = schedule_1601c_row(key)
    {
        return (row >= DYNAMIC_SCHEDULE_ROW_LIMIT)
            .then(|| DYNAMIC_SCHEDULE_ROW_LIMIT_MESSAGE.to_string());
    }
    if kind.form_code() != "2200A" {
        return None;
    }
    let row = others_2200a_row(key)?;
    if row > DYNAMIC_SCHEDULE_ROW_LIMIT {
        return Some(DYNAMIC_SCHEDULE_ROW_LIMIT_MESSAGE.to_string());
    }
    if row <= 3 {
        return None;
    }
    let typed: super::form_2200a::Form2200ADraft = serde_json::from_value(draft.clone()).ok()?;
    if typed
        .others
        .get(row - 1)
        .is_some_and(|current| !current.is_blank())
    {
        return None;
    }
    for n in 1..row {
        let earlier = typed.others.get(n - 1).cloned().unwrap_or_default();
        if let Some(error) = incomplete_2200a_row(&earlier, n) {
            return Some(error);
        }
    }
    None
}

fn fill_box(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    allowed: &[Lock],
    from_profile: Option<&FromProfile>,
    as_host: bool,
) -> Result<(), String> {
    prepare_for_writes(kind, draft)?;
    let def = kind.definition();
    if let Some(error) = dynamic_schedule_write_refusal(kind, draft, key, value) {
        return Err(error);
    }
    let verdict = gate(def, key, from_profile);
    // The assistant (as_host = false) is refused what the page's
    // draft-dependent layer locks on the draft as it stands. A page writer
    // (as_host = true) never consults it: the page's own UX governs those
    // boxes (its Edit penalties opens a group).
    if !as_host
        && kind == AgentFormKind::Form2551Q
        && key == draft_locks::item16_2551q::KEY
        && draft_locks::item16_2551q::lock_reason(draft).is_some()
        && draft_locks::item16_2551q::zero_noop(value)
    {
        return Ok(());
    }
    if !as_host
        && let Some(spec) = draft_locks::previous_payment::spec(kind)
        && key == spec.key
        && draft_locks::previous_payment::lock_reason(spec, draft).is_some()
        && draft_locks::previous_payment::zero_write(value)
    {
        return Ok(());
    }
    if !as_host && let Some(lock) = draft_refusal(kind, def, draft, key, verdict.as_ref()) {
        return Err(format!("`{key}` {lock}"));
    }
    // Only the lock itself is lifted: a computed or hidden box, an alias
    // and an unbound key stay refused whatever `allowed` holds. A
    // host-written key stays refused to `fill` (as_host = false, the
    // assistant); a page writer (as_host = true) is that host's own edit
    // pipeline, so a bound host key writes — an unbound one has nothing to
    // write and keeps its refusal.
    let let_through = match verdict {
        Some(Gate::HostWritten) => as_host && def.bindings.contains_key(key),
        Some(Gate::Lock(Lock::Election)) => allowed.contains(&Lock::Election),
        Some(Gate::Lock(Lock::Profile)) => allowed.contains(&Lock::Profile),
        Some(Gate::Lock(Lock::Identity)) => {
            allowed.contains(&Lock::Identity)
                && def
                    .bindings
                    .get(key)
                    .is_some_and(|bound| writes_only_the_period(&bound.binding))
        }
        _ => false,
    };
    if let Some(gate) = verdict
        && !let_through
    {
        return Err(gate.fill_reason(def, key));
    }
    write_bound_box(kind, def, draft, key, value)
}

/// Why [`fill`] refuses `key` on a new return, or `None` when it may write
/// it: the same gate, so host-written boxes, aliases and group locks count,
/// and the computed penalty boxes, locked until Edit opens them in the app.
pub fn fill_refusal(kind: AgentFormKind, key: &str) -> Option<String> {
    let def = kind.definition();
    gate(def, key, None)
        .map(|verdict| verdict.fill_reason(def, key))
        .or_else(|| {
            draft_locks::penalties::computed_groups(kind)
                .iter()
                .any(|group| group.keys.iter().any(|k| k == key))
                .then(|| format!("`{key}` is {}", draft_locks::penalties::LOCK))
        })
}

/// The page's draft-dependent lock on `key` ([`draft_locks::draft_lock`]
/// over the base map's own entry for it), where that layer decides what the
/// page shows: a box the base map leaves open, or one whose base reason the
/// layer replaces (2000's 15B, which the probe calls derived, is computed).
/// A box the page draws no lock for that `fill` still refuses (a hidden
/// control, a host-written box, an election, not a box) keeps that refusal,
/// and so does a base lock the layer leaves as it is.
fn draft_refusal(
    kind: AgentFormKind,
    def: &AgentFormDef,
    draft: &Value,
    key: &str,
    verdict: Option<&Gate>,
) -> Option<String> {
    let base = match verdict {
        None => None,
        Some(gate) => Some(gate.page_reason(def, key)?),
    };
    let lock = draft_locks::draft_lock(kind, draft, key, base.as_deref())?;
    (base.as_deref() != Some(lock.as_str())).then_some(lock)
}

/// Write `value` at JSON pointer `path` of `draft`: over the value there,
/// or as a new leaf of the object at its parent; a path whose parent is not
/// an object is left as it is. The form page's edits write this way
/// (bir-shell's `set_draft_path` is this function), and so does the
/// penalty layer's trial ([`draft_locks::penalties::applies`]).
pub fn set_draft_path(draft: &mut Value, path: &str, value: Value) {
    if let Some(slot) = draft.pointer_mut(path) {
        *slot = value;
    } else if let Some((parent, leaf)) = path.rsplit_once('/')
        && let Some(Value::Object(object)) =
            draft.pointer_mut(if parent.is_empty() { "" } else { parent })
    {
        object.insert(leaf.to_string(), value);
    }
}

/// [`fill`] for a key the host itself edits — the page writes it through
/// its own editors (a row cell, a catalog control), and the desktop host
/// does the same for the keys its own gate owns (2553's item 13, where
/// `tax_treaty_2553` is the editor `rows_dst_pt_vat`'s cells are on the
/// page). Only the host-written gate is lifted — the caller applies the
/// host's own rule — so an unbound, computed, hidden, aliased or locked
/// box is refused exactly as in [`fill`]. Like every host writer it does
/// not consult the draft-dependent layer ([`draft_locks`]): the host's own
/// rule governs these boxes.
///
/// Never for a key the assistant names itself: `fill` keeps refusing it.
pub fn fill_host_owned(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    prepare_for_writes(kind, draft)?;
    let def = kind.definition();
    match gate(def, key, None) {
        None | Some(Gate::HostWritten) => {}
        Some(gate) => return Err(gate.fill_reason(def, key)),
    }
    write_bound_box(kind, def, draft, key, value)
}

fn write_bound_box(
    kind: AgentFormKind,
    def: &AgentFormDef,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    let employer_row = (kind.form_code() == "1701")
        .then(|| {
            if key.contains("chkPg2IShed1a_1") {
                Some(0)
            } else if key.contains("chkPg2IShed2a_2") {
                Some(1)
            } else {
                None
            }
        })
        .flatten();
    let before_owner = employer_row.and_then(|index| {
        draft
            .pointer(&format!("/employers/{index}/owner"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    write_bound_box_here(kind, def, draft, key, value)?;
    if let Some(index) = employer_row {
        let after_owner = draft
            .pointer(&format!("/employers/{index}/owner"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let switched = matches!(before_owner.as_deref(), Some("Taxpayer" | "Spouse"))
            && matches!(after_owner.as_deref(), Some("Taxpayer" | "Spouse"))
            && before_owner != after_owner;
        if switched {
            // The owner radios' only onclick is emplyrDetails(row, 'TP'/'SPS')
            // (BIR-Form1701v2018.hta :1347/:1358/:1396/:1407 -> :11135-11177):
            // that runtime handler blanks the losing NAME box, enables the
            // TIN boxes without clearing them, and leaves the row's own CI/TW
            // amounts untouched. TPName and SName bind the same stored name,
            // so clearing that one draft field is exactly the losing name box;
            // the TIN, compensation income and tax withheld stay with the row.
            if let Some(row) = draft
                .pointer_mut(&format!("/employers/{index}"))
                .and_then(Value::as_object_mut)
            {
                row.insert("employer_name".to_string(), Value::String(String::new()));
            }
        }
    }
    Ok(())
}

fn write_bound_box_here(
    kind: AgentFormKind,
    def: &AgentFormDef,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    let bound = def.bindings.get(key).expect("a gate-passed key is bound");
    let text = value_text(value);
    // 0605's split dates write through the typed draft: the generic path
    // writer cannot create the Option parents a fresh return leaves null
    // (B1).
    if kind == AgentFormKind::Queueable(QueueableKind::Form0605) {
        use crate::forms::form_0605::Form0605DateField as Which;
        match key {
            "frm0605:txtDueDateMonth" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::DueDate,
                    DatePart::Month,
                );
            }
            "frm0605:txtDueDateDay" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::DueDate,
                    DatePart::Day,
                );
            }
            "frm0605:txtDueDateYear" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::DueDate,
                    DatePart::Year,
                );
            }
            "frm0605:txtReturnPeriodMonth" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::ReturnPeriod,
                    DatePart::Month,
                );
            }
            "frm0605:txtReturnPeriodDay" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::ReturnPeriod,
                    DatePart::Day,
                );
            }
            "frm0605:txtReturnPeriodYear" => {
                return write_0605_date_part(
                    kind,
                    draft,
                    key,
                    &text,
                    Which::ReturnPeriod,
                    DatePart::Year,
                );
            }
            // Item 6's ATC and Item 8's tax type are picks from the
            // official lists, stored as the whole indexed code (B2).
            "txtATCCode" => return write_0605_pick(kind, draft, |d| d.set_atc(&text)),
            "txtTaxTypeCode" => return write_0605_pick(kind, draft, |d| d.set_tax_type(&text)),
            _ => {}
        }
    }
    // 2550M's Item 13A and Item 18 base boxes run blur handlers in the
    // official HTA (`compute13B`, `changedTxtTax13A`, `compute18TransactionbyRow`)
    // that fill the paired 12% tax box (and 7% Schedule 4 standard input tax
    // for 13A) while leaving those tax boxes editable for user overrides.
    if kind == AgentFormKind::Queueable(QueueableKind::Form2550M) {
        use crate::forms::form_2550m::Form2550MPurchase as Purchase;
        match key {
            "frm2550m:txtTax13A" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_sales_to_government(v);
                });
            }
            "frm2550m:txtTax18E" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_purchase(Purchase::DomesticGoods, v);
                });
            }
            "frm2550m:txtTax18G" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_purchase(Purchase::ImportedGoods, v);
                });
            }
            "frm2550m:txtTax18I" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_purchase(Purchase::DomesticServices, v);
                });
            }
            "frm2550m:txtTax18K" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_purchase(Purchase::NonResidentServices, v);
                });
            }
            "frm2550m:txtTax18N" => {
                return write_2550m_base_amount(kind, draft, key, &text, |d, v| {
                    d.set_purchase(Purchase::Others, v);
                });
            }
            _ => {}
        }
    }
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
            set_path(draft, path, leaf.clone(), &def.rows)?;
            // `WholePesoInput` (1702MX) stores `{ amount, raw }` and serializes
            // `raw` into the official BIR XML and print field map: keep `raw`
            // in step with `amount` in official format (`1,234.00`, `0`, or ``).
            if let Some(parent) = path.strip_suffix("/amount")
                && let Some(Value::Object(slot)) = draft.pointer_mut(parent)
                && slot.contains_key("raw")
            {
                let raw_text = match leaf.as_i64() {
                    None => String::new(),
                    Some(0) => "0".to_string(),
                    Some(number) => crate::official_xml::official_amount(number as f64),
                };
                slot.insert("raw".to_string(), Value::String(raw_text));
            }
            Ok(())
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

/// One 0605 split-date part, through the typed draft's merge-and-settle
/// (B1): blank clears the part, digits set it, and a complete real date
/// lands whole while an impossible one is refused untouched.
fn write_0605_date_part(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    text: &str,
    which: crate::forms::form_0605::Form0605DateField,
    part: DatePart,
) -> Result<(), String> {
    let value = if text.trim().is_empty() {
        None
    } else {
        Some(
            text.trim()
                .parse::<u16>()
                .map_err(|_| format!("`{key}` must be a whole number (got `{text}`)"))?,
        )
    };
    let mut typed = parse::<crate::forms::form_0605::Form0605Draft>(kind, draft)?;
    guarded("write", || typed.set_date_part(which, part, value))??;
    *draft = to_json(&typed)?;
    Ok(())
}

/// One 0605 official-list pick (B2): the typed setter decides what the
/// code stores — a listed code's whole indexed entry, a blank's clear, an
/// unlisted or unoffered code's refusal — and the draft is replaced only
/// on success.
fn write_0605_pick(
    kind: AgentFormKind,
    draft: &mut Value,
    set: impl FnOnce(&mut crate::forms::form_0605::Form0605Draft) -> Result<(), String>,
) -> Result<(), String> {
    let mut typed = parse::<crate::forms::form_0605::Form0605Draft>(kind, draft)?;
    guarded("write", || set(&mut typed))??;
    *draft = to_json(&typed)?;
    Ok(())
}

/// One 2550M base-amount blur (`13A`, `18E/G/I/K/N`): parses the amount and
/// invokes the typed setter so the paired 12% tax box (and 7% Schedule 4
/// standard input tax for `13A`) is computed while staying editable for
/// subsequent user overrides.
fn write_2550m_base_amount(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    text: &str,
    set: impl FnOnce(&mut crate::forms::form_2550m::Form2550MDraft, f64),
) -> Result<(), String> {
    let amount = if text.trim().is_empty() {
        0.0
    } else {
        parse_amount(text)
            .ok_or_else(|| format!("`{key}` must be an amount such as 1234.50 (got `{text}`)"))?
    };
    let mut typed = parse::<crate::forms::form_2550m::Form2550MDraft>(kind, draft)?;
    set(&mut typed, amount);
    *draft = to_json(&typed)?;
    Ok(())
}

/// The page's batch for one 0605 split date: `[month, day, year]` as the
/// parts now stand after this batch (the caller merges what the batch did
/// not send with what the boxes show). All three present must be a real
/// date or nothing lands; fewer keeps the typed parts (B1).
pub fn fill_0605_date_parts(
    kind: AgentFormKind,
    draft: &mut Value,
    which: crate::forms::form_0605::Form0605DateField,
    parts: [Option<u16>; 3],
) -> Result<(), String> {
    if kind != AgentFormKind::Queueable(QueueableKind::Form0605) {
        return Err(format!("{} has no split 0605 dates", kind.form_code()));
    }
    let mut typed = parse::<crate::forms::form_0605::Form0605Draft>(kind, draft)?;
    guarded("write", || typed.set_date_parts(which, parts))??;
    *draft = to_json(&typed)?;
    Ok(())
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
const ERROR_ALIASES: &[(&str, &str, &[&str])] = &[
    (
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
    ),
    (
        // form_2551q.rs `validate`: the annual income-tax election snapshot
        // (missing, conflicting, or unrecorded on a later-quarter Section 116
        // return) is the election Item 13 asks. Without this the message
        // fell to the page's label words and sat under Item 16.
        "2551Q",
        "annual_income_tax_election",
        &["/item_13_election"],
    ),
];

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
        let refused = fill(
            kind,
            &mut draft,
            "frm2551Qv2018:taxRate",
            &Value::from("2"),
            None,
        );
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

    /// A choice the return cannot hold unanswered is refused, and the
    /// refusal leaves the draft as it was: the page goes on with that draft.
    #[test]
    fn a_refused_forget_leaves_the_draft_as_it_was() {
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
        let before = draft.clone();
        let refused = forget(kind, &mut draft, "frm2551Qv2018:amendedRtn").unwrap_err();
        assert!(refused.contains("cannot be left unanswered"), "{refused}");
        assert_eq!(draft, before);
        compute(kind, &mut draft).expect("the draft still computes");
    }

    /// 2551Q's annual-election message names a field no box binds; it is
    /// the election Item 13 asks, so it lands on Item 13's radios and
    /// never on Item 16 (whose label shares its words).
    #[test]
    fn the_2551q_annual_election_message_names_item_13() {
        let kind = AgentFormKind::Form2551Q;
        let keys = error_keys(kind, "annual_income_tax_election");
        for radio in ["frm2551Qv2018:taxRate1", "frm2551Qv2018:taxRate2"] {
            assert!(keys.iter().any(|key| key == radio), "{radio} in {keys:?}");
        }
        let def = kind.definition();
        assert!(
            keys.iter()
                .all(|key| def.box_def(key).is_some_and(|entry| entry.item == "13")),
            "only Item 13 boxes: {keys:?}"
        );
        // The alias is the 2551Q's own: another form's field of that name is untouched.
        assert!(error_keys(AgentFormKind::Form1601C, "annual_income_tax_election").is_empty());
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
                    ("fill", fill(kind, &mut draft, key, &value, None)),
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
    fn the_period_writer_lifts_only_period_fields() {
        let text = |path: &str| Binding::Text {
            path: path.into(),
            nullable: false,
            date: None,
            part: None,
        };
        let choice = |paths: &[&str]| Binding::Choice {
            options: vec![ChoiceOption {
                value: "true".into(),
                set: paths
                    .iter()
                    .map(|p| (p.to_string(), Value::from(1)))
                    .collect(),
            }],
        };
        assert!(writes_only_the_period(&text("/taxable_year")));
        assert!(writes_only_the_period(&choice(&["/quarter"])));
        // The TIN is identity too, and never a sample's to type.
        assert!(!writes_only_the_period(&text("/tin")));
        assert!(!writes_only_the_period(&text("/estimated_life_months")));
        assert!(!writes_only_the_period(&choice(&["/quarter", "/tin"])));
        assert!(!writes_only_the_period(&choice(&[])));
    }

    #[test]
    fn an_amended_copy_is_new_and_the_filed_return_stays_byte_for_byte() {
        use crate::db::Database;
        use crate::forms::form_0619e::Form0619EDraft;
        let db = Database::open_in_memory_for_tests().unwrap();
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
            "default_form_type": "0619E"
        }))
        .unwrap();
        let kind = AgentFormKind::from_code("0619E").unwrap();
        let draft = Form0619EDraft::new_from_profile(&profile, 2025, 6);
        let tin = draft.tin().to_string();
        let id = db.save_queueable_draft(&draft).unwrap();
        // A return that reached BIR, stored the way submission leaves it.
        let mut filed: Form0619EDraft = db.get_queueable_draft(&tin, 2025, 6).unwrap().unwrap();
        filed.lifecycle.status = FilingStatus::Submitted;
        filed.lifecycle.submitted_at = Some("2025-07-10T01:02:03+00:00".into());
        filed.lifecycle.submission_filename = Some(filed.submission_filename());
        let filed_json = serde_json::to_string(&filed).unwrap();
        db.conn
            .execute(
                "UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = ?2",
                rusqlite::params![filed_json, id],
            )
            .unwrap();
        let row = |db: &Database| -> (String, String) {
            db.conn
                .query_row(
                    "SELECT status, data_json FROM form_drafts WHERE id = ?1",
                    rusqlite::params![id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        };
        let before = row(&db);

        // The copy: a Draft, "Amended Return?" Yes, no row id.
        let amended = amended_copy(kind, &serde_json::to_value(&filed).unwrap()).unwrap();
        let copy: Form0619EDraft = serde_json::from_value(amended.clone()).unwrap();
        assert_eq!(copy.lifecycle.status, FilingStatus::Draft);
        assert_eq!(copy.amended_return_answer(), Some(true));
        assert_eq!(copy.draft_id(), None);
        assert_eq!(
            row(&db),
            before,
            "nothing stored: the filed row is byte-for-byte"
        );

        // Its first save goes through the user's revert: the filed return is
        // kept byte-for-byte as the amendment snapshot.
        let reverted = db
            .revert_queueable_for_amendment::<Form0619EDraft>(&tin, 2025, 6)
            .unwrap();
        let snapshots = db
            .list_amendment_snapshots("0619E", &tin, 2025, &filed.filing_period().to_period_key())
            .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(
            snapshots[0].draft_json, before.1,
            "the original, byte for byte"
        );
        assert_eq!(Some(id), reverted.draft.draft_id());

        // Never for a return that did not reach BIR.
        assert!(amended_copy(kind, &serde_json::to_value(&draft).unwrap()).is_err());
    }

    /// A dry run reached nobody: there is nothing to amend. A real send, one
    /// with a relay receipt and a Confirmed return can be amended; Paid never.
    #[test]
    fn a_dry_run_has_no_amended_copy() {
        use crate::forms::form_0619e::Form0619EDraft;
        let profile: crate::profile::TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Example Store",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street",
            "zip_code": "1600",
            "phone": "09170000000",
            "email": "store@example.com",
            "default_form_type": "0619Ev2018",
            "taxpayer_type": "Corporation"
        }))
        .unwrap();
        let kind = AgentFormKind::from_code("0619E").unwrap();
        let mut sent = Form0619EDraft::new_from_profile(&profile, 2025, 6);
        sent.lifecycle.status = FilingStatus::Submitted;
        sent.lifecycle.submitted_at = Some("2025-07-10T01:02:03+00:00".into());
        let json = |draft: &Form0619EDraft| serde_json::to_value(draft).unwrap();

        let mut dry = sent.clone();
        dry.lifecycle.submitted_dry_run = true;
        let error = amended_copy(kind, &json(&dry)).unwrap_err();
        assert!(error.contains("dry run"), "{error}");
        // The mark under `lifecycle` (another build's shape) reads the same.
        let mut nested = json(&sent);
        nested.as_object_mut().unwrap().remove("submitted_dry_run");
        nested["lifecycle"] = serde_json::json!({"submitted_dry_run": true});
        assert!(amended_copy(kind, &nested).is_err());

        let mut relay = sent.clone();
        relay.lifecycle.relay_receipt = Some("a.b.c".into());
        let mut confirmed = sent.clone();
        confirmed.lifecycle.status = FilingStatus::Confirmed;
        // A Confirmed return carrying a stale mark reached the BIR all the same.
        confirmed.lifecycle.submitted_dry_run = true;
        for filed in [&sent, &relay, &confirmed] {
            assert!(
                amended_copy(kind, &json(filed)).is_ok(),
                "{:?}",
                filed.lifecycle.status
            );
        }
        let mut paid = sent.clone();
        paid.lifecycle.status = FilingStatus::Paid;
        assert!(amended_copy(kind, &json(&paid)).is_err());
    }

    /// `apply_penalty_profile` resolves the penalty inputs the Calendar
    /// resolves now — an override moves the stored due date — and touches
    /// only a Draft: a return that reached BIR stays byte-for-byte.
    #[test]
    fn apply_penalty_profile_resolves_now_and_touches_only_a_draft() {
        use crate::forms::form_0619e::Form0619EDraft;
        use chrono::Datelike;
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
            "default_form_type": "0619E"
        }))
        .unwrap();
        let kind = AgentFormKind::from_code("0619E").unwrap();
        let draft = Form0619EDraft::new_from_profile(&profile, 2025, 6);
        let mut json = serde_json::to_value(&draft).unwrap();

        // The compiled deadline first, then an advisory moving it.
        apply_penalty_profile(kind, &mut json, &profile, &[]).unwrap();
        let base: Form0619EDraft = serde_json::from_value(json.clone()).unwrap();
        let base_due = base.penalty.penalty_due_date.expect("0619E has a deadline");
        // Seven days ahead lands on a non-holiday workday, so the override
        // date is the resolved one as stored.
        let moved = base_due + chrono::Duration::days(7);
        assert_eq!(moved.weekday(), chrono::Weekday::Thu, "test math");
        let override_ = crate::calendar_rules::DeadlineOverride {
            id: "test-move".into(),
            title: "Move".into(),
            source_reference: "TEST".into(),
            affected_form_codes: vec!["0619E".into()],
            original_deadline: base_due,
            adjusted_deadline: moved,
            affected_regions: vec![],
            affected_taxpayer_types: vec![],
            effective_from: None,
            effective_until: None,
            expires_at: None,
        };
        apply_penalty_profile(kind, &mut json, &profile, std::slice::from_ref(&override_)).unwrap();
        let reparsed: Form0619EDraft = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            reparsed.penalty.penalty_due_date,
            Some(moved),
            "the due date the Calendar resolves now, not the compiled one"
        );

        // A return that reached BIR: byte-for-byte.
        let mut filed = reparsed;
        filed.lifecycle.status = FilingStatus::Submitted;
        let filed_json = serde_json::to_value(&filed).unwrap();
        let mut as_json = filed_json.clone();
        apply_penalty_profile(
            kind,
            &mut as_json,
            &profile,
            std::slice::from_ref(&override_),
        )
        .unwrap();
        assert_eq!(as_json, filed_json, "a filed return is never rewritten");

        // A form with its own penalty state (1601C) and a non-queueable
        // kind are left alone.
        for code in ["1601C", "2551Q"] {
            let other = AgentFormKind::from_code(code).unwrap();
            let mut json = serde_json::json!({"anything": true});
            apply_penalty_profile(other, &mut json, &profile, &[]).unwrap();
            assert_eq!(json, serde_json::json!({"anything": true}), "{code}");
        }
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

    /// The profile alone decides whether a profile box is locked: a box it
    /// filled stays locked to the assistant, while one it left blank loses
    /// the lock and is an ordinary box (`None` cannot prove the blank and
    /// stays locked — the conservative verdict). 2551Q's Item 10 phone and
    /// Item 11 email are the profile boxes this profile leaves empty.
    #[test]
    fn a_profile_box_the_profile_left_blank_is_an_ordinary_box() {
        let kind = AgentFormKind::Form2551Q;
        let profile: crate::profile::TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Example Store",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street",
            "zip_code": "1600",
            "phone": "",
            "email": "",
            "default_form_type": "2551Qv2018"
        }))
        .unwrap();
        let mut from_profile_draft =
            serde_json::to_value(Form2551QDraft::new_from_profile(&profile, 2026, 3)).unwrap();
        compute(kind, &mut from_profile_draft).unwrap();
        let map = field_map(kind, &from_profile_draft).unwrap();
        let from_profile = FromProfile {
            values: Value::Object(
                map.into_iter()
                    .map(|(key, value)| (key, Value::String(value)))
                    .collect(),
            ),
            draft: from_profile_draft,
        };
        // The blank ones open in the shared lock map and take a fill; the
        // filled RDO box stays locked.
        let locks = locks(kind, &from_profile);
        for blank in ["frm2551Qv2018:telNo", "txtEmail#occurrence-1"] {
            assert!(!locks.contains_key(blank), "{blank} is open on the page");
            assert!(
                !profile_keys(kind, &from_profile)
                    .iter()
                    .any(|key| key == blank),
                "{blank} is not a filled profile box"
            );
            let mut draft =
                serde_json::to_value(Form2551QDraft::new_from_profile(&profile, 2026, 3)).unwrap();
            fill(
                kind,
                &mut draft,
                blank,
                &Value::from("the assistant's answer"),
                Some(&from_profile),
            )
            .unwrap_or_else(|error| panic!("{blank}: {error}"));
            // Without the profile context the same box keeps its lock.
            let refused = fill(kind, &mut draft, blank, &Value::from("x"), None).expect_err(blank);
            assert!(
                refused.contains(Lock::Profile.reason()),
                "{blank}: {refused}"
            );
        }
        assert_eq!(
            draft_contact(&from_profile.draft),
            ("".to_string(), "".to_string()),
            "the profile really left them blank"
        );
        let mut draft =
            serde_json::to_value(Form2551QDraft::new_from_profile(&profile, 2026, 3)).unwrap();
        let refused = fill(
            kind,
            &mut draft,
            "frm2551Qv2018:txtRDOCode",
            &Value::from("x"),
            Some(&from_profile),
        )
        .expect_err("a filled profile box stays locked");
        assert!(refused.contains(Lock::Profile.reason()), "{refused}");
    }

    fn draft_contact(draft: &Value) -> (String, String) {
        (
            draft["contact_number"].as_str().unwrap_or("").to_string(),
            draft["email"].as_str().unwrap_or("").to_string(),
        )
    }

    /// Every host-written key and every occurrence alias the pinned
    /// `agent-locks.json` names is refused to `fill` — bound or not — with
    /// the reason the page shows for it. This is the artifact's coverage
    /// over every form, and the assistant side of the byte-pinned
    /// `agent_locks_table` test in bir-shell.
    #[test]
    fn the_assistant_is_refused_what_the_host_writes_and_an_alias_repeats() {
        let mut host_written = 0;
        let mut aliases = 0;
        for kind in AgentFormKind::all() {
            let def = kind.definition();
            for (key, official) in occurrence_aliases(kind) {
                let mut draft = Value::Null;
                let refused = fill(kind, &mut draft, key, &Value::from("x"), None).expect_err(key);
                assert!(refused.contains("repeats"), "{key}: {refused}");
                assert!(refused.contains(official), "{key}: {refused}");
                aliases += 1;
            }
            for key in def
                .boxes
                .iter()
                .map(|entry| entry.key.as_str())
                .chain(def.bindings.keys().map(String::as_str))
            {
                if matches!(gate(def, key, None), Some(Gate::HostWritten)) {
                    let mut draft = Value::Null;
                    let refused =
                        fill(kind, &mut draft, key, &Value::from("x"), None).expect_err(key);
                    assert!(
                        refused.contains("written through the form's own editors"),
                        "{key}: {refused}"
                    );
                    host_written += 1;
                }
            }
        }
        assert!(
            host_written > 0 && aliases > 0,
            "agent-locks.json covers every form: {host_written} host-written, {aliases} aliases"
        );
    }

    /// The election-group fallback is deliberate: a bound radio member
    /// with no lock of its own under an election-locked group is still
    /// refused to `fill`, though the page draws no lock for it (no pinned
    /// form has such a member today — the group locks its members — so a
    /// synthetic definition pins the rule).
    #[test]
    fn a_member_without_a_lock_under_an_election_group_is_still_refused() {
        let def: AgentFormDef = serde_json::from_value(serde_json::json!({
            "package": "test",
            "form_code": "TEST",
            "boxes": [
                {
                    "key": "grp", "label": "g", "kind": "select",
                    "required": "optional", "radio_group": true,
                    "members": ["mA", "mB"],
                    "options": [{"value": "a"}, {"value": "b"}]
                },
                {"key": "mA", "label": "a", "kind": "radio", "required": "optional", "group": "grp"},
                {"key": "mB", "label": "b", "kind": "radio", "required": "optional", "group": "grp"}
            ],
            "bindings": {
                "grp": {"type": "choice", "lock": "election", "options": [
                    {"value": "a", "set": {"/pick": "a"}},
                    {"value": "b", "set": {"/pick": "b"}}
                ]},
                "mA": {"type": "choice", "lock": "election", "options": [
                    {"value": "true", "set": {"/pick": "a"}}
                ]},
                "mB": {"type": "choice", "options": [
                    {"value": "true", "set": {"/pick": "b"}}
                ]}
            }
        }))
        .unwrap();
        assert!(
            matches!(gate(&def, "mB", None), Some(Gate::Lock(Lock::Election))),
            "a bound member inherits its election-locked group's lock"
        );
        assert_eq!(
            Gate::Lock(Lock::Election).page_reason(&def, "mB"),
            None,
            "the page shows it open — the refusal is the assistant's half"
        );
        assert!(
            gate(&def, "mA", None).is_some_and(|gate| matches!(gate, Gate::Lock(Lock::Election)))
        );
    }

    /// Official 1702MX runtime fixture (`rules/forms/1702mx-v2018c/fixtures/
    /// runtime-control-inventory-v796.json`, lines 4011-4058 and 4205-4252):
    /// Schedule 7.1 and 8.1 rows 4-6 columns A-D are open NOLCO inputs.
    /// Filling them via `agent::fill` (used by both the form page and
    /// `form.fill`) must populate `WholePesoInput.raw` alongside `amount` so
    /// `Form1702MXDraft::to_bir_xml_payload()` carries every value to BIR.
    #[test]
    fn form_1702mx_nolco_fill_populates_official_xml() {
        let kind = AgentFormKind::from_code("1702MX").unwrap();
        let profile: crate::profile::TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "DEMO CORP",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "039",
            "line_of_business": "Services",
            "registered_address": "1 Example Street",
            "zip_code": "1600",
            "phone": "",
            "email": "",
            "default_form_type": "1702MXv2018c"
        }))
        .unwrap();
        let mut draft = serde_json::to_value(
            crate::forms::form_1702mx::Form1702MXDraft::new_from_profile(&profile, 2025),
        )
        .unwrap();
        fill(
            kind,
            &mut draft,
            "frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized",
            &Value::from("true"),
            None,
        )
        .unwrap();

        let cases = [
            ("frm1702MX:txtPg3IShed7_4A", "300000", "300,000.00"),
            ("frm1702MX:txtPg3IShed7_4B", "50000", "50,000.00"),
            ("frm1702MX:txtPg3IShed7_4C", "20000", "20,000.00"),
            ("frm1702MX:txtPg3IShed7_4D", "80000", "80,000.00"),
            ("frm1702MX:txtPg3IShed7_5A", "200000", "200,000.00"),
            ("frm1702MX:txtPg3IShed7_5B", "30000", "30,000.00"),
            ("frm1702MX:txtPg3IShed7_5C", "10000", "10,000.00"),
            ("frm1702MX:txtPg3IShed7_5D", "40000", "40,000.00"),
            ("frm1702MX:txtPg3IShed7_6A", "100000", "100,000.00"),
            ("frm1702MX:txtPg3IShed7_6B", "15000", "15,000.00"),
            ("frm1702MX:txtPg3IShed7_6C", "5000", "5,000.00"),
            ("frm1702MX:txtPg3IShed7_6D", "25000", "25,000.00"),
            ("frm1702MX:txtPg4IShed8_4A", "400000", "400,000.00"),
            ("frm1702MX:txtPg4IShed8_4B", "60000", "60,000.00"),
            ("frm1702MX:txtPg4IShed8_4C", "25000", "25,000.00"),
            ("frm1702MX:txtPg4IShed8_4D", "90000", "90,000.00"),
            ("frm1702MX:txtPg4IShed8_5A", "250000", "250,000.00"),
            ("frm1702MX:txtPg4IShed8_5B", "35000", "35,000.00"),
            ("frm1702MX:txtPg4IShed8_5C", "15000", "15,000.00"),
            ("frm1702MX:txtPg4IShed8_5D", "45000", "45,000.00"),
            ("frm1702MX:txtPg4IShed8_6A", "150000", "150,000.00"),
            ("frm1702MX:txtPg4IShed8_6B", "20000", "20,000.00"),
            ("frm1702MX:txtPg3IShed8_6C", "10000", "10,000.00"),
            ("frm1702MX:txtPg4IShed8_6D", "30000", "30,000.00"),
        ];
        for (key, value, _) in cases {
            fill(kind, &mut draft, key, &Value::from(value), None).unwrap();
        }
        compute(kind, &mut draft).unwrap();

        let typed: crate::forms::form_1702mx::Form1702MXDraft =
            serde_json::from_value(draft.clone()).unwrap();
        let layout = typed.official_layout().unwrap();
        let official_xml =
            crate::official_xml::write(&layout, &typed.to_official_field_map()).unwrap();
        let official_parsed = crate::bir_xml::parse_bir_xml_checked(&official_xml).unwrap();
        let print_map = typed.to_print_field_map();
        for (key, _, expected) in cases {
            assert_eq!(
                official_parsed.get(key).map(String::as_str),
                Some(expected),
                "official upload XML missing or mismatched for {key}"
            );
            assert_eq!(
                print_map.get(key).map(String::as_str),
                Some(expected),
                "print field map missing or mismatched for {key}"
            );
        }
        // Computed Col E and Row 8 totals carry thousands commas in BOTH
        // the official upload XML and the print/raw map.
        for (key, expected) in [
            ("frm1702MX:txtPg3IShed7_4E", "150,000"),
            ("frm1702MX:txtPg3IShed78D", "145,000"),
            ("frm1702MX:txtPg4IShed8_4E", "225,000"),
            ("frm1702MX:txtPg3IShed88D", "165,000"),
        ] {
            assert_eq!(
                official_parsed.get(key).map(String::as_str),
                Some(expected),
                "official upload XML computed {key}"
            );
            assert_eq!(
                print_map.get(key).map(String::as_str),
                Some(expected),
                "print field map computed {key}"
            );
        }

        // Clearing back to 0 (what the form page's CLEARED = "0" sends when
        // emptying a non-nullable number box) keeps amount/raw synchronized and
        // files "0".
        fill(
            kind,
            &mut draft,
            "frm1702MX:txtPg3IShed7_4A",
            &Value::from("0"),
            None,
        )
        .unwrap();
        let zeroed: crate::forms::form_1702mx::Form1702MXDraft =
            serde_json::from_value(draft).unwrap();
        assert_eq!(
            zeroed.schedule_7_1.rows[0].amount.amount,
            Some(crate::forms::form_1702mx::WholePeso(0))
        );
        assert_eq!(zeroed.schedule_7_1.rows[0].amount.raw, "0");
        let zeroed_off = zeroed.to_official_field_map();
        assert_eq!(zeroed_off["frm1702MX:txtPg3IShed7_4A"], "0");
        // Untouched current-year row also uploads as "0".
        assert_eq!(zeroed_off["frm1702MX:txtPg3IShed7_7A"], "0");
    }
}
