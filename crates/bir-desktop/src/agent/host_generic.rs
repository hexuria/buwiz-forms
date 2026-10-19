//! Agent ops on every form beyond the hand-built 1601C and 2551Q: the 41
//! drafts of the generic queue (`bir_core::forms::agent`), plus the ops
//! every one of the 43 forms shares — print preview, the chat form card and
//! the hand-over to the user's own Submit.
//!
//! Draft-only: nothing here queues, submits, uploads or changes a return's
//! filing status. `form.review_in_app` saves the draft and brings the page
//! up for the user; only the user's click on Submit files it.

use bir_core::forms::FilingStatus;
use bir_core::forms::agent::{self, AgentFormKind, Binding, CardBox};
use bir_core::forms::form_1600pt::{Form1600PtAgentCategory, form_1600pt_popup};
use bir_core::forms::form_1600vt::{Form1600VtAgentCategory, form_1600vt_popup};
use bir_core::forms::form_1601fq::Form1601FqCategory;
use bir_core::forms::queueable::{QueueableKind, period_column};
use bir_core::forms::registry::FilingFrequency;
use bir_core::profile::TaxpayerProfile;
use gpui_agent::dispatch::DispatchResult;
use gpui_agent::protocol::PlatformKind;
use serde_json::{Value, json};

use super::{BirAgentHost, field_errors_json, fill_result, write_agent_frozen_html};
use crate::agent::assist::{AssistState, BoxSource};
use crate::app::ActiveView;
use crate::views::form_header::AGENT_REVIEW_MESSAGE;

/// The open draft of a generic-queue form, as the agent host holds it.
#[derive(Debug, Clone)]
pub struct GenericFormState {
    pub kind: AgentFormKind,
    pub draft: Value,
    pub validation_errors: Vec<(String, String)>,
    pub validated: bool,
    /// One-shot: `form.save_draft` / `form.review_in_app` saved it in this request.
    pub saved: bool,
    /// Per-box sources (official keys), unsaved-edit flags and the one-form lock.
    pub assist: AssistState,
    /// One-shot: `form.review_in_app` asked the page to take over.
    pub review_requested: bool,
}

impl GenericFormState {
    pub fn fresh(kind: AgentFormKind, draft: Value) -> Self {
        Self {
            kind,
            draft,
            validation_errors: Vec::new(),
            validated: false,
            saved: false,
            assist: AssistState::default(),
            review_requested: false,
        }
    }

    /// `(tin, year, period column)` of the draft.
    pub fn identity(&self) -> Option<(String, u16, i64)> {
        agent::identity(self.kind, &self.draft)
            .ok()
            .map(|id| (id.tin, id.taxable_year, period_column(&id.period)))
    }

    fn status(&self) -> Option<FilingStatus> {
        agent::identity(self.kind, &self.draft)
            .ok()
            .map(|id| id.status)
    }

    fn is_editable(&self) -> bool {
        self.status() == Some(FilingStatus::Draft)
    }
}

/// The page a generic-queue form opens on.
pub fn generic_view(kind: QueueableKind) -> ActiveView {
    if kind == QueueableKind::Form2550Q {
        ActiveView::Form2550Q
    } else {
        ActiveView::Queueable(kind)
    }
}

fn queueable(kind: AgentFormKind) -> Option<QueueableKind> {
    match kind {
        AgentFormKind::Queueable(kind) => Some(kind),
        _ => None,
    }
}

/// The period the page opens for `code`: month 1-12, quarter 1-4, `0` for
/// an annual return, the open-ended key otherwise (as the dashboard passes it).
pub fn page_period(code: &str, period: u8) -> Result<u8, String> {
    let frequency = bir_core::forms::find_form(code)
        .map(|form| form.frequency.clone())
        .unwrap_or(FilingFrequency::Annual);
    match frequency {
        FilingFrequency::Monthly if (1..=12).contains(&period) => Ok(period),
        FilingFrequency::Monthly => Err(format!("{code} is monthly: period must be 1-12")),
        FilingFrequency::Quarterly if (1..=4).contains(&period) => Ok(period),
        FilingFrequency::Quarterly => Err(format!("{code} is quarterly: period must be 1-4")),
        FilingFrequency::Annual => Ok(0),
        FilingFrequency::OpenEnded => Ok(period.max(1)),
    }
}

/// The 2550Q page's own open: the stored draft, else a new one, with its
/// live row identities prepared (as `AppState::handle_file_form` does).
fn load_2550q(
    db: &bir_core::db::Database,
    profile: &TaxpayerProfile,
    year: u16,
    quarter: u8,
) -> Result<Value, String> {
    use bir_core::forms::form_2550q::Form2550QDraft;
    let tin = profile.tin.full();
    let mut draft = db
        .get_form_draft::<Form2550QDraft>(&tin, "2550Q", year, Some(quarter))
        .map_err(|error| format!("the 2550Q draft could not be read safely: {error}"))?
        .unwrap_or_else(|| Form2550QDraft::new_from_profile(profile, year, quarter));
    bir_core::form_rules::prepare_form_2550q_live_row_identities(&mut draft)
        .map_err(|error| format!("the 2550Q draft's row identities are unsafe: {error}"))?;
    serde_json::to_value(draft).map_err(|error| error.to_string())
}

fn card_json(card: &CardBox, assist: &AssistState) -> Value {
    let mut value = serde_json::to_value(card).unwrap_or(Value::Null);
    if let Value::Object(object) = &mut value {
        // `field` as in the 1601C/2551Q lists, `key` for form.fill.
        object.insert("field".into(), json!(card.key));
        if let Some(source) = assist.sources.get(&card.key) {
            object.insert("source".into(), json!(source.as_str()));
        }
    }
    value
}

impl BirAgentHost {
    /// The open generic form, when its page is the active view.
    pub(super) fn active_generic(&self) -> Option<&GenericFormState> {
        let form = self.generic_form.as_ref()?;
        let kind = queueable(form.kind)?;
        (self.active_view == generic_view(kind)).then_some(form)
    }

    fn active_generic_mut(&mut self) -> Option<&mut GenericFormState> {
        let view = self.active_view;
        let form = self.generic_form.as_mut()?;
        let kind = queueable(form.kind)?;
        (view == generic_view(kind)).then_some(form)
    }

    pub fn generic_form(&self) -> Option<&GenericFormState> {
        self.generic_form.as_ref()
    }

    /// Copy the open page's draft and assist state in (desktop drain).
    pub fn replace_generic_form(
        &mut self,
        kind: AgentFormKind,
        draft: Value,
        validated: bool,
        assist: AssistState,
    ) {
        self.form_loaded = true;
        let mut state = GenericFormState::fresh(kind, draft);
        state.validated = validated;
        state.assist = assist;
        self.generic_form = Some(state);
    }

    /// `form.open` for a generic-queue form: the page's own draft for this
    /// taxpayer and period (stored, else prefilled from the profile).
    pub(super) fn open_generic(
        &mut self,
        kind: QueueableKind,
        year: u16,
        period: u8,
    ) -> Result<DispatchResult, String> {
        let code = kind.form_code();
        let view = generic_view(kind);
        self.gate_dirty(view)?;
        let period = page_period(code, period)?;
        if self.gate_form_lock(code, year, period, None)? {
            self.form_loaded = true;
            self.submit_confirmation_visible = false;
            self.active_view = view;
            return Ok(DispatchResult::json(json!({
                "form": code,
                "page": crate::agent::ids::page_root(view),
                "year": year,
                "period": period,
                "already_open": true,
            })));
        }
        let tin = self
            .selected_tin
            .clone()
            .ok_or("select a taxpayer profile before opening a form")?;
        let profile = self.load_profile(&tin)?;
        let draft = {
            let db = self.db.as_ref().ok_or("agent host has no database")?;
            let guard = db.lock().map_err(|err| err.to_string())?;
            if kind == QueueableKind::Form2550Q {
                load_2550q(&guard, &profile, year, period)?
            } else {
                let spec = crate::views::queueable_forms::spec_for_kind(kind)
                    .ok_or_else(|| format!("form {code} has no page"))?;
                (spec.load)(&guard, &profile, year, period)
            }
        };
        self.generic_form = Some(GenericFormState::fresh(
            AgentFormKind::Queueable(kind),
            draft,
        ));
        self.form_1601c = None;
        self.form_2551q = None;
        self.form_loaded = true;
        self.submit_confirmation_visible = false;
        self.active_view = view;
        Ok(DispatchResult::json(json!({
            "form": code,
            "page": crate::agent::ids::page_root(view),
            "year": year,
            "period": period,
        })))
    }

    pub(super) fn validate_generic(&mut self) -> Option<Result<DispatchResult, String>> {
        let form = self.active_generic_mut()?;
        Some((|| {
            form.validation_errors = agent::validate(form.kind, &mut form.draft)?;
            form.validated = true;
            Ok(DispatchResult::json(json!({
                "ok": form.validation_errors.is_empty(),
                "errors": form.validation_errors.len(),
                "field_errors": field_errors_json(&form.validation_errors),
                "form": form.kind.form_code(),
            })))
        })())
    }

    /// Every user box of the form (`fields.json`) the agent may fill, with
    /// its value and source. Computed, hidden and locked boxes are counted,
    /// not listed.
    pub(super) fn official_boxes(
        kind: AgentFormKind,
        draft: &Value,
        assist: &AssistState,
    ) -> Result<Value, String> {
        let def = kind.definition();
        let map = agent::field_map(kind, draft)?;
        let boxes: Vec<Value> = def
            .fillable()
            .map(|entry| card_json(&CardBox::new(def, entry, &map), assist))
            .collect();
        let questions = def.boxes.iter().filter(|entry| entry.is_question()).count();
        Ok(json!({
            "boxes": boxes,
            // A popup's tick boxes the page does not draw for the draft's
            // category are not read-only boxes the filer sees.
            "read_only": questions
                .saturating_sub(boxes.len())
                .saturating_sub(undrawn_popup_slots(def, draft)),
            "computed": def.boxes.iter().filter(|entry| entry.computed).count(),
        }))
    }

    /// `form.needs_you` from the form's own definition (`agent::needs_you`):
    /// fillable boxes only, a radio item as one box, and the validation
    /// errors that name no such box (labelled where the form names them).
    pub(super) fn needs_you_json(
        kind: AgentFormKind,
        draft: &Value,
        assist: &AssistState,
    ) -> Result<Value, String> {
        let mut draft = draft.clone();
        let errors = agent::validate(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let needs = agent::needs_you(kind, &map, &errors);
        Ok(json!({
            "ok": errors.is_empty(),
            "boxes": needs
                .boxes
                .iter()
                .map(|card| card_json(card, assist))
                .collect::<Vec<_>>(),
            "other_errors": labelled_errors(kind, &needs.other_errors),
        }))
    }

    pub(super) fn generic_fields(&self) -> Option<Result<DispatchResult, String>> {
        let form = self.active_generic()?;
        Some((|| {
            let mut out = Self::official_boxes(form.kind, &form.draft, &form.assist)?;
            if let Value::Object(object) = &mut out {
                object.insert("form".into(), json!(form.kind.form_code()));
                object.insert(
                    "status".into(),
                    json!(form.status().map(|status| format!("{status:?}"))),
                );
                object.insert(
                    "id".into(),
                    form.draft.get("id").cloned().unwrap_or(Value::Null),
                );
            }
            Ok(DispatchResult::json(out))
        })())
    }

    /// `form.fill` on a generic form: official box keys. Every rejected
    /// key — a box with no field in this app's draft, a computed, locked
    /// or hidden box, a key that is not a box — is refused on its own in
    /// `refused` and the rest of the batch still applies.
    pub(super) fn fill_generic(
        &mut self,
        fields: &serde_json::Map<String, Value>,
        source: BoxSource,
    ) -> Option<Result<DispatchResult, String>> {
        let db = self.db.clone();
        let form = self.active_generic_mut()?;
        Some((|| {
            if !form.is_editable() {
                return Err("this return is no longer a draft".into());
            }
            let record = trigger_record(form.kind, &form.draft);
            let (draft, filled, kept, not_shown, refused) =
                fill_official(form.kind, &form.draft, &form.assist, fields)?;
            form.draft = draft;
            // A write that moved the record the penalties resolve from (a
            // typed trigger date) resolves them again, as the page's own
            // sync does (decision P5-9).
            if trigger_record(form.kind, &form.draft) != record
                && let Some(db) = &db
                && let Ok(guard) = db.lock()
                && let Some(tin) = form.draft["tin"].as_str().map(str::to_string)
                && let Ok(Some(profile)) = guard.get_profile_by_tin(&tin)
            {
                let overrides = guard.get_deadline_overrides();
                match agent::apply_penalty_profile(form.kind, &mut form.draft, &profile, &overrides)
                {
                    Ok(()) => {
                        let _ = agent::compute(form.kind, &mut form.draft);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "penalty inputs not re-resolved; the draft keeps its state");
                    }
                }
            }
            for key in &filled {
                form.assist.note_agent_fill(key, source);
            }
            if !filled.is_empty() {
                form.validated = false;
            }
            Ok(with_refused(
                with_not_shown(
                    fill_result(form.kind.form_code(), source, filled, kept),
                    not_shown,
                ),
                refused,
            ))
        })())
    }

    pub(super) fn needs_you_generic(&self) -> Option<Result<DispatchResult, String>> {
        let form = self.active_generic()?;
        Some((|| {
            let mut out = Self::needs_you_json(form.kind, &form.draft, &form.assist)?;
            if let Value::Object(object) = &mut out {
                object.insert("form".into(), json!(form.kind.form_code()));
            }
            Ok(DispatchResult::json(out))
        })())
    }

    pub(super) fn save_generic(&mut self) -> Option<Result<DispatchResult, String>> {
        let db = self.db.clone();
        let form = self.active_generic_mut()?;
        Some((|| {
            let db = db.ok_or("agent host has no database")?;
            {
                let guard = db.lock().map_err(|err| err.to_string())?;
                // The save keeps the due dates the Calendar resolves now,
                // as the page's own save does (decision P5-9); never a due
                // date an older state of this draft resolved.
                if let Some(tin) = form.draft["tin"].as_str().map(str::to_string)
                    && let Ok(Some(profile)) = guard.get_profile_by_tin(&tin)
                {
                    let overrides = guard.get_deadline_overrides();
                    agent::apply_penalty_profile(form.kind, &mut form.draft, &profile, &overrides)?;
                }
                agent::save_draft(form.kind, &guard, &mut form.draft)?;
            }
            form.saved = true;
            form.assist.note_saved();
            Ok(DispatchResult::json(json!({
                "status": form.status().map(|status| format!("{status:?}")),
                "saved": true,
                "form": form.kind.form_code(),
            })))
        })())
    }

    /// The open form's official field map, recomputed, for any of the 43.
    fn open_official(&self) -> Result<(AgentFormKind, Value, AssistState), String> {
        if let Some(form) = self.active_generic() {
            let mut draft = form.draft.clone();
            agent::compute(form.kind, &mut draft)?;
            return Ok((form.kind, draft, form.assist.clone()));
        }
        if let Some(form) = &self.form_1601c
            && self.active_view == ActiveView::Form1601C
        {
            let draft = serde_json::to_value(&form.draft).map_err(|e| e.to_string())?;
            return Ok((AgentFormKind::Form1601C, draft, form.assist.clone()));
        }
        if let Some(form) = &self.form_2551q
            && self.active_view == ActiveView::Form2551Q
        {
            let draft = serde_json::to_value(&form.draft).map_err(|e| e.to_string())?;
            return Ok((AgentFormKind::Form2551Q, draft, form.assist.clone()));
        }
        Err("open a form first (bir_form_open)".into())
    }

    /// `form.preview`: the open draft's filled frozen print sheet, written
    /// to a local HTML file (offline, no official page rasters). The painted
    /// app also opens it the way the page's Print preview does.
    pub(super) fn form_preview(&mut self, open_in_app: bool) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.reconcile_open_forms_from_db();
        let (kind, draft, _) = self.open_official()?;
        let (slug, fields) = print_sheet(kind, &draft)?;
        let identity = agent::identity(kind, &draft)?;
        let receipt = match kind {
            AgentFormKind::Form1601C | AgentFormKind::Form2551Q => {
                let email_key = if kind == AgentFormKind::Form1601C {
                    "email_address"
                } else {
                    "email"
                };
                let mailbox = draft[email_key].as_str().unwrap_or_default().to_string();
                let receipt_id = draft["receipt_id"].as_i64();
                self.db.as_ref().and_then(|db| {
                    crate::views::form_html_preview_launcher::receipt_page_for(
                        db, receipt_id, &mailbox,
                    )
                })
            }
            _ => None,
        };
        let path = write_agent_frozen_html(slug, &fields, receipt.as_ref())?;
        let path = path.canonicalize().unwrap_or(path);
        if !path.is_absolute() {
            return Err("form.preview must return an absolute path, not file bytes".into());
        }
        let opened = open_in_app && self.platform == PlatformKind::Desktop;
        if opened {
            self.print_requested = true;
        }
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "status": format!("{:?}", identity.status),
            "path": path.to_string_lossy(),
            "kind": "frozen-html",
            "sheet": slug,
            "pdf": Value::Null,
            "opened_in_app": opened,
            "receipt_page": receipt.is_some(),
            "note": "Draft preview: the filled official sheet as local HTML (no PDF export in this app). It is not filed.",
        })))
    }

    /// `form.card`: what an AG-UI `request_user_form` card shows for the
    /// open form — the needs-you boxes and every box with a validation error.
    pub(super) fn form_card(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        self.reconcile_open_forms_from_db();
        let (kind, mut draft, assist) = self.open_official()?;
        let errors = agent::validate(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let (boxes, unplaced) = agent::card_boxes(kind, &map, &errors);
        let fields: Vec<Value> = boxes.iter().map(|card| card_json(card, &assist)).collect();
        let unplaced = labelled_errors(kind, &unplaced);
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "title": format!("Form {}: what BIR still needs", kind.form_code()),
            "fields": fields,
            "other_errors": unplaced,
            "submit": {
                "tool": "bir_form_fill",
                "source": "user",
                "from_card": true,
            },
        })))
    }

    /// `form.review_in_app`: hand a validated draft to the user. Saves it,
    /// brings the app to that form's page in Draft status, runs the page's
    /// own Validate and highlights Submit with a note in the status box.
    /// Never queues, submits, uploads or marks anything filed: only the
    /// user's own Submit click does that.
    pub(super) fn review_in_app(&mut self) -> Result<DispatchResult, String> {
        self.gate_locked()?;
        if self.platform != PlatformKind::Desktop {
            return Err(
                "form.review_in_app needs the BIR desktop app: open this return there and review it yourself"
                    .into(),
            );
        }
        self.reconcile_open_forms_from_db();
        let (kind, mut draft, _) = self.open_official()?;
        let status = agent::identity(kind, &draft)?.status;
        if status != FilingStatus::Draft {
            return Err(format!(
                "this return is {status:?}, not a Draft; there is nothing to hand over"
            ));
        }
        let errors = agent::validate(kind, &mut draft)?;
        if !errors.is_empty() {
            return Err(format!(
                "fix the {} validation error(s) first (bir_form_validate); review is only for a return that validates",
                errors.len()
            ));
        }
        match kind {
            AgentFormKind::Form1601C => {
                self.save_form_draft()?;
                if let Some(form) = self.form_1601c.as_mut() {
                    form.validated = true;
                    form.validation_errors.clear();
                }
            }
            AgentFormKind::Form2551Q => {
                self.save_form_draft()?;
                if let Some(form) = self.form_2551q.as_mut() {
                    form.validated = true;
                    form.validation_errors.clear();
                }
            }
            AgentFormKind::Queueable(_) => {
                self.save_generic().ok_or("open a form first")??;
                if let Some(form) = self.generic_form.as_mut() {
                    form.validated = true;
                    form.validation_errors.clear();
                }
            }
        }
        if let Some(form) = self.generic_form.as_mut()
            && form.kind == kind
        {
            form.review_requested = true;
        }
        self.review_requested = Some(kind);
        self.submit_confirmation_visible = false;
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "status": "Draft",
            "saved": true,
            "validated": true,
            "queued": false,
            "filed": false,
            "message": AGENT_REVIEW_MESSAGE,
        })))
    }

    /// The source a `form.fill` may claim. `user` only for answers the user
    /// gave on the chat form card (`from_card`), and only for boxes that card
    /// asks for right now; anything else follows `BoxSource::parse_fill`.
    pub(super) fn fill_source(
        &self,
        args: &Value,
        fields: &serde_json::Map<String, Value>,
    ) -> Result<BoxSource, String> {
        let from_card = args.get("from_card").and_then(Value::as_bool) == Some(true);
        let raw = args.get("source").and_then(Value::as_str);
        if !(from_card && raw.map(str::trim) == Some("user")) {
            return BoxSource::parse_fill(raw);
        }
        let (kind, mut draft, _) = self.open_official()?;
        let errors = agent::validate(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let (boxes, _) = agent::card_boxes(kind, &map, &errors);
        if let Some(key) = fields
            .keys()
            .find(|key| !boxes.iter().any(|card| &card.key == *key))
        {
            return Err(format!(
                "`{key}` is not on the form card; source `user` is only for the user's own answers to bir_form_card"
            ));
        }
        Ok(BoxSource::User)
    }

    /// One-shot: the form `form.review_in_app` handed over in this request.
    pub fn take_review_request(&mut self) -> Option<AgentFormKind> {
        self.review_requested.take()
    }
}

/// `(draft, filled, kept_user_boxes, not_shown, refused)` of
/// [`fill_official`].
pub(super) type OfficialFill = (
    Value,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    serde_json::Map<String, Value>,
);

/// The official page draws an ATC popup's tick boxes only for the ATCs the
/// draft's category of withholding agent offers (1600PT/1600VT
/// `AtcCode<n>`, 1601FQ `AtcCd<n>`): a slot beyond that list is a box
/// nobody sees, so it is not a read-only question (review F5).
fn undrawn_popup_slots(def: &agent::AgentFormDef, draft: &Value) -> usize {
    let stored = |field: &str| draft.get(field).cloned().unwrap_or_default();
    let (prefix, drawn) = match def.form_code.as_str() {
        "1600PT" => (
            "AtcCode",
            serde_json::from_value::<Option<Form1600PtAgentCategory>>(stored("agent_category"))
                .ok()
                .flatten()
                .map_or(0, |category| form_1600pt_popup(category).count()),
        ),
        "1600VT" => (
            "AtcCode",
            serde_json::from_value::<Option<Form1600VtAgentCategory>>(stored("agent_category"))
                .ok()
                .flatten()
                .map_or(0, |category| form_1600vt_popup(category).count()),
        ),
        // 1601FQ draws the private list before item 11 is answered, like
        // `init()` (`atc_options`).
        "1601FQ" => (
            "AtcCd",
            serde_json::from_value::<Option<Form1601FqCategory>>(stored("category"))
                .ok()
                .flatten()
                .unwrap_or(Form1601FqCategory::Private)
                .atc_options()
                .len(),
        ),
        _ => return 0,
    };
    def.boxes
        .iter()
        .filter(|entry| entry.is_question())
        .filter(|entry| {
            entry
                .key
                .strip_prefix(prefix)
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n > drawn)
        })
        .count()
}

/// The record a draft's penalty resolution stands on: its typed trigger
/// state, the trigger date its fields hold and the due rule that date is
/// read by (2552's Item 11 moves it). `None` for a form without a trigger
/// date.
fn trigger_record(kind: AgentFormKind, draft: &Value) -> Option<Value> {
    let code = kind.form_code();
    if bir_core::forms::event_trigger::trigger_paths(code).is_empty() {
        return None;
    }
    Some(json!({
        "entered": draft.get("trigger_date_entered").cloned().unwrap_or(Value::Null),
        "parts": draft.get("trigger_parts_typed").cloned().unwrap_or(Value::Null),
        "date": bir_core::forms::event_trigger::held_trigger_date(code, draft),
        "rule": format!("{:?}", bir_core::forms::event_trigger::due_rule_for(code, draft)),
    }))
}

/// The trigger fields box `key` of `kind` writes (its binding's paths whose
/// top-level field is one of the form's trigger fields).
fn trigger_fields_of(kind: AgentFormKind, key: &str) -> Vec<&'static str> {
    let code = kind.form_code();
    kind.definition()
        .bindings
        .get(key)
        .into_iter()
        .flat_map(|bound| agent::binding_paths(&bound.binding))
        .filter_map(|path| bir_core::forms::event_trigger::trigger_field_of(code, path))
        .collect()
}

/// Apply official keys to a copy of `draft` and recompute it. Boxes the user
/// typed are kept (`kept`); boxes that did not show the value after the
/// recompute (an item that must be answered first) are reported back. Every
/// refusal is the box's own: a box with no field in this app's draft
/// (unmapped, such as an official ATC popup slot), a computed, locked or
/// hidden box, or a key that is not a box is refused on its own with the
/// reason (`refused`) and the rest of the batch still applies.
pub(super) fn fill_official(
    kind: AgentFormKind,
    draft: &Value,
    assist: &AssistState,
    fields: &serde_json::Map<String, Value>,
) -> Result<OfficialFill, String> {
    let mut next = draft.clone();
    let (mut filled, mut kept) = (Vec::new(), Vec::new());
    let mut refused = serde_json::Map::new();
    let def = kind.definition();
    for (key, value) in fields {
        if assist.is_user_box(key) {
            kept.push(key.clone());
            continue;
        }
        if let Err(why) = agent::fill(kind, &mut next, key, value) {
            refused.insert(key.clone(), Value::String(why));
            continue;
        }
        // An accepted write to a trigger box is the filer's instruction on
        // a draft that follows the typed-date model: record the part (a
        // cleared box takes it out). An unmarked row is left as stored.
        if next["trigger_date_model"] == true {
            for field in trigger_fields_of(kind, key) {
                bir_core::forms::event_trigger::note_trigger_field_written(
                    kind.form_code(),
                    &mut next,
                    field,
                );
            }
        }
        filled.push(key.clone());
    }
    tax_treaty_2553(kind, draft, &mut next, fields, &filled)?;
    agent::compute(kind, &mut next)?;
    let map = agent::field_map(kind, &next)?;
    let text = |value: &Value| match value {
        Value::String(text) => text.trim().to_string(),
        other => other.to_string(),
    };
    let not_shown = filled
        .iter()
        .filter(|key| {
            let wanted = fields.get(*key).map(text).unwrap_or_default();
            let shown = def.box_value(key, &map);
            if shown.is_empty()
                && !map.contains_key(*key)
                && def.box_def(key).is_none_or(|entry| !entry.radio_group)
            {
                return true;
            }
            let shown = &shown;
            if def.box_def(key).is_some_and(|entry| entry.radio_group) {
                return shown.is_empty();
            }
            match def.bindings.get(*key).map(|bound| &bound.binding) {
                Some(Binding::Amount { .. }) => {
                    match (agent::parse_amount(&wanted), agent::parse_amount(shown)) {
                        (Some(wanted), Some(shown)) => (wanted - shown).abs() >= 0.5,
                        _ => !wanted.is_empty(),
                    }
                }
                Some(Binding::Whole { .. }) => {
                    wanted.parse::<i64>().ok() != shown.trim().parse::<i64>().ok()
                }
                Some(Binding::Text { date: None, .. }) => {
                    !shown.to_uppercase().contains(&wanted.to_uppercase())
                }
                Some(Binding::Text { .. }) => shown.is_empty(),
                Some(Binding::Choice { .. }) | None => {
                    !wanted.eq_ignore_ascii_case(shown) && shown != "true"
                }
            }
        })
        .cloned()
        .collect();
    Ok((next, filled, kept, not_shown, refused))
}

/// 2553 item 13's list (`lstTaxTreaty`).
const TAX_TREATY_LIST: &str = "frm2553:lstTaxTreaty";

/// What the assistant is told when it answers 2553 item 13 Yes without its
/// list: the page's own reason, that nothing in the call was applied, and
/// what to do.
fn tax_treaty_refusal() -> String {
    format!(
        "{} Item 13 is Yes, so its list must be answered. Nothing in this call was applied: the other boxes in it were not filled either. Ask the user which applies and fill `{TAX_TREATY_LIST}` with it (\"1\" Special Rate, \"2\" International Tax Treaty) in the same call as the Yes.",
        bir_core::forms::form_2553::TAX_TREATY_UNCHOSEN
    )
}

/// A fill value as text to compare with the list's answers: a string
/// trimmed, null as blank, any other scalar as its JSON text (`1`), as
/// `agent::fill`'s `value_text` reads it (which does not trim).
fn fill_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 2553 item 13 (maintainer decision D7, 2026-10-18). The pin binds the
/// Yes radio to Special Rate, so `form.fill` answering Yes alone filed a
/// relief nobody chose, where the page opens the list blank and Check asks
/// for it. Here a Yes with its list answered in the same fill (`1` or `2`,
/// as a string or a number) takes the list's answer (applied last, so the
/// radio's binding cannot replace it); a Yes alone keeps the relief already
/// chosen on the return, and on a return with none is refused with the
/// page's reason. Only these item 13 keys are read; every other fill is
/// unchanged.
fn tax_treaty_2553(
    kind: AgentFormKind,
    before: &Value,
    next: &mut Value,
    fields: &serde_json::Map<String, Value>,
    filled: &[String],
) -> Result<(), String> {
    if kind.form_code() != "2553" {
        return Ok(());
    }
    let chosen = |draft: &Value| {
        matches!(
            draft["tax_treaty"].as_str(),
            Some("special_rate" | "international_tax_treaty")
        )
    };
    // Whether this fill answers item 13 Yes: one of its item 13 radio keys,
    // filled alone, makes the return's item 13 a Yes.
    let says_yes = filled
        .iter()
        .filter(|key| key.starts_with("frm2553:optTreaty"))
        .any(|key| {
            let mut alone = before.clone();
            fields
                .get(key)
                .is_some_and(|value| agent::fill(kind, &mut alone, key, value).is_ok())
                && chosen(&alone)
        });
    if !says_yes {
        return Ok(());
    }
    if let Some(list) = fields
        .get(TAX_TREATY_LIST)
        .filter(|_| filled.iter().any(|key| key == TAX_TREATY_LIST))
        && matches!(fill_text(list).as_str(), "1" | "2")
    {
        return agent::fill(kind, next, TAX_TREATY_LIST, list);
    }
    if chosen(before) {
        next["tax_treaty"] = before["tax_treaty"].clone();
        return Ok(());
    }
    Err(tax_treaty_refusal())
}

/// Validation errors with the label of the box they name, when the form
/// has one (`{field, label, message}`).
fn labelled_errors(kind: AgentFormKind, errors: &[(String, String)]) -> Vec<Value> {
    let def = kind.definition();
    errors
        .iter()
        .map(|(field, message)| {
            let label = def
                .box_def(field)
                .map(|entry| entry.label.clone())
                .or_else(|| {
                    agent::error_keys(kind, field)
                        .first()
                        .and_then(|key| def.box_def(&def.question_of(key)))
                        .map(|entry| entry.label.clone())
                })
                .unwrap_or_else(|| field.replace(['_', '.'], " "));
            json!({ "field": field, "label": label, "message": message })
        })
        .collect()
}

/// The `note` every refused fill result carries: the tool result itself
/// tells the model to pass the refusals on, since the caller only ever
/// hears of `refused` if the model says it (review F6).
const REFUSED_NOTE: &str =
    "some boxes were refused and not written; report each `refused` key and its reason to the user";

/// Boxes refused on their own (`refused`, key -> reason), beside what was
/// applied. A result value that is not an object is wrapped, so the
/// refusals are never dropped (review F7).
pub(super) fn with_refused(
    result: DispatchResult,
    refused: serde_json::Map<String, Value>,
) -> DispatchResult {
    if refused.is_empty() {
        return result;
    }
    let value = match result.value.unwrap_or(Value::Null) {
        Value::Object(mut object) => {
            object.insert("refused".into(), Value::Object(refused));
            object.insert("note".into(), json!(REFUSED_NOTE));
            Value::Object(object)
        }
        other => json!({ "result": other, "refused": refused, "note": REFUSED_NOTE }),
    };
    DispatchResult::json(value)
}

pub(super) fn with_not_shown(result: DispatchResult, not_shown: Vec<String>) -> DispatchResult {
    if not_shown.is_empty() {
        return result;
    }
    let mut value = result.value.unwrap_or(Value::Null);
    if let Value::Object(object) = &mut value {
        object.insert("not_shown".into(), json!(not_shown));
        object.insert(
            "note".into(),
            json!("These boxes were written but the form does not show them yet; answer the item that opens them (for example Amended Return or Tax Relief)."),
        );
    }
    DispatchResult::json(value)
}

/// The sheet slug and filled field map the page's Print preview uses.
pub(crate) fn print_sheet(
    kind: AgentFormKind,
    draft: &Value,
) -> Result<(&'static str, std::collections::BTreeMap<String, String>), String> {
    crate::views::queueable_forms::print_sheet(kind, draft)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bir_core::db::Database;
    use bir_core::forms::agent::{self, AgentFormKind, Binding};
    use bir_core::forms::registry::FilingFrequency;
    use chrono::Datelike;
    use gpui_agent::dispatch::handle_request;
    use gpui_agent::protocol::{Op, PlatformKind, Request, Response};
    use serde_json::{Value, json};

    use super::super::{BirAgentHost, fixture_profile, parse_tin};
    use crate::agent::assist::BoxSource;

    const TIN: &str = "11111111400000";

    fn host(platform: PlatformKind) -> (BirAgentHost, Arc<Mutex<Database>>) {
        let db = Arc::new(Mutex::new(
            Database::open_ephemeral().expect("ephemeral db"),
        ));
        let mut profile = fixture_profile();
        profile.tin = parse_tin(TIN).expect("test TIN");
        db.lock().unwrap().save_profile(profile).expect("profile");
        let mut host = BirAgentHost::new(platform).with_database(Arc::clone(&db));
        host.select_profile(TIN).expect("select");
        (host, db)
    }

    fn call(host: &mut BirAgentHost, name: &str, args: Value) -> Response {
        handle_request(
            host,
            Request::new(
                "t",
                Op::Invoke {
                    name: name.into(),
                    args,
                },
            ),
            None,
            None,
        )
    }

    fn ok(response: Response, what: &str) -> Value {
        assert!(response.ok, "{what}: {:?}", response.error);
        response.result.unwrap_or(Value::Null)
    }

    /// A period each form's page accepts: last year's first month/quarter,
    /// the annual return, or the first as-needed return.
    fn period_for(code: &str) -> u8 {
        match bir_core::forms::find_form(code).map(|form| form.frequency.clone()) {
            Some(FilingFrequency::Annual) => 0,
            _ => 1,
        }
    }

    fn year() -> u16 {
        chrono::Local::now().year() as u16 - 1
    }

    #[test]
    fn every_form_opens_lists_validates_previews_and_saves() {
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let (mut host, db) = host(PlatformKind::Headless);
            let opened = ok(
                call(
                    &mut host,
                    "form.open",
                    json!({"code": code, "year": year(), "period": period_for(code), "tin": TIN}),
                ),
                code,
            );
            assert_eq!(opened["form"], code);
            let fields = ok(call(&mut host, "form.fields", json!({})), code);
            let boxes = fields
                .get("official_boxes")
                .or_else(|| fields.get("boxes"))
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{code}: no boxes in {fields}"));
            assert_eq!(
                boxes.len(),
                kind.definition().fillable().count(),
                "{code}: form.fields lists every fillable box"
            );
            let validated = ok(call(&mut host, "form.validate", json!({})), code);
            assert_eq!(validated["form"], code);
            assert!(validated["field_errors"].is_array(), "{code}");
            ok(call(&mut host, "form.needs_you", json!({})), code);
            let card = ok(call(&mut host, "form.card", json!({})), code);
            assert!(card["fields"].is_array(), "{code}");
            let preview = ok(call(&mut host, "form.preview", json!({})), code);
            assert_eq!(preview["kind"], "frozen-html", "{code}");
            assert_eq!(
                preview["pdf"],
                Value::Null,
                "{code}: never calls HTML a PDF"
            );
            assert_eq!(preview["opened_in_app"], false, "{code}: headless");
            let path = preview["path"].as_str().expect("path");
            let html = std::fs::read_to_string(path).expect("preview html");
            assert!(html.contains("<html"), "{code}");
            assert!(!html.contains("background-image: url(data:image"), "{code}");
            let _ = std::fs::remove_dir_all(std::path::Path::new(path).parent().unwrap());
            let review = call(&mut host, "form.review_in_app", json!({}));
            assert!(!review.ok, "{code}: headless refuses the hand-over");
            ok(call(&mut host, "form.save_draft", json!({})), code);
            ok(call(&mut host, "form.dismiss", json!({})), code);
            let queued = db.lock().unwrap().list_all_queued_submissions().unwrap();
            assert!(queued.is_empty(), "{code}: nothing was queued");
        }
    }

    /// `form.validate` runs the same seam the page's Validate runs, on the
    /// draft the page opens.
    #[test]
    fn generic_validate_is_the_page_validate() {
        for kind in AgentFormKind::all() {
            let AgentFormKind::Queueable(queueable) = kind else {
                continue;
            };
            let code = kind.form_code();
            let (mut host, db) = host(PlatformKind::Headless);
            ok(
                call(
                    &mut host,
                    "form.open",
                    json!({"code": code, "year": year(), "period": period_for(code), "tin": TIN}),
                ),
                code,
            );
            let mut page = host.generic_form().expect("open").draft.clone();
            let expected = agent::validate(kind, &mut page).expect("page validate");
            let validated = ok(call(&mut host, "form.validate", json!({})), code);
            let got: Vec<(String, String)> = validated["field_errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| {
                    (
                        error["field"].as_str().unwrap().to_string(),
                        error["message"].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            assert_eq!(got, expected, "{code}");
            drop(db);
            let _ = queueable;
        }
    }

    /// PR 14 review F5: a box with no field in this app's draft (here
    /// 1600PT's ATC popup slot, unbound in core) is refused on its own with
    /// the reason; the rest of the batch applies and item 10 stays as
    /// answered. Computed, locked, hidden and unknown keys are refused the
    /// same per-key way (fill_official).
    #[test]
    fn an_unmapped_box_is_refused_alone_and_the_batch_applies() {
        let (mut host, _db) = host(PlatformKind::Headless);
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "1600PT", "year": year(), "period": 1, "tin": TIN}),
            ),
            "open",
        );
        let before = host.generic_form().expect("open").draft["agent_category"].clone();
        let filled = ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {
                    "AtcCode1": "true",
                    "AtcCode5": "true",
                    "frm1600PT:Pg2Sc1Name1": "PAYEE ONE",
                }}),
            ),
            "fill",
        );
        assert_eq!(filled["filled"], json!(["frm1600PT:Pg2Sc1Name1"]));
        let refused = filled["refused"].as_object().expect("refused");
        assert_eq!(refused.len(), 2, "{filled}");
        for key in ["AtcCode1", "AtcCode5"] {
            let why = refused[key].as_str().unwrap();
            assert!(
                why.starts_with(&format!(
                    "`{key}` cannot be filled from here: the ATC popup's tick box"
                )),
                "{why}"
            );
        }
        // Each slot keeps the probe's own evidence (review F6).
        assert!(
            refused["AtcCode1"]
                .as_str()
                .unwrap()
                .ends_with("; the probe tied it to /agent_category")
        );
        assert!(refused["AtcCode5"].as_str().unwrap().ends_with(
            "; probe: written by the app without a draft field (a constant or the period)"
        ));
        let draft = &host.generic_form().expect("open").draft;
        assert_eq!(draft["agent_category"], before, "item 10 is untouched");
        assert_eq!(draft["schedule"][0]["payee_name"], "PAYEE ONE");
    }

    /// An accepted write to a trigger box is the filer's instruction on a
    /// typed-date-model draft (decision P5-7): the part joins the parts the
    /// return is dated by, a cleared box takes its part out, and once the
    /// date counts the penalty inputs resolve on it (decision P5-9).
    #[test]
    fn an_agent_fill_records_the_typed_trigger_parts() {
        let (mut host, _db) = host(PlatformKind::Headless);
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "1606", "year": year(), "period": 1, "tin": TIN}),
            ),
            "open",
        );
        // The page's own loader marked this new draft for the model.
        assert_eq!(
            host.generic_form().unwrap().draft["trigger_date_model"],
            true
        );
        assert_eq!(
            host.generic_form().unwrap().draft["penalty_due_date"],
            Value::Null,
            "no sale date yet: no due date"
        );

        ok(
            call(
                &mut host,
                "form.fill",
                json!({
                    "fields": {"frm1606:txtDateDay": "14"},
                    "source": "document"
                }),
            ),
            "fill day",
        );
        let draft = &host.generic_form().unwrap().draft;
        assert_eq!(draft["trigger_parts_typed"], json!(["transaction_day"]));
        assert_ne!(
            draft["trigger_date_entered"], true,
            "the month and year are not typed yet"
        );

        ok(
            call(
                &mut host,
                "form.fill",
                json!({
                    "fields": {
                        "frm1606:txtDateMonth": "3",
                        "frm1606:txtDateYear": "2026"
                    },
                    "source": "document"
                }),
            ),
            "fill month and year",
        );
        let draft = &host.generic_form().unwrap().draft;
        assert_eq!(draft["trigger_date_entered"], true);
        assert!(
            draft["penalty_due_date"].is_string(),
            "the record moved: the due date resolved on the typed sale date"
        );

        // A cleared box takes its part out; the date no longer counts and
        // the due date awaits the trigger again.
        ok(
            call(
                &mut host,
                "form.fill",
                json!({
                    "fields": {"frm1606:txtDateDay": "0"},
                    "source": "document"
                }),
            ),
            "clear day",
        );
        let draft = &host.generic_form().unwrap().draft;
        assert_ne!(draft["trigger_date_entered"], true);
        assert!(
            !draft["trigger_parts_typed"]
                .as_array()
                .is_some_and(|parts| parts.iter().any(|part| part == "transaction_day")),
            "the cleared box's part is out"
        );
        assert_eq!(draft["penalty_due_date"], Value::Null);
    }

    #[test]
    fn official_keys_fill_with_sources_and_keep_user_boxes() {
        let (mut host, _db) = host(PlatformKind::Headless);
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "0619E", "year": year(), "period": 1, "tin": TIN}),
            ),
            "open",
        );
        let def = AgentFormKind::from_code("0619E").unwrap().definition();
        let all: Vec<String> = def
            .fillable()
            .filter(|entry| matches!(def.bindings[&entry.key].binding, Binding::Amount { .. }))
            .map(|entry| entry.key.clone())
            .collect();
        // Boxes behind an unanswered item come back as `not_shown`; take
        // two the form shows right away.
        let mut amounts = Vec::new();
        for key in &all {
            let filled = ok(
                call(
                    &mut host,
                    "form.fill",
                    json!({"fields": {key.clone(): "1,234.50"}, "source": "document"}),
                ),
                "fill",
            );
            assert_eq!(filled["filled"], json!([key]));
            if filled.get("not_shown").is_none() {
                amounts.push(key.clone());
            }
            if amounts.len() == 2 {
                break;
            }
        }
        assert_eq!(amounts.len(), 2, "0619E shows amount boxes: {all:?}");
        let form = host.generic_form().unwrap();
        assert_eq!(
            form.assist.sources.get(&amounts[0]),
            Some(&BoxSource::Document)
        );
        let map = agent::field_map(form.kind, &form.draft).unwrap();
        assert_eq!(map[&amounts[0]].replace(',', ""), "1234.50");

        // A box the user typed is never overwritten.
        let mut assist = form.assist.clone();
        assist.note_user_edits([amounts[1].clone()]);
        let (kind, draft) = (form.kind, form.draft.clone());
        host.replace_generic_form(kind, draft, false, assist);
        let kept = ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {amounts[1].clone(): "9"}}),
            ),
            "fill",
        );
        assert_eq!(kept["kept_user_boxes"], json!([amounts[1]]));

        // Computed, locked, hidden and unknown keys are each refused with
        // the reason beside what applied — none aborts the batch.
        let computed = def
            .boxes
            .iter()
            .find(|entry| entry.computed)
            .expect("a computed box");
        let locked = def
            .bindings
            .iter()
            .find(|(_, bound)| bound.lock.is_some())
            .map(|(key, _)| key.clone())
            .expect("a locked box");
        let mut fields = serde_json::Map::new();
        fields.insert(computed.key.clone(), json!("1"));
        fields.insert(locked.clone(), json!("1"));
        fields.insert("nope".into(), json!("1"));
        fields.insert(amounts[0].clone(), json!("2"));
        // A hidden box too, where the form has one.
        let hidden = def
            .boxes
            .iter()
            .find(|entry| entry.kind.contains("hidden") || entry.required == "hidden")
            .map(|entry| entry.key.clone());
        let mut count = 3;
        if let Some(hidden) = &hidden {
            fields.insert(hidden.clone(), json!("1"));
            count = 4;
        }
        let filled = ok(
            call(&mut host, "form.fill", json!({ "fields": fields })),
            "fill",
        );
        assert_eq!(filled["filled"], json!([amounts[0]]));
        let refused = filled["refused"].as_object().expect("refused");
        assert_eq!(refused.len(), count, "{filled}");
        assert!(
            refused[&computed.key]
                .as_str()
                .unwrap()
                .contains("computed")
        );
        assert!(refused[&locked].as_str().unwrap().contains("`"));
        if let Some(hidden) = &hidden {
            assert!(refused[hidden].as_str().unwrap().contains("hidden"));
        }
        assert!(refused["nope"].as_str().unwrap().contains("not a box"));
        assert!(filled["note"].as_str().unwrap().contains("refused"));

        // `user` only for the card's own boxes.
        let refused = call(
            &mut host,
            "form.fill",
            json!({"fields": {amounts[0].clone(): "1"}, "source": "user"}),
        );
        assert!(!refused.ok, "user source needs from_card");
    }

    #[test]
    fn form_1601c_official_keys_and_item_3_needs_you() {
        let (mut host, _db) = host(PlatformKind::Headless);
        let year = chrono::Local::now().year() as u16;
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "1601C", "year": year, "period": 1, "tin": TIN}),
            ),
            "open",
        );
        // A new 1601C leaves Item 3 unanswered: it needs the user.
        let needs = ok(call(&mut host, "form.needs_you", json!({})), "needs");
        let listed: Vec<&str> = needs["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["field"].as_str().unwrap())
            .collect();
        assert!(listed.contains(&"any_taxes_withheld"), "{listed:?}");
        // The chat card asks for it too: one box for the radio item, with
        // the printed Yes/No as its options.
        let card = ok(call(&mut host, "form.card", json!({})), "card");
        let item_3 = card["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["key"] == "frm1601c:TaxWithheld")
            .unwrap_or_else(|| panic!("{card}"))
            .clone();
        assert_eq!(
            item_3["options"],
            json!([{"value": "1", "label": "Yes"}, {"value": "2", "label": "No"}])
        );
        assert_eq!(item_3["value"], "");
        assert!(
            !card["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["key"] == "frm1601c:TaxWithheld_2"),
            "radios are asked once, through their group"
        );
        // The user's answer from the card, by the group key and a label.
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {"frm1601c:TaxWithheld": "Yes"}, "source": "user", "from_card": true}),
            ),
            "card answer",
        );
        let draft = host.form_1601c_draft().unwrap();
        assert_eq!(draft.taxes_withheld_answer(), Some(true));
        assert_eq!(
            host.form_1601c_assist()
                .unwrap()
                .sources
                .get("frm1601c:TaxWithheld"),
            Some(&BoxSource::User)
        );
        // The radio's own key still works (compatibility alias).
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {"frm1601c:CatAgent_G": "true"}}),
            ),
            "radio alias",
        );
        assert_eq!(host.form_1601c_draft().unwrap().category_of_agent, "G");
        let needs = ok(call(&mut host, "form.needs_you", json!({})), "needs");
        assert!(
            !needs["boxes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["field"] == "any_taxes_withheld")
        );
        // Old keys still work beside the official ones.
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {"tax_14_total_compensation": "1000", "frm1601c:txtTax25": "100"}}),
            ),
            "mixed keys",
        );
        let draft = host.form_1601c_draft().unwrap();
        assert_eq!(draft.tax_14_total_compensation, 1000.0);
        assert_eq!(draft.tax_25_total_taxes_withheld, 100.0);
        // The profile owns the name; the period picks the month. A locked
        // box is refused on its own — the call itself still succeeds.
        for locked in ["frm1601c:txtTaxpayerName", "frm1601c:txtMonth"] {
            let filled = ok(
                call(&mut host, "form.fill", json!({"fields": {locked: "X"}})),
                "locked box refused alone",
            );
            assert!(
                filled["refused"][locked].as_str().unwrap().contains("`"),
                "{locked}: {filled}"
            );
        }
    }

    /// `form.review_in_app` hands a validated draft to the user: it saves
    /// and asks the app to bring the page up, and never changes the filing
    /// status or queues anything.
    #[test]
    fn review_in_app_never_changes_the_status_or_queues() {
        let (mut host, db) = host(PlatformKind::Desktop);
        let year = chrono::Local::now().year() as u16;
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "1601C", "year": year, "period": 1, "tin": TIN}),
            ),
            "open",
        );
        // Not before it validates.
        assert!(!call(&mut host, "form.review_in_app", json!({})).ok);
        assert!(host.take_review_request().is_none());
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {
                    "any_taxes_withheld": "Yes",
                    "category_of_agent": "P",
                    "tax_14_total_compensation": "1000",
                    "tax_25_total_taxes_withheld": "100",
                }}),
            ),
            "fill",
        );
        let validated = ok(call(&mut host, "form.validate", json!({})), "validate");
        assert_eq!(validated["ok"], true, "{validated}");
        let review = ok(call(&mut host, "form.review_in_app", json!({})), "review");
        assert_eq!(review["queued"], false);
        assert_eq!(review["filed"], false);
        assert_eq!(review["status"], "Draft");
        assert_eq!(
            review["message"],
            crate::views::form_header::AGENT_REVIEW_MESSAGE
        );
        assert_eq!(host.take_review_request(), Some(AgentFormKind::Form1601C));
        assert_eq!(
            host.form_1601c_status(),
            Some(bir_core::forms::FilingStatus::Draft)
        );
        let guard = db.lock().unwrap();
        assert!(guard.list_all_queued_submissions().unwrap().is_empty());
        assert!(guard.list_all_filed_submissions().unwrap().is_empty());
        let saved = guard
            .get_1601c_draft(TIN, year, 1)
            .unwrap()
            .expect("the draft was saved");
        assert_eq!(saved.status, bir_core::forms::FilingStatus::Draft);
    }

    #[test]
    fn review_in_app_on_a_generic_form_needs_a_clean_draft() {
        let (mut host, db) = host(PlatformKind::Desktop);
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "0619E", "year": year(), "period": 1, "tin": TIN}),
            ),
            "open",
        );
        let validated = ok(call(&mut host, "form.validate", json!({})), "validate");
        let review = call(&mut host, "form.review_in_app", json!({}));
        if validated["ok"] == true {
            assert!(review.ok, "{:?}", review.error);
        } else {
            assert!(!review.ok, "a draft with errors is not handed over");
        }
        let form = host.generic_form().unwrap();
        assert_eq!(
            agent::identity(form.kind, &form.draft).unwrap().status,
            bir_core::forms::FilingStatus::Draft
        );
        assert!(
            db.lock()
                .unwrap()
                .list_all_queued_submissions()
                .unwrap()
                .is_empty()
        );
    }

    /// Right after open, on every form: needs-you and the card list only
    /// boxes the user or the assistant can fill (a radio item once, by its
    /// group), never contradict validate (a form that validates needs
    /// nothing), and every validation error that names a fillable box is
    /// on the card with BIR's message.
    #[test]
    fn needs_you_and_card_match_validate_on_every_form() {
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let def = kind.definition();
            let (mut host, _db) = host(PlatformKind::Headless);
            ok(
                call(
                    &mut host,
                    "form.open",
                    json!({"code": code, "year": year(), "period": period_for(code), "tin": TIN}),
                ),
                code,
            );
            let needs = ok(call(&mut host, "form.needs_you", json!({})), code);
            let needs = needs.get("official").cloned().unwrap_or(needs);
            let card = ok(call(&mut host, "form.card", json!({})), code);
            let validated = ok(call(&mut host, "form.validate", json!({})), code);
            let fillable: Vec<&str> = def.fillable().map(|entry| entry.key.as_str()).collect();
            for (what, boxes) in [("needs-you", &needs["boxes"]), ("card", &card["fields"])] {
                for item in boxes.as_array().unwrap() {
                    let key = item["key"].as_str().unwrap();
                    assert!(
                        fillable.contains(&key),
                        "{code} {what} lists `{key}`, which cannot be filled"
                    );
                    assert_ne!(
                        item["label"], item["key"],
                        "{code} {what} `{key}` has no label"
                    );
                }
            }
            let listed = needs["boxes"].as_array().unwrap().len();
            if validated["ok"] == true {
                assert_eq!(listed, 0, "{code}: validates, yet needs-you lists {needs}");
                assert!(
                    card["fields"].as_array().unwrap().is_empty(),
                    "{code}: validates, yet the card asks {card}"
                );
            }
            assert_eq!(needs["ok"], validated["ok"], "{code}");
            for error in validated["field_errors"].as_array().unwrap() {
                let field = error["field"].as_str().unwrap();
                let keys: Vec<String> = agent::error_keys(kind, field)
                    .iter()
                    .map(|key| def.question_of(key))
                    .filter(|key| fillable.contains(&key.as_str()))
                    .collect();
                for key in keys {
                    assert!(
                        card["fields"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|item| item["key"] == key),
                        "{code}: error on `{field}` is not on the card as `{key}`"
                    );
                }
            }
        }
    }

    /// Sample output for review: `form.fields`, `form.needs_you` and
    /// `form.card` right after open (`--nocapture` prints them).
    #[test]
    fn sample_fields_needs_you_and_card_output() {
        for (code, period) in [("0619E", 1), ("2551Q", 1), ("1701Q", 1)] {
            let (mut host, _db) = host(PlatformKind::Headless);
            ok(
                call(
                    &mut host,
                    "form.open",
                    json!({"code": code, "year": year(), "period": period, "tin": TIN}),
                ),
                code,
            );
            for op in ["form.fields", "form.needs_you", "form.card"] {
                let mut out = ok(call(&mut host, op, json!({})), op);
                // The legacy 2551Q lists stay beside the official ones.
                if let Value::Object(object) = &mut out
                    && op == "form.fields"
                {
                    object.remove("fields");
                }
                println!(
                    "===== {code} {op}\n{}",
                    serde_json::to_string_pretty(&out).unwrap()
                );
            }
        }
    }

    /// Maintainer decision D7 (2026-10-18): 2553 item 13 Yes needs its list
    /// answered, as on the page. The pin binds the Yes radio to Special
    /// Rate, so the assistant answering Yes alone filed a relief nobody
    /// chose; now it is refused with the page's reason and the return is
    /// unchanged. A Yes with its list answered takes the list's answer, in
    /// either order; a Yes alone keeps a relief already chosen; No is
    /// taken as before.
    #[test]
    fn form_2553_item_13_yes_asks_for_its_list() {
        use bir_core::forms::form_2553::TAX_TREATY_UNCHOSEN;
        let (mut host, _db) = host(PlatformKind::Headless);
        ok(
            call(
                &mut host,
                "form.open",
                json!({"code": "2553", "year": year(), "period": period_for("2553"), "tin": TIN}),
            ),
            "open",
        );
        let treaty = |host: &BirAgentHost| host.generic_form().unwrap().draft["tax_treaty"].clone();
        assert_eq!(treaty(&host), "no");
        let before = host.generic_form().unwrap().draft.clone();
        for yes in [
            json!({"frm2553:optTreaty_1": "true"}),
            json!({"frm2553:optTreaty_1": "true", "frm2553:optTreaty_2": "false"}),
            json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": "0"}),
            json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": 0}),
            json!({"frm2553:txtSheets": "2", "frm2553:optTreaty_1": "true"}),
        ] {
            let refused = call(&mut host, "form.fill", json!({ "fields": yes }));
            assert!(!refused.ok, "{yes}");
            let error = refused.error.unwrap();
            assert!(error.contains(TAX_TREATY_UNCHOSEN), "{error}");
            assert!(
                error.contains("Nothing in this call was applied: the other boxes in it were not filled either."),
                "{error}"
            );
            assert!(error.contains("frm2553:lstTaxTreaty"), "{error}");
            assert_eq!(host.generic_form().unwrap().draft, before, "unchanged");
        }
        // The page's reason is the form's own message for an unanswered list.
        let kind = AgentFormKind::from_code("2553").unwrap();
        let mut unanswered = before.clone();
        unanswered["tax_treaty"] = json!("yes_unspecified");
        let errors = agent::validate(kind, &mut unanswered).unwrap();
        assert!(errors.contains(&("tax_treaty".to_string(), TAX_TREATY_UNCHOSEN.to_string())));
        // Answered in the same call, the list wins, whichever comes first.
        for (fields, wanted) in [
            (
                json!({"frm2553:lstTaxTreaty": "2", "frm2553:optTreaty_1": "true"}),
                "international_tax_treaty",
            ),
            (
                json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": "1"}),
                "special_rate",
            ),
            (
                json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": "2"}),
                "international_tax_treaty",
            ),
        ] {
            ok(
                call(&mut host, "form.fill", json!({ "fields": fields })),
                "fill",
            );
            assert_eq!(treaty(&host), wanted, "{fields}");
        }
        let map = agent::field_map(kind, &host.generic_form().unwrap().draft).unwrap();
        assert_eq!(map["frm2553:optTreaty_1"], "true");
        assert_eq!(map["frm2553:lstTaxTreaty"], "2");
        // A list answer the agent sends as a JSON number is the same answer,
        // in either order, and replaces the relief the return holds.
        for (fields, wanted) in [
            (
                json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": 1}),
                "special_rate",
            ),
            (
                json!({"frm2553:lstTaxTreaty": 2, "frm2553:optTreaty_1": "true"}),
                "international_tax_treaty",
            ),
            (
                json!({"frm2553:lstTaxTreaty": 1, "frm2553:optTreaty_1": "true"}),
                "special_rate",
            ),
            (
                json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": 2}),
                "international_tax_treaty",
            ),
            (
                json!({"frm2553:optTreaty_1": "true", "frm2553:lstTaxTreaty": "1"}),
                "special_rate",
            ),
            (
                json!({"frm2553:lstTaxTreaty": "2", "frm2553:optTreaty_1": "true"}),
                "international_tax_treaty",
            ),
        ] {
            ok(
                call(&mut host, "form.fill", json!({ "fields": fields })),
                "fill",
            );
            assert_eq!(treaty(&host), wanted, "{fields}");
        }
        // Yes alone on a return with a relief chosen keeps it.
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {"frm2553:optTreaty_1": "true"}}),
            ),
            "fill",
        );
        assert_eq!(treaty(&host), "international_tax_treaty");
        // No is taken as before.
        ok(
            call(
                &mut host,
                "form.fill",
                json!({"fields": {"frm2553:optTreaty_2": "true"}}),
            ),
            "fill",
        );
        assert_eq!(treaty(&host), "no");
    }
}
