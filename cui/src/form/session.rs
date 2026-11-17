//! The form page's session (port of `nsh/screens/form/session.ts`): the FormSession state machine.
//!
//! NSH signals become plain state: the page draws every frame, so nothing is "published" — the
//! session's `shown` map is what the boxes display, and `poll()`/`tick()` run the timers and the
//! in-flight host calls. Door answers the TS got through Promises arrive on channels polled here.
//!
//! What the session asks of the page around it comes back as [`SessionEvent`]s on `events`, which
//! the frame drains into `AppState` (toasts, cache bumps, retargets).

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::bir;
use crate::form::model;
use crate::form::schema::{self, FormSchema, OneDate, SchemaField, SchemaItem, keys_of};
use crate::lib::dates::{self, DATE_ERROR};
use crate::lib::errors::error_text;
use crate::lib::mask;
use crate::lib::money;
use crate::lib::queue_message::cancel_queue_message;
use crate::lib::todo::{self, TodoRowsOptions};
use crate::state::FormTarget;
use crate::types::{
    IncomeTaxElectionReadiness, ScreenFormAsk, ScreenFormError, ScreenFormExisting,
    ScreenFormFilled, ScreenFormNote, ScreenFormOpened, ScreenFormOption, ScreenFormPenalties,
    ScreenFormSamplePeriod, ScreenFormValidated,
};

// ---------------------------------------------------------------- constants (session.ts)

const NUMBER_KINDS: [&str; 3] = ["money", "pct", "int"];
const CLEARED: &str = "0";
const NO_PREVIEW: &str = "Print preview is not available on this device.";
const ELECTION_UNRECORDED: &str =
    "needs the taxpayer profile's recorded annual income-tax election";
const SAVING_PAINT_MS: u64 = 40;
const UNDO_WINDOW_MS: u64 = 5000;
const VALIDATE_DELAY_MS: u64 = 300;
const AUTOSAVE_DELAY_MS: u64 = 700;
const PERIOD_ASK_DELAY_MS: u64 = 300;
const LIVE_PREVIEW_DELAY_MS: u64 = 120;

/// 2551Q's Item 13 radio for each answer of the election.
fn election_radios(form_code: &str) -> Option<(&'static str, &'static str)> {
    if form_code == "2551Q" {
        Some(("frm2551Qv2018:taxRate1", "frm2551Qv2018:taxRate2"))
    } else {
        None
    }
}

// ---------------------------------------------------------------- public types

#[derive(Debug, Clone)]
pub enum SaveState {
    Idle,
    Saving,
    Saved,
    /// Something is staged or changed and not saved yet.
    Unsaved,
    Failed(String),
    /// Not autosaved: why.
    Manual(String),
}

impl SaveState {
    pub fn label(&self) -> String {
        match self {
            SaveState::Idle => "".to_owned(),
            SaveState::Saving => "Saving…".to_owned(),
            SaveState::Saved => "Saved".to_owned(),
            SaveState::Unsaved => "Unsaved changes".to_owned(),
            SaveState::Failed(m) => m.clone(),
            SaveState::Manual(m) => m.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AskCard {
    pub fact: String,
    pub label: String,
    pub note: String,
    pub control: String,
    pub choices: Vec<crate::types::AskChoice>,
    /// The picked choice's value as JSON, or None.
    pub pick: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum ElectionCard {
    Ask {
        question: String,
        pick: Option<String>,
    },
    Blocked {
        title: String,
        body: String,
        action: Option<&'static str>, // "fp-election-retry" | "fp-election-follow"
    },
}

/// Everything a row draws besides its boxes.
#[derive(Debug, Clone, Default)]
pub struct RowInfo {
    pub message: String,
    pub notes: Vec<String>,
    pub needs: Vec<String>,
    pub kept: &'static str, // "template" | "last" | ""
    pub step_here: bool,
    pub stoppable: bool,
    pub is_stop: bool,
    pub from_profile: String,
    pub can_edit: bool,
    pub editing: bool,
    pub to_profile: Option<(bool, String)>,
    pub penalty: Option<&'static str>, // "edit" | "computed"
    pub tucked: bool,
    pub asks: Vec<AskCard>,
    pub ask_saved: String,
    pub election: Option<ElectionCard>,
    pub election_saved: String,
    pub checkable: Vec<(String, String)>,
    pub from: String,
}

#[derive(Debug, Clone, Default)]
pub struct PartInfo {
    pub errors: usize,
    pub todo: usize,
    pub open: bool,
    pub fold: Option<PartFold>,
}

#[derive(Debug, Clone)]
pub struct PartFold {
    pub first: usize,
    pub items: Vec<usize>,
    pub label: String,
    pub flagged: usize,
    pub open: bool,
}

#[derive(Debug, Clone, Default)]
pub struct HeaderInfo {
    pub loading: bool,
    pub error: String,
    pub status: String,
    pub editable: bool,
    pub sample: bool,
    pub period_set: bool,
    pub amends: bool,
    pub amend_unsaved: bool,
    pub filed_original: bool,
    pub filed_original_view: Option<crate::types::FiledVersion>,
    pub submitted_line: String,
    pub profile_updates: Vec<String>,
    pub profile_differs: Vec<String>,
    pub profile_changed: Vec<String>,
    pub notes: Vec<String>,
    pub form_errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Overlay {
    Removal {
        message: String,
    },
    Existing {
        existing: ScreenFormExisting,
    },
    QueueConfirm {
        warns: Vec<String>,
    },
    SamplePeriod {
        parts: Vec<SamplePeriodPart>,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct SamplePeriodPart {
    pub key: String,
    pub part: String,
    pub quarter: Option<i64>,
    pub value: String,
}

/// What the session asks of the page around it (the frame drains these into AppState).
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Toast {
        sid: u64,
        message: String,
        icon: Option<String>,
        action: Option<(String, UndoCmd)>,
    },
    DismissToast {
        sid: u64,
    },
    /// A write went through the door (name: the host function): the frame drops its cached reads.
    AfterWrite(String),
    /// A changed return was saved, or one was stored for the first time.
    Saved(FormTarget),
    /// The session now shows another return.
    Retarget(FormTarget),
    /// The return was queued or taken out of the queue.
    QueueChanged(FormTarget),
}

/// A toast action that reaches back into the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoCmd {
    /// A schedule removal's Undo.
    Removal,
    /// "Keep this form" on a sample's period offer.
    KeepPeriod,
    /// Undo a Load / View / Amend of the sample's period offer.
    RestoreForm,
}

// ---------------------------------------------------------------- internal state (the old page's `st`)

#[derive(Debug, Clone, Default)]
struct RemovalHeld {
    r: ScreenFormFilled,
    edits: HashMap<String, String>,
    typed: HashMap<String, String>,
    draft: Value,
}

#[derive(Debug, Clone)]
struct PeriodSnapshot {
    fields: PeriodFields,
    shown: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct PeriodFields {
    draft: Value,
    values: HashMap<String, String>,
    options: HashMap<String, Vec<Value>>,
    locks: HashMap<String, String>,
    lock_notes: HashMap<String, String>,
    notes_always: HashSet<String>,
    blank: HashSet<String>,
    needs: HashMap<String, String>,
    whys: HashMap<String, String>,
    errors: Vec<ScreenFormError>,
    messages: HashMap<String, String>,
    form_errors: Vec<String>,
    period_values: HashMap<String, String>,
    period_asked: Option<String>,
    period_set: bool,
    opens: Option<crate::types::OpenTarget>,
    existing: Option<ScreenFormExisting>,
}

/// What a sample's period Save waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeriodThen {
    Queue,
}

#[derive(Debug, Clone)]
struct St {
    target: FormTarget,
    back: Option<FormTarget>,
    loading: bool,
    error: String,
    draft: Option<Value>,
    status: String,
    editable: bool,
    stored_period: Option<i64>,
    values: HashMap<String, String>,
    options: HashMap<String, Vec<Value>>,
    locks: HashMap<String, String>,
    lock_notes: HashMap<String, String>,
    notes_always: HashSet<String>,
    profile: HashSet<String>,
    profile_edit: HashSet<String>,
    editing: HashSet<String>,
    opened: HashSet<String>,
    focus_key: Option<String>,
    profile_open: HashSet<usize>,
    sample: bool,
    blank: HashSet<String>,
    needs: HashMap<String, String>,
    whys: HashMap<String, String>,
    waits: HashMap<String, String>,
    off: HashMap<String, String>,
    penalties: Vec<ScreenFormPenalties>,
    penalty_open: HashSet<String>,
    pool_texts: HashMap<String, String>,
    typed: HashSet<String>,
    typed_edits: HashMap<String, String>,
    period_parts: HashMap<String, crate::types::ScreenFormPeriodPart>,
    period_set: bool,
    period_values: HashMap<String, String>,
    period_asked: Option<String>,
    period_answered: Option<String>,
    period_complete: bool,
    period_seq: u64,
    period_token: u64,
    period_in_flight: Option<String>,
    opens: Option<crate::types::OpenTarget>,
    existing: Option<ScreenFormExisting>,
    save_after_answer: Option<PeriodThen>,
    submitted_at: Option<String>,
    confirmed_at: Option<String>,
    amends: Option<String>,
    filed_original: Option<crate::types::FiledVersion>,
    filed_original_view: Option<crate::types::FiledVersion>,
    profile_updates: Vec<String>,
    profile_differs: Vec<String>,
    notes: Vec<NoteState>,
    saved: bool,
    quiet: Option<crate::types::ScreenFormQuiet>,
    kept: HashMap<String, String>,
    auto_stops: Vec<String>,
    stop_edits: HashMap<String, bool>,
    stop_keys: HashSet<String>,
    stoppable: HashSet<String>,
    passed: HashSet<String>,
    step_at: Option<String>,
    step_from_top: bool,
    asks: Vec<ScreenFormAsk>,
    ask_at: HashMap<String, Vec<ScreenFormAsk>>,
    ask_pick: HashMap<String, Value>,
    ask_text: HashMap<String, String>,
    ask_saved_at: HashMap<String, String>,
    fact_errors: Vec<ScreenFormError>,
    profile_save: HashMap<String, crate::types::ProfileSaveField>,
    profile_edited: HashSet<String>,
    to_profile: HashSet<String>,
    profile_changed: Vec<String>,
    errors: Vec<ScreenFormError>,
    messages: HashMap<String, String>,
    /// Keys each row's messages name (a red border on those boxes).
    message_keys: HashMap<String, Vec<String>>,
    /// How many messages each row carries.
    message_counts: HashMap<String, usize>,
    /// Boxes the host refused at the last fill, with its message.
    rejected: HashMap<String, String>,
    form_errors: Vec<String>,
    checkable: HashMap<String, Vec<(String, String)>>,
    election_at: Option<String>,
    election_pick: Option<String>,
    election_saved_at: Option<String>,
    profile_refreshed: bool,
    date_errors: HashMap<String, String>,
    dates_edited: HashSet<String>,
    pending: HashMap<String, String>,
    /// What the boxes showed before a choice, for a removal the filer cancels.
    chosen_from: HashMap<String, String>,
    dirty: bool,
    edit_seq: u64,
    validate_seq: u64,
    checking: bool,
    removal: Option<RemovalHeld>,
    queue_warn: Option<Vec<String>>,
    open_parts: HashSet<usize>,
    current: usize,
    touched: HashSet<String>,
    show_all: bool,
    preview_open: bool,
    overlay: Option<Overlay>,
    save_state: SaveState,
    election_ready: Option<Option<IncomeTaxElectionReadiness>>,
    more_open: HashSet<String>,
}

#[derive(Debug, Clone)]
struct NoteState {
    key: String,
    text: String,
    recorded: bool,
}

fn fresh_state(target: &FormTarget) -> St {
    let mut open_parts = HashSet::new();
    open_parts.insert(0);
    St {
        target: target.clone(),
        back: None,
        loading: true,
        error: String::new(),
        draft: None,
        status: "Draft".to_owned(),
        editable: true,
        stored_period: None,
        values: HashMap::new(),
        options: HashMap::new(),
        locks: HashMap::new(),
        lock_notes: HashMap::new(),
        notes_always: HashSet::new(),
        profile: HashSet::new(),
        profile_edit: HashSet::new(),
        editing: HashSet::new(),
        opened: HashSet::new(),
        focus_key: None,
        profile_open: HashSet::new(),
        sample: target.mode == "sample",
        blank: HashSet::new(),
        needs: HashMap::new(),
        whys: HashMap::new(),
        waits: HashMap::new(),
        off: HashMap::new(),
        penalties: Vec::new(),
        penalty_open: HashSet::new(),
        pool_texts: HashMap::new(),
        typed: HashSet::new(),
        typed_edits: HashMap::new(),
        period_parts: HashMap::new(),
        period_set: false,
        period_values: HashMap::new(),
        period_asked: None,
        period_answered: None,
        period_complete: false,
        period_seq: 0,
        period_token: 0,
        period_in_flight: None,
        opens: None,
        existing: None,
        save_after_answer: None,
        submitted_at: None,
        confirmed_at: None,
        amends: None,
        filed_original: None,
        filed_original_view: None,
        profile_updates: Vec::new(),
        profile_differs: Vec::new(),
        notes: Vec::new(),
        saved: false,
        quiet: None,
        kept: HashMap::new(),
        auto_stops: Vec::new(),
        stop_edits: HashMap::new(),
        stop_keys: HashSet::new(),
        stoppable: HashSet::new(),
        passed: HashSet::new(),
        step_at: None,
        step_from_top: false,
        asks: Vec::new(),
        ask_at: HashMap::new(),
        ask_pick: HashMap::new(),
        ask_text: HashMap::new(),
        ask_saved_at: HashMap::new(),
        fact_errors: Vec::new(),
        profile_save: HashMap::new(),
        profile_edited: HashSet::new(),
        to_profile: HashSet::new(),
        profile_changed: Vec::new(),
        errors: Vec::new(),
        messages: HashMap::new(),
        message_keys: HashMap::new(),
        message_counts: HashMap::new(),
        rejected: HashMap::new(),
        form_errors: Vec::new(),
        checkable: HashMap::new(),
        election_at: None,
        election_pick: None,
        election_saved_at: None,
        profile_refreshed: false,
        date_errors: HashMap::new(),
        dates_edited: HashSet::new(),
        pending: HashMap::new(),
        chosen_from: HashMap::new(),
        dirty: false,
        edit_seq: 0,
        validate_seq: 0,
        checking: false,
        removal: None,
        queue_warn: None,
        open_parts,
        current: 0,
        touched: HashSet::new(),
        show_all: false,
        preview_open: false,
        overlay: None,
        save_state: SaveState::Idle,
        election_ready: None,
        more_open: HashSet::new(),
    }
}

// ---------------------------------------------------------------- timers and async

#[derive(Debug)]
enum Timer {
    Flush { seq: u64, token: u64 },
    LivePreview { seq: u64, token: u64 },
    Autosave { token: u64 },
    SavePaint { token: u64 },
    PeriodAsk { asked: String, seq: u64, token: u64 },
    UndoWindow { undo_id: u64, token: u64 },
    CheckWait { token: u64 },
    EndProfileEdit { token: u64, row: String },
}

struct PendingOpen {
    token: u64,
    target: FormTarget,
    keep: bool,
    rx: crate::bir::AsyncReply,
}

struct PendingPeriod {
    asked: String,
    token: u64,
    rx: crate::bir::AsyncReply,
}

// ---------------------------------------------------------------- the session

pub struct FormSession {
    pub schema: Option<FormSchema>,
    st: St,
    token: u64,
    st_gen: u64,
    /// What the boxes show (the sValue map): key -> shown text, one-date keys included.
    shown: HashMap<String, String>,
    events: Vec<SessionEvent>,
    toast_seq: u64,
    timers: Vec<(Instant, Timer)>,
    pending_open: Option<PendingOpen>,
    pending_validate: Option<(u64, u64, crate::bir::AsyncReply)>,
    pending_period: Option<PendingPeriod>,
    pending_placeholder: Option<(u64, u64, crate::bir::AsyncReply)>,
    reopening: bool,
    period_waiting: bool,
    period_then: Option<PeriodThen>,
    /// The undo a removal toast runs (its generation guards staleness).
    removal_undo: Option<(u64, u64)>, // (st_gen, undo_id)
    undo_toast_sid: Option<u64>,
    undo_seq: u64,
    /// "Keep this form" undo: the period snapshot and the offer it restores.
    period_undo: Option<(PeriodSnapshot, ScreenFormExisting)>,
    /// Undo a Load/View/Amend: the sample's whole state and the offer it restores.
    form_undo: Option<(Box<St>, Option<ScreenFormExisting>, HashMap<String, String>)>,
    fast_lane_drafts: bool,
    disposed: bool,
    has_second_window: bool,

    // Schema indexes (static per session).
    key_index: HashMap<String, String>,
    field_of: HashMap<String, SchemaField>,
    kind_of: HashMap<String, String>,
    computed_keys: HashSet<String>,
    whole_keys: HashSet<String>,
    pools: HashMap<String, schema::PoolKeys>,
    all_keys: Vec<String>,
    row_ids: Vec<String>,
    items: HashMap<String, SchemaItem>,
    one_dates: HashMap<String, (OneDate, String)>, // oneKey -> (date, row id)
    /// A step's or jump's reveal request for the frame (scroll + focus).
    pub reveal: Option<Reveal>,
    reveal_seq: u64,
    /// A toast to show once an async open lands (take_existing's "Loaded…" + Undo).
    after_open_toast: Option<AfterOpenToast>,
    /// Removal undo payloads by undo_id (the draft to restore).
    removal_payloads: HashMap<u64, Value>,
    /// The `then` a sample-save reopen runs (queue after save).
    pending_open_then: Option<PeriodThen>,
}

impl FormSession {
    /// Loads the schema and starts the open of `target`.
    pub fn new(target: FormTarget) -> Self {
        let mut s = FormSession {
            schema: None,
            st: fresh_state(&target),
            token: 0,
            st_gen: 0,
            shown: HashMap::new(),
            events: Vec::new(),
            toast_seq: 0,
            timers: Vec::new(),
            pending_open: None,
            pending_validate: None,
            pending_period: None,
            pending_placeholder: None,
            reopening: false,
            period_waiting: false,
            period_then: None,
            removal_undo: None,
            undo_toast_sid: None,
            undo_seq: 0,
            period_undo: None,
            form_undo: None,
            fast_lane_drafts: false,
            disposed: false,
            // CUI has no second window or HTML renderer on any platform (print.rs answers
            // every request honestly), so the preview is never "available" here.
            has_second_window: false,
            key_index: HashMap::new(),
            field_of: HashMap::new(),
            kind_of: HashMap::new(),
            computed_keys: HashSet::new(),
            whole_keys: HashSet::new(),
            pools: HashMap::new(),
            all_keys: Vec::new(),
            row_ids: Vec::new(),
            items: HashMap::new(),
            one_dates: HashMap::new(),
            reveal: None,
            reveal_seq: 0,
            after_open_toast: None,
            removal_payloads: HashMap::new(),
            pending_open_then: None,
        };
        match bir::call1("screen_form_schema", json!(target.form_code))
            .ok()
            .and_then(|v| serde_json::from_value::<FormSchema>(v).ok())
        {
            Some(schema) => {
                let schema = schema::without_hidden(&schema);
                s.index_schema(&schema);
                s.schema = Some(schema);
            }
            None => {
                s.st.loading = false;
                if s.st.error.is_empty() {
                    s.st.error = "This form has no on-screen layout.".to_owned();
                }
            }
        }
        s
    }

    /// Starts (or restarts) the open of `target`. Answers the error text, or None.
    pub fn open(&mut self, target: Option<FormTarget>, keep: bool) -> Option<String> {
        let target = target.unwrap_or_else(|| self.st.target.clone());
        if self.schema.is_none() {
            return Some(self.st.error.clone());
        }
        let back = self.st.back.clone();
        self.token += 1;
        self.reopening = keep;
        if !keep {
            self.st = fresh_state(&target);
            if target.mode == "filed_original" {
                self.st.back = back;
            }
            self.st_gen += 1;
            self.shown.clear();
        }
        let token = self.token;
        let (name, args): (&str, Vec<Value>) = match target.mode.as_str() {
            "sample" => (
                "screen_form_sample",
                vec![json!(target.tin), json!(target.form_code)],
            ),
            "filed_original" => (
                "screen_form_filed_original",
                vec![
                    json!(target.form_code),
                    json!(target.snapshot_id.unwrap_or(0)),
                ],
            ),
            "amend" => (
                "screen_form_amend",
                vec![
                    json!(target.tin),
                    json!(target.form_code),
                    json!(target.year),
                    json!(target.period),
                    json!(target.key),
                ],
            ),
            _ => {
                if target.new_key_from.is_some() {
                    (
                        "screen_form_open",
                        vec![
                            json!(target.tin),
                            json!(target.form_code),
                            json!(target.year),
                            json!(target.period),
                            json!(target.key),
                            json!(target.new_key),
                            json!(target.new_key_from),
                        ],
                    )
                } else {
                    (
                        "screen_form_open",
                        vec![
                            json!(target.tin),
                            json!(target.form_code),
                            json!(target.year),
                            json!(target.period),
                            json!(target.key),
                        ],
                    )
                }
            }
        };
        let rx = bir::call_async_spawn(name, args);
        self.pending_open = Some(PendingOpen {
            token,
            target: target.clone(),
            keep,
            rx,
        });
        None
    }

    /// Convenience: open the route's target.
    pub fn open_route(&mut self, target: FormTarget) {
        self.open(Some(target), false);
    }

    /// Polls timers and in-flight host calls. Call once per frame.
    pub fn poll(&mut self) {
        if self.disposed {
            return;
        }
        self.poll_timers();
        self.poll_open();
        self.poll_validate();
        self.poll_period();
        self.poll_placeholder();
    }

    /// Drains the events the page around must apply.
    pub fn take_events(&mut self) -> Vec<SessionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Whether a session exists and is not disposed (the frame asks before drawing).
    pub fn alive(&self) -> bool {
        !self.disposed
    }

    // ---------------------------------------------------------------- timers

    fn later(&mut self, ms: u64, timer: Timer) {
        self.timers
            .push((Instant::now() + Duration::from_millis(ms), timer));
    }

    fn poll_timers(&mut self) {
        let now = Instant::now();
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for (at, t) in std::mem::take(&mut self.timers) {
            if at <= now {
                due.push(t);
            } else {
                keep.push((at, t));
            }
        }
        self.timers = keep;
        for t in due {
            if self.disposed {
                return;
            }
            match t {
                Timer::Flush { seq, token } => {
                    if token == self.token && seq == self.st.edit_seq {
                        self.flush();
                    }
                }
                Timer::LivePreview { seq, token } => {
                    if token == self.token && seq == self.st.edit_seq {
                        self.live_preview();
                    }
                }
                Timer::Autosave { token } => {
                    if token == self.token {
                        self.autosave();
                    }
                }
                Timer::SavePaint { token } => {
                    if token == self.token && !self.disposed {
                        if self.st.sample {
                            self.save_sample(None);
                        } else {
                            self.save_return(false);
                        }
                    }
                }
                Timer::PeriodAsk { asked, seq, token } => {
                    if token == self.token && seq == self.st.period_seq {
                        self.ask_period(&asked);
                    }
                }
                Timer::UndoWindow { undo_id, token } => {
                    if token == self.token {
                        if let Some((_, id)) = self.removal_undo {
                            if id == undo_id {
                                self.retire_removal_undo();
                                self.schedule_autosave();
                            }
                        }
                    }
                }
                Timer::CheckWait { token } => {
                    if token == self.token {
                        self.check_wait();
                    }
                }
                Timer::EndProfileEdit { token, row } => {
                    if token != self.token {
                        continue;
                    }
                    let now = self.st.focus_key.clone();
                    if let Some(now) = now {
                        if self.key_index.get(&now) == self.key_index.get(&row) {
                            continue;
                        }
                    }
                    let keys = self.item_edit_keys(&row);
                    if self.end_profile_edit(&keys) {
                        // publish is implicit
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- async polls

    fn poll_open(&mut self) {
        let Some(p) = self.pending_open.take() else {
            return;
        };
        match p.rx.try_recv() {
            Ok(Ok(v)) => {
                if p.token != self.token || self.disposed {
                    return;
                }
                match serde_json::from_value::<ScreenFormOpened>(v) {
                    Ok(r) => {
                        if p.keep {
                            self.st = self.carried(&p.target);
                        }
                        self.reopening = false;
                        self.st.loading = false;
                        self.take_opened(r);
                        // A stored return was checked before: its messages show at once.
                        if !p.keep && self.st.saved {
                            let ids: Vec<String> = self.st.messages.keys().cloned().collect();
                            for id in ids {
                                self.st.touched.insert(id);
                            }
                        }
                        if !self.st.pending.is_empty() {
                            self.st.dirty = true;
                            self.st.save_state = SaveState::Unsaved;
                            self.flush();
                        }
                    }
                    Err(e) => {
                        self.st.loading = false;
                        self.st.error = format!("Could not open the return: {e}");
                        self.toast(
                            self.st.error.clone(),
                            Some("triangle-alert".to_owned()),
                            None,
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                if p.token == self.token && !self.disposed {
                    if p.keep {
                        self.st = self.carried(&p.target);
                    }
                    self.reopening = false;
                    self.st.loading = false;
                    self.st.error = error_text(&e);
                    self.toast(
                        self.st.error.clone(),
                        Some("triangle-alert".to_owned()),
                        None,
                    );
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.pending_open = Some(p);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if p.token == self.token {
                    self.st.loading = false;
                    self.st.error = "The open call dropped.".to_owned();
                }
            }
        }
    }

    fn poll_validate(&mut self) {
        let Some((seq, token, rx)) = self.pending_validate.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(v)) => {
                if token != self.token || self.disposed || seq != self.st.validate_seq {
                    return;
                }
                match serde_json::from_value::<ScreenFormValidated>(v) {
                    Ok(r) => {
                        let same_draft = self.st.draft.as_ref() == Some(&r.draft);
                        if same_draft {
                            // draft already updated by fill; values merge below
                        }
                        // st.values = shown({...st.values, ...r.values})
                        let mut merged = self.st.values.clone();
                        for (k, v) in string_map(&r.values) {
                            merged.insert(k, v);
                        }
                        self.st.values = self.shown_values(merged);
                        self.st.options = options_map(&r.options);
                        self.st.off = string_map(&r.off);
                        if !r.penalties.is_empty() {
                            self.st.penalties = r.penalties;
                        }
                        if !r.pools.is_empty() {
                            self.st.pool_texts = string_map(&r.pools);
                        }
                        self.st.fact_errors = r.fact_errors;
                        self.sync_computed();
                        self.set_errors(r.errors);
                        self.st.form_errors = Vec::new();
                        self.st.checking = false;
                    }
                    Err(_) => {
                        self.st.checking = false;
                    }
                }
            }
            Ok(Err(e)) => {
                if token == self.token {
                    self.st.form_errors = vec![error_text(&e)];
                    if seq == self.st.validate_seq {
                        self.st.checking = false;
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.pending_validate = Some((seq, token, rx));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if token == self.token {
                    self.st.checking = false;
                }
            }
        }
    }

    fn poll_period(&mut self) {
        let Some(p) = self.pending_period.take() else {
            return;
        };
        match p.rx.try_recv() {
            Ok(Ok(v)) => {
                if self.st.period_in_flight.as_deref() == Some(p.asked.as_str()) {
                    self.st.period_in_flight = None;
                }
                if p.token != self.token
                    || self.disposed
                    || Some(&p.asked) != self.st.period_asked.as_ref()
                {
                    return;
                }
                self.st.period_answered = Some(p.asked.clone());
                match serde_json::from_value::<ScreenFormSamplePeriod>(v) {
                    Ok(r) => {
                        let save_after = self.st.save_after_answer.take();
                        self.take_period(r);
                        if let Some(then) = save_after {
                            self.after_period_for_save(Some(then));
                        }
                    }
                    Err(_) => {}
                }
            }
            Ok(Err(e)) => {
                if self.st.period_in_flight.as_deref() == Some(p.asked.as_str()) {
                    self.st.period_in_flight = None;
                }
                if p.token == self.token {
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.pending_period = Some(p);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if self.st.period_in_flight.as_deref() == Some(p.asked.as_str()) {
                    self.st.period_in_flight = None;
                }
            }
        }
    }

    fn poll_placeholder(&mut self) {
        let Some((period, token, rx)) = self.pending_placeholder.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(v)) => {
                if token != self.token || self.disposed || period != self.st.period_token {
                    return;
                }
                if let Ok(r) = serde_json::from_value::<ScreenFormOpened>(v) {
                    self.rebuild(r);
                }
            }
            Ok(Err(e)) => {
                if token == self.token {
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.pending_placeholder = Some((period, token, rx));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
        }
    }

    // ---------------------------------------------------------------- open plumbing

    fn carried(&self, target: &FormTarget) -> St {
        let old = &self.st;
        let mut s = fresh_state(target);
        s.pending = old.pending.clone();
        s.open_parts = old.open_parts.clone();
        s.current = old.current;
        s.touched = old.touched.clone();
        s.show_all = old.show_all;
        s.more_open = old.more_open.clone();
        s.profile_open = old.profile_open.clone();
        s
    }

    /// Take what the host opened (a return, a sample, or a sample rebuilt for its period).
    fn take_opened(&mut self, r: ScreenFormOpened) {
        {
            let s = &mut self.st;
            s.draft = Some(r.draft.clone());
            s.status = r.status.clone();
            s.editable = r.editable;
            s.stored_period = r.stored_period;
            s.blank = r.blank.iter().cloned().collect();
            s.period_parts = r
                .period_parts
                .iter()
                .map(|p| (p.key.clone(), p.clone()))
                .collect();
            s.period_set = r.period_set;
            s.submitted_at = r.submitted_at.clone();
            s.confirmed_at = r.confirmed_at.clone();
            s.amends = r.amends.clone();
            s.filed_original = r.filed_original.clone();
            s.filed_original_view = r.filed_original_view.clone();
            s.options = options_map(&r.options);
            s.locks = string_map(&r.locks);
            s.lock_notes = string_map(&r.lock_notes);
            s.notes_always = r.notes_always.iter().cloned().collect();
            s.profile = r.profile.iter().cloned().collect();
            s.profile_edit = r.profile_edit.iter().cloned().collect();
            s.needs = string_map(&r.needs);
            s.whys = string_map(&r.whys);
            s.waits = string_map(&r.waits);
            s.off = string_map(&r.off);
            s.penalties = r.penalties.clone();
            s.pool_texts = string_map(&r.pools);
            s.profile_updates = r.profile_updates.clone();
            s.profile_differs = r.profile_differs.clone();
            let notes: Vec<ScreenFormNote> = r.notes.clone();
            s.notes = notes
                .iter()
                .map(|n| NoteState {
                    key: n.key.clone(),
                    text: n.text.clone(),
                    recorded: false,
                })
                .collect();
            s.saved = r.draft.get("id").and_then(|v| v.as_i64()).is_some();
            s.quiet = r.quiet.clone();
            s.kept = string_map(&r.kept);
            s.auto_stops = r.auto_stops.clone();
            s.stop_edits = bool_map(&r.stop_edits);
            s.stop_keys = todo::effective_stops(&s.auto_stops, &s.stop_edits);
            s.stoppable = r.stoppable.iter().cloned().collect();
            s.profile_save = profile_save_map(&r.profile_save);
            s.profile_changed = r.profile_changed.clone();
        }
        let vals = string_map(&r.values);
        self.st.values = self.shown_values(vals);
        if self.st.period_set {
            let mut pv = HashMap::new();
            for (k, p) in self.st.period_parts.clone() {
                let v = if p.part == "quarter" {
                    (self.st.values.get(&k).map(String::as_str) == Some("true")).to_string()
                } else {
                    self.st.values.get(&k).cloned().unwrap_or_default()
                };
                pv.insert(k, v);
            }
            self.st.period_asked = Some(serde_json::to_string(&pv).unwrap_or_default());
            self.st.period_answered = self.st.period_asked.clone();
            self.st.period_complete = true;
        }
        self.take_asks(r.asks.clone());
        self.st.fact_errors = r.fact_errors.clone();
        self.set_errors(r.errors.clone());
        self.st.save_state = self.resting_save_state();
        self.reset_shown();
        self.st_gen += 1;
    }

    /// What the save line says while nothing is being saved.
    fn resting_save_state(&self) -> SaveState {
        let s = &self.st;
        if !s.editable || s.draft.is_none() {
            return SaveState::Idle;
        }
        if s.sample && !s.period_set {
            return SaveState::Manual("Sample · set its period to save".to_owned());
        }
        if s.amends.is_some() && !s.saved {
            return SaveState::Manual("Amendment · not saved yet".to_owned());
        }
        if s.dirty {
            return SaveState::Unsaved;
        }
        if s.saved {
            SaveState::Saved
        } else {
            SaveState::Idle
        }
    }

    /// Whether autosave may save this return now.
    fn can_autosave(&self) -> bool {
        let s = &self.st;
        if s.draft.is_none() || !s.editable || s.loading || s.removal.is_some() || self.reopening {
            return false;
        }
        if s.target.mode == "filed_original" {
            return false;
        }
        if s.amends.is_some() && !s.saved {
            return false;
        }
        if s.sample {
            return s.period_set
                && s.existing.is_none()
                && s.period_answered == s.period_asked
                && s.period_asked.is_some();
        }
        true
    }

    fn take_asks(&mut self, asks: Vec<ScreenFormAsk>) {
        let s = &mut self.st;
        s.asks = if !s.sample && s.editable {
            asks
        } else {
            Vec::new()
        };
        s.ask_at = HashMap::new();
        let first = self
            .schema
            .as_ref()
            .and_then(|sc| sc.parts.first())
            .filter(|p| !p.items.is_empty())
            .map(|_| "0:0".to_owned());
        for ask in s.asks.clone() {
            let id = ask
                .key
                .as_ref()
                .and_then(|k| self.key_index.get(k).cloned())
                .or_else(|| first.clone());
            let Some(id) = id else {
                continue;
            };
            s.ask_at.entry(id).or_default().push(ask.clone());
        }
        let facts: Vec<String> = s.ask_text.keys().cloned().collect();
        for fact in facts {
            if !s.asks.iter().any(|a| a.fact == fact) {
                s.ask_text.remove(&fact);
            }
        }
    }

    fn unquiet(&mut self, keys: Option<&[String]>) {
        let s = &mut self.st;
        if let Some(q) = &s.quiet {
            if keys.is_none() || keys.unwrap().iter().any(|k| q.keys.contains(k)) {
                s.quiet = None;
            }
        }
    }

    /// The page has drawn the opening: its one-time notes are recorded seen.
    pub fn notes_drawn(&mut self) {
        let pending: Vec<String> = self
            .st
            .notes
            .iter()
            .filter(|n| !n.recorded)
            .map(|n| n.key.clone())
            .collect();
        if pending.is_empty() || self.st.loading {
            return;
        }
        let recorded = match bir::call1("screen_form_notes_seen", json!(pending)) {
            Ok(v) => v
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            Err(_) => return,
        };
        for n in &mut self.st.notes {
            if recorded.contains(&n.key) {
                n.recorded = true;
            }
        }
    }

    // ---------------------------------------------------------------- editing

    /// Typing in box k (or "<month>#date"); a sel or chk commits at once.
    pub fn edit(&mut self, k: &str, text: &str) {
        if !self.st.editable || self.st.draft.is_none() {
            return;
        }
        if let Some((date, _)) = self.one_dates.get(k).cloned() {
            return self.edit_one_date(&date, text);
        }
        let kind = self.kind_of.get(k).cloned().unwrap_or_default();
        if kind == "sel" || kind == "chk" || kind == "radio" {
            let mut e = HashMap::new();
            e.insert(k.to_owned(), text.to_owned());
            return self.choose(e);
        }
        self.shown.insert(k.to_owned(), text.to_owned());
        if self.st.period_parts.contains_key(k) {
            let mut e = HashMap::new();
            e.insert(k.to_owned(), text.to_owned());
            return self.set_period_part(e);
        }
        let mut value = text.to_owned();
        if kind == "date" {
            let date = dates::parse_typed_date(text);
            self.st.dates_edited.insert(k.to_owned());
            if date.is_some() || text.trim().is_empty() {
                self.set_date_error(k, "");
            }
            value = date
                .map(dates::format_mdy)
                .unwrap_or_else(|| text.to_owned());
        }
        let mut e = HashMap::new();
        e.insert(k.to_owned(), value);
        self.stage(e);
    }

    fn edit_one_date(&mut self, date: &OneDate, text: &str) {
        self.shown
            .insert(date.one_key_placeholder(), text.to_owned());
        let typed = dates::parse_typed_date(text);
        if typed.is_some() || text.trim().is_empty() {
            self.set_date_error(&date.month.k, "");
        }
        let Some(typed) = typed else {
            if !text.trim().is_empty() {
                return;
            }
            // blank: clear the parts
            let mut edits = HashMap::new();
            edits.insert(date.month.k.clone(), String::new());
            edits.insert(date.year.k.clone(), String::new());
            if let Some(d) = &date.day {
                edits.insert(d.k.clone(), String::new());
            }
            return self.stage(edits);
        };
        let mdy = dates::format_mdy(typed);
        let parts: Vec<&str> = mdy.split('/').collect();
        let mut edits = HashMap::new();
        edits.insert(date.month.k.clone(), parts[0].to_owned());
        edits.insert(date.year.k.clone(), parts[2].to_owned());
        if let Some(d) = &date.day {
            edits.insert(d.k.clone(), parts[1].to_owned());
        }
        self.stage(edits);
    }

    /// A day picked on the calendar beside an item's split month, day and year boxes.
    pub fn pick_date(&mut self, date: &schema::DateParts, iso: &str) {
        if !self.st.editable || self.st.draft.is_none() {
            return;
        }
        let parts: Vec<&str> = iso[..10.min(iso.len())].split('-').collect();
        if parts.len() != 3 {
            return;
        }
        let (y, m, d) = (parts[0], parts[1], parts[2]);
        // (month, day, year) with their values
        let trip: [(Option<&SchemaField>, &str); 3] = [
            (Some(&date.month), m),
            (date.day.as_ref(), d),
            (Some(&date.year), y),
        ];
        let mut edits = HashMap::new();
        for (f, v) in trip {
            let Some(f) = f else {
                continue;
            };
            if self.is_shut(&f.k) {
                continue;
            }
            let chosen = self.sel_value_for(f, v, f.k == date.year.k);
            match chosen {
                Some(c) => {
                    edits.insert(f.k.clone(), c);
                }
                None => {
                    self.toast(
                        "That date is not on this form's list.".to_owned(),
                        Some("triangle-alert".to_owned()),
                        None,
                    );
                    return;
                }
            }
        }
        if edits.is_empty() {
            return;
        }
        self.set_date_error(&date.month.k, "");
        self.choose(edits);
    }

    /// A "sel" field's listed options mapped through optionOf: the value a typed part becomes.
    fn sel_value_for(&self, f: &SchemaField, v: &str, is_year: bool) -> Option<String> {
        if f.t != "sel" {
            return Some(v.to_owned());
        }
        let listed: Vec<(String, String)> =
            f.o.as_ref()
                .map(|os| os.iter().filter_map(schema::option_of).collect())
                .unwrap_or_default();
        if listed.is_empty() {
            return Some(v.to_owned());
        }
        let vnum = v.parse::<i64>().ok().map(|n| n.to_string());
        for (value, _label) in &listed {
            if value == v || Some(value.clone()) == vnum || (is_year && value == v) {
                return Some(value.clone());
            }
        }
        // a year list by the whole year it shows ("26" listed for 2026)
        if is_year {
            if let Ok(n) = v.parse::<i64>() {
                for (value, label) in &listed {
                    if label == &n.to_string() || value == &n.to_string() {
                        return Some(value.clone());
                    }
                }
            }
        }
        None
    }

    fn stage(&mut self, edits: HashMap<String, String>) {
        {
            let s = &mut self.st;
            for (k, v) in &edits {
                s.pending.insert(k.clone(), v.clone());
                if s.opened.contains(k) && s.profile_save.contains_key(k) {
                    s.profile_edited.insert(k.clone());
                }
                s.kept.remove(k);
                s.rejected.remove(k);
            }
            s.dirty = true;
        }
        let keys: Vec<String> = edits.keys().cloned().collect();
        self.unquiet(Some(&keys));
        self.cancel_autosave();
        self.mark_unsaved();
        self.st.edit_seq += 1;
        let seq = self.st.edit_seq;
        let token = self.token;
        self.timers
            .retain(|(_, t)| !matches!(t, Timer::Flush { .. } | Timer::LivePreview { .. }));
        self.later(VALIDATE_DELAY_MS, Timer::Flush { seq, token });
        self.later(LIVE_PREVIEW_DELAY_MS, Timer::LivePreview { seq, token });
    }

    /// A choice (sel, chk, a radio pick, a cleared row): committed at once.
    pub fn choose(&mut self, edits: HashMap<String, String>) {
        if !self.st.editable || self.st.draft.is_none() || edits.is_empty() {
            return;
        }
        let mut edits = edits;
        // Period parts go through setPeriodPart.
        let period: HashMap<String, String> = edits
            .iter()
            .filter(|(k, _)| self.st.period_parts.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !period.is_empty() {
            self.set_period_part(period.clone());
            edits.retain(|k, _| !period.contains_key(k));
            if edits.is_empty() {
                return;
            }
        }
        {
            let s = &mut self.st;
            for (k, v) in &edits {
                s.pending.insert(k.clone(), v.clone());
            }
            s.dirty = true;
            for k in edits.keys() {
                s.kept.remove(k);
                s.rejected.remove(k);
            }
            s.chosen_from = edits
                .keys()
                .map(|k| (k.clone(), s.values.get(k).cloned().unwrap_or_default()))
                .collect();
            for (k, v) in &edits {
                s.values.insert(k.clone(), v.clone());
            }
        }
        for k in edits.keys() {
            let text = self.shown_text(k);
            self.shown.insert(k.clone(), text);
        }
        self.cancel_autosave();
        self.mark_unsaved();
        self.flush();
    }

    /// The save line says "Unsaved changes" the moment something is staged.
    fn mark_unsaved(&mut self) {
        if matches!(self.st.save_state, SaveState::Manual(_)) {
            return;
        }
        self.st.save_state = SaveState::Unsaved;
    }

    /// A pick in a radio group, with the old group rules.
    pub fn pick_radio(&mut self, k: &str) {
        let id = self.key_index.get(k).cloned();
        let Some(id) = id else {
            return;
        };
        let Some(it) = self.items.get(&id).cloned() else {
            return;
        };
        let Some(f) = self.field_of.get(k).cloned() else {
            return;
        };
        if self.is_shut(k) {
            return;
        }
        self.st.touched.insert(id.clone());
        let on = self.st.values.get(k).map(String::as_str) == Some("true");
        let period = self.st.period_parts.contains_key(k);
        if on && !period && f.rg.unwrap_or(0.0) >= 3.0 {
            let mut e = HashMap::new();
            e.insert(k.to_owned(), "false".to_owned());
            return self.choose(e);
        }
        if on && !period && f.rg.is_none() {
            return;
        }
        let mut edits = HashMap::new();
        edits.insert(k.to_owned(), "true".to_owned());
        if let Some(grp) = &f.grp {
            for r in &it.f {
                if r.t == "radio" && r.k != *k && r.grp.as_deref() == Some(grp.as_str()) {
                    edits.insert(r.k.clone(), "false".to_owned());
                }
            }
        }
        self.choose(edits);
    }

    /// A box gained focus.
    pub fn focus(&mut self, k: &str) {
        self.st.focus_key = Some(k.to_owned());
        let base = self
            .one_dates
            .get(k)
            .map(|(d, _)| d.month.k.clone())
            .unwrap_or_else(|| k.to_owned());
        let here = self.key_index.get(&base).cloned();
        let editing: Vec<String> = self
            .st
            .editing
            .iter()
            .filter(|key| self.key_index.get(*key) != here.as_ref())
            .cloned()
            .collect();
        if self.end_profile_edit(&editing) {
            // republish implicit
        }
    }

    /// A box lost focus: its row is touched, an amount or a date takes its shown form, and it flushes.
    pub fn blur(&mut self, k: &str) {
        let one = self.one_dates.get(k).map(|(d, r)| (d.clone(), r.clone()));
        let base_key = one
            .as_ref()
            .map(|(d, _)| d.month.k.clone())
            .unwrap_or_else(|| k.to_owned());
        let id = self.key_index.get(&base_key).cloned();
        if self.st.focus_key.as_deref() == Some(k) {
            self.st.focus_key = None;
        }
        let kind = one
            .as_ref()
            .map(|_| "date".to_owned())
            .or_else(|| self.kind_of.get(k).cloned())
            .unwrap_or_default();
        let raw = self.shown.get(k).cloned().unwrap_or_default();
        if kind == "money" && !self.is_shut(k) {
            let whole = self.whole_keys.contains(k);
            let formatted = mask::format_money_full(&raw, whole, true);
            self.shown.insert(k.to_owned(), formatted);
        } else if kind == "date" && !self.is_shut(k) {
            let date = dates::parse_typed_date(&raw);
            if let Some(date) = date {
                self.shown
                    .insert(k.to_owned(), dates::format_long_date(date));
                self.set_date_error(&base_key, "");
            } else {
                let msg = if !raw.trim().is_empty()
                    && (one.is_some() || self.st.dates_edited.contains(k))
                {
                    DATE_ERROR
                } else {
                    ""
                };
                self.set_date_error(&base_key, msg);
            }
        }
        if let Some(id) = &id {
            self.st.touched.insert(id.clone());
        }
        let edit_keys = self.item_edit_keys(&base_key);
        if edit_keys.iter().any(|key| self.st.editing.contains(key)) {
            let token = self.token;
            let row = base_key.clone();
            self.later(50, Timer::EndProfileEdit { token, row });
        }
        if !self.st.pending.is_empty() {
            self.flush();
        }
    }

    /// The part whose box has the keyboard now.
    pub fn focused_part(&self) -> Option<usize> {
        let k = self.st.focus_key.as_ref()?;
        let base = self
            .one_dates
            .get(k)
            .map(|(d, _)| d.month.k.as_str())
            .unwrap_or(k.as_str());
        let id = self.key_index.get(base)?;
        id.split(':').next()?.parse().ok()
    }

    /// Mark a row touched without a blur.
    pub fn touch(&mut self, id: &str) {
        self.st.touched.insert(id.to_owned());
    }

    fn set_date_error(&mut self, k: &str, msg: &str) {
        let Some(id) = self.key_index.get(k).cloned() else {
            return;
        };
        if msg.is_empty() {
            self.st.date_errors.remove(&id);
        } else {
            self.st.date_errors.insert(id, msg.to_owned());
        }
    }

    /// Push the draft and staged edits to an open print preview (best effort).
    fn live_preview(&mut self) {
        if !self.st.preview_open || self.st.draft.is_none() {
            return;
        }
        let unlocked = self.unlocked_for(&self.st.pending);
        let _ = bir::call(
            "screen_form_preview_sync",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
                edits_json(&self.st.pending),
                json!(unlocked),
            ],
        );
    }

    /// Apply staged edits to the draft (sync), then validate; a successful fill schedules autosave.
    pub fn flush(&mut self) {
        self.timers
            .retain(|(_, t)| !matches!(t, Timer::Flush { .. }));
        if self.st.draft.is_none() || self.reopening {
            return;
        }
        let edits = std::mem::take(&mut self.st.pending);
        let mut filled = false;
        if !edits.is_empty() {
            self.retire_removal_undo();
            match self.fill(edits.clone()) {
                Ok(r) => {
                    if r.removes.is_some() {
                        self.hold_removal(r, edits);
                        return;
                    }
                    self.apply_fill(r, &edits);
                    filled = true;
                }
                Err(e) => {
                    let mut pending = edits.clone();
                    pending.extend(self.st.pending.clone());
                    self.st.pending = pending;
                    self.st.save_state = SaveState::Failed(error_text(&e));
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                }
            }
        }
        self.validate();
        if filled {
            self.schedule_autosave();
        }
    }

    /// The host's fill for staged edits, with the emptied-number-box rule.
    fn fill(&mut self, edits: HashMap<String, String>) -> Result<ScreenFormFilled, String> {
        let code = self.st.target.form_code.clone();
        let unlocked = self.unlocked_for(&edits);
        let mut r: ScreenFormFilled = serde_json::from_value(bir::call(
            "screen_form_fill",
            vec![
                json!(code),
                self.st.draft.clone().unwrap_or(Value::Null),
                edits_json(&edits),
                json!(unlocked),
            ],
        )?)
        .map_err(|e| e.to_string())?;
        r.cleared = Vec::new();
        if r.removes.is_some() {
            return Ok(r);
        }
        let refused: Vec<String> = r
            .rejected
            .keys()
            .filter(|k| {
                edits.contains_key(*k)
                    && edits[*k].trim().is_empty()
                    && NUMBER_KINDS
                        .contains(&self.kind_of.get(*k).map(String::as_str).unwrap_or(""))
            })
            .cloned()
            .collect();
        if refused.is_empty() {
            return Ok(r);
        }
        let zero: HashMap<String, String> = refused
            .iter()
            .map(|k| (k.clone(), CLEARED.to_owned()))
            .collect();
        let unlocked = self.unlocked_for(&zero);
        let mut z: ScreenFormFilled = serde_json::from_value(bir::call(
            "screen_form_fill",
            vec![
                json!(code),
                r.draft.clone(),
                edits_json(&zero),
                json!(unlocked),
            ],
        )?)
        .map_err(|e| e.to_string())?;
        let cleared: Vec<String> = refused
            .iter()
            .filter(|k| !z.rejected.contains_key(*k))
            .cloned()
            .collect();
        if cleared.is_empty() {
            return Ok(r);
        }
        // The first answer's refusals stay for the boxes that could not be cleared either.
        let mut rejected = r.rejected.clone();
        for k in &cleared {
            rejected.remove(k);
        }
        z.rejected = rejected;
        z.cleared = cleared;
        // edits is &mut in TS; our caller passes owned — record the zero staging on st.pending side
        // through a side channel: apply_fill receives `edits` and we overlay cleared keys.
        Ok(z)
    }

    /// Take what the host applied for edits.
    fn apply_fill(&mut self, r: ScreenFormFilled, edits: &HashMap<String, String>) {
        // cleared keys staged as zero count as accepted edits of "0"
        let mut edits = edits.clone();
        for k in &r.cleared {
            edits.insert(k.clone(), CLEARED.to_owned());
        }
        let rejected = r.rejected.clone();
        let sample = self.st.sample;
        let blank = r.blank.clone();
        let draft = r.draft.clone();
        let values_raw = string_map(&r.values);
        let options_raw = options_map(&r.options);
        let penalties = r.penalties.clone();
        let pools_raw = string_map(&r.pools);
        let relocks = string_map(&r.relocks);
        let relock_notes = r.relock_notes.clone();
        let rejected_map = r.rejected.clone();
        {
            let s = &mut self.st;
            self_accepted(s, &edits, Some(&rejected));
            if sample {
                if let Some(blank) = blank {
                    s.blank = blank.iter().cloned().collect();
                }
            }
            let before_vals = s.values.clone();
            s.draft = Some(draft);
            s.values = values_raw;
            s.options = options_raw;
            if !penalties.is_empty() {
                s.penalties = penalties;
            }
            if !pools_raw.is_empty() {
                s.pool_texts = pools_raw;
            }
            for (k, lock) in relocks {
                if lock.is_empty() {
                    s.locks.remove(&k);
                    s.lock_notes.remove(&k);
                } else {
                    s.locks.insert(k.clone(), lock);
                    if let Some(note) = relock_notes.get(&k).and_then(|v| v.as_str()) {
                        s.lock_notes.insert(k, note.to_owned());
                    }
                }
            }
            let _ = before_vals;
        }
        // shown_values needs &self.st; apply after values are in.
        let vals = self.st.values.clone();
        self.st.values = self.shown_values(vals);
        let before = self.st.values.clone();
        // sync_shown compares pre-fill values; approximate with post-fill (shut boxes resync anyway)
        self.sync_shown(&before, &edits);
        self.live_preview();
        // A refused box shows what the draft holds again; its message shows until edited again.
        for (k, msgv) in rejected_map {
            let msg = match &msgv {
                Value::String(s) => s.clone(),
                _ => msgv.to_string(),
            };
            let id = self.key_index.get(&k).cloned();
            let entry_id = id.clone().unwrap_or_else(|| "form".to_owned());
            let entry = self.st.messages.entry(entry_id).or_default();
            if entry.is_empty() {
                *entry = msg.clone();
            } else {
                *entry = format!("{msg} · {entry}");
            }
            let Some(id) = id else {
                continue;
            };
            self.st.rejected.insert(k.clone(), msg);
            self.st.touched.insert(id);
            if !self.st.pending.contains_key(&k) {
                let text = self.shown_text(&k);
                self.shown.insert(k, text);
            }
        }
        // An emptied number box the host holds as zero shows the zero.
        for k in &r.cleared {
            if !self.st.pending.contains_key(k) {
                let text = self.shown_text(k);
                self.shown.insert(k.clone(), text);
            }
        }
    }

    /// An answer that removes typed rows asks first.
    fn hold_removal(&mut self, r: ScreenFormFilled, edits: HashMap<String, String>) {
        let chosen = self.st.chosen_from.clone();
        for k in edits.keys() {
            if let Some(v) = chosen.get(k) {
                self.st.values.insert(k.clone(), v.clone());
            }
        }
        let typed: HashMap<String, String> = edits
            .iter()
            .filter(|(k, _)| !chosen.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let draft = self.st.draft.clone().unwrap_or(Value::Null);
        let message = r
            .removes
            .as_ref()
            .map(|m| m.message.clone())
            .unwrap_or_default();
        self.st.removal = Some(RemovalHeld {
            r,
            edits,
            typed,
            draft,
        });
        self.st.overlay = Some(Overlay::Removal { message });
        for k in chosen.keys() {
            let text = self.shown_text(k);
            self.shown.insert(k.clone(), text);
        }
        self.mark_unsaved();
    }

    /// Continue: the answer and its removal apply; Undo (on the toast) brings both back.
    pub fn confirm_removal(&mut self) {
        let Some(held) = self.st.removal.take() else {
            return;
        };
        self.st.overlay = None;
        let done = held
            .r
            .removes
            .as_ref()
            .map(|m| m.done.clone())
            .unwrap_or_default();
        self.apply_fill(held.r.clone(), &held.edits);
        self.st.dirty = true;
        self.validate();
        // The Undo: the draft from before the answer comes back, on the same page only.
        self.undo_seq += 1;
        let undo_id = self.undo_seq;
        self.removal_undo = Some((self.st_gen, undo_id));
        let held_draft = held.draft.clone();
        // store the undo payload inside removal_undo's companion map
        self.removal_payloads.insert(undo_id, held_draft);
        let sid = self.toast(
            format!("{done}."),
            Some("trash".to_owned()),
            Some(("Undo".to_owned(), UndoCmd::Removal)),
        );
        self.undo_toast_sid = Some(sid);
        let token = self.token;
        self.later(UNDO_WINDOW_MS, Timer::UndoWindow { undo_id, token });
        self.schedule_autosave();
    }

    /// Cancel: the old answer and the rows stay, and so does what was typed with it.
    pub fn cancel_removal(&mut self) {
        let held = self.st.removal.take();
        self.st.overlay = None;
        if let Some(held) = held {
            if !held.typed.is_empty() {
                match self.fill(held.typed.clone()) {
                    Ok(r) => {
                        if r.removes.is_some() {
                            for k in held.typed.keys() {
                                let text = self.shown_text(k);
                                self.shown.insert(k.clone(), text);
                            }
                        } else {
                            self.apply_fill(r, &held.typed);
                            self.st.dirty = true;
                            self.validate();
                            self.schedule_autosave();
                        }
                    }
                    Err(e) => {
                        let mut pending = held.typed.clone();
                        pending.extend(self.st.pending.clone());
                        self.st.pending = pending;
                        self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                    }
                }
            }
        }
    }

    /// Whether the removal's Undo is still offered.
    pub fn can_undo_removal(&self) -> bool {
        self.removal_undo.is_some()
    }

    /// Runs a toast's undo command (the frame calls this when the toast's action is pressed).
    pub fn run_undo(&mut self, cmd: UndoCmd) {
        match cmd {
            UndoCmd::Removal => self.undo_removal(),
            UndoCmd::KeepPeriod => self.undo_keep_period(),
            UndoCmd::RestoreForm => self.undo_restore_form(),
        }
    }

    /// The removal's Undo.
    fn undo_removal(&mut self) {
        let Some((r#gen, undo_id)) = self.removal_undo.take() else {
            return;
        };
        self.undo_toast_sid = None;
        let Some(draft) = self.removal_payloads.remove(&undo_id) else {
            return;
        };
        if r#gen != self.st_gen || !self.st.editable {
            return;
        }
        let id = draft.get("id").and_then(|v| v.as_i64());
        let cur = self
            .st
            .draft
            .as_ref()
            .and_then(|d| d.get("id"))
            .and_then(|v| v.as_i64());
        if id != cur {
            self.toast(
                "Undo is no longer available: the return was saved since.".to_owned(),
                Some("triangle-alert".to_owned()),
                None,
            );
            return;
        }
        match bir::call(
            "screen_form_fill",
            vec![json!(self.st.target.form_code), draft, json!({}), json!([])],
        )
        .ok()
        .and_then(|v| serde_json::from_value::<ScreenFormFilled>(v).ok())
        {
            Some(r) => {
                let before = self.st.values.clone();
                self.apply_fill(r, &HashMap::new());
                for k in self.all_keys.clone() {
                    if before.get(&k) != self.st.values.get(&k) {
                        let text = self.shown_text(&k);
                        self.shown.insert(k, text);
                    }
                }
                self.st.dirty = true;
                self.validate();
                self.schedule_autosave();
            }
            None => {
                self.toast(
                    "Undo failed.".to_owned(),
                    Some("triangle-alert".to_owned()),
                    None,
                );
            }
        }
    }

    /// The removal's Undo goes (an edit, a save, leaving the page), and its toast with it.
    fn retire_removal_undo(&mut self) {
        if self.removal_undo.take().is_some() {
            if let Some(sid) = self.undo_toast_sid.take() {
                self.events.push(SessionEvent::DismissToast { sid });
            }
        }
    }

    /// Validate the draft off the engine thread; take values, options, off, penalties and errors.
    fn validate(&mut self) {
        let Some(draft) = self.st.draft.clone() else {
            return;
        };
        self.st.checking = true;
        self.st.validate_seq += 1;
        let seq = self.st.validate_seq;
        let token = self.token;
        let rx = bir::call_async_spawn(
            "screen_form_validate",
            vec![json!(self.st.target.form_code), draft],
        );
        self.pending_validate = Some((seq, token, rx));
    }

    /// Bind each validation message to an item (bindError).
    fn set_errors(&mut self, errors: Vec<ScreenFormError>) {
        let s = &mut self.st;
        let mut all: Vec<ScreenFormError> = errors;
        all.extend(s.fact_errors.clone());
        if let Some(q) = &s.quiet {
            all.retain(|e| e.field != q.field);
        }
        s.errors = all;
        s.messages = HashMap::new();
        s.message_keys = HashMap::new();
        s.message_counts = HashMap::new();
        s.checkable = HashMap::new();
        s.election_at = None;
        let items: Vec<(String, SchemaItem)> = self
            .row_ids
            .iter()
            .filter_map(|id| self.items.get(id).map(|it| (id.clone(), it.clone())))
            .collect();
        for e in s.errors.clone() {
            let id = bind_error(&e, &items, |k| self.key_index.get(k).cloned());
            let Some(id) = id else {
                continue;
            };
            let entry = s.messages.entry(id.clone()).or_default();
            if !entry.is_empty() {
                *entry = format!("{entry} · {}", e.message);
            } else {
                *entry = e.message.clone();
            }
            s.message_keys
                .entry(id.clone())
                .or_default()
                .extend(e.keys.iter().cloned());
            *s.message_counts.entry(id.clone()).or_default() += 1;
            if let Some(dismiss) = &e.dismiss {
                s.checkable.entry(id.clone()).or_default().push((
                    dismiss.clone(),
                    e.dismiss_label
                        .clone()
                        .unwrap_or_else(|| "Checked".to_owned()),
                ));
            }
            if e.field == "annual_income_tax_election" && e.message.contains(ELECTION_UNRECORDED) {
                s.election_at = Some(id);
            }
        }
        let rejected = s.rejected.clone();
        for (k, msg) in rejected {
            let Some(id) = self.key_index.get(&k).cloned() else {
                continue;
            };
            let entry = s.messages.entry(id.clone()).or_default();
            if entry.is_empty() {
                *entry = msg.clone();
            } else {
                *entry = format!("{msg} · {entry}");
            }
            s.message_keys.entry(id.clone()).or_default().push(k);
            *s.message_counts.entry(id).or_default() += 1;
        }
    }

    // ---------------------------------------------------------------- shown values

    /// A sample's blank boxes stay blank until typed; a period part shows what was typed into it.
    fn shown_values(&self, values: HashMap<String, String>) -> HashMap<String, String> {
        let s = &self.st;
        if !s.sample || s.blank.is_empty() {
            return values;
        }
        let mut out = values;
        for k in &s.blank {
            if s.period_parts.contains_key(k) {
                out.insert(
                    k.clone(),
                    s.period_values.get(k).cloned().unwrap_or_default(),
                );
            } else if out.contains_key(k) && !s.typed.contains(k) {
                out.insert(k.clone(), String::new());
            }
        }
        out
    }

    /// The text box k shows for what the draft holds.
    pub fn shown_text(&self, k: &str) -> String {
        // A key the user is typing keeps its text (shown map wins while it differs).
        let s = &self.st;
        if let Some((date, _)) = self.one_dates.get(k) {
            return self.one_date_shown(date);
        }
        if let Some(pool) = self.pools.get(k) {
            return pool
                .keys
                .first()
                .and_then(|fk| s.pool_texts.get(fk).cloned())
                .unwrap_or_default();
        }
        let kind = self.kind_of.get(k).map(String::as_str).unwrap_or("");
        let raw = s.values.get(k).cloned().unwrap_or_default();
        match kind {
            "money" => {
                let zero = self.is_shut(k)
                    && (self.is_computed_box(k) || self.penalty_group_of(k).is_some());
                let v = if raw.is_empty() && zero && self.reason_for(k).is_empty() {
                    "0".to_owned()
                } else {
                    raw
                };
                if v.is_empty() {
                    String::new()
                } else {
                    mask::format_money_full(&v, self.whole_keys.contains(k), false)
                }
            }
            "date" => dates::shown_date(&raw),
            "sel" => self.sel_shown(k).0,
            _ => raw,
        }
    }

    fn one_date_shown(&self, date: &OneDate) -> String {
        let text = one_date_text_of(&self.st.values, date);
        dates::shown_date(&text)
    }

    /// A select's shown value and choices (old selectBox).
    pub fn sel_shown(&self, k: &str) -> (String, Vec<ScreenFormOption>) {
        let s = &self.st;
        let mut listed: Vec<ScreenFormOption> = Vec::new();
        if let Some(opts) = s.options.get(k) {
            for o in opts {
                if let Ok(v) = serde_json::from_value::<ScreenFormOption>(o.clone()) {
                    listed.push(v);
                }
            }
        }
        if listed.is_empty() {
            if let Some(f) = self.field_of.get(k) {
                if let Some(os) = &f.o {
                    for o in os {
                        if let Some((value, label)) = schema::option_of(o) {
                            listed.push(ScreenFormOption {
                                value,
                                label,
                                selected: None,
                            });
                        }
                    }
                }
            }
        }
        let value = s.values.get(k).cloned().unwrap_or_default();
        // controls/model.ts `selectModel`: the marked choice wins, zeros and a lone dash read as
        // empty, an unknown value stays choosable (appended below, without the "Choose…" head the
        // list itself does not carry here).
        let (_options, shown) = model::select_model(&listed, &value);
        let mut options = listed;
        if !shown.is_empty() && !options.iter().any(|o| o.value == shown) {
            options.push(ScreenFormOption {
                value: shown.clone(),
                label: shown.clone(),
                selected: None,
            });
        }
        (shown, options)
    }

    /// Every box shows what the draft holds (an opening, a rebuild, a restore).
    fn reset_shown(&mut self) {
        let pending = self.st.pending.clone();
        let keys = self.all_keys.clone();
        for k in &keys {
            if !pending.contains_key(k) {
                let text = self.shown_text(k);
                self.shown.insert(k.clone(), text);
            }
        }
        let one_keys: Vec<(String, OneDate)> = self
            .one_dates
            .iter()
            .map(|(k, (d, _))| (k.clone(), d.clone()))
            .collect();
        for (k, date) in one_keys {
            let parts = [Some(&date.month), date.day.as_ref(), Some(&date.year)]
                .into_iter()
                .flatten()
                .map(|f| f.k.as_str())
                .collect::<Vec<_>>();
            if parts.iter().any(|p| pending.contains_key(*p)) {
                continue;
            }
            let text = self.shown_text(&k);
            self.shown.insert(k, text);
        }
    }

    /// Shut boxes follow the draft; a box the user can type keeps its text unless the host changed it.
    fn sync_shown(&mut self, before: &HashMap<String, String>, edits: &HashMap<String, String>) {
        let keys = self.all_keys.clone();
        for k in &keys {
            if edits.contains_key(k) {
                if !self.st.pending.contains_key(k)
                    && !self.pools.contains_key(k)
                    && self.host_changed(k, &edits[k])
                {
                    let text = self.shown_text(k);
                    self.shown.insert(k.clone(), text);
                }
                continue;
            }
            if self.is_shut(k)
                || self.pools.contains_key(k)
                || before.get(k).cloned().unwrap_or_default()
                    != self.st.values.get(k).cloned().unwrap_or_default()
            {
                let text = self.shown_text(k);
                self.shown.insert(k.clone(), text);
            }
        }
        let one_keys: Vec<(String, OneDate)> = self
            .one_dates
            .iter()
            .map(|(k, (d, _))| (k.clone(), d.clone()))
            .collect();
        for (k, date) in one_keys {
            let parts: Vec<&str> = [Some(&date.month), date.day.as_ref(), Some(&date.year)]
                .into_iter()
                .flatten()
                .map(|f| f.k.as_str())
                .collect();
            if parts.iter().any(|p| edits.contains_key(*p)) {
                continue;
            }
            let b = one_date_text_of(before, &date);
            let a = one_date_text_of(&self.st.values, &date);
            if a != b {
                let text = self.shown_text(&k);
                self.shown.insert(k, text);
            }
        }
    }

    /// The draft holds something else for box k than the text typed into it.
    fn host_changed(&self, k: &str, typed: &str) -> bool {
        let host = self.st.values.get(k).cloned().unwrap_or_default();
        let kind = self.kind_of.get(k).map(String::as_str).unwrap_or("");
        match kind {
            "money" | "pct" | "int" => !same_number(typed, &host),
            "sel" | "chk" | "radio" => true,
            "date" => {
                let a = dates::parse_typed_date(typed);
                let b = dates::parse_typed_date(&host);
                match (a, b) {
                    (Some(a), Some(b)) => dates::format_mdy(a) != dates::format_mdy(b),
                    _ => typed.trim().to_lowercase() != host.trim().to_lowercase(),
                }
            }
            _ => typed.trim().to_lowercase() != host.trim().to_lowercase(),
        }
    }

    /// Computed and locked boxes follow the draft (after a validation).
    fn sync_computed(&mut self) {
        let keys = self.all_keys.clone();
        for k in keys {
            if self.is_shut(&k) {
                let text = self.shown_text(&k);
                self.shown.insert(k, text);
            }
        }
    }

    // ---------------------------------------------------------------- locks

    fn is_computed_box(&self, k: &str) -> bool {
        self.computed_keys.contains(k)
            || self
                .st
                .locks
                .get(k)
                .map(|l| l.starts_with("computed"))
                .unwrap_or(false)
    }

    fn penalty_group_of(&self, k: &str) -> Option<ScreenFormPenalties> {
        self.st
            .penalties
            .iter()
            .find(|g| g.keys.iter().any(|x| x == k))
            .cloned()
    }

    /// Locked on the page: off, a penalty not opened, or a host lock unless a profile box opened with Edit.
    fn is_locked(&self, k: &str) -> bool {
        let s = &self.st;
        if s.off.contains_key(k) {
            return true;
        }
        if let Some(penalty) = self.penalty_group_of(k) {
            return !penalty.applies || !(penalty.edited || s.penalty_open.contains(k));
        }
        s.locks.contains_key(k) && !(s.profile_edit.contains(k) && s.editing.contains(k))
    }

    /// The box takes no typing at all.
    pub fn is_shut(&self, k: &str) -> bool {
        if let Some((date, _)) = self.one_dates.get(k) {
            let parts = [Some(&date.month), date.day.as_ref(), Some(&date.year)]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            return !self.st.editable
                || parts
                    .iter()
                    .all(|f| f.c == Some(true) || self.is_locked(&f.k));
        }
        !self.st.editable || self.computed_keys.contains(k) || self.is_locked(k)
    }

    /// What a locked blank box shows in its place.
    fn reason_for(&self, k: &str) -> String {
        let s = &self.st;
        if s.blank.contains(k) {
            if let Some(n) = s.needs.get(k) {
                return n.clone();
            }
        }
        s.whys
            .get(k)
            .or_else(|| s.waits.get(k))
            .cloned()
            .unwrap_or_default()
    }

    /// Boxes of an edit batch the page has open (screen_form_fill's unlocked).
    fn unlocked_for(&self, edits: &HashMap<String, String>) -> Vec<String> {
        edits
            .keys()
            .filter(|k| {
                !self.st.locks.contains_key(*k)
                    || self.st.opened.contains(*k)
                    || self.st.penalty_open.contains(*k)
            })
            .cloned()
            .collect()
    }

    fn accepted(&mut self, edits: &HashMap<String, String>, rejected: Option<&Map<String, Value>>) {
        let s = &mut self.st;
        for (k, v) in edits {
            if rejected.is_some_and(|r| r.contains_key(k)) {
                continue;
            }
            s.typed.insert(k.clone());
            if s.sample {
                s.typed_edits.insert(k.clone(), v.clone());
            }
        }
    }

    /// Why an item's shut boxes show blank, in the filer's words.
    fn lock_notes_for(&self, it: &SchemaItem) -> Vec<String> {
        let s = &self.st;
        if !s.editable || s.draft.is_none() {
            return Vec::new();
        }
        let mut notes: Vec<String> = Vec::new();
        let mut add = |text: &str, notes: &mut Vec<String>| {
            if !text.is_empty() && !notes.iter().any(|n| n == text) {
                notes.push(text.to_owned());
            }
        };
        let note = |k: &str| -> String {
            s.lock_notes
                .get(k)
                .cloned()
                .or_else(|| {
                    if self.computed_keys.contains(k) {
                        Some("Computed by the form.".to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
        };
        let shut = |k: &str, c: Option<bool>| c == Some(true) || self.is_locked(k);
        let waits = |k: &str| -> bool {
            if s.off.contains_key(k) && self.is_computed_box(k) {
                // side effect in TS add(); handled by caller collecting
            }
            !self.reason_for(k).is_empty() || s.off.contains_key(k)
        };
        let always = |k: &str| s.notes_always.contains(k);
        let blank_text = |k: &str, t: &str| -> bool {
            let v = s.values.get(k).cloned().unwrap_or_default();
            let v = v.trim();
            if t == "money" && self.is_computed_box(k) {
                return false;
            }
            if t == "sel" {
                return v.is_empty() || v.chars().all(|c| c == '0' || c == '-');
            }
            v.is_empty()
        };
        if it.rows.is_some() {
            for row in &it.keys {
                for (c, k) in row.iter().enumerate() {
                    let Some(col) = it.cols.get(c) else {
                        continue;
                    };
                    if col.t == "hidden" || !shut(k, col.c) || waits(k) {
                        continue;
                    }
                    let tick = col.t == "radio" || col.t == "chk";
                    let blank = if tick {
                        !matches!(
                            s.values.get(k).map(String::as_str),
                            Some("true") | Some("1") | Some("Y")
                        )
                    } else {
                        blank_text(k, &col.t)
                    };
                    if always(k) || blank {
                        add(&note(k), &mut notes);
                    }
                }
            }
            return notes;
        }
        let fs = &it.f;
        let radios: Vec<&SchemaField> = fs.iter().filter(|f| f.t == "radio").collect();
        if !radios.is_empty()
            && radios.iter().all(|f| shut(&f.k, f.c))
            && !radios.iter().any(|f| {
                matches!(
                    s.values.get(&f.k).map(String::as_str),
                    Some("true") | Some("1") | Some("Y")
                )
            })
            && !radios.iter().any(|f| waits(&f.k))
        {
            add(&note(&radios[0].k), &mut notes);
        }
        for f in fs {
            if f.t == "radio" || !shut(&f.k, f.c) || waits(&f.k) {
                continue;
            }
            let tick = f.t == "chk";
            let blank = if tick {
                !matches!(
                    s.values.get(&f.k).map(String::as_str),
                    Some("true") | Some("1") | Some("Y")
                )
            } else {
                blank_text(&f.k, &f.t)
            };
            if always(&f.k) || blank {
                add(&note(&f.k), &mut notes);
            }
        }
        notes
    }

    // ---------------------------------------------------------------- profile boxes

    fn item_edit_keys(&self, k: &str) -> Vec<String> {
        let Some(id) = self.key_index.get(k) else {
            return vec![k.to_owned()];
        };
        let Some(it) = self.items.get(id) else {
            return vec![k.to_owned()];
        };
        it.f.iter()
            .map(|f| f.k.clone())
            .filter(|key| self.st.profile_edit.contains(key))
            .collect()
    }

    /// Edit on a profile item: its profile boxes take typing.
    pub fn start_profile_edit(&mut self, id: &str) {
        let Some(it) = self.items.get(id).cloned() else {
            return;
        };
        if !self.st.editable {
            return;
        }
        let keys: Vec<String> =
            it.f.iter()
                .map(|f| f.k.clone())
                .filter(|k| self.st.profile_edit.contains(k))
                .collect();
        let others: Vec<String> = self
            .st
            .editing
            .iter()
            .filter(|k| self.key_index.get(*k).map(String::as_str) != Some(id))
            .cloned()
            .collect();
        self.end_profile_edit(&others);
        if it.l == "Additional details" {
            self.st.more_open.insert(id.to_owned());
        }
        for k in &keys {
            self.st.editing.insert(k.clone());
            self.st.opened.insert(k.clone());
        }
        // Focus the first non-radio (focusers are a no-op in CUI for now).
    }

    /// Back to the profile look; the value stays on this return.
    fn end_profile_edit(&mut self, keys: &[String]) -> bool {
        let mut changed = false;
        for k in keys {
            changed = self.st.editing.remove(k) || changed;
        }
        changed
    }

    /// A part's items the profile filled whole.
    fn tucked_items(&self, pi: usize) -> Vec<usize> {
        let s = &self.st;
        let Some(part) = self.schema.as_ref().and_then(|sc| sc.parts.get(pi)) else {
            return Vec::new();
        };
        if s.profile.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (ii, it) in part.items.iter().enumerate() {
            if it.rows.is_some() || it.l == "Additional details" {
                continue;
            }
            let typed: Vec<&SchemaField> = it.f.iter().filter(|f| f.c != Some(true)).collect();
            if !typed.is_empty() && typed.iter().all(|f| s.profile.contains(&f.k)) {
                out.push(ii);
            }
        }
        out
    }

    /// Fold or unfold a part's "From tax profile" group.
    pub fn toggle_profile_fold(&mut self, pi: usize) {
        if !self.st.profile_open.remove(&pi) {
            self.st.profile_open.insert(pi);
        }
    }

    /// "Save to profile too" on a profile item.
    pub fn set_to_profile(&mut self, id: &str, on: bool) {
        let keys = self.saveable_keys(id);
        for k in &keys {
            if on {
                self.st.to_profile.insert(k.clone());
            } else {
                self.st.to_profile.remove(k);
            }
        }
        if on {
            self.st.dirty = true;
            self.schedule_autosave();
        }
    }

    fn saveable_keys(&self, id: &str) -> Vec<String> {
        let s = &self.st;
        let Some(it) = self.items.get(id) else {
            return Vec::new();
        };
        if !s.editable || s.sample {
            return Vec::new();
        }
        it.f.iter()
            .map(|f| f.k.clone())
            .filter(|k| s.profile_edit.contains(k) && s.profile_save.contains_key(k))
            .collect()
    }

    /// After a save: the switched-on profile boxes go to the tax profile.
    fn save_to_profile(&mut self) -> Option<String> {
        let keys: Vec<String> = self
            .st
            .to_profile
            .iter()
            .filter(|k| self.st.profile_edited.contains(*k))
            .cloned()
            .collect();
        if keys.is_empty() {
            return None;
        }
        let r: Value = match bir::call(
            "screen_form_save_to_profile",
            vec![
                json!(self.st.target.form_code),
                json!(self.st.target.year),
                json!(self.st.target.period),
                self.st.draft.clone().unwrap_or(Value::Null),
                json!(keys),
            ],
        ) {
            Ok(v) => v,
            Err(e) => {
                self.toast(
                    schema::plain_refusal(&error_text(&e)),
                    Some("triangle-alert".to_owned()),
                    None,
                );
                return None;
            }
        };
        self.events.push(SessionEvent::AfterWrite(
            "screen_form_save_to_profile".to_owned(),
        ));
        self.st.draft = r.get("draft").cloned();
        let before = self.st.values.clone();
        let mut vals = self.st.values.clone();
        for (k, v) in string_map_v(&r.get("values").cloned().unwrap_or(Value::Null)) {
            vals.insert(k, v);
        }
        self.st.values = self.shown_values(vals);
        if let Some(pools) = r.get("pools") {
            self.st.pool_texts = string_map_v(pools);
        }
        self.sync_shown(&before, &HashMap::new());
        for k in &keys {
            self.st.to_profile.remove(k);
            self.st.profile_edited.remove(k);
        }
        self.st.election_ready = None;
        let saved: Vec<String> = r
            .get("saved")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let notes: Vec<String> = r
            .get("notes")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut parts = vec![format!(
            "Saved to the tax profile too: {}.",
            saved.join(", ")
        )];
        parts.extend(notes);
        Some(parts.join(" "))
    }

    /// "Use profile values" on a draft that differs from the profile.
    pub fn use_profile_values(&mut self) {
        if self.st.draft.is_none() || self.st.profile_differs.is_empty() {
            return;
        }
        self.flush();
        if self.apply_use_profile(None).is_none() {
            return;
        }
        self.st.profile_differs = Vec::new();
        self.st.dirty = true;
        if self.st.amends.is_some() && !self.st.saved {
            return;
        }
        self.save_soon();
    }

    /// screen_form_use_profile, taken into the page; None when it failed (toasted).
    fn apply_use_profile(&mut self, facts: Option<Vec<String>>) -> Option<Value> {
        let before = self.st.values.clone();
        let args = vec![
            json!(self.st.target.form_code),
            json!(self.st.target.year),
            json!(self.st.target.period),
            self.st.draft.clone().unwrap_or(Value::Null),
            json!(facts.unwrap_or_default()),
        ];
        match bir::call("screen_form_use_profile", args) {
            Ok(r) => {
                self.st.draft = r.get("draft").cloned();
                let mut vals = self.st.values.clone();
                for (k, v) in string_map_v(&r.get("values").cloned().unwrap_or(Value::Null)) {
                    vals.insert(k, v);
                }
                self.st.values = self.shown_values(vals);
                if let Some(opts) = r.get("options") {
                    self.st.options = options_map_v(opts);
                }
                if let Some(pools) = r.get("pools") {
                    self.st.pool_texts = string_map_v(pools);
                }
                if let Some(pen) = r.get("penalties") {
                    if let Ok(p) = serde_json::from_value::<Vec<ScreenFormPenalties>>(pen.clone()) {
                        if !p.is_empty() {
                            self.st.penalties = p;
                        }
                    }
                }
                if let Some(asks) = r.get("asks") {
                    if let Ok(a) = serde_json::from_value::<Vec<ScreenFormAsk>>(asks.clone()) {
                        self.take_asks(a);
                    }
                }
                self.sync_shown(&before, &HashMap::new());
                // A validation sent before this would bring back the pre-profile numbers.
                self.validate();
                Some(r)
            }
            Err(e) => {
                self.toast(
                    schema::plain_refusal(&error_text(&e)),
                    Some("triangle-alert".to_owned()),
                    None,
                );
                None
            }
        }
    }

    // ---------------------------------------------------------------- penalties, from, notices, stops, more

    /// Edit penalties: the row's group opens together.
    pub fn open_penalties(&mut self, id: &str) {
        let Some(g) = self.row_penalty(id) else {
            return;
        };
        if !self.st.editable || !g.applies {
            return;
        }
        for k in &g.keys {
            self.st.penalty_open.insert(k.clone());
        }
    }

    /// Use the computed amounts: the group's overrides are cleared.
    pub fn use_penalty_computed(&mut self, id: &str) {
        let Some(g) = self.row_penalty(id) else {
            return;
        };
        if g.edited || g.kept == Some(true) {
            let edits: HashMap<String, String> =
                g.keys.iter().map(|k| (k.clone(), String::new())).collect();
            self.choose(edits);
        }
        for k in &g.keys {
            self.st.penalty_open.remove(k);
        }
    }

    fn row_penalty(&self, id: &str) -> Option<ScreenFormPenalties> {
        let it = self.items.get(id)?;
        it.f.iter()
            .filter_map(|f| self.penalty_group_of(&f.k))
            .find(|g| g.applies)
    }

    /// "Use it" on an item's from hint.
    pub fn use_from(&mut self, id: &str) {
        let Some(from) = self.items.get(id).and_then(|it| it.from.clone()) else {
            return;
        };
        if !self.st.editable || self.is_shut(&from.to) {
            return;
        }
        let Some(pesos) =
            money::whole_pesos_of(&self.st.values.get(&from.k).cloned().unwrap_or_default())
        else {
            return;
        };
        let shown = mask::format_money_full(&pesos.amount.to_string(), true, true);
        self.shown.insert(from.to.clone(), shown.clone());
        self.st.pending.insert(from.to.clone(), shown);
        self.st.dirty = true;
        let keys = vec![from.to.clone()];
        self.unquiet(Some(&keys));
        self.flush();
    }

    /// Mark a notice checked (the host records it), then check again.
    pub fn mark_checked(&mut self, token: &str) {
        if self.st.draft.is_none() {
            return;
        }
        if let Err(e) = bir::call(
            "screen_form_check_notice",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
                json!(token),
            ],
        ) {
            self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
            return;
        }
        self.validate();
    }

    fn stop_boxes(&self, it: &SchemaItem) -> Vec<String> {
        if self.st.stoppable.is_empty() {
            return Vec::new();
        }
        keys_of(it)
            .into_iter()
            .filter(|k| self.st.stoppable.contains(k))
            .collect()
    }

    /// "Stop here" on row id: on or off for every box of it that may carry one.
    pub fn set_stop(&mut self, id: &str, on: bool) {
        let Some(it) = self.items.get(id).cloned() else {
            return;
        };
        if !self.st.editable {
            return;
        }
        for k in self.stop_boxes(&it) {
            let call = todo::stop_call(&k, on, &self.st.auto_stops);
            if self.st.stop_edits.get(&k).copied() == call {
                continue;
            }
            match bir::call(
                "set_template_stop",
                vec![
                    json!(self.st.target.tin),
                    json!(self.st.target.form_code),
                    json!(k),
                    match call {
                        Some(v) => json!(v),
                        None => Value::Null,
                    },
                ],
            ) {
                Ok(v) => {
                    self.events
                        .push(SessionEvent::AfterWrite("set_template_stop".to_owned()));
                    if let Some(edits) = v.get("stop_edits") {
                        self.st.stop_edits = bool_map_v(edits);
                    }
                }
                Err(e) => {
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                    return;
                }
            }
        }
        self.st.stop_keys = todo::effective_stops(&self.st.auto_stops, &self.st.stop_edits);
        if on {
            self.st.passed.remove(id);
        }
    }

    /// "Additional details": its fields shown or folded.
    pub fn toggle_more(&mut self, id: &str) {
        if !self.st.more_open.remove(id) {
            self.st.more_open.insert(id.to_owned());
        }
    }
    pub fn more_open(&self, id: &str) -> bool {
        self.st.more_open.contains(id)
    }

    // ---------------------------------------------------------------- facts and the election

    fn tax_year(&self) -> i64 {
        self.st
            .draft
            .as_ref()
            .and_then(|d| d.get("taxable_year"))
            .and_then(|v| v.as_i64())
            .unwrap_or(self.st.target.year)
    }

    fn taxpayer_name(&self) -> String {
        let root: String = self.st.target.tin.chars().take(9).collect();
        if let Ok(v) = bir::call0("list_taxpayers") {
            if let Ok(list) = serde_json::from_value::<Vec<crate::types::Taxpayer>>(v) {
                if let Some(tp) = list.iter().find(|x| x.tin_root == root) {
                    return tp.name.clone();
                }
            }
        }
        root
    }

    fn election_readiness(&mut self) -> Option<IncomeTaxElectionReadiness> {
        if self.st.election_ready.is_none() {
            self.st.election_ready = Some(
                bir::call(
                    "income_tax_election_readiness",
                    vec![json!(self.st.target.tin), json!(self.tax_year())],
                )
                .ok()
                .and_then(|v| serde_json::from_value(v).ok()),
            );
        }
        self.st.election_ready.clone().unwrap_or(None)
    }

    /// A choice card's answer picked.
    pub fn pick_ask(&mut self, fact: &str, value: Value) {
        self.st.ask_pick.insert(fact.to_owned(), value);
    }

    /// What was typed into a fact card so far.
    pub fn ask_text_of(&self, fact: &str) -> String {
        self.st.ask_text.get(fact).cloned().unwrap_or_default()
    }
    pub fn type_ask(&mut self, fact: &str, text: &str) {
        self.st.ask_text.insert(fact.to_owned(), text.to_owned());
    }

    /// A fact card's Save: the answer goes to the tax profile, the draft takes it, the return is saved.
    pub fn save_ask(&mut self, fact: &str) {
        let Some(ask) = self.st.asks.iter().find(|a| a.fact == fact).cloned() else {
            return;
        };
        if self.st.draft.is_none() || !self.st.editable || self.st.sample {
            return;
        }
        let value: Value = if ask.control == "choice" {
            match self.st.ask_pick.get(fact) {
                Some(v) => v.clone(),
                None => return,
            }
        } else {
            let raw = self.st.ask_text.get(fact).cloned().unwrap_or_default();
            let raw = raw.trim().to_owned();
            if ask.control == "date" {
                let Some(date) = dates::parse_typed_date(&raw) else {
                    self.toast(
                        DATE_ERROR.to_owned(),
                        Some("triangle-alert".to_owned()),
                        None,
                    );
                    return;
                };
                json!(format!("{:04}-{:02}-{:02}", date.y, date.m, date.d))
            } else {
                if raw.is_empty() {
                    self.toast(
                        format!("Type the {} first.", ask.label.to_lowercase()),
                        Some("triangle-alert".to_owned()),
                        None,
                    );
                    return;
                }
                json!(raw)
            }
        };
        let at = self
            .st
            .ask_at
            .iter()
            .find(|(_, asks)| asks.iter().any(|a| a.fact == fact))
            .map(|(k, _)| k.clone());
        if !self.st.pending.is_empty() {
            self.flush();
        }
        let tin_root: String = self.st.target.tin.chars().take(9).collect();
        let year = self.tax_year();
        let write = if fact == "birth_date" {
            bir::call(
                "update_taxpayer",
                vec![json!(tin_root), json!({ "birth_date": value })],
            )
        } else {
            bir::call(
                "set_year_facts",
                vec![
                    json!(self.st.target.tin),
                    json!(year),
                    json!({ fact: value }),
                ],
            )
        };
        if let Err(e) = write {
            self.toast(
                schema::plain_refusal(&error_text(&e)),
                Some("triangle-alert".to_owned()),
                None,
            );
            return;
        }
        self.events.push(SessionEvent::AfterWrite(
            if fact == "birth_date" {
                "update_taxpayer"
            } else {
                "set_year_facts"
            }
            .to_owned(),
        ));
        self.st.election_ready = None;
        let r = self.apply_use_profile(Some(vec![fact.to_owned()]));
        match r {
            Some(rv) => {
                if let Some(asks) = rv.get("asks") {
                    if let Ok(a) = serde_json::from_value::<Vec<ScreenFormAsk>>(asks.clone()) {
                        self.take_asks(a);
                    }
                } else {
                    let remaining: Vec<ScreenFormAsk> = self
                        .st
                        .asks
                        .iter()
                        .filter(|a| a.fact != fact)
                        .cloned()
                        .collect();
                    self.take_asks(remaining);
                }
            }
            None => {
                let remaining: Vec<ScreenFormAsk> = self
                    .st
                    .asks
                    .iter()
                    .filter(|a| a.fact != fact)
                    .cloned()
                    .collect();
                self.take_asks(remaining);
            }
        }
        self.st.ask_pick.remove(fact);
        self.st
            .fact_errors
            .retain(|e| e.field != format!("tax_profile.{fact}"));
        let errs: Vec<ScreenFormError> = self
            .st
            .errors
            .iter()
            .filter(|e| !e.field.starts_with("tax_profile."))
            .cloned()
            .collect();
        self.set_errors(errs);
        if let Some(at) = at {
            self.st.ask_saved_at.insert(
                at,
                format!(
                    "{} saved to the tax profile. Later returns use it.",
                    ask.label
                ),
            );
        }
        self.st.dirty = true;
        if self.st.amends.is_some() && !self.st.saved {
            self.flush();
        } else {
            self.save_soon();
        }
        let name = self.taxpayer_name();
        self.toast(
            format!("Saved to {name}'s tax profile."),
            Some("circle-check".to_owned()),
            None,
        );
    }

    /// Item 13's answer picked.
    pub fn pick_election(&mut self, value: &str) {
        self.st.election_pick = Some(value.to_owned());
    }

    /// Item 13's Save.
    pub fn record_election(&mut self) {
        let pick = self.st.election_pick.clone();
        let Some(pick) = pick else {
            return;
        };
        if self.st.draft.is_none() || !self.st.editable {
            return;
        }
        {
            let ready = self.election_readiness();
            if ready.as_ref().map(|r| r.gap.as_str()) != Some("ready") {
                return;
            }
        }
        let year = self.tax_year();
        let at = self.st.election_at.clone();
        if let Err(e) = bir::call(
            "record_income_tax_election",
            vec![json!(self.st.target.tin), json!(year), json!(pick)],
        ) {
            self.st.election_ready = None;
            self.toast(
                schema::plain_refusal(&error_text(&e)),
                Some("triangle-alert".to_owned()),
                None,
            );
            return;
        }
        self.events.push(SessionEvent::AfterWrite(
            "record_income_tax_election".to_owned(),
        ));
        self.st.election_ready = None;
        if self.apply_use_profile(None).is_none() {
            self.validate();
            return;
        }
        if let Some((graduated, eight)) = election_radios(&self.st.target.form_code) {
            let unanswered = self
                .st
                .draft
                .as_ref()
                .and_then(|d| d.get("item_13_election"))
                .and_then(|v| v.as_str())
                == Some("unanswered");
            if unanswered {
                let gv = if pick == "graduated" { "true" } else { "false" };
                let ev = if pick == "eight_percent" {
                    "true"
                } else {
                    "false"
                };
                self.st.pending.insert(graduated.to_owned(), gv.to_owned());
                self.st.values.insert(graduated.to_owned(), gv.to_owned());
                self.shown.insert(graduated.to_owned(), gv.to_owned());
                self.st.pending.insert(eight.to_owned(), ev.to_owned());
                self.st.values.insert(eight.to_owned(), ev.to_owned());
                self.shown.insert(eight.to_owned(), ev.to_owned());
            }
        }
        self.st.dirty = true;
        self.st.election_pick = None;
        self.st.election_saved_at = at;
        self.flush();
        if !(self.st.sample || (self.st.amends.is_some() && !self.st.saved)) {
            self.save_soon();
        }
        let name = self.taxpayer_name();
        self.toast(
            format!("Saved to {name}'s tax profile for {year}."),
            Some("circle-check".to_owned()),
            None,
        );
    }

    /// Item 13's "Update from tax profile".
    pub fn follow_profile(&mut self) {
        if self.st.draft.is_none() || !self.st.editable {
            return;
        }
        self.flush();
        if self.apply_use_profile(None).is_none() {
            return;
        }
        self.st.profile_differs = Vec::new();
        self.st.profile_refreshed = true;
        self.st.dirty = true;
        let item13 = if self
            .st
            .draft
            .as_ref()
            .and_then(|d| d.get("item_13_election"))
            .and_then(|v| v.as_str())
            == Some("unanswered")
        {
            " Item 13 still needs an answer: choose Not applicable."
        } else {
            ""
        };
        if self.st.sample || (self.st.amends.is_some() && !self.st.saved) {
            self.flush();
        } else {
            self.save_soon();
        }
        let name = self.taxpayer_name();
        self.toast(
            format!("This return now follows {name}'s tax profile.{item13}"),
            Some("circle-check".to_owned()),
            None,
        );
    }

    /// The election card's action.
    pub fn election_action(&mut self, id: &str) {
        if id == "fp-election-follow" {
            return self.follow_profile();
        }
        self.st.election_ready = None;
    }

    fn election_card(&mut self) -> ElectionCard {
        let ready = self.election_readiness();
        let name = self.taxpayer_name();
        let year = self.tax_year();
        if ready.as_ref().map(|r| r.gap.as_str()) == Some("ready") {
            return ElectionCard::Ask {
                question: format!("Which income-tax rate did {name} choose for {year}?"),
                pick: self.st.election_pick.clone(),
            };
        }
        let type_of = |t: Option<&str>| t.unwrap_or("non-individual").to_lowercase();
        let draft_type = self
            .st
            .draft
            .as_ref()
            .and_then(|d| d.get("taxpayer_type"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let Some(r) = ready else {
            return ElectionCard::Blocked {
                title: format!("Buwiz could not read {name}'s tax profile for {year}."),
                body: "Try again in a moment. The rate can be chosen once the profile is read."
                    .to_owned(),
                action: Some("fp-election-retry"),
            };
        };
        if r.gap == "not_business_income"
            && r.classification.is_none()
            && self.st.asks.iter().any(|a| a.fact == "tax_classification")
        {
            // TS returns null; the row draws no card. We signal that with a sentinel:
            return ElectionCard::Blocked {
                title: String::new(),
                body: String::new(),
                action: None,
            };
        }
        if r.gap == "not_business_income" {
            let title = if r.classification.as_deref() == Some("PurelyCompensation") {
                format!(
                    "{name}'s tax profile has {name} earning compensation income only in {year}."
                )
            } else {
                format!(
                    "{name}'s tax profile does not say whether {name} is self-employed or has mixed income in {year}."
                )
            };
            return ElectionCard::Blocked {
                title,
                body: "The income-tax rate is chosen by self-employed and mixed-income earners. If that is wrong, change the income type on the tax profile; then come back here to choose the rate.".to_owned(),
                action: None,
            };
        }
        if draft_type.as_deref() == Some("Individual") && !self.st.profile_refreshed {
            let t = type_of(r.taxpayer_type.as_deref());
            return ElectionCard::Blocked {
                title: format!(
                    "This return still treats {name} as an individual, but the tax profile registers {name} as a {t} in {year}."
                ),
                body: format!(
                    "Only individuals choose this income-tax rate. Update this return from the tax profile: it then follows the {t} registration, and the rate question goes away."
                ),
                action: Some("fp-election-follow"),
            };
        }
        if draft_type.as_deref() == Some("Individual") {
            return ElectionCard::Blocked {
                title: format!("This quarter of {name}'s tax profile is still an individual's registration, while {year}'s latest is a {}'s.", type_of(r.taxpayer_type.as_deref())),
                body: "Check the registration dates on the tax profile. Until they agree, this return asks for the income-tax rate and the profile cannot record one.".to_owned(),
                action: None,
            };
        }
        let tt = r
            .taxpayer_type
            .as_deref()
            .map(|t| format!("a {t}"))
            .unwrap_or_else(|| "a non-individual".to_owned());
        ElectionCard::Blocked {
            title: format!("{name}'s tax profile is registered as {tt} in {year}."),
            body: "Only individuals choose this income-tax rate. If that registration is wrong, correct it on the tax profile.".to_owned(),
            action: None,
        }
    }

    // ---------------------------------------------------------------- parts and stepping

    pub fn toggle_part(&mut self, pi: usize) {
        if !self.st.open_parts.remove(&pi) {
            self.st.open_parts.insert(pi);
            self.st.current = pi;
        }
    }

    /// Expand all, or Collapse all when every part is open.
    pub fn toggle_all_parts(&mut self) {
        let Some(schema) = &self.schema else {
            return;
        };
        if schema
            .parts
            .iter()
            .enumerate()
            .all(|(i, _)| self.st.open_parts.contains(&i))
        {
            self.st.open_parts = HashSet::new();
        } else {
            self.st.open_parts = (0..schema.parts.len()).collect();
        }
    }

    /// Navigator and pill: open the chosen part and fold the others.
    pub fn jump_to(&mut self, pi: usize, row: Option<String>) {
        self.st.open_parts = [pi].into_iter().collect();
        self.st.current = pi;
        self.st.step_at = row.clone();
        // The reveal is consumed by the frame's scroll; store it on st for the draw.
        self.reveal = Some(Reveal {
            row,
            part: pi,
            seq: self.reveal_seq + 1,
        });
        self.reveal_seq += 1;
    }

    /// The part the scroll shows now.
    pub fn set_current(&mut self, pi: usize) {
        self.st.current = pi;
    }

    /// The next part with a shown message after the open one.
    pub fn next_needing_part(&self) -> Option<usize> {
        let schema = self.schema.as_ref()?;
        let needing: Vec<usize> = (0..schema.parts.len())
            .filter(|&i| self.part_errors(i) > 0)
            .collect();
        if needing.is_empty() {
            return None;
        }
        let open_one: i64 = if self.st.open_parts.len() == 1 {
            *self.st.open_parts.iter().next().unwrap() as i64
        } else {
            -1
        };
        needing
            .iter()
            .find(|&&i| i as i64 > open_one)
            .or_else(|| needing.first())
            .copied()
    }

    /// Where ‹ (-1) or › (1) goes.
    pub fn step_target(&self, dir: i32) -> Option<StepTarget> {
        if self.schema.is_none() {
            return None;
        }
        let ids: Vec<String> = self.todo_now().iter().map(|r| r.id.clone()).collect();
        if !ids.is_empty() {
            if let Some(row) = todo::next_row(
                &ids,
                dir,
                self.st.step_at.as_deref(),
                self.st.current,
                self.st.step_from_top,
            ) {
                let part = row
                    .split(':')
                    .next()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);
                return Some(StepTarget::Row {
                    part,
                    row: Some(row),
                });
            }
            if dir < 0 && self.st.step_at.is_none() && self.st.step_from_top {
                let last = ids.last().unwrap().clone();
                let part = last
                    .split(':')
                    .next()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);
                return Some(StepTarget::Row {
                    part,
                    row: Some(last),
                });
            }
            return if dir > 0 {
                Some(StepTarget::Check)
            } else {
                None
            };
        }
        let part = self.st.current as i64 + dir as i64;
        if part >= 0 && (part as usize) < self.schema.as_ref().unwrap().parts.len() {
            return Some(StepTarget::Row {
                part: part as usize,
                row: None,
            });
        }
        if dir > 0 && self.st.editable && !self.st.errors.is_empty() {
            return Some(StepTarget::Check);
        }
        None
    }

    /// ‹ and › (Cmd/Ctrl+Left/Right).
    pub fn step(&mut self, dir: i32) {
        let Some(target) = self.step_target(dir) else {
            return;
        };
        match target {
            StepTarget::Check => {
                if let Some(at) = self.st.step_at.take() {
                    self.st.passed.insert(at);
                }
                self.st.step_from_top = true;
                self.check();
            }
            StepTarget::Row { part, row } => {
                if let Some(at) = self.st.step_at.take() {
                    if dir > 0 {
                        self.st.passed.insert(at);
                    }
                }
                self.st.step_from_top = false;
                self.st.open_parts.insert(part);
                self.st.current = part;
                self.st.step_at = row.clone();
                self.reveal = Some(Reveal {
                    row,
                    part,
                    seq: self.reveal_seq + 1,
                });
                self.reveal_seq += 1;
            }
        }
    }

    /// The keyboard into a row's box (CUI has no per-widget focus yet; returns whether it would).
    pub fn focus_row(&mut self, row: &str) -> bool {
        let Some(it) = self.items.get(row).cloned() else {
            return false;
        };
        if it.rows.is_some() || !self.st.editable {
            return false;
        }
        let fs: Vec<SchemaField> =
            it.f.iter()
                .filter(|f| f.c != Some(true) && !self.is_locked(&f.k))
                .cloned()
                .collect();
        let entry = self.todo_now().into_iter().find(|r| r.id == row);
        let empty = |f: &SchemaField| {
            if f.t == "radio" || f.t == "chk" {
                self.st.values.get(&f.k).map(String::as_str) != Some("true")
            } else {
                self.st
                    .values
                    .get(&f.k)
                    .map(|v| v.trim().is_empty())
                    .unwrap_or(true)
            }
        };
        let pick = fs
            .iter()
            .find(|f| entry.as_ref().is_some_and(|e| e.stops.contains(&f.k)))
            .or_else(|| fs.iter().find(|f| f.r == Some(true) && empty(f)))
            .or_else(|| fs.first());
        let Some(pick) = pick else {
            return false;
        };
        let _date = schema::one_date(&it);
        // Focus is a no-op in CUI for now; the row still counts as handled.
        let _ = pick;
        true
    }

    /// Messages as they show: a row's after it was touched, every one after Check.
    pub fn shown_messages(&self) -> HashMap<String, String> {
        let s = &self.st;
        let mut out = HashMap::new();
        let ids: HashSet<String> = s
            .messages
            .keys()
            .chain(s.date_errors.keys())
            .cloned()
            .collect();
        for id in ids {
            let msg = s
                .date_errors
                .get(&id)
                .or_else(|| s.messages.get(&id))
                .cloned()
                .unwrap_or_default();
            if !msg.is_empty() && (s.show_all || s.touched.contains(&id)) {
                out.insert(id, msg);
            }
        }
        out
    }

    /// How many messages each row with a shown message carries.
    pub fn shown_counts(&self) -> HashMap<String, usize> {
        let mut out = HashMap::new();
        for id in self.shown_messages().keys() {
            let s = &self.st;
            out.insert(
                id.clone(),
                (if s.date_errors.contains_key(id) { 1 } else { 0 })
                    .max(*s.message_counts.get(id).unwrap_or(&0))
                    .max(1),
            );
        }
        out
    }

    /// How many rows of part pi hold a validation message, shown or not.
    pub fn part_errors(&self, pi: usize) -> usize {
        let prefix = format!("{pi}:");
        self.st
            .messages
            .keys()
            .filter(|id| id.starts_with(&prefix))
            .count()
    }

    /// The to-do list: every row with a message counts, whether its text shows yet or not.
    pub fn todo_now(&self) -> Vec<todo::TodoRow> {
        let Some(schema) = &self.schema else {
            return Vec::new();
        };
        if self.st.draft.is_none() {
            return Vec::new();
        }
        let stops = if self.st.editable {
            self.st.stop_keys.clone()
        } else {
            HashSet::new()
        };
        let blocked = |k: &str| !self.st.editable || self.is_locked(k);
        todo::todo_rows(&TodoRowsOptions {
            parts: &schema.parts,
            messages: &self.st.messages,
            values: &self.st.values,
            stops: &stops,
            passed: &self.st.passed,
            blocked: &blocked,
        })
    }

    /// The pill's red count.
    pub fn attention(&self) -> usize {
        let todo = self.todo_now();
        if !todo.is_empty() {
            todo.iter()
                .filter(|r| r.reason == todo::TodoReason::Message)
                .count()
        } else {
            self.schema
                .as_ref()
                .map(|sc| {
                    (0..sc.parts.len())
                        .filter(|&i| self.part_errors(i) > 0)
                        .count()
                })
                .unwrap_or(0)
        }
    }

    // ---------------------------------------------------------------- check, save, queue, print

    /// Check: every message shows; a toast says how many items need attention once validation is back.
    pub fn check(&mut self) {
        self.unquiet(None);
        self.st.show_all = true;
        self.flush();
        let token = self.token;
        self.later(50, Timer::CheckWait { token });
    }

    fn check_wait(&mut self) {
        if self.st.checking {
            let token = self.token;
            self.later(100, Timer::CheckWait { token });
            return;
        }
        let n: usize = self.shown_counts().values().sum();
        let msg = if n > 0 {
            format!(
                "{n} item{} need{} attention.",
                if n > 1 { "s" } else { "" },
                if n > 1 { "" } else { "s" }
            )
        } else {
            "No problems found.".to_owned()
        };
        let icon = if n > 0 {
            "triangle-alert"
        } else {
            "circle-check"
        };
        self.toast(msg, Some(icon.to_owned()), None);
    }

    fn cancel_autosave(&mut self) {
        self.timers
            .retain(|(_, t)| !matches!(t, Timer::Autosave { .. } | Timer::SavePaint { .. }));
        if matches!(self.st.save_state, SaveState::Saving) {
            self.st.save_state = self.resting_save_state();
        }
    }

    /// After a successful fill: AUTOSAVE_DELAY of quiet saves.
    fn schedule_autosave(&mut self) {
        self.cancel_autosave();
        if !self.can_autosave() || self.removal_undo.is_some() {
            if !matches!(self.st.save_state, SaveState::Failed(_)) {
                self.st.save_state = self.resting_save_state();
            }
            return;
        }
        let token = self.token;
        self.later(AUTOSAVE_DELAY_MS, Timer::Autosave { token });
    }

    /// Save at once: the autosave path without the quiet.
    fn save_soon(&mut self) {
        self.cancel_autosave();
        if !self.can_autosave() {
            return;
        }
        self.autosave();
    }

    fn autosave(&mut self) {
        if !self.can_autosave() {
            return;
        }
        if !self.st.pending.is_empty() {
            return;
        }
        if !self.st.dirty && self.st.saved {
            return;
        }
        self.st.save_state = SaveState::Saving;
        let token = self.token;
        self.later(SAVING_PAINT_MS, Timer::SavePaint { token });
    }

    /// The explicit save: an amendment's first save, a sample's "Save as return".
    pub fn save_now(&mut self) {
        if self.st.draft.is_none() || !self.st.editable {
            return;
        }
        self.cancel_autosave();
        if self.st.sample {
            self.save_sample(None);
        } else {
            self.save_return(true);
        }
    }

    /// Save a return. announce: an explicit save toasts. True when saved.
    fn save_return(&mut self, announce: bool) -> bool {
        if self.st.draft.is_none() {
            return false;
        }
        self.unquiet(None);
        let edits = std::mem::take(&mut self.st.pending);
        let changed = self.st.dirty || !edits.is_empty() || !self.st.saved;
        if !edits.is_empty() {
            self.retire_removal_undo();
            match self.fill(edits.clone()) {
                Ok(f) => {
                    if f.removes.is_some() {
                        self.st.save_state = self.resting_save_state();
                        self.hold_removal(f, edits);
                        return false;
                    }
                    self.apply_fill(f, &edits);
                }
                Err(e) => {
                    let mut pending = edits.clone();
                    pending.extend(self.st.pending.clone());
                    self.st.pending = pending;
                    self.st.save_state = SaveState::Failed(error_text(&e));
                    self.st.show_all = true;
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                    return false;
                }
            }
        }
        match bir::call(
            "screen_form_save",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
            ],
        ) {
            Ok(r) => {
                self.events
                    .push(SessionEvent::AfterWrite("screen_form_save".to_owned()));
                self.retire_removal_undo();
                if let Some(d) = r.get("draft") {
                    self.st.draft = Some(d.clone());
                }
                if let Some(status) = r.get("status").and_then(|v| v.as_str()) {
                    self.st.status = status.to_owned();
                }
                if let Some(sp) = r.get("stored_period").and_then(|v| v.as_i64()) {
                    self.st.stored_period = Some(sp);
                }
                self.st.dirty = false;
                self.st.saved = true;
                self.st.save_state = SaveState::Saved;
                if changed {
                    self.events
                        .push(SessionEvent::Saved(self.st.target.clone()));
                }
                let profiled = if !self.st.to_profile.is_empty() {
                    self.save_to_profile()
                } else {
                    None
                };
                if let Some(p) = profiled {
                    self.toast(format!("Draft saved. {p}"), Some("save".to_owned()), None);
                } else if announce {
                    self.toast("Draft saved.".to_owned(), Some("save".to_owned()), None);
                }
                self.validate();
                true
            }
            Err(e) => {
                let mut pending = edits.clone();
                pending.extend(self.st.pending.clone());
                self.st.pending = pending;
                self.st.save_state = SaveState::Failed(error_text(&e));
                self.st.show_all = true;
                false
            }
        }
    }

    /// Save on a Form library sample: it becomes the return for its period.
    fn save_sample(&mut self, then: Option<PeriodThen>) {
        let s = &mut self.st;
        if s.period_asked.is_some()
            && s.period_answered != s.period_asked
            && !s.period_values.is_empty()
        {
            s.save_after_answer = then;
            if s.period_in_flight.as_deref() != s.period_asked.as_deref() {
                s.period_seq += 1;
                let asked = s.period_asked.clone().unwrap();
                let seq = s.period_seq;
                let token = self.token;
                self.later(0, Timer::PeriodAsk { asked, seq, token });
            }
            return;
        }
        if s.existing.is_some() {
            let existing = s.existing.clone().unwrap();
            self.offer_existing(existing);
            return;
        }
        if !s.period_set {
            let _ = s;
            self.st.save_state = SaveState::Manual("Sample · set its period to save".to_owned());
            self.ask_sample_period("", then);
            return;
        }
        let edits = std::mem::take(&mut self.st.pending);
        if !edits.is_empty() {
            match self.fill(edits.clone()) {
                Ok(f) => {
                    self.accepted(&edits, Some(&f.rejected));
                    self.st.draft = Some(f.draft.clone());
                    let mut vals = self.st.values.clone();
                    for (k, v) in string_map(&f.values) {
                        vals.insert(k, v);
                    }
                    self.st.values = self.shown_values(vals);
                    if !f.pools.is_empty() {
                        self.st.pool_texts = string_map(&f.pools);
                    }
                }
                Err(e) => {
                    let mut pending = edits.clone();
                    pending.extend(self.st.pending.clone());
                    self.st.pending = pending;
                    self.st.save_state = SaveState::Failed(error_text(&e));
                    self.st.show_all = true;
                    return;
                }
            }
        }
        let r = match bir::call(
            "screen_form_save",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
            ],
        ) {
            Ok(r) => r,
            Err(e) => {
                let mut pending = edits.clone();
                pending.extend(self.st.pending.clone());
                self.st.pending = pending;
                self.st.save_state = SaveState::Failed(error_text(&e));
                self.st.show_all = true;
                return;
            }
        };
        self.events
            .push(SessionEvent::AfterWrite("screen_form_save".to_owned()));
        let saved = r.get("saved").and_then(|v| v.as_bool()).unwrap_or(true);
        if !saved {
            self.st.opens = r
                .get("opens")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            self.st.save_state = self.resting_save_state();
            if let Some(existing) = r.get("existing") {
                if let Ok(e) = serde_json::from_value::<ScreenFormExisting>(existing.clone()) {
                    self.offer_existing(e);
                    return;
                }
            }
            return;
        }
        self.st.dirty = false;
        let Some(opens) = r
            .get("opens")
            .and_then(|v| serde_json::from_value::<crate::types::OpenTarget>(v.clone()).ok())
        else {
            return;
        };
        let target = FormTarget {
            tin: self.st.target.tin.clone(),
            form_code: self.st.target.form_code.clone(),
            year: opens.year,
            period: opens.period,
            key: opens.key,
            mode: "return".to_owned(),
            snapshot_id: None,
            new_key: opens.new_key.map(Value::from),
            new_key_from: opens.new_key_from.map(Value::from),
        };
        self.toast(
            "Saved. The sample is now your return for this period.".to_owned(),
            Some("save".to_owned()),
            None,
        );
        self.period_waiting = true;
        // The page stays up while it reopens: what is typed meanwhile waits.
        let then_action = then;
        self.open_keep(target.clone(), then_action);
    }

    /// open(target, keep=true) with a then-callback stored for after it lands.
    fn open_keep(&mut self, target: FormTarget, then: Option<PeriodThen>) {
        // Store `then` on the pending open by piggybacking on period_then? TS chains .then on open.
        // We keep a queue: pending_open_then.
        self.pending_open_then = then;
        self.open(Some(target), true);
    }

    /// Saved first when anything changed; false when it could not be.
    fn ensure_saved(&mut self) -> bool {
        if self.st.sample {
            self.save_sample(None);
            return false;
        }
        if !self.st.dirty && self.st.pending.is_empty() && self.st.saved {
            return true;
        }
        self.cancel_autosave();
        self.save_return(false) && !self.st.dirty
    }

    /// 1601C and 2551Q keep their own queue path, keyed by month or quarter.
    fn uses_period_path(&self) -> bool {
        self.st.target.form_code == "1601C" || self.st.target.form_code == "2551Q"
    }

    /// Queue: saved first; tax facts not set that change what is filed stop it.
    pub fn queue(&mut self) {
        if self.st.sample {
            self.period_then = Some(PeriodThen::Queue);
            self.save_sample(Some(PeriodThen::Queue));
            return;
        }
        if !self.ensure_saved() {
            return;
        }
        let check = bir::call(
            "screen_form_queue_check",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
            ],
        )
        .ok()
        .and_then(|v| serde_json::from_value::<crate::types::QueueCheck>(v).ok());
        if let Some(check) = &check {
            if !check.blocks.is_empty() {
                self.st.fact_errors = check.blocks.clone();
                self.st.show_all = true;
                let errs: Vec<ScreenFormError> = self
                    .st
                    .errors
                    .iter()
                    .filter(|e| !e.field.starts_with("tax_profile."))
                    .cloned()
                    .collect();
                self.set_errors(errs);
                self.toast(
                    check.blocks[0].message.clone(),
                    Some("triangle-alert".to_owned()),
                    None,
                );
                return;
            }
            if !check.warns.is_empty() {
                self.st.queue_warn = Some(check.warns.clone());
                self.st.overlay = Some(Overlay::QueueConfirm {
                    warns: check.warns.clone(),
                });
                return;
            }
        }
        self.queue_now();
    }

    pub fn confirm_queue(&mut self) {
        if self.st.queue_warn.is_none() {
            return;
        }
        self.st.queue_warn = None;
        self.st.overlay = None;
        self.queue_now();
    }

    pub fn cancel_queue_confirm(&mut self) {
        self.st.queue_warn = None;
        self.st.overlay = None;
    }

    fn queue_now(&mut self) {
        let result = if self.uses_period_path() {
            bir::call(
                "queue_form_draft",
                vec![
                    json!(self.st.target.tin),
                    json!(self.st.target.form_code),
                    json!(self.st.target.year),
                    json!(if self.st.target.period == 0 {
                        Value::Null
                    } else {
                        json!(self.st.target.period)
                    }),
                ],
            )
            .map(|_| "queue_form_draft")
        } else {
            bir::call(
                "screen_form_queue",
                vec![
                    json!(self.st.target.form_code),
                    self.st.draft.clone().unwrap_or(Value::Null),
                ],
            )
            .map(|_| "screen_form_queue")
        };
        match result {
            Ok(name) => {
                self.events.push(SessionEvent::AfterWrite(name.to_owned()));
                self.toast(
                    "Queued for background submission.".to_owned(),
                    Some("send".to_owned()),
                    None,
                );
                self.events
                    .push(SessionEvent::QueueChanged(self.st.target.clone()));
                let t = self.st.target.clone();
                self.open(
                    Some(FormTarget {
                        mode: if t.mode == "amend" {
                            "return".to_owned()
                        } else {
                            t.mode
                        },
                        ..t
                    }),
                    false,
                );
            }
            Err(e) => {
                self.st.show_all = true;
                self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
            }
        }
    }

    pub fn cancel_queue(&mut self) {
        if self.st.sample {
            return;
        }
        let name = if self.uses_period_path() {
            "cancel_queued"
        } else {
            "screen_form_cancel_queue"
        };
        let result = if self.uses_period_path() {
            bir::call(
                "cancel_queued",
                vec![
                    json!(self.st.target.tin),
                    json!(self.st.target.form_code),
                    json!(self.st.target.year),
                    json!(if self.st.target.period == 0 {
                        Value::Null
                    } else {
                        json!(self.st.target.period)
                    }),
                ],
            )
        } else {
            bir::call(
                "screen_form_cancel_queue",
                vec![
                    json!(self.st.target.form_code),
                    self.st.draft.clone().unwrap_or(Value::Null),
                ],
            )
        };
        match result {
            Ok(_) => {
                self.events.push(SessionEvent::AfterWrite(name.to_owned()));
                self.toast(
                    "Queue cancelled. The return is an editable draft again.".to_owned(),
                    Some("undo-2".to_owned()),
                    None,
                );
                self.events
                    .push(SessionEvent::QueueChanged(self.st.target.clone()));
            }
            Err(e) => {
                self.toast(
                    cancel_queue_message(&error_text(&e)),
                    Some("triangle-alert".to_owned()),
                    None,
                );
                self.events.push(SessionEvent::AfterWrite(name.to_owned()));
            }
        }
        let t = self.st.target.clone();
        self.open(
            Some(FormTarget {
                mode: if t.mode == "amend" {
                    "return".to_owned()
                } else {
                    t.mode
                },
                ..t
            }),
            false,
        );
    }

    /// Print preview.
    pub fn print(&mut self) {
        if self.st.draft.is_none() {
            return;
        }
        if !self.has_second_window {
            self.toast(NO_PREVIEW.to_owned(), Some("printer".to_owned()), None);
            return;
        }
        if self.st.sample {
            let unlocked = self.unlocked_for(&self.st.pending);
            match bir::call(
                "screen_form_print_sample",
                vec![
                    json!(self.st.target.form_code),
                    self.st.draft.clone().unwrap_or(Value::Null),
                    edits_json(&self.st.pending),
                    json!(unlocked),
                ],
            ) {
                Ok(_) => {
                    self.st.preview_open = true;
                    self.toast(
                        "Print preview opened. It follows your edits live.".to_owned(),
                        Some("printer".to_owned()),
                        None,
                    );
                }
                Err(e) => {
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                }
            }
            return;
        }
        if self.st.amends.is_some() && !self.st.saved {
            self.flush();
        } else if self.st.editable && !self.ensure_saved() {
            return;
        }
        match bir::call(
            "screen_form_print",
            vec![
                json!(self.st.target.form_code),
                self.st.draft.clone().unwrap_or(Value::Null),
            ],
        ) {
            Ok(_) => {
                self.st.preview_open = true;
                self.toast(
                    "Print preview opened. It follows your edits live.".to_owned(),
                    Some("printer".to_owned()),
                    None,
                );
            }
            Err(e) => {
                self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
            }
        }
    }

    /// View an amendment's filed original (read-only).
    pub fn open_filed_original(&mut self) {
        let Some(fo) = self.st.filed_original.clone() else {
            return;
        };
        if self.is_dirty() && ((self.st.amends.is_some() && !self.st.saved) || !self.ensure_saved())
        {
            self.toast(
                "Save your edits first: viewing the filed original leaves this return.".to_owned(),
                Some("triangle-alert".to_owned()),
                None,
            );
            return;
        }
        self.st.back = Some(self.st.target.clone());
        let mut t = self.st.target.clone();
        t.mode = "filed_original".to_owned();
        t.snapshot_id = Some(fo.id);
        self.open(Some(t), false);
    }

    pub fn back_to_amendment(&mut self) {
        if let Some(back) = self.st.back.clone() {
            self.open(Some(back), false);
        }
    }

    // ---------------------------------------------------------------- a sample's period

    /// A sample's period part changed.
    fn set_period_part(&mut self, parts: HashMap<String, String>) {
        let s = &mut self.st;
        for (k, v) in &parts {
            s.period_values.insert(k.clone(), v.clone());
            s.values.insert(k.clone(), v.clone());
            self.shown.insert(k.clone(), v.clone());
        }
        let asked = serde_json::to_string(&s.period_values).unwrap_or_default();
        if Some(&asked) == s.period_asked.as_ref() {
            return;
        }
        s.period_asked = Some(asked.clone());
        if matches!(s.overlay, Some(Overlay::Existing { .. })) {
            s.overlay = None;
        }
        s.opens = None;
        s.existing = None;
        s.period_seq += 1;
        let seq = s.period_seq;
        self.period_waiting = true;
        let token = self.token;
        self.later(PERIOD_ASK_DELAY_MS, Timer::PeriodAsk { asked, seq, token });
    }

    fn ask_period(&mut self, asked: &str) {
        let token = self.token;
        self.st.period_in_flight = Some(asked.to_owned());
        self.period_waiting = false;
        let parts: HashMap<String, String> = serde_json::from_str(asked).unwrap_or_default();
        let rx = bir::call_async_spawn(
            "screen_form_sample_period",
            vec![
                json!(self.st.target.tin),
                json!(self.st.target.form_code),
                json!(parts),
            ],
        );
        self.pending_period = Some(PendingPeriod {
            asked: asked.to_owned(),
            token,
            rx,
        });
    }

    /// The host's answer for a sample's typed period.
    fn take_period(&mut self, r: ScreenFormSamplePeriod) {
        self.st.period_complete = r.complete;
        self.st.opens = if r.complete { r.opens.clone() } else { None };
        self.st.existing = r.existing.clone();
        self.st.period_token += 1;
        let period = self.st.period_token;
        self.close_offer();
        if let Some(sample) = r.sample.clone() {
            self.rebuild(sample);
            if let Some(existing) = &r.existing {
                self.offer_existing(existing.clone());
            } else if !self.st.typed_edits.is_empty() {
                self.st.dirty = true;
                self.schedule_autosave();
            }
        } else if self.st.period_set {
            self.to_placeholder(period, None);
        } else if let Some(existing) = &r.existing {
            self.offer_existing(existing.clone());
        }
    }

    /// The placeholder sample again (the period not set).
    fn to_placeholder(&mut self, period: u64, then: Option<PeriodThen>) {
        let token = self.token;
        let rx = bir::call_async_spawn(
            "screen_form_sample",
            vec![json!(self.st.target.tin), json!(self.st.target.form_code)],
        );
        self.pending_placeholder = Some((period, token, rx));
        let _ = then;
    }

    /// Take a sample the host built, with what the filer typed outside the period applied again.
    fn rebuild(&mut self, sample: ScreenFormOpened) {
        let typed = self.st.typed_edits.clone();
        let period_values = self.st.period_values.clone();
        self.take_opened(sample);
        if !self.st.period_set {
            self.st.period_values = period_values.clone();
        }
        self.st.typed_edits = typed.clone();
        for k in typed.keys() {
            self.st.typed.insert(k.clone());
        }
        if !typed.is_empty() {
            match self.fill(typed.clone()) {
                Ok(f) => {
                    self.st.draft = Some(f.draft.clone());
                    let mut vals = self.st.values.clone();
                    for (k, v) in string_map(&f.values) {
                        vals.insert(k, v);
                    }
                    self.st.values = self.shown_values(vals);
                    self.st.options = options_map(&f.options);
                }
                Err(e) => {
                    self.toast(error_text(&e), Some("triangle-alert".to_owned()), None);
                }
            }
        }
        if !self.st.period_set {
            for (k, v) in &period_values {
                self.st.values.insert(k.clone(), v.clone());
            }
        }
        self.st.save_state = self.resting_save_state();
        self.reset_shown();
        self.validate();
    }

    fn close_offer(&mut self) {
        if matches!(self.st.overlay, Some(Overlay::Existing { .. })) {
            self.st.overlay = None;
        }
    }

    /// A stored return already has the sample's period: the dialog offers what its status allows.
    fn offer_existing(&mut self, existing: ScreenFormExisting) {
        self.st.existing = Some(existing.clone());
        if existing.status == "Draft" && self.fast_lane_drafts {
            self.take_existing("load", true);
            return;
        }
        self.st.overlay = Some(Overlay::Existing { existing });
    }

    /// "Don't ask again for drafts" (while this page lives).
    pub fn set_fast_lane(&mut self, on: bool) {
        self.fast_lane_drafts = on;
    }

    /// The filer's choice for a stored return of the sample's period.
    pub fn take_existing(&mut self, choice: &str, auto: bool) {
        self.close_offer();
        let (Some(existing), Some(opens)) = (self.st.existing.clone(), self.st.opens.clone())
        else {
            return;
        };
        if choice == "keep" {
            let before = self.snapshot_period();
            self.keep_form();
            // Undo restores the period and reopens the offer.
            self.period_undo = Some((before, existing.clone()));
            self.undo_seq += 1;
            let _ = self.undo_seq;
            self.toast(
                "Kept this form; its period is cleared.".to_owned(),
                Some("undo-2".to_owned()),
                Some(("Undo".to_owned(), UndoCmd::KeepPeriod)),
            );
            return;
        }
        // Snapshot the sample state for the Undo.
        let sample_st = self.st.clone();
        let sample_shown = self.shown.clone();
        let offer = if auto { None } else { Some(existing.clone()) };
        self.form_undo = Some((Box::new(sample_st), offer, sample_shown));
        let target = FormTarget {
            tin: self.st.target.tin.clone(),
            form_code: self.st.target.form_code.clone(),
            year: opens.year,
            period: opens.period,
            key: opens.key,
            mode: if choice == "amend" {
                "amend".to_owned()
            } else {
                "return".to_owned()
            },
            snapshot_id: None,
            new_key: opens.new_key.map(Value::from),
            new_key_from: opens.new_key_from.map(Value::from),
        };
        let what = match choice {
            "load" if auto => format!(
                "Loaded your saved {} draft (you chose not to be asked again)",
                existing.label
            ),
            "load" => format!("Loaded your saved {} draft", existing.label),
            "view" => format!("Opened {} read-only", existing.label),
            _ => format!("Started an amended {} return", existing.label),
        };
        // open() is async: the toast with Undo comes after it lands — queue it via pending_open_then?
        // TS chains .then; we store a "after open" toast request.
        self.after_open_toast = Some(AfterOpenToast {
            message: format!("{what}."),
            icon: "undo-2".to_owned(),
            cmd: UndoCmd::RestoreForm,
        });
        self.open(Some(target.clone()), false);
        self.events.push(SessionEvent::Retarget(target));
    }

    /// Undo "Keep this form": the period snapshot and its offer come back.
    fn undo_keep_period(&mut self) {
        let Some((snap, offer)) = self.period_undo.take() else {
            return;
        };
        if snap.fields.period_asked != self.st.period_asked {
            // staleness: the snapshot was of this same st (we don't replace st on keep)
        }
        self.st.period_token += 1;
        self.st.period_seq += 1;
        let f = snap.fields;
        self.st.draft = Some(f.draft.clone());
        self.st.values = f.values.clone();
        self.st.options = f.options.clone();
        self.st.locks = f.locks.clone();
        self.st.lock_notes = f.lock_notes.clone();
        self.st.notes_always = f.notes_always.clone();
        self.st.blank = f.blank.clone();
        self.st.needs = f.needs.clone();
        self.st.whys = f.whys.clone();
        self.st.errors = f.errors.clone();
        self.st.messages = f.messages.clone();
        self.st.form_errors = f.form_errors.clone();
        self.st.period_values = f.period_values.clone();
        self.st.period_asked = f.period_asked.clone();
        self.st.period_set = f.period_set;
        self.st.opens = f.opens.clone();
        self.st.existing = f.existing.clone();
        self.shown = snap.shown.clone();
        if self.st.existing.as_ref().map(|e| e.form_code.clone()) == Some(offer.form_code.clone())
            && self.st.existing.as_ref().map(|e| e.period) == Some(offer.period)
        {
            self.st.overlay = Some(Overlay::Existing { existing: offer });
        }
    }

    /// Undo a Load / View / Amend: the sample comes back exactly as it was.
    fn undo_restore_form(&mut self) {
        let Some((sample_st, offer, sample_shown)) = self.form_undo.take() else {
            return;
        };
        if self.is_dirty() {
            self.toast(
                "Undo would lose edits you have not saved. Save them first, or leave this return."
                    .to_owned(),
                Some("triangle-alert".to_owned()),
                None,
            );
            return;
        }
        let saved_amendment = self.st.amends.is_some() && self.st.saved;
        self.cancel_autosave();
        self.st = *sample_st;
        self.st_gen += 1;
        self.token += 1;
        self.shown = sample_shown;
        if let Some(offer) = offer {
            if self
                .st
                .existing
                .as_ref()
                .map(|e| (e.form_code.clone(), e.period, e.year.clone()))
                == Some((offer.form_code.clone(), offer.period, offer.year.clone()))
            {
                self.st.overlay = Some(Overlay::Existing { existing: offer });
            }
        }
        self.reset_shown();
        self.events
            .push(SessionEvent::Retarget(self.st.target.clone()));
        if saved_amendment {
            self.toast(
                "Back to the sample. The amended return was saved and stays saved.".to_owned(),
                Some("undo-2".to_owned()),
                None,
            );
        }
    }

    fn snapshot_period(&self) -> PeriodSnapshot {
        let s = &self.st;
        let fields = PeriodFields {
            draft: s.draft.clone().unwrap_or(Value::Null),
            values: s.values.clone(),
            options: s.options.clone(),
            locks: s.locks.clone(),
            lock_notes: s.lock_notes.clone(),
            notes_always: s.notes_always.clone(),
            blank: s.blank.clone(),
            needs: s.needs.clone(),
            whys: s.whys.clone(),
            errors: s.errors.clone(),
            messages: s.messages.clone(),
            form_errors: s.form_errors.clone(),
            period_values: s.period_values.clone(),
            period_asked: s.period_asked.clone(),
            period_set: s.period_set,
            opens: s.opens.clone(),
            existing: s.existing.clone(),
        };
        let mut shown = HashMap::new();
        for k in self.all_keys.iter().chain(self.one_dates.keys()) {
            shown.insert(k.clone(), self.shown.get(k).cloned().unwrap_or_default());
        }
        PeriodSnapshot { fields, shown }
    }

    /// Keep this form: the same sample, every period part blank again.
    fn keep_form(&mut self) {
        self.st.period_values = HashMap::new();
        self.st.period_asked = Some("{}".to_owned());
        self.st.period_answered = None;
        self.st.period_complete = false;
        self.st.period_seq += 1;
        self.st.opens = None;
        self.st.existing = None;
        let keys: Vec<String> = self.st.period_parts.keys().cloned().collect();
        for k in &keys {
            self.st.values.insert(k.clone(), String::new());
            self.shown.insert(k.clone(), String::new());
        }
        if self.st.period_set {
            self.st.period_token += 1;
            self.to_placeholder(self.st.period_token, None);
        }
    }

    /// The host answered the period Save waited for.
    fn after_period_for_save(&mut self, then: Option<PeriodThen>) {
        if self.st.existing.is_some() {
            return;
        }
        if !self.st.period_complete {
            self.ask_sample_period("Enter the whole period to save this sample.", then);
            return;
        }
        self.save_sample(then);
    }

    /// The period dialog for a sample's Save.
    fn ask_sample_period(&mut self, error: &str, then: Option<PeriodThen>) {
        self.period_then = then;
        let parts: Vec<SamplePeriodPart> = self
            .st
            .period_parts
            .iter()
            .map(|(key, p)| SamplePeriodPart {
                key: key.clone(),
                part: p.part.clone(),
                quarter: p.quarter,
                value: self
                    .st
                    .period_values
                    .get(key)
                    .or_else(|| self.st.values.get(key))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        self.st.overlay = Some(Overlay::SamplePeriod {
            parts,
            error: error.to_owned(),
        });
    }

    pub fn close_sample_period(&mut self) {
        if matches!(self.st.overlay, Some(Overlay::SamplePeriod { .. })) {
            self.st.overlay = None;
        }
        self.period_then = None;
    }

    /// The period dialog's Save.
    pub fn take_sample_period(&mut self, parts: HashMap<String, String>) {
        let then = self.period_then.take();
        self.close_sample_period();
        self.set_period_part(parts);
        if self.st.period_answered == self.st.period_asked && self.st.period_answered.is_some() {
            self.after_period_for_save(then);
            return;
        }
        self.st.save_after_answer = then;
        self.st.period_seq += 1;
        if let Some(asked) = self.st.period_asked.clone() {
            let seq = self.st.period_seq;
            let token = self.token;
            self.later(0, Timer::PeriodAsk { asked, seq, token });
        }
    }

    // ---------------------------------------------------------------- flushNow / dispose

    /// Typing flushed and saved now where autosave may. False when a save was refused.
    pub fn flush_now(&mut self) -> bool {
        if self.disposed {
            return true;
        }
        // Wait for the open: poll() has already run this frame; if still loading, the caller
        // retries next frame (the frame calls flush_now only when leaving).
        if self.reopening || self.st.loading {
            return true;
        }
        let failed = self.finish();
        if let Some(f) = failed {
            self.toast(f, Some("triangle-alert".to_owned()), None);
            return false;
        }
        true
    }

    /// The reason finish() could not save, or None.
    fn finish(&mut self) -> Option<String> {
        let s = &self.st;
        if s.draft.is_none() || !s.editable || s.loading || self.reopening {
            return None;
        }
        self.cancel_autosave();
        if !self.st.pending.is_empty() {
            let edits = std::mem::take(&mut self.st.pending);
            match self.fill(edits.clone()) {
                Ok(r) => {
                    if r.removes.is_some() {
                        self.st.pending = edits;
                        return Some(format!(
                            "Not saved: {}",
                            r.removes
                                .as_ref()
                                .map(|m| m.message.clone())
                                .unwrap_or_default()
                        ));
                    }
                    self.apply_fill(r, &edits);
                    self.st.dirty = true;
                }
                Err(e) => {
                    let mut pending = edits;
                    pending.extend(self.st.pending.clone());
                    self.st.pending = pending;
                    return Some(error_text(&e));
                }
            }
        }
        if !self.st.dirty || self.st.sample || !self.can_autosave() {
            return None;
        }
        if self.save_return(false) {
            return None;
        }
        match &self.st.save_state {
            SaveState::Failed(m) => Some(m.clone()),
            _ => None,
        }
    }

    /// The page goes: what is typed is flushed and, where autosave may, saved.
    pub fn dispose(&mut self) {
        if self.disposed {
            return;
        }
        self.retire_removal_undo();
        let failed = self.finish();
        if let Some(f) = failed {
            self.toast(f, Some("triangle-alert".to_owned()), None);
        }
        self.disposed = true;
        self.timers.clear();
        self.token += 1;
    }

    // ---------------------------------------------------------------- reads for the page

    /// One box's shown text (what the input displays).
    pub fn shown_of(&self, k: &str) -> String {
        self.shown
            .get(k)
            .cloned()
            .unwrap_or_else(|| self.shown_text(k))
    }

    /// Sets what a box shows (the page writes the input's text back here on edit/blur).
    pub fn set_shown(&mut self, k: &str, text: &str) {
        self.shown.insert(k.to_owned(), text.to_owned());
    }

    pub fn value_of(&self, k: &str) -> String {
        self.st.values.get(k).cloned().unwrap_or_default()
    }

    pub fn message_of(&self, id: &str) -> String {
        self.st.messages.get(id).cloned().unwrap_or_default()
    }

    pub fn item_id_for_key(&self, k: &str) -> Option<String> {
        self.key_index.get(k).cloned()
    }

    pub fn item(&self, id: &str) -> Option<&SchemaItem> {
        self.items.get(id)
    }

    pub fn is_dirty(&self) -> bool {
        self.st.dirty || !self.st.pending.is_empty()
    }

    pub fn is_saved(&self) -> bool {
        self.st.saved
    }

    pub fn draft(&self) -> Option<&Value> {
        self.st.draft.as_ref()
    }

    pub fn status(&self) -> &str {
        &self.st.status
    }

    pub fn field_count(&self) -> usize {
        self.all_keys.len()
    }

    pub fn row_count(&self) -> usize {
        self.row_ids.len()
    }

    pub fn loading(&self) -> bool {
        self.st.loading
    }

    pub fn error(&self) -> &str {
        &self.st.error
    }

    pub fn editable(&self) -> bool {
        self.st.editable
    }

    pub fn sample(&self) -> bool {
        self.st.sample
    }

    pub fn period_set(&self) -> bool {
        self.st.period_set
    }

    pub fn st_ref(&self) -> &St {
        &self.st
    }

    /// A row's RowInfo (computed on demand; immediate mode needs no signals).
    pub fn row_info(&mut self, id: &str) -> RowInfo {
        let shown = self.shown_messages();
        let pi: usize = id
            .split(':')
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        let tucked = self.st.profile_open.contains(&pi)
            || !self.tucked_items(pi).contains(
                &id.split(':')
                    .nth(1)
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0),
            );
        let tucked = tucked
            && self.tucked_items(pi).contains(
                &id.split(':')
                    .nth(1)
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(usize::MAX),
            );
        self.row_info_inner(id, &shown, tucked)
    }

    fn row_info_inner(
        &mut self,
        id: &str,
        shown: &HashMap<String, String>,
        tucked: bool,
    ) -> RowInfo {
        let election_now = self.st.editable
            && self.st.election_at.is_some()
            && self.st.election_at.as_deref() == Some(id);
        let election_card_pre = if election_now {
            Some(self.election_card())
        } else {
            None
        };
        let s = &self.st;
        let Some(it) = self.items.get(id).cloned() else {
            return RowInfo::default();
        };
        let fs = it.f.clone();
        let keys = keys_of(&it);
        let tin = schema::is_tin_item(&it);
        let edit_keys: Vec<String> = fs
            .iter()
            .map(|x| x.k.clone())
            .filter(|k| s.profile_edit.contains(k))
            .collect();
        let from_profile =
            (tin && it.typed_tin != Some(true)) || fs.iter().any(|x| s.profile.contains(&x.k));
        let kept: Vec<&str> = keys
            .iter()
            .filter_map(|k| s.kept.get(k).map(String::as_str))
            .collect();
        let penalty = if s.editable {
            fs.iter()
                .filter_map(|f| self.penalty_group_of(&f.k))
                .find(|g| g.applies)
        } else {
            None
        };
        let penalty_open = penalty
            .as_ref()
            .is_some_and(|p| p.edited || p.keys.iter().any(|k| s.penalty_open.contains(k)));
        let stop_boxes = self.stop_boxes(&it);
        let editing_here = edit_keys.iter().any(|k| s.editing.contains(k));
        let saveable = self.saveable_keys(id);
        let to_on = saveable.iter().any(|k| s.to_profile.contains(k));
        let entry = saveable.first().and_then(|k| s.profile_save.get(k));
        let mut from = String::new();
        if let Some(from_hint) = &it.from {
            if s.editable && !self.is_shut(&from_hint.to) {
                if let Some(pesos) =
                    money::whole_pesos_of(&s.values.get(&from_hint.k).cloned().unwrap_or_default())
                {
                    from = format!(
                        "{}: ₱{}{}",
                        from_hint.l,
                        mask::format_money_full(&pesos.amount.to_string(), true, true),
                        if pesos.rounded {
                            " (rounded to whole pesos)"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
        let asks = if s.editable && !s.sample {
            s.ask_at
                .get(id)
                .map(|asks| {
                    asks.iter()
                        .map(|a| AskCard {
                            fact: a.fact.clone(),
                            label: a.label.clone(),
                            note: a.note.clone(),
                            control: a.control.clone(),
                            choices: a.choices.clone().unwrap_or_default(),
                            pick: s.ask_pick.get(&a.fact).cloned(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let election = election_card_pre.and_then(|card| match &card {
            ElectionCard::Blocked { title, .. } if title.is_empty() => None,
            _ => Some(card),
        });
        let election_saved =
            if s.election_saved_at.as_deref() == Some(id) && s.election_at.as_deref() != Some(id) {
                format!(
                    "Saved to the tax profile for {}. Later returns reuse it.",
                    self.tax_year()
                )
            } else {
                String::new()
            };
        let needs: Vec<String> = {
            let mut v: Vec<String> = keys
                .iter()
                .filter_map(|k| s.off.get(k).cloned())
                .filter(|x| !x.is_empty())
                .collect();
            v.sort();
            v.dedup();
            v
        };
        RowInfo {
            message: shown.get(id).cloned().unwrap_or_default(),
            notes: self.lock_notes_for(&it),
            needs,
            kept: if kept.contains(&"template") {
                "template"
            } else if kept.contains(&"last") {
                "last"
            } else {
                ""
            },
            step_here: s.step_at.as_deref() == Some(id),
            stoppable: s.editable && !stop_boxes.is_empty(),
            is_stop: stop_boxes.iter().any(|k| s.stop_keys.contains(k)),
            from_profile: if from_profile {
                if !edit_keys.is_empty() {
                    "From tax profile. Edit changes this return only; the profile stays as it is."
                        .to_owned()
                } else if tin {
                    "From tax profile. Change it in the tax profile.".to_owned()
                } else {
                    "From tax profile. It repeats a box above; use Edit there.".to_owned()
                }
            } else {
                String::new()
            },
            can_edit: !edit_keys.is_empty() && s.editable,
            editing: editing_here,
            to_profile: if !saveable.is_empty()
                && (editing_here || saveable.iter().any(|k| s.profile_edited.contains(k)))
            {
                let note = if to_on {
                    format!(
                        "Your next save writes it to the tax profile too. {}",
                        entry.map(|e| e.note.clone()).unwrap_or_default()
                    )
                    .trim()
                    .to_owned()
                } else {
                    "Off: the change stays on this return.".to_owned()
                };
                Some((to_on, note))
            } else {
                None
            },
            penalty: penalty.map(|p| if penalty_open { "computed" } else { "edit" }),
            tucked,
            asks,
            ask_saved: s.ask_saved_at.get(id).cloned().unwrap_or_default(),
            election,
            election_saved,
            checkable: if s.editable {
                s.checkable.get(id).cloned().unwrap_or_default()
            } else {
                Vec::new()
            },
            from,
        }
    }

    /// A part's PartInfo.
    pub fn part_info(&mut self, pi: usize) -> PartInfo {
        let Some(schema) = &self.schema else {
            return PartInfo::default();
        };
        if pi >= schema.parts.len() {
            return PartInfo::default();
        }
        let todo = self.todo_now();
        let shown = self.shown_messages();
        let tucked = self.tucked_items(pi);
        let fold = if !tucked.is_empty() {
            let mut filled = 0usize;
            for &ii in &tucked {
                if let Some(item) = schema.parts[pi].items.get(ii) {
                    let fs: Vec<&SchemaField> = item
                        .f
                        .iter()
                        .filter(|f| self.st.profile.contains(&f.k))
                        .collect();
                    filled += fs.iter().filter(|f| f.t != "radio").count()
                        + if fs.iter().any(|f| f.t == "radio") {
                            1
                        } else {
                            0
                        };
                }
            }
            Some(PartFold {
                first: tucked[0],
                items: tucked.clone(),
                label: format!(
                    "From tax profile · {} filled box{}",
                    filled,
                    if filled == 1 { "" } else { "es" }
                ),
                flagged: tucked
                    .iter()
                    .filter(|ii| {
                        let id = format!("{pi}:{ii}");
                        self.st.messages.contains_key(&id)
                            || self.st.date_errors.contains_key(&id)
                            || shown.contains_key(&id)
                    })
                    .count(),
                open: self.st.profile_open.contains(&pi),
            })
        } else {
            None
        };
        PartInfo {
            errors: self.part_errors(pi),
            todo: todo
                .iter()
                .filter(|r| r.id.starts_with(&format!("{pi}:")))
                .count(),
            open: self.st.open_parts.contains(&pi),
            fold,
        }
    }

    /// The header's info.
    pub fn header_info(&self) -> HeaderInfo {
        let s = &self.st;
        let mut sent = Vec::new();
        if let Some(t) = &s.submitted_at {
            sent.push(format!("Sent to BIR {}", dates::short_iso_date(t)));
        }
        if let Some(t) = &s.confirmed_at {
            sent.push(format!("confirmed {}", dates::short_iso_date(t)));
        }
        HeaderInfo {
            loading: s.loading,
            error: s.error.clone(),
            status: s.status.clone(),
            editable: s.editable,
            sample: s.sample,
            period_set: s.period_set,
            amends: s.amends.is_some() || s.filed_original.is_some(),
            amend_unsaved: s.amends.is_some() && !s.saved && s.editable,
            filed_original: s.filed_original.is_some() && s.filed_original_view.is_none(),
            filed_original_view: s.filed_original_view.clone(),
            submitted_line: if !s.editable && s.filed_original_view.is_none() && !sent.is_empty() {
                format!("{}. Nothing here can be changed.", sent.join(", "))
            } else {
                String::new()
            },
            profile_updates: s.profile_updates.clone(),
            profile_differs: if s.editable {
                s.profile_differs.clone()
            } else {
                Vec::new()
            },
            profile_changed: if !s.editable {
                s.profile_changed.clone()
            } else {
                Vec::new()
            },
            notes: s.notes.iter().map(|n| n.text.clone()).collect(),
            form_errors: s.form_errors.clone(),
        }
    }

    /// The current overlay.
    pub fn overlay(&self) -> Option<&Overlay> {
        self.st.overlay.as_ref()
    }

    pub fn save_state(&self) -> &SaveState {
        &self.st.save_state
    }

    pub fn checking(&self) -> bool {
        self.st.checking
    }

    pub fn current(&self) -> usize {
        self.st.current
    }

    pub fn step_at(&self) -> Option<&str> {
        self.st.step_at.as_deref()
    }

    pub fn all_parts_open(&self) -> bool {
        self.schema
            .as_ref()
            .map(|sc| {
                sc.parts
                    .iter()
                    .enumerate()
                    .all(|(i, _)| self.st.open_parts.contains(&i))
            })
            .unwrap_or(false)
    }

    pub fn open_parts(&self) -> &HashSet<usize> {
        &self.st.open_parts
    }

    /// Part count for the draw.
    pub fn schema_part_count(&self) -> usize {
        self.schema.as_ref().map(|s| s.parts.len()).unwrap_or(0)
    }
    /// A part's name as NSH's `partName`: "Return details" for an untitled "Page 1", else `p.n`.
    pub fn part_title(&self, pi: usize) -> String {
        self.schema
            .as_ref()
            .and_then(|s| s.parts.get(pi))
            .map(|p| schema::part_name(p, pi))
            .unwrap_or_else(|| format!("Part {}", pi + 1))
    }
    pub fn part_item_count(&self, pi: usize) -> usize {
        self.schema
            .as_ref()
            .and_then(|s| s.parts.get(pi))
            .map(|p| p.items.len())
            .unwrap_or(0)
    }
    /// End a profile edit on a row (the "Done" button).
    pub fn end_row_edit(&mut self, id: &str) {
        let keys = self.item_edit_keys(id);
        self.end_profile_edit(&keys);
    }
    /// A box's kind ("money", "sel", ...) for the mask picker.
    pub fn field_kind(&self, k: &str) -> String {
        if k.contains("#date") {
            return "date".to_owned();
        }
        self.kind_of
            .get(k)
            .cloned()
            .unwrap_or_else(|| "text".to_owned())
    }
    /// A box's max typed length (int mask).
    pub fn field_max(&self, k: &str) -> Option<usize> {
        self.field_of.get(k).and_then(|f| f.m).map(|m| m as usize)
    }
    /// What a locked blank box shows in its place (public for the draw).
    pub fn lock_reason(&self, k: &str) -> String {
        self.reason_for(k)
    }

    /// Index the schema: key -> id, field metadata, pools, computed/whole keys.
    fn index_schema(&mut self, schema: &FormSchema) {
        self.key_index.clear();
        self.field_of.clear();
        self.kind_of.clear();
        self.computed_keys.clear();
        self.whole_keys.clear();
        self.pools.clear();
        self.all_keys.clear();
        self.row_ids.clear();
        self.items.clear();
        self.one_dates.clear();
        for (pi, part) in schema.parts.iter().enumerate() {
            for (ii, it) in part.items.iter().enumerate() {
                let id = format!("{pi}:{ii}");
                self.items.insert(id.clone(), it.clone());
                self.row_ids.push(id.clone());
                for f in &it.f {
                    if let Some(pool) = &f.pool {
                        self.pools.insert(f.k.clone(), pool.clone());
                    }
                }
                for f in &it.f {
                    self.kind_of.insert(f.k.clone(), f.t.clone());
                    if f.t == "money" {
                        // whole-peso boxes have no decimals in the schema already; mask decides by value
                    }
                    self.key_index.insert(f.k.clone(), id.clone());
                    self.field_of.insert(f.k.clone(), f.clone());
                    if f.t == "computed" || f.c == Some(true) {
                        self.computed_keys.insert(f.k.clone());
                    }
                    if f.w == Some(true) {
                        self.whole_keys.insert(f.k.clone());
                    }
                    self.all_keys.push(f.k.clone());
                }
                if let Some(rows) = &it.rows {
                    let _ = rows;
                    for row in &it.keys {
                        for k in row {
                            self.kind_of.insert(k.clone(), String::new());
                            self.key_index.insert(k.clone(), id.clone());
                            self.all_keys.push(k.clone());
                        }
                    }
                    for row in &it.keys {
                        for (c, k) in row.iter().enumerate() {
                            if let Some(col) = it.cols.get(c) {
                                self.kind_of.insert(k.clone(), col.t.clone());
                            }
                        }
                    }
                }
                if let Some(date) = schema::one_date(it) {
                    let one_key = format!("{}#date", date.month.k);
                    self.one_dates
                        .insert(one_key.clone(), (date.clone(), id.clone()));
                    self.kind_of.insert(one_key, "date".to_owned());
                }
            }
        }
    }

    /// Queue a toast; returns its sid (the frame shows it).
    fn toast(
        &mut self,
        message: String,
        icon: Option<String>,
        action: Option<(String, UndoCmd)>,
    ) -> u64 {
        self.toast_seq += 1;
        let sid = self.toast_seq;
        self.events.push(SessionEvent::Toast {
            sid,
            message,
            icon,
            action: action.map(|(label, cmd)| (label, cmd)),
        });
        sid
    }
}

#[derive(Debug, Clone)]
pub struct Reveal {
    pub row: Option<String>,
    pub part: usize,
    pub seq: u64,
}

#[derive(Debug, Clone)]
struct AfterOpenToast {
    message: String,
    icon: String,
    cmd: UndoCmd,
}

#[derive(Debug, Clone)]
pub enum StepTarget {
    Row { part: usize, row: Option<String> },
    Check,
}

// ---------------------------------------------------------------- helpers

/// accepted() on a bare &mut St (used where self is split-borrowed).
fn self_accepted(
    s: &mut St,
    edits: &HashMap<String, String>,
    rejected: Option<&serde_json::Map<String, Value>>,
) {
    for (k, v) in edits {
        if rejected.is_some_and(|r| r.contains_key(k)) {
            continue;
        }
        s.typed.insert(k.clone());
        if s.sample {
            s.typed_edits.insert(k.clone(), v.clone());
        }
    }
}

fn string_map(m: &serde_json::Map<String, Value>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, val) in m {
        out.insert(
            k.clone(),
            match val {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            },
        );
    }
    out
}

fn bool_map(m: &serde_json::Map<String, Value>) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for (k, val) in m {
        out.insert(
            k.clone(),
            match val {
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                Value::String(s) => s == "true" || s == "1",
                _ => false,
            },
        );
    }
    out
}

fn options_map(m: &serde_json::Map<String, Value>) -> HashMap<String, Vec<Value>> {
    let mut out = HashMap::new();
    for (k, val) in m {
        if let Some(arr) = val.as_array() {
            out.insert(k.clone(), arr.clone());
        }
    }
    out
}

fn string_map_v(v: &Value) -> HashMap<String, String> {
    v.as_object().map(string_map).unwrap_or_default()
}
fn bool_map_v(v: &Value) -> HashMap<String, bool> {
    v.as_object().map(bool_map).unwrap_or_default()
}
fn options_map_v(v: &Value) -> HashMap<String, Vec<Value>> {
    v.as_object().map(options_map).unwrap_or_default()
}

fn profile_save_map(
    v: &serde_json::Map<String, Value>,
) -> HashMap<String, crate::types::ProfileSaveField> {
    let mut out = HashMap::new();
    for (k, val) in v {
        if let Ok(f) = serde_json::from_value::<crate::types::ProfileSaveField>(val.clone()) {
            out.insert(k.clone(), f);
        }
    }
    out
}

/// edits as the JSON object screen_form_fill takes.
fn edits_json(edits: &HashMap<String, String>) -> Value {
    let mut m = Map::new();
    for (k, v) in edits {
        m.insert(k.clone(), json!(v));
    }
    Value::Object(m)
}

/// Two amounts are the same number; a blank one is zero.
fn same_number(a: &str, b: &str) -> bool {
    let plain = |t: &str| {
        let v: String = t
            .chars()
            .filter(|c| !matches!(c, ',' | ' ' | '₱' | '%'))
            .collect();
        if v.is_empty() || regex::Regex::new(r"^[-+]?0*\.?0*$").unwrap().is_match(&v) {
            "0".to_owned()
        } else {
            v
        }
    };
    let x = plain(a);
    let y = plain(b);
    x == y || mask::same_amount(&x, &y)
}

fn ticked(v: Option<&String>) -> bool {
    matches!(v.map(String::as_str), Some("true") | Some("1") | Some("Y"))
}

/// The three boxes' values as the MM/DD/YYYY text a date box reads.
fn one_date_text_of(values: &HashMap<String, String>, date: &OneDate) -> String {
    let get = |k: &str| values.get(k).cloned().unwrap_or_default();
    let mut parts = vec![get(&date.month.k).trim().to_owned()];
    if let Some(d) = &date.day {
        parts.push(get(&d.k).trim().to_owned());
    }
    parts.push(get(&date.year.k).trim().to_owned());
    if parts.iter().all(|p| p.is_empty()) {
        String::new()
    } else {
        parts.join("/")
    }
}

/// Where a validation message goes: by key, then item number, then a TIN item, then label words.
pub fn bind_error(
    e: &ScreenFormError,
    items: &[(String, SchemaItem)],
    id_for_key: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let words = |t: &str| -> Vec<String> {
        regex::Regex::new(r"[a-z]{4,}")
            .unwrap()
            .find_iter(&t.to_lowercase())
            .map(|m| m.as_str().to_owned())
            .collect()
    };
    let mut id: Option<String> = None;
    for k in &e.keys {
        id = id_for_key(k);
        if id.is_some() {
            break;
        }
    }
    if id.is_none() {
        if let Some(caps) = regex::Regex::new(r"(?i)\bItem\s+0*(\d+[A-Z]?)\b")
            .unwrap()
            .captures(&e.message)
        {
            let want = caps[1].to_uppercase();
            if let Some((id2, _)) = items
                .iter()
                .find(|(_, it)| it.no.trim_start_matches('0').to_uppercase() == want)
            {
                id = Some(id2.clone());
            }
        }
    }
    if id.is_none() && e.message.contains("TIN") {
        if let Some((id2, _)) = items.iter().find(|(_, it)| schema::is_tin_item(it)) {
            id = Some(id2.clone());
        }
    }
    if id.is_none() {
        let w: HashSet<String> = words(&format!("{} {}", e.field, e.message))
            .into_iter()
            .collect();
        let mut best = 0usize;
        for (id2, it) in items {
            let n = words(&it.l).iter().filter(|v| w.contains(*v)).count();
            if n > best {
                best = n;
                id = Some(id2.clone());
            }
        }
    }
    id.or_else(|| items.first().map(|(id, _)| id.clone()))
}

// OneDate helpers used above
impl OneDate {
    fn one_key_placeholder(&self) -> String {
        format!("{}#date", self.month.k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// The dev environment of the door tests: a scratch database, no keychain, auth bypassed.
    /// `bir::call` reaches the host through the engine thread, which reads the same process env.
    fn dev_env<R>(database: &Path, home: &Path, body: impl FnOnce() -> R) -> R {
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_AUTH_BYPASS", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_LIVE", None),
                ("BIR_SFTP_HOST", None),
                (
                    "BIR_DATABASE_PATH",
                    Some(database.to_str().expect("utf8 path")),
                ),
            ],
            || crate::guards::with_home_for_test(home, body),
        )
    }

    /// Poll until every in-flight host call and pending open/validate/period resolves.
    fn settle(s: &mut FormSession, what: &str) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            s.poll();
            let busy = s.st.loading
                || s.st.checking
                || s.pending_open.is_some()
                || s.pending_validate.is_some()
                || s.pending_period.is_some()
                || s.pending_placeholder.is_some();
            if !busy {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!("{what}: timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// One form's full session lifecycle on a library sample: open, step through every step
    /// target, edit one field of each editable kind, flush, validate, let autosave run.
    fn exercise(code: &str) -> Result<String, String> {
        let target = FormTarget {
            tin: "00012345300000".to_owned(),
            form_code: code.to_owned(),
            year: 2026,
            period: 0,
            key: None,
            mode: "sample".to_owned(),
            snapshot_id: None,
            new_key: None,
            new_key_from: None,
        };
        let mut s = FormSession::new(target);
        if s.schema.is_none() {
            return Err(format!("schema: {}", s.st.error));
        }
        if let Some(e) = s.open(None, false) {
            return Err(format!("open refused: {e}"));
        }
        settle(&mut s, "open")?;
        if !s.st.error.is_empty() {
            return Err(format!("open error: {}", s.st.error));
        }
        if s.st.draft.is_none() {
            return Err("opened with no draft".to_owned());
        }
        let parts = s.schema.as_ref().map(|sc| sc.parts.len()).unwrap_or(0);

        // Step through every step target once (bounded), then back to the top.
        let mut steps = 0usize;
        while steps < 80 {
            if s.step_target(1).is_none() {
                break;
            }
            s.step(1);
            steps += 1;
        }
        s.jump_to(0, None);

        // One edit of each editable kind the schema offers.
        let mut by_kind: HashMap<String, Vec<String>> = HashMap::new();
        for k in &s.all_keys {
            if let Some(t) = s.kind_of.get(k) {
                by_kind.entry(t.clone()).or_default().push(k.clone());
            }
        }
        let mut edits = 0usize;
        for (kind, text) in [
            ("text", "Tester"),
            ("money", "12.5"),
            ("pct", "12.5"),
            ("int", "12"),
            ("date", "01/15/2026"),
            ("tin", "000-123-453-00000"),
        ] {
            if let Some(k) = by_kind.get(kind).and_then(|v| v.first()) {
                s.edit(k, text);
                edits += 1;
            }
        }
        for kind in ["sel", "chk"] {
            if let Some(k) = by_kind.get(kind).and_then(|v| v.first()).cloned() {
                if kind == "sel" {
                    let (_shown, options) = s.sel_shown(&k);
                    if let Some(o) = options.first() {
                        s.choose(HashMap::from([(k.clone(), o.value.clone())]));
                        edits += 1;
                    }
                } else {
                    s.choose(HashMap::from([(k.clone(), "true".to_owned())]));
                    edits += 1;
                }
            }
        }
        if let Some(k) = by_kind.get("radio").and_then(|v| v.first()) {
            s.pick_radio(k);
            edits += 1;
        }
        s.flush();
        s.check();
        settle(&mut s, "check")?;

        // Let the autosave/save_sample path run to quiescence.
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            s.poll();
            let quiescent = !s.st.checking
                && !matches!(s.st.save_state, SaveState::Saving)
                && s.pending_open.is_none()
                && s.pending_validate.is_none()
                && s.pending_period.is_none()
                && s.pending_placeholder.is_none()
                && bir::async_in_flight() == 0;
            if quiescent {
                break;
            }
            if Instant::now() > deadline {
                return Err("save wait timed out".to_owned());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let problems: usize = s.shown_counts().values().sum::<usize>();
        let save = match &s.st.save_state {
            SaveState::Failed(m) => format!("FAILED({m})"),
            SaveState::Manual(m) => format!("manual({m})"),
            other => format!("{other:?}"),
        };
        if let SaveState::Failed(m) = &s.st.save_state {
            return Err(format!("save failed: {m}"));
        }
        Ok(format!(
            "parts={parts} steps={steps} keys={} edits={edits} problems={problems} save={save}",
            s.all_keys.len()
        ))
    }

    /// One form as a real return (not a library sample): open for its first valid period,
    /// answer the asks the host offers, take the income-tax election when validation asks
    /// for it, edit, check, save — the non-sample half of the lifecycle.
    fn exercise_new(code: &str, period: i64) -> Result<String, String> {
        let target = FormTarget {
            tin: "00012345300000".to_owned(),
            form_code: code.to_owned(),
            year: 2026,
            period,
            key: None,
            mode: "return".to_owned(),
            snapshot_id: None,
            new_key: None,
            new_key_from: None,
        };
        let mut s = FormSession::new(target);
        if s.schema.is_none() {
            return Err(format!("schema: {}", s.st.error));
        }
        if let Some(e) = s.open(None, false) {
            return Err(format!("open refused: {e}"));
        }
        settle(&mut s, "open")?;
        if !s.st.error.is_empty() {
            return Err(format!("open error: {}", s.st.error));
        }
        if s.st.draft.is_none() {
            return Err("opened with no draft".to_owned());
        }

        // Answer every ask the host offers: a choice takes its first answer, a date a
        // typed date, anything else a short text — through the same save_ask path the UI uses.
        let asks = s.st.asks.clone();
        let mut answered = 0usize;
        for ask in &asks {
            match ask.control.as_str() {
                "choice" => {
                    if let Some(c) = ask.choices.as_ref().and_then(|c| c.first()) {
                        s.st.ask_pick.insert(ask.fact.clone(), c.value.clone());
                        s.save_ask(&ask.fact);
                        answered += 1;
                    }
                }
                "date" => {
                    s.st.ask_text
                        .insert(ask.fact.clone(), "01/15/2026".to_owned());
                    s.save_ask(&ask.fact);
                    answered += 1;
                }
                _ => {
                    s.st.ask_text
                        .insert(ask.fact.clone(), "Test answer".to_owned());
                    s.save_ask(&ask.fact);
                    answered += 1;
                }
            }
        }
        settle(&mut s, "asks")?;

        // Validation can ask for the year's income-tax election; answer it when it does.
        s.check();
        settle(&mut s, "check")?;
        let mut election = "none".to_owned();
        if s.st.election_at.is_some() {
            s.pick_election("graduated");
            s.record_election();
            election = "graduated".to_owned();
            settle(&mut s, "election")?;
        }

        // One edit of the first editable text field, then check and save for real.
        let mut edits = 0usize;
        let first_text = s
            .all_keys
            .iter()
            .find(|k| s.kind_of.get(*k).is_some_and(|t| t == "text"))
            .cloned();
        if let Some(k) = first_text {
            s.edit(&k, "Tester");
            edits += 1;
        }
        s.flush();
        s.check();
        settle(&mut s, "check")?;
        s.save_now();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            s.poll();
            let quiescent = !s.st.checking
                && !matches!(s.st.save_state, SaveState::Saving)
                && s.pending_open.is_none()
                && s.pending_validate.is_none()
                && s.pending_period.is_none()
                && s.pending_placeholder.is_none()
                && bir::async_in_flight() == 0;
            if quiescent {
                break;
            }
            if Instant::now() > deadline {
                return Err("save wait timed out".to_owned());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let SaveState::Failed(m) = &s.st.save_state {
            return Err(format!("save failed: {m}"));
        }
        Ok(format!(
            "asks={answered}/{} election={election} edits={edits} save={:?}",
            asks.len(),
            s.st.save_state
        ))
    }

    /// Every form in the bundled schema index opens a sample, steps, edits, validates and
    /// autosaves on a scratch database — the engine-level half of the all-43 audit.
    #[test]
    fn all_form_sessions_open_edit_and_check() {
        let folder = tempfile::tempdir().expect("temp folder");
        let database = folder.path().join("data").join("bir_data.db");
        dev_env(&database, folder.path(), || {
            bir::call(
                "create_taxpayer",
                vec![json!({
                    "tin_root": "000-123-453",
                    "name": "Juana Santos",
                    "taxpayer_type": "Individual",
                    "head_office": {
                        "rdo_code": "039",
                        "registered_address": "Quezon City",
                        "zip_code": "1100"
                    }
                })],
            )
            .expect("create_taxpayer");
            let text = std::fs::read_to_string("../app/forms/forms-schema.json")
                .expect("bundled schema index");
            let forms: Vec<Value> = serde_json::from_str(&text).unwrap();
            assert_eq!(forms.len(), 43, "the bundled schema index lists every form");
            let now = chrono::Local::now();
            let mut failures = Vec::new();
            for f in &forms {
                let code = f["c"].as_str().unwrap().to_owned();
                match exercise(&code) {
                    Ok(report) => eprintln!("{code}\tsample\tOK\t{report}"),
                    Err(e) => {
                        eprintln!("{code}\tsample\tFAIL\t{e}");
                        failures.push(format!("{code} sample: {e}"));
                    }
                }
                let freq =
                    crate::lib::periods::Frequency::parse(f["freq"].as_str().unwrap_or("Annual"));
                let period = crate::lib::periods::period_options(&code, freq, 2026, now)
                    .first()
                    .map(|o| o.value)
                    .unwrap_or(0);
                match exercise_new(&code, period) {
                    Ok(report) => eprintln!("{code}\treturn\tOK\t{report}"),
                    Err(e) => {
                        eprintln!("{code}\treturn\tFAIL\t{e}");
                        failures.push(format!("{code} return: {e}"));
                    }
                }
            }
            assert!(
                failures.is_empty(),
                "form session failures:\n{}",
                failures.join("\n")
            );
        });
    }
}
