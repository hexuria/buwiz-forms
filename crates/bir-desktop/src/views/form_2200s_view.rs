//! Editor for BIR Form 2200-S (January 2018), Excise Tax Return for
//! Sweetened Beverages. Rust owns every calculation, validation and the
//! official submit plaintext (`bir_core::forms::form_2200s`); this view only
//! edits source values. The layout reflows for desktop, tablet and phone.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::excise_places::{self, ExcisePlace};
use bir_core::forms::form_2200s::{
    FORM_2200S_BASIC_TAX_LIMIT_ALERT, FORM_2200S_BEVERAGES, Form2200SDraft, Form2200SManner,
    Form2200SOtherRow, Form2200STaxRelief, round_down_computation_text, round_down_input_text,
};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use super::event_form_kit as kit;
use crate::components::combobox::{ComboboxEvent, ComboboxState};
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{self, messages_for, select_field, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form2200SView {}

const PLACE_FORM: &str = "2200S";

/// Width classes the page lays out for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    Phone,
    Tablet,
    Desktop,
}

impl Layout {
    fn for_width(width: Pixels) -> Self {
        if width < px(700.) {
            Self::Phone
        } else if width < px(1100.) {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("day", "Item 1 — Day", "DD"),
    ("sheets", "Item 3 — Number of Sheet/s Attached", "0"),
    ("name", "Item 6 — Taxpayer's Name", ""),
    ("address", "Item 7 — Registered Address", ""),
    ("zip", "Item 7A — Zip Code", "4 digits"),
    ("phone", "Item 8 — Contact Number", "digits only"),
    ("lob", "Item 9 — Main Line of Business", ""),
    ("psic", "Item 10 — PSIC", "4 to 6 digits"),
    ("email", "Item 11 — Email Address", ""),
    ("scheme", "Item 17 — Other similar scheme (specify)", ""),
];

/// Columns of an "Others" row: (key prefix, label, placeholder). The input
/// key is the prefix plus the row index (`oatc0`, `oatc1`, ...).
const OTHER_COLUMNS: &[(&str, &str, &str)] = &[
    ("oatc", "ATC (XB + 3 digits)", "XB"),
    ("odesc", "Description", ""),
    ("obracket", "Tax bracket / unit of measure", ""),
    ("orate", "Applicable tax rate", "0.00"),
    ("osales", "Sales value (in peso)", "0.00"),
    ("ovol", "Volume of removals", "0.00"),
];

/// Money inputs: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("bal", "19A — Balance carried over from previous return"),
    ("cred", "19B — Creditable excise tax, if applicable"),
    (
        "prev",
        "21 — Payment on returns previously filed (amended only)",
    ),
    ("sur", "23A — Surcharge (prepayment only)"),
    ("int", "23B — Interest (prepayment only)"),
    ("comp", "23C — Compromise (prepayment only)"),
    ("pay", "25A — Tax payment / deposit"),
];

fn row_key(prefix: &str, row: usize) -> String {
    format!("{prefix}{row}")
}

/// Accepts `1,234.56`, `1234.5`, blank (zero).
fn parse_amount(value: &str) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Some(0.0);
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        round_down_input_text(value)
    }
}

/// Level of a place dropdown (Items 12 and 13).
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceLevel {
    Region,
    Province,
    City,
}

const PLACE_LEVELS: [PlaceLevel; 3] = [PlaceLevel::Region, PlaceLevel::Province, PlaceLevel::City];

/// Index into `Form2200SView::places` of item 12 or 13 at `level`.
fn place_index(item: u8, level: PlaceLevel) -> usize {
    usize::from(item == 13) * 3 + level as usize
}

pub struct Form2200SView {
    draft: Form2200SDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Region, province and city dropdowns of Item 12, then of Item 13.
    places: Vec<Entity<ComboboxState>>,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2200SView {
    fn initial(draft: &Form2200SDraft, key: &str) -> String {
        if let Some((column, row)) = OTHER_COLUMNS.iter().find_map(|(prefix, _, _)| {
            key.strip_prefix(prefix)
                .and_then(|rest| rest.parse::<usize>().ok())
                .map(|row| (*prefix, row))
        }) {
            let other = draft.others.get(row).cloned().unwrap_or_default();
            return match column {
                "oatc" => other.atc,
                "odesc" => other.description,
                "obracket" => other.tax_bracket,
                "orate" => money_text(other.rate),
                "osales" => money_text(other.sales_value),
                _ => money_text(other.volume),
            };
        }
        match key {
            "day" => format!("{:02}", draft.day),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "lob" => draft.line_of_business.clone(),
            "psic" => draft.psic.clone(),
            "email" => draft.email.clone(),
            "scheme" => draft.other_scheme_text.clone(),
            "bal" => money_text(draft.balance_carried_over),
            "cred" => money_text(draft.creditable_excise_tax),
            "prev" => money_text(draft.previous_payment),
            "sur" => money_text(draft.surcharge),
            "int" => money_text(draft.interest),
            "comp" => money_text(draft.compromise),
            "pay" => money_text(draft.tax_payment),
            _ => {
                let (prefix, index) = key.split_at(key.len().saturating_sub(1));
                let row = index.parse::<usize>().ok();
                let entry = row.and_then(|row| draft.beverages.get(row));
                match (prefix, entry) {
                    ("bsales", Some(entry)) => money_text(entry.sales_value),
                    ("bvol", Some(entry)) => money_text(entry.volume),
                    _ => String::new(),
                }
            }
        }
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Read every editor value into the draft, then recompute and validate.
    /// True when a Schedule 1 row was reset for passing the Basic Excise Tax
    /// Due limit (its editors then need [`Self::reload_schedule_inputs`]).
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.draft.lifecycle.is_editable() {
            return false;
        }
        let mut parse_errors = Vec::new();
        let mut number = |key: &str, label: &str, cx: &App| -> Option<f64> {
            let text = self.input_text(key, cx);
            let parsed = parse_amount(&text);
            if parsed.is_none() {
                parse_errors.push((
                    key.to_string(),
                    format!("{label}: \"{text}\" is not a number."),
                ));
            }
            parsed
        };
        let mut draft = self.draft.clone();
        if let Some(day) = number("day", "Item 1 day", cx) {
            draft.day = if (0.0..=99.0).contains(&day) {
                day as u8
            } else {
                0
            };
        }
        if let Some(sheets) = number("sheets", "Item 3", cx) {
            // `txtSheets` has maxlength 2.
            draft.number_of_attached_sheets = if (0.0..=99.0).contains(&sheets) {
                sheets as u16
            } else {
                0
            };
        }
        for (key, label) in MONEY_INPUTS {
            let Some(value) = number(key, label, cx) else {
                continue;
            };
            match *key {
                "bal" => draft.balance_carried_over = value,
                "cred" => draft.creditable_excise_tax = value,
                "prev" => draft.previous_payment = value,
                "sur" => draft.surcharge = value,
                "int" => draft.interest = value,
                "comp" => draft.compromise = value,
                "pay" => draft.tax_payment = value,
                _ => {}
            }
        }
        if draft.manner == Form2200SManner::ActualRemoval {
            draft.beverages.resize_with(10, Default::default);
            for row in 0..10 {
                if let Some(value) = number(&row_key("bsales", row), "Sales value", cx) {
                    draft.beverages[row].sales_value = value;
                }
                if let Some(value) = number(&row_key("bvol", row), "Volume of removals", cx) {
                    draft.beverages[row].volume = value;
                }
            }
            // Every "Others" row on the page, in order.
            for row in 0..draft.others.len() {
                let n = row + 1;
                let rate = number(&row_key("orate", row), &format!("Others row {n} rate"), cx)
                    .unwrap_or(0.0);
                let sales = number(
                    &row_key("osales", row),
                    &format!("Others row {n} sales value"),
                    cx,
                )
                .unwrap_or(0.0);
                let volume = number(&row_key("ovol", row), &format!("Others row {n} volume"), cx)
                    .unwrap_or(0.0);
                draft.others[row] = Form2200SOtherRow {
                    atc: self
                        .input_text(&row_key("oatc", row), cx)
                        .trim()
                        .to_string(),
                    description: self
                        .input_text(&row_key("odesc", row), cx)
                        .trim()
                        .to_string(),
                    tax_bracket: self
                        .input_text(&row_key("obracket", row), cx)
                        .trim()
                        .to_string(),
                    rate,
                    sales_value: sales,
                    volume,
                    basic_tax_due: 0.0,
                };
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.line_of_business = self.input_text("lob", cx);
        draft.psic = self.input_text("psic", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.other_scheme_text = self.input_text("scheme", cx);
        let reset = draft.recompute_reporting();
        if reset {
            self.status_message = Some(FORM_2200S_BASIC_TAX_LIMIT_ALERT.into());
        }
        draft.lifecycle.updated_at = self.draft.lifecycle.updated_at.clone();
        let changed = draft != self.draft;
        if changed {
            draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        }
        let previous_errors = std::mem::take(&mut self.validation_errors);
        self.validation_errors = draft.validate();
        // validateForm: a one-character day alerts after the year checks.
        if self.input_text("day", cx).trim().chars().count() == 1
            && !self
                .validation_errors
                .iter()
                .any(|(key, _)| matches!(key.as_str(), "month" | "day" | "year"))
        {
            self.validation_errors
                .insert(0, ("day".to_string(), DAY_FORMAT_ALERT.to_string()));
        }
        if changed || parse_errors != self.parse_errors || previous_errors != self.validation_errors
        {
            self.validated = false;
        }
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
        reset
    }

    /// Show the draft's Schedule 1 values again after a row was reset.
    fn reload_schedule_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys: Vec<String> = self
            .inputs
            .keys()
            .filter(|key| {
                key.starts_with("bsales")
                    || key.starts_with("bvol")
                    || OTHER_COLUMNS
                        .iter()
                        .any(|(prefix, _, _)| key.starts_with(prefix))
            })
            .cloned()
            .collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2200SDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        self.validated = false;
        cx.notify();
    }

    /// Choice edits that reshape the schedule reload every editor.
    fn edit_and_reload(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form2200SDraft),
    ) {
        self.edit(cx, change);
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.sync_from_inputs(cx);
    }

    /// One editor per input key, wired to [`Self::sync_from_inputs`].
    fn new_input(
        draft: &Form2200SDraft,
        key: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let value = Self::initial(draft, key);
        let placeholder = placeholder.to_string();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |state, cx| state.set_value(value, window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) && this.sync_from_inputs(cx) {
                    this.reload_schedule_inputs(window, cx);
                }
            },
        );
        (input, subscription)
    }

    /// Editors for every "Others" row the draft holds.
    fn ensure_other_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for row in 0..self.draft.others.len() {
            for (prefix, _, placeholder) in OTHER_COLUMNS {
                let key = row_key(prefix, row);
                if self.inputs.contains_key(&key) {
                    continue;
                }
                let (input, subscription) =
                    Self::new_input(&self.draft, &key, placeholder, window, cx);
                self.inputs.insert(key, input);
                self._subscriptions.push(subscription);
            }
        }
    }

    /// Add row / Remove on the "Others" rows (`addFieldsForSched1` /
    /// `deleteFieldForSched1`): read the page first, change the rows, then
    /// show every row again from the draft.
    fn edit_others(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Vec<Form2200SOtherRow>),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        change(&mut self.draft.others);
        self.ensure_other_inputs(window, cx);
        self.edit_and_reload(window, cx, |_| {});
    }

    fn place_mut(draft: &mut Form2200SDraft, item: u8) -> &mut ExcisePlace {
        if item == 12 {
            &mut draft.place_of_production
        } else {
            &mut draft.place_of_removal
        }
    }

    /// The options a place dropdown lists, given the draft's current place.
    fn place_options(place: &ExcisePlace, level: PlaceLevel) -> Vec<(&'static str, &'static str)> {
        match level {
            PlaceLevel::Region => excise_places::regions(PLACE_FORM),
            PlaceLevel::Province => excise_places::provinces(PLACE_FORM, &place.region),
            PlaceLevel::City => excise_places::cities(PLACE_FORM, &place.region, &place.province),
        }
    }

    fn place_of(&self, item: u8) -> &ExcisePlace {
        if item == 12 {
            &self.draft.place_of_production
        } else {
            &self.draft.place_of_removal
        }
    }

    /// A place dropdown choice: a new region clears province and city, a new
    /// province clears the city.
    fn pick_place(&mut self, item: u8, level: PlaceLevel, label: &str, cx: &mut Context<Self>) {
        let options = Self::place_options(self.place_of(item), level);
        let Some(code) = options.iter().find(|(_, l)| *l == label).map(|(c, _)| *c) else {
            return;
        };
        self.edit(cx, |d| {
            let place = Self::place_mut(d, item);
            match level {
                PlaceLevel::Region => {
                    *place = ExcisePlace {
                        region: code.to_string(),
                        ..Default::default()
                    }
                }
                PlaceLevel::Province => {
                    place.province = code.to_string();
                    place.city.clear();
                }
                PlaceLevel::City => place.city = code.to_string(),
            }
        });
    }

    /// Make every place dropdown list and show the draft's current place.
    fn sync_places(&self, window: &mut Window, cx: &mut Context<Self>) {
        for item in [12u8, 13] {
            let place = self.place_of(item);
            for level in PLACE_LEVELS {
                let value = match level {
                    PlaceLevel::Region => &place.region,
                    PlaceLevel::Province => &place.province,
                    PlaceLevel::City => &place.city,
                };
                form_controls::sync_select(
                    &self.places[place_index(item, level)],
                    &Self::place_options(place, level),
                    Some(value.as_str()).filter(|v| !v.is_empty()),
                    window,
                    cx,
                );
            }
        }
    }

    fn notify(
        &self,
        kind: notification::NotificationType,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(
            notification::Notification::new()
                .message(message)
                .with_type(kind)
                .autohide(true),
            cx,
        );
    }

    /// Cancel a Queued return (claimed: ask to release the claim first).
    fn cancel_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.draft.lifecycle.status, FilingStatus::Queued) {
            self.status_message =
                Some("This return cannot be reverted after submission has started.".into());
            cx.notify();
            return;
        }
        if !self.draft.lifecycle.is_unclaimed() {
            self.release_claim_confirm_open = true;
            self.status_message = Some(
                "Submission was claimed. Confirm nothing reached BIR to return it to an editable Draft. This does not file.".into(),
            );
            cx.notify();
            return;
        }
        let queued = self.draft.clone();
        let canceled = match self.db.lock() {
            Ok(db) => db
                .cancel_queued_queueable(&queued)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match canceled {
            Ok(draft) => {
                self.draft = draft;
                self.status_message = None;
                self.validation_errors = self.draft.validate();
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200SDraft>(
                        &queued.tin,
                        queued.year,
                        period_column(&queued.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Warning,
                    format!("The queued return was not canceled: {error}"),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form2200SDraft>(
                    &draft.tin,
                    draft.year,
                    period_column(&draft.filing_period()),
                    ABANDONED_CLAIM_RELEASE_REASON,
                )
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.release_claim_confirm_open = false;
        self.validated = false;
        match result {
            Ok(AbandonedClaimRelease::Released { draft, .. }) => {
                self.draft = draft;
                self.status_message = None;
                cx.emit(QueueableFormEvent::Saved);
            }
            Ok(AbandonedClaimRelease::AlreadyClear { draft, .. }) => {
                if let Some(draft) = draft {
                    self.draft = draft;
                }
            }
            Err(error) => {
                self.notify(
                    notification::NotificationType::Error,
                    format!("Could not release the claim: {error}"),
                    window,
                    cx,
                );
            }
        }
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    // ── Rendering helpers ──

    fn section(&self, title: &str, children: Vec<AnyElement>, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_col gap_4 p_5 bg={cx.theme().background} border_1 border_color={cx.theme().border} rounded_lg>
                <div text_lg font_weight={FontWeight::BOLD}>{title.to_string()}</div>
                {...children}
            </div>
        }
        .into_any_element()
    }

    /// Validator keys (`Form2200SDraft::validate`) behind input `key`.
    fn model_keys(key: &str) -> Vec<String> {
        let fixed: &[&str] = match key {
            "day" => &["day"],
            "name" => &["taxpayer_name"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "phone" => &["contact_number"],
            "lob" => &["line_of_business"],
            "psic" => &["psic"],
            "email" => &["email"],
            "scheme" => &["other_scheme_text"],
            "bal" => &["balance_carried_over"],
            "cred" => &["creditable_excise_tax"],
            "prev" => &["previous_payment"],
            "sur" => &["surcharge"],
            "int" => &["interest"],
            "comp" => &["compromise"],
            "pay" => &["tax_payment"],
            _ => &[],
        };
        let mut keys: Vec<String> = fixed.iter().map(|k| k.to_string()).collect();
        // "Others" rows: `oatc3` is `others[3].atc`.
        for (prefix, field) in [
            ("oatc", "atc"),
            ("odesc", "description"),
            ("obracket", "tax_bracket"),
            ("orate", "rate"),
            ("ovol", "volume"),
        ] {
            if let Some(row) = key
                .strip_prefix(prefix)
                .and_then(|rest| rest.parse::<usize>().ok())
            {
                keys.push(format!("others[{row}].{field}"));
            }
        }
        keys
    }

    /// Messages for any of `keys`, from malformed input and from validation.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter()),
            keys,
        )
    }

    /// Put the messages for `keys` directly under an already-built control.
    fn with_errors(&self, content: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(content, self.errors_for(keys))
    }

    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let mut keys = Self::model_keys(key);
        keys.push(key.to_string());
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let message = self.errors_for(&key_refs);
        let error = message.is_some();
        let label = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if error {
            label.text_color(gpui::red())
        } else {
            label
        };
        let row = match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(body.w_full()),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.45)))
                .child(body.w(relative(0.55))),
        }
        .into_any_element();
        with_field_error(row, message)
    }

    /// A printed label with its read-only value (the library tile's period,
    /// TIN, RDO), showing any validation message under it.
    fn read_only_row(&self, label: &str, value: String, key: &str) -> AnyElement {
        let message = self.errors_for(&[key]);
        let text = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let text = if message.is_some() {
            text.text_color(gpui::red())
        } else {
            text
        };
        let shown = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(text)
            .child(div().font_weight(FontWeight::BOLD).child(value))
            .into_any_element();
        with_field_error(shown, message)
    }

    fn grid(&self, layout: Layout, children: Vec<AnyElement>) -> AnyElement {
        if layout == Layout::Desktop {
            let mut rows = div().flex().flex_col().gap_3().w_full();
            let mut iter = children.into_iter();
            while let Some(left) = iter.next() {
                let mut row = div()
                    .flex()
                    .gap_6()
                    .w_full()
                    .child(div().flex_1().child(left));
                row = match iter.next() {
                    Some(right) => row.child(div().flex_1().child(right)),
                    None => row.child(div().flex_1()),
                };
                rows = rows.child(row);
            }
            rows.into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .children(children)
                .into_any_element()
        }
    }

    fn computed(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{round_down_computation_text(value)}</div>
            </div>
        }
        .into_any_element()
    }

    /// A question label with its choice chips. On Phone the label sits on its
    /// own line and the chips wrap in a full-width row below it.
    fn label_row(label: &str, layout: Layout, chips: Vec<AnyElement>) -> AnyElement {
        let text = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w_full()
                .child(text)
                .child(div().flex().flex_wrap().gap_2().w_full().children(chips))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(text.flex_1())
                .children(chips)
                .into_any_element()
        }
    }

    fn choice(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: bool,
        disabled: bool,
    ) -> Button {
        let label: SharedString = label.into();
        let button = Button::new(id).label(if selected {
            format!("✓ {label}")
        } else {
            label.to_string()
        });
        let button = if selected {
            button.primary()
        } else {
            button.outline()
        };
        button.disabled(disabled)
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        // Month and year come from the library tile and are read-only; the
        // Day stays editable (the draft is keyed by the Item 1 date).
        let month_name = MONTHS
            .get((self.draft.month as usize).wrapping_sub(1))
            .copied()
            .unwrap_or("-");
        let amended = self.draft.is_amended;
        let children = vec![
            self.read_only_row("Item 1 — Month", month_name.to_string(), "month"),
            self.field("day", "Item 1 — Day", layout, !editable),
            self.read_only_row("Item 1 — Year", self.draft.year.to_string(), "year"),
            self.field(
                "sheets",
                "Item 3 — Number of Sheet/s Attached",
                layout,
                !editable,
            ),
            Self::label_row(
                "Item 2 — Amended return?",
                layout,
                vec![
                    Self::choice("2200s_amended_yes", "Yes", amended, !editable)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                        )
                        .into_any_element(),
                    Self::choice("2200s_amended_no", "No", !amended, !editable)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                        )
                        .into_any_element(),
                ],
            ),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    /// Item 12 or 13: a searchable dropdown per level (Region, Province, City).
    fn render_place(&self, item: u8, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let place = self.place_of(item);
        let (title, key) = if item == 12 {
            ("Item 12 — Place of production", "place_of_production")
        } else {
            ("Item 13 — Place of removal", "place_of_removal")
        };
        let selects: Vec<AnyElement> = [
            (PlaceLevel::Region, "Region", true, &place.region),
            (
                PlaceLevel::Province,
                "Province",
                !place.region.is_empty(),
                &place.province,
            ),
            (
                PlaceLevel::City,
                "City",
                !place.province.is_empty(),
                &place.city,
            ),
        ]
        .into_iter()
        .map(|(level, label, enabled, value)| {
            select_field(
                label,
                &self.places[place_index(item, level)],
                &Self::place_options(place, level),
                Some(value.as_str()).filter(|v| !v.is_empty()),
                !editable || !enabled,
                None,
                cx,
            )
        })
        .collect();
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title.to_string()),
            )
            .child(self.grid(layout, selects))
            .into_any_element();
        self.with_errors(body, &[key])
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let relief = self.draft.tax_relief;
        let mut fields = vec![
            self.read_only_row("Item 4 — TIN", format_tin(&self.draft.tin), "tin"),
            self.read_only_row("Item 5 — RDO Code", self.draft.rdo_code.clone(), "rdo_code"),
        ];
        for key in ["name", "address", "zip", "phone", "lob", "psic", "email"] {
            let label = TEXT_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, layout, !editable));
        }
        let relief_yes = relief != Form2200STaxRelief::No;
        let relief_row = Self::label_row(
            "Item 14 — Are you availing of tax relief under a Special Law or International Tax Treaty?",
            layout,
            vec![
                Self::choice("2200s_relief_yes", "Yes", relief_yes, !editable)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !relief_yes {
                            this.edit(cx, |d| d.tax_relief = Form2200STaxRelief::YesUnspecified)
                        }
                    }))
                    .into_any_element(),
                Self::choice("2200s_relief_no", "No", !relief_yes, !editable)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| d.tax_relief = Form2200STaxRelief::No)
                    }))
                    .into_any_element(),
            ],
        );
        let mut children = vec![
            self.grid(layout, fields),
            self.render_place(12, layout, cx),
            self.render_place(13, layout, cx),
            relief_row,
        ];
        if relief_yes {
            let mut specify: Vec<AnyElement> = Vec::new();
            for (index, (value, label)) in [
                (Form2200STaxRelief::SpecialLaw, "Special Law"),
                (
                    Form2200STaxRelief::InternationalTaxTreaty,
                    "International Tax Treaty",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                specify.push(
                    Self::choice(("2200s_relief", index), label, relief == value, !editable)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.tax_relief = value)
                        }))
                        .into_any_element(),
                );
            }
            children.push(self.with_errors(
                Self::label_row("Item 14A — If yes, specify", layout, specify),
                &["tax_relief"],
            ));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn render_manner(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let manner = self.draft.manner;
        let mut chips: Vec<AnyElement> = Vec::new();
        for (index, (value, label)) in [
            (
                Form2200SManner::ActualRemoval,
                "15 — Payment on Actual Removal",
            ),
            (
                Form2200SManner::Prepayment,
                "16 — Prepayment/Advance Deposit",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            chips.push(
                Self::choice(("2200s_manner", index), label, manner == value, !editable)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_and_reload(window, cx, |d| d.manner = value)
                    }))
                    .into_any_element(),
            );
        }
        let mut children = vec![self.with_errors(
            Self::label_row("Manner of payment", layout, chips),
            &["manner"],
        )];
        if manner == Form2200SManner::Prepayment {
            let scheme = self.draft.other_scheme;
            children.push(Self::label_row(
                "Item 17 — Other similar schemes?",
                layout,
                vec![
                    Self::choice("2200s_scheme", "Yes", scheme, !editable)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.other_scheme = !scheme)
                        }))
                        .into_any_element(),
                ],
            ));
            if scheme {
                children.push(self.field("scheme", TEXT_INPUTS[9].1, layout, !editable));
            }
        }
        self.section("Part II — Manner of Payment", children, cx)
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for (row, beverage) in FORM_2200S_BEVERAGES.iter().enumerate() {
            let due = self
                .draft
                .beverages
                .get(row)
                .map(|r| r.basic_tax_due)
                .unwrap_or(0.0);
            let rate = if beverage.rate == 0.0 {
                "Exempt".to_string()
            } else {
                format!("P {:.2} per liter", beverage.rate)
            };
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .min_w_0()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{} — {}", beverage.atc, beverage.description)),
                    )
                    .child(div().min_w_0().text_sm().child(format!("Rate: {rate}")))
                    .child(self.grid(
                        layout,
                        vec![
                            self.field(
                                &row_key("bsales", row),
                                "Sales value (in peso)",
                                layout,
                                !editable,
                            ),
                            self.field(
                                &row_key("bvol", row),
                                "Volume of removals (liters)",
                                layout,
                                !editable,
                            ),
                            self.computed("Basic excise tax due", due, cx),
                        ],
                    ))
                    .into_any_element(),
            );
        }
        let actual = self.draft.manner == Form2200SManner::ActualRemoval;
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Others (specify)")
                .into_any_element(),
        );
        for (row, other) in self.draft.others.iter().enumerate() {
            let remove = Button::new(("2200s_others_remove", row))
                .disabled(!editable)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_others(window, cx, |rows| {
                        if row < rows.len() {
                            rows.remove(row);
                        }
                    })
                }));
            let fields = OTHER_COLUMNS
                .iter()
                .map(|(prefix, label, _)| {
                    self.field(&row_key(prefix, row), label, layout, !editable)
                })
                .chain(std::iter::once(self.computed(
                    "Basic excise tax due",
                    other.basic_tax_due,
                    cx,
                )))
                .collect();
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(kit::row_heading(&format!("Others row {}", row + 1), remove))
                    .child(self.grid(layout, fields))
                    .into_any_element(),
            );
        }
        children.push(
            kit::add_row_button("2200s_others_add", !editable || !actual)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit_others(window, cx, |rows| {
                        rows.push(Form2200SOtherRow {
                            atc: "XB".into(),
                            ..Default::default()
                        })
                    })
                }))
                .into_any_element(),
        );
        children.push(self.with_errors(
            self.computed(
                "GRAND TOTAL EXCISE TAX DUE (To Item 18 of Part III)",
                self.draft.schedule_total,
                cx,
            ),
            &["beverages"],
        ));
        self.section("Part V — Schedule 1", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let prepayment = d.manner == Form2200SManner::Prepayment;
        let children = vec![
            self.computed(
                "18 — Excise tax due (from Schedule 1)",
                d.excise_tax_due,
                cx,
            ),
            self.field("bal", MONEY_INPUTS[0].1, layout, !editable),
            self.field("cred", MONEY_INPUTS[1].1, layout, !editable),
            self.computed("19C — Total", d.total_credits, cx),
            self.computed("20 — Net tax due/(overpayment)", d.net_tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed("22 — Tax still due/(overpayment)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[3].1, layout, !editable || !prepayment),
            self.field("int", MONEY_INPUTS[4].1, layout, !editable || !prepayment),
            self.field("comp", MONEY_INPUTS[5].1, layout, !editable || !prepayment),
            self.computed("23D — Total penalties", d.total_penalties, cx),
            self.computed("24 — Amount payable", d.amount_payable, cx),
            self.field("pay", MONEY_INPUTS[6].1, layout, !editable),
            self.computed("25B — Penalties paid", d.penalties_paid, cx),
            self.computed("25C — Total payment made today", d.total_payment, cx),
            self.with_errors(
                self.computed(
                    "26 — Balance to be carried over to next return",
                    d.balance_carried_forward,
                    cx,
                ),
                &["balance_carried_forward"],
            ),
        ];
        // "amounts" is raised for any money value in the return.
        let grid = self.with_errors(self.grid(layout, children), &["amounts"]);
        self.section("Part III — Payments and Application", vec![grid], cx)
    }
}

impl QueueableFormView for Form2200SView {
    type Draft = Form2200SDraft;

    const PRINT_SLUG: &'static str = "2200s-2018";

    fn print_fields(draft: &Form2200SDraft) -> std::collections::BTreeMap<String, String> {
        draft.to_bir_field_map()
    }

    fn agent_draft(&self) -> &Form2200SDraft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        HEADER_ACTIONS
    }

    fn new(
        mut draft: Form2200SDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A stored draft shows its totals as the official chain leaves them.
        if draft.lifecycle.is_editable() {
            draft.recompute();
        }
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        for row in 0..10 {
            keys.push((row_key("bsales", row), "0.00".into()));
            keys.push((row_key("bvol", row), "0.00".into()));
        }
        for row in 0..draft.others.len() {
            for (prefix, _, placeholder) in OTHER_COLUMNS {
                keys.push((row_key(prefix, row), placeholder.to_string()));
            }
        }
        for (key, placeholder) in keys {
            let (input, subscription) = Self::new_input(&draft, &key, &placeholder, window, cx);
            subscriptions.push(subscription);
            inputs.insert(key, input);
        }
        // Lists and values are kept in step with the draft by `render`.
        let mut places = Vec::new();
        for item in [12u8, 13] {
            for level in PLACE_LEVELS {
                let state = form_controls::new_select_state(&[], window, cx);
                subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this: &mut Self, _, event: &ComboboxEvent, _, cx| {
                        if let Some(label) = event.selected.as_deref() {
                            this.pick_place(item, level, label, cx);
                        }
                    },
                ));
                places.push(state);
            }
        }
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            places,
            header: FormHeaderState::new(),
            validated: false,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2200SDraft {
        Form2200SDraft::new_from_profile(profile, year, period)
    }

    /// One return per Item 1 date (`MMDD`). Reopen the latest saved return
    /// dated in the tile's month, else start a new one for that month.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2200SDraft {
        let tin = profile.tin.full();
        let month = i64::from(period.clamp(1, 12));
        (1..=31i64)
            .map(|day| month * 100 + day)
            .filter_map(|key| {
                db.get_queueable_draft::<Form2200SDraft>(&tin, year, key)
                    .ok()
                    .flatten()
            })
            .max_by(|a, b| a.lifecycle.updated_at.cmp(&b.lifecycle.updated_at))
            .unwrap_or_else(|| Self::new_draft(profile, year, period))
    }
}

fn format_tin(tin: &str) -> String {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 9 {
        return tin.to_string();
    }
    let branch = digits.get(9..).unwrap_or("");
    format!(
        "{}-{}-{}-{:0>5}",
        &digits[0..3],
        &digits[3..6],
        &digits[6..9],
        branch
    )
}

impl FormViewTrait for Form2200SView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2200-S"
    }
    fn form_subtitle(&self) -> &'static str {
        "Excise Tax Return for Sweetened Beverages"
    }
    fn form_version(&self) -> &'static str {
        "January 2018"
    }
    fn current_status(&self) -> FilingStatus {
        self.draft.lifecycle.status.clone()
    }
    fn submitted_at(&self) -> Option<&str> {
        self.draft.lifecycle.submitted_at.as_deref()
    }
    fn confirmed_at(&self) -> Option<&str> {
        self.draft.lifecycle.confirmed_at.as_deref()
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            self.notify(
                notification::NotificationType::Error,
                "Fix the highlighted numbers before saving.".into(),
                window,
                cx,
            );
            return;
        }
        let result = match self.db.lock() {
            Ok(db) => db
                .save_queueable_draft(&self.draft)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(id) => {
                self.draft.id = Some(id);
                self.notify(
                    notification::NotificationType::Success,
                    "2200-S draft saved.".into(),
                    window,
                    cx,
                );
                // initialValidateBeforeSave: the draft is saved anyway; its
                // first official alert shows in the status line and under
                // Needs review. Queueing still blocks on validation.
                self.status_message = None;
                if let Some((key, alert)) = self.draft.save_alert() {
                    self.status_message = Some(alert.to_string());
                    if !self.validation_errors.iter().any(|(_, m)| m == alert) {
                        self.validation_errors
                            .insert(0, (key.to_string(), alert.to_string()));
                    }
                }
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Save failed".into(),
                error,
            )),
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !can_queue_for_submission(Form2200SDraft::FORM_CODE) {
            self.status_message =
                Some("2200-S is not enabled for in-app submission in this build.".into());
            cx.notify();
            return;
        }
        if !self.draft.lifecycle.is_editable() {
            self.status_message =
                Some("This return is already queued or filed and cannot be queued again.".into());
            cx.notify();
            return;
        }
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() || !self.validation_errors.is_empty() {
            self.status_message =
                Some("Fix the items listed under Needs review before submitting.".into());
            cx.notify();
            return;
        }
        let before = self.draft.clone();
        if let Err(errors) = self.draft.queue(QueueAuthSource::Gui) {
            self.validation_errors = errors;
            self.status_message =
                Some("Fix the items listed under Needs review before submitting.".into());
            cx.notify();
            return;
        }
        let saved = match self.db.lock() {
            Ok(db) => db
                .save_queued_queueable(&self.draft)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = saved {
            self.draft = before;
            self.status_message = Some(format!(
                "Could not queue Form 2200-S. No submission was started: {error}"
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 2200-S queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2200-S payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    /// D7A + D8A: a claimed Queued return keeps the release-claim
    /// confirmation; everything else goes through
    /// `Database::revert_queueable_for_amendment`.
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !can_user_revert(&self.draft.lifecycle.status) {
            return;
        }
        if matches!(self.draft.lifecycle.status, FilingStatus::Queued)
            && !self.draft.lifecycle.is_unclaimed()
        {
            self.cancel_queue(window, cx);
            return;
        }
        match revert_for_amendment(&self.db, &self.draft) {
            Ok(reverted) => {
                self.draft = reverted.draft;
                self.validated = false;
                self.parse_errors.clear();
                self.validation_errors = self.draft.validate();
                self.status_message = reverted
                    .snapshot_id
                    .map(|_| AMENDED_REVERT_MESSAGE.to_string());
                if reverted.snapshot_id.is_some() {
                    self.header.load_sent_snapshot(&self.db, &self.draft);
                }
                self.notify(
                    notification::NotificationType::Success,
                    "Form 2200-S reverted to Draft.".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => self.notify(
                notification::NotificationType::Error,
                format!("Could not revert to Draft: {error}"),
                window,
                cx,
            ),
        }
        // Reload the editors from the reverted draft.
        self.ensure_other_inputs(window, cx);
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                "Fix the highlighted numbers first. No filing state was changed.".into(),
            ));
            return;
        }
        let fields = Self::print_fields(&self.draft);
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            Self::PRINT_SLUG,
            &fields,
            "2200-S — Print Preview",
            cx,
        ) {
            Ok(kind) => cx.emit(QueueableFormEvent::PushNotification(
                "info".into(),
                "Print preview".into(),
                format!("{} No filing state was changed.", kind.status_message()),
            )),
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                format!("{error}. No filing state was changed."),
            )),
        }
    }
}

/// validateForm's alert for a one-character Item 1 day (BIR-Form2200S.hta,
/// rule 2200s-validate-006-day-format).
const DAY_FORMAT_ALERT: &str = "Please enter a valid day on item 1. Format should be MM/DD/YYYY.";

impl Render for Form2200SView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        self.sync_places(window, cx);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
            .map(|(_, message)| {
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.clone()}</div> }
                    .into_any_element()
            })
            .collect();

        let phone = layout == Layout::Phone;
        self.header.ensure_sent_snapshot(&self.db, &self.draft);
        let props = self.header_props();
        let header_buttons = render_header_buttons(self, &props, HEADER_ACTIONS, phone, cx);
        let status_box = render_status_box(self, &props, HEADER_ACTIONS, cx);

        let toolbar =
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px(pad)
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Button::new("2200s_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(header_buttons);

        let needs_review = (!issues.is_empty()).then(|| self.section("Needs review", issues, cx));
        let mut body = div()
            .w_full()
            .max_w(px(1180.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("2200s_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2200s_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .children(needs_review)
            .child(self.render_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_manner(layout, cx))
            .child(self.render_totals(layout, cx));
        if self.draft.manner == Form2200SManner::ActualRemoval {
            body = body.child(self.render_schedule(layout, cx));
        }
        // The printed 2200S sheet uses the plural wording and items 27 to 30.
        body = body.children(
            crate::views::event_form_kit::declaration_and_payment_with_text(
                layout == Layout::Phone,
                "We declare under the penalties of perjury, that this return and all its attachments, has been made in good faith, verified by us, and to the best of our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, we give our consent to the processing of our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes.",
                "Part IV – Details of Payment",
                &[
                    "27 Cash/Bank Debit Memo",
                    "28 Check",
                    "29 Tax Debit Memo",
                    "30 Others (specify)",
                ],
                crate::views::event_form_kit::PAYMENT_COLUMNS_2200,
                cx,
            ),
        );

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(toolbar)
            .child(
                div()
                    .px(pad)
                    .py_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(self.render_header(cx))
                    .when(!phone, |d| {
                        d.child(div().mt_4().child(self.render_status_pipeline(cx)))
                    })
                    // Phone has no stepper; the box sits under the titles.
                    .children(status_box.map(|b| div().mt_3().child(b))),
            )
            .child(
                div()
                    .id("2200s_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form2200SView {
    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "2200s",
            status: self.draft.lifecycle.status.clone(),
            editor_safe: true,
            validated: self.validated,
            blocking_errors: self.blocking_errors(),
            first_error: self
                .parse_errors
                .iter()
                .chain(self.validation_errors.iter())
                .next()
                .map(|(_, message)| message.replace('\n', " ")),
            success_text: DEFAULT_SUCCESS_MESSAGE,
            message: self.status_message.clone(),
            separate_validate: separate_validate_button(&self.db),
        }
    }

    fn blocking_errors(&self) -> usize {
        self.parse_errors.len() + self.validation_errors.len()
    }

    /// The header's Validate: read the editors, run the official checks
    /// and record that they ran at the current entries.
    fn header_validate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        if self.sync_from_inputs(cx) {
            self.reload_schedule_inputs(window, cx);
        }
        self.validated = true;
        cx.notify();
    }

    /// Dev Mode "Simulate BIR confirmation" (debug builds only).
    #[cfg(debug_assertions)]
    fn header_simulate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filename = self.draft.lifecycle.submission_filename.clone();
        match crate::dev_mode::simulate_confirmation(&self.db, filename.as_deref()) {
            Ok(()) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200SDraft>(
                        &self.draft.tin,
                        self.draft.year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "2200-S marked Confirmed (simulated, nothing was sent to BIR).".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => self.notify(
                notification::NotificationType::Error,
                format!("Simulated confirmation failed: {error}"),
                window,
                cx,
            ),
        }
        cx.notify();
    }
}

const HEADER_ACTIONS: FormHeaderActions<Form2200SView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form2200SView::blocking_errors,
    on_validate: Form2200SView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form2200SView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_2200s_view.rs");
        assert!(src.contains(concat!("render_header_", "buttons(")));
        assert!(src.contains(concat!("render_status_", "box(")));
        assert!(src.contains(concat!("revert_for_", "amendment(")));
        assert!(!src.contains(concat!("Queued for background ", "submission")));
        assert!(!src.contains(concat!("\"Queue for ", "submission\"")));
        assert!(!src.contains(concat!("to queue (see Needs ", "review)")));
    }

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn amounts_accept_official_formatting() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }
}
