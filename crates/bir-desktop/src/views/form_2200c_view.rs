//! Editor for BIR Form 2200-C (January 2018), Excise Tax Return for Cosmetic
//! Procedures. Rust owns every calculation, validation and the official
//! submit plaintext (`bir_core::forms::form_2200c`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::excise_places::{self, ExcisePlace};
use bir_core::forms::form_2200c::{FORM_2200C_ROWS, Form2200CDraft, Form2200CManner, Form2200CRow};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::combobox::{ComboboxEvent, ComboboxState};
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{self, messages_for, select_field, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form2200CView {}

const PLACE_FORM: &str = "2200C";

/// Where Items 5 and 6 (TIN, RDO) are fixed: they have no input on this page.
const PROFILE_NOTE: &str = "These come from your taxpayer profile. Edit them in Profile Settings.";

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
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("day", "Item 1 — Day", "DD"),
    ("name", "Item 7 — Taxpayer's Name", ""),
    ("address", "Item 8 — Registered Address", ""),
    ("zip", "Item 8A — Zip Code", ""),
    ("phone", "Item 9 — Contact Number", "digits only"),
    ("email", "Item 10 — Email Address", ""),
    ("relief", "Item 12A — If yes, specify", ""),
    ("other", "Item 15 — Other manner of payment (specify)", ""),
];

/// Whole-peso inputs outside Schedule 1: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("bal", "17A — Balance carried over from previous return"),
    ("cred", "17B — Creditable excise tax, if applicable"),
    (
        "prev",
        "19 — Payment on returns previously filed (amended only)",
    ),
    ("sur", "21A — Surcharge"),
    ("int", "21B — Interest"),
    ("comp", "21C — Compromise"),
    ("pay", "23A — Tax payment / deposit"),
];

/// Schedule 1 row inputs: (prefix, label).
const ROW_INPUTS: &[(&str, &str)] = &[
    ("ca", "(A) Procedures — exempt"),
    ("cb", "(B) Procedures — excisable"),
    ("cc", "(C) Procedures — non-excisable"),
    ("gd", "(D) Non-invasive, net of VAT"),
    ("ge", "(E) Invasive excisable, net of VAT and excise"),
    ("gf", "(F) Invasive excisable, VAT exempt"),
    ("gg", "(G) Invasive non-excisable, net of VAT"),
];

/// Row keys end in the row number (0–9).
fn row_key(prefix: &str, row: usize) -> String {
    format!("{prefix}{row}")
}

/// Accepts `1,234`, `1234.00`, blank (zero).
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
        official_amount(value)
    }
}

fn row_value(row: &Form2200CRow, prefix: &str) -> f64 {
    match prefix {
        "ca" => row.exempt_procedures,
        "cb" => row.excisable_procedures,
        "cc" => row.non_excisable_procedures,
        "gd" => row.net_of_vat,
        "ge" => row.excisable_net,
        "gf" => row.excisable_vat_exempt,
        _ => row.non_excisable,
    }
}

fn set_row_value(row: &mut Form2200CRow, prefix: &str, value: f64) {
    match prefix {
        "ca" => row.exempt_procedures = value,
        "cb" => row.excisable_procedures = value,
        "cc" => row.non_excisable_procedures = value,
        "gd" => row.net_of_vat = value,
        "ge" => row.excisable_net = value,
        "gf" => row.excisable_vat_exempt = value,
        _ => row.non_excisable = value,
    }
}

/// Level of the Item 11 place dropdowns.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceLevel {
    Region,
    Province,
    City,
}

/// Region, province and city dropdowns for Item 11.
struct PlaceSelects {
    region: Entity<ComboboxState>,
    province: Entity<ComboboxState>,
    city: Entity<ComboboxState>,
}

pub struct Form2200CView {
    draft: Form2200CDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    place: PlaceSelects,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2200CView {
    fn initial(draft: &Form2200CDraft, key: &str) -> String {
        match key {
            "day" => format!("{:02}", draft.day),
            "year" => draft.year.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "relief" => draft.tax_relief_specify.clone(),
            "other" => draft.manner_other.clone(),
            "bal" => money_text(draft.balance_carried_over),
            "cred" => money_text(draft.creditable_excise_tax),
            "prev" => money_text(draft.previous_payment),
            "sur" => money_text(draft.surcharge),
            "int" => money_text(draft.interest),
            "comp" => money_text(draft.compromise),
            "pay" => money_text(draft.tax_payment),
            _ => {
                let (prefix, index) = key.split_at(key.len().saturating_sub(1));
                index
                    .parse::<usize>()
                    .ok()
                    .and_then(|row| draft.schedule.get(row))
                    .map(|row| money_text(row_value(row, prefix)))
                    .unwrap_or_default()
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
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
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
            // An out-of-range day is kept (capped to the field's range) so
            // validate() raises the official valid-date alert, not "required".
            draft.day = day.clamp(0.0, f64::from(u8::MAX)) as u8;
        }
        // Item 1 month and year come from the library tile (read-only).
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
                _ => draft.tax_payment = value,
            }
        }
        if matches!(
            draft.manner,
            Form2200CManner::ActualRemoval | Form2200CManner::Other
        ) {
            let mut rows = Vec::new();
            for row in 0..FORM_2200C_ROWS {
                let mut entry = Form2200CRow::default();
                for (prefix, label) in ROW_INPUTS {
                    if let Some(value) = number(&row_key(prefix, row), label, cx) {
                        set_row_value(&mut entry, prefix, value);
                    }
                }
                rows.push(entry);
            }
            while rows.last().is_some_and(|r| *r == Form2200CRow::default()) {
                rows.pop();
            }
            draft.schedule = rows;
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.tax_relief_specify = self.input_text("relief", cx);
        draft.manner_other = self.input_text("other", cx);
        draft.recompute();
        draft.lifecycle.updated_at = self.draft.lifecycle.updated_at.clone();
        let changed = draft != self.draft;
        if changed {
            draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        }
        if changed || parse_errors != self.parse_errors {
            self.validated = false;
        }
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2200CDraft)) {
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
        change: impl FnOnce(&mut Form2200CDraft),
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

    /// The official choices for one level, given the levels above it.
    fn place_options(place: &ExcisePlace, level: PlaceLevel) -> Vec<(&'static str, &'static str)> {
        match level {
            PlaceLevel::Region => excise_places::regions(PLACE_FORM),
            PlaceLevel::Province => excise_places::provinces(PLACE_FORM, &place.region),
            PlaceLevel::City => excise_places::cities(PLACE_FORM, &place.region, &place.province),
        }
    }

    /// A new region clears province and city; a new province clears the city.
    fn pick_place(&mut self, level: PlaceLevel, label: &str, cx: &mut Context<Self>) {
        let Some(code) = Self::place_options(&self.draft.place, level)
            .into_iter()
            .find(|(_, option)| *option == label)
            .map(|(code, _)| code)
        else {
            return;
        };
        self.edit(cx, |d| match level {
            PlaceLevel::Region => {
                d.place = ExcisePlace {
                    region: code.to_string(),
                    ..Default::default()
                }
            }
            PlaceLevel::Province => {
                d.place.province = code.to_string();
                d.place.city.clear();
            }
            PlaceLevel::City => d.place.city = code.to_string(),
        });
    }

    fn new_place_select(
        level: PlaceLevel,
        place: &ExcisePlace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<ComboboxState>, Subscription) {
        let state = form_controls::new_select_state(&Self::place_options(place, level), window, cx);
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |this: &mut Self, _, event: &ComboboxEvent, _, cx| {
                if let Some(label) = event.selected.as_deref() {
                    this.pick_place(level, label, cx);
                }
            },
        );
        (state, subscription)
    }

    fn new_place_selects(
        place: &ExcisePlace,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> PlaceSelects {
        let mut make = |level| {
            let (state, subscription) = Self::new_place_select(level, place, window, cx);
            subscriptions.push(subscription);
            state
        };
        PlaceSelects {
            region: make(PlaceLevel::Region),
            province: make(PlaceLevel::Province),
            city: make(PlaceLevel::City),
        }
    }

    /// Make every dropdown show the draft's current value and choices.
    fn sync_controls(&self, window: &mut Window, cx: &mut App) {
        let place = &self.draft.place;
        for (state, level, value) in [
            (&self.place.region, PlaceLevel::Region, &place.region),
            (&self.place.province, PlaceLevel::Province, &place.province),
            (&self.place.city, PlaceLevel::City, &place.city),
        ] {
            let options = Self::place_options(place, level);
            let value = (!value.is_empty()).then_some(value.as_str());
            form_controls::sync_select(state, &options, value, window, cx);
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200CDraft>(
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
                .release_abandoned_claimed_queueable::<Form2200CDraft>(
                    &draft.tin,
                    draft.year,
                    period_column(&draft.filing_period()),
                    ABANDONED_CLAIM_RELEASE_REASON,
                )
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.release_claim_confirm_open = false;
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

    /// Validator keys (`Form2200CDraft::validate`) behind input `key`.
    fn model_keys(key: &str) -> &'static [&'static str] {
        match key {
            "day" => &["day"],
            "year" => &["year"],
            "name" => &["taxpayer_name"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "phone" => &["contact_number"],
            "email" => &["email"],
            "relief" => &["tax_relief_specify"],
            "other" => &["manner_other"],
            // The whole-number rule ("amounts") is shown once per section.
            _ => &[],
        }
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
        let mut keys = vec![key];
        keys.extend_from_slice(Self::model_keys(key));
        let message = self.errors_for(&keys);
        // A Schedule 1 row message sits under its row card; its inputs only
        // turn red so the failing row is easy to find.
        let row_flagged = key
            .chars()
            .last()
            .and_then(|c| c.to_digit(10))
            .filter(|_| {
                ROW_INPUTS
                    .iter()
                    .any(|(p, _)| key.len() == p.len() + 1 && key.starts_with(p))
            })
            .is_some_and(|row| {
                let row_key = format!("schedule[{row}]");
                self.errors_for(&[row_key.as_str()]).is_some()
            });
        let label = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if message.is_some() || row_flagged {
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
                <div flex_none text_right font_weight={FontWeight::BOLD}>{official_amount(value)}</div>
            </div>
        }
        .into_any_element()
    }

    /// Question label plus option chips. Phone stacks the label above a
    /// full-width wrapping chip row; other layouts keep one wrapping row.
    fn label_row(label: &str, layout: Layout, choices: Vec<Button>) -> AnyElement {
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w_full()
                .child(
                    div()
                        .min_w_0()
                        .max_w_full()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.to_string()),
                )
                .child(div().flex().flex_wrap().gap_2().w_full().children(choices))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .max_w_full()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.to_string()),
                )
                .children(choices)
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
        button.disabled(disabled).min_w_0().max_w_full()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let read_only_row = |label: &str, value: String, key: &str| {
            let shown = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .when(self.errors_for(&[key]).is_some(), |d| {
                            d.text_color(gpui::red())
                        })
                        .child(label.to_string()),
                )
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element();
            self.with_errors(shown, &[key])
        };
        let month_name = MONTHS
            .get(usize::from(self.draft.month).wrapping_sub(1))
            .copied()
            .unwrap_or("-");
        let amended = self.draft.is_amended;
        let children = vec![
            read_only_row("Item 1 — Month", month_name.to_string(), "month"),
            self.field("day", "Item 1 — Day", layout, !editable),
            read_only_row("Item 1 — Year", self.draft.year.to_string(), "year"),
            Self::label_row(
                "Item 2 — Amended return?",
                layout,
                vec![
                    Self::choice("2200c_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("2200c_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
            ),
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 3 — ATC"),
                )
                .child(div().font_weight(FontWeight::BOLD).child("XC010"))
                .into_any_element(),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_place(&self, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let place = &self.draft.place;
        // Each level reports its own validator key under its own dropdown.
        let level = |label: &str,
                     state: &Entity<ComboboxState>,
                     level: PlaceLevel,
                     value: &str,
                     disabled: bool,
                     error_key: &str| {
            let options = Self::place_options(place, level);
            let value = (!value.is_empty()).then_some(value);
            let message = self.errors_for(&[error_key]);
            select_field(
                label,
                state,
                &options,
                value,
                disabled,
                message.as_deref(),
                cx,
            )
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Item 11 — Place where the invasive cosmetic procedures took place"),
            )
            .child(level(
                "Region",
                &self.place.region,
                PlaceLevel::Region,
                &place.region,
                !editable,
                "place.region",
            ))
            .child(level(
                "Province",
                &self.place.province,
                PlaceLevel::Province,
                &place.province,
                !editable || place.region.is_empty(),
                "place.province",
            ))
            .child(level(
                "City",
                &self.place.city,
                PlaceLevel::City,
                &place.city,
                !editable || place.province.is_empty(),
                "place.city",
            ))
            .into_any_element()
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let relief = self.draft.tax_relief;
        let mut fields = vec![
            self.with_errors(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Item 5 — TIN"),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format_tin(&self.draft.tin)),
                    )
                    .into_any_element(),
                &["tin"],
            ),
            self.with_errors(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Item 6 — RDO Code"),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(self.draft.rdo_code.clone()),
                    )
                    .into_any_element(),
                &["rdo_code"],
            ),
        ];
        if self.errors_for(&["tin", "rdo_code"]).is_some() {
            fields.push(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(PROFILE_NOTE)
                    .into_any_element(),
            );
        }
        for key in ["name", "address", "zip", "phone", "email"] {
            let label = TEXT_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, layout, !editable));
        }
        let relief_row = self.with_errors(
            Self::label_row(
                "Item 12 — Are you availing of tax relief under Special Law or International Tax Treaty?",
                layout,
                vec![
                    Self::choice("2200c_relief_yes", "Yes", relief == Some(true), !editable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_relief = Some(true))
                        })),
                    Self::choice("2200c_relief_no", "No", relief == Some(false), !editable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_relief = Some(false))
                        })),
                ],
            ),
            &["tax_relief"],
        );
        let mut children = vec![self.grid(layout, fields), self.render_place(cx), relief_row];
        if relief == Some(true) {
            children.push(self.field("relief", TEXT_INPUTS[6].1, layout, !editable));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn render_manner(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let manner = self.draft.manner;
        let mut buttons = Vec::new();
        for (index, (value, label)) in [
            (
                Form2200CManner::ActualRemoval,
                "13 — Payment on Actual Performance of Services",
            ),
            (
                Form2200CManner::Prepayment,
                "14 — Prepayment / Advance Deposit",
            ),
            (Form2200CManner::Other, "15 — Other Similar Schemes"),
        ]
        .into_iter()
        .enumerate()
        {
            buttons.push(
                Self::choice(("2200c_manner", index), label, manner == value, !editable).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.edit_and_reload(window, cx, |d| d.manner = value)
                    }),
                ),
            );
        }
        let mut children = vec![self.with_errors(
            Self::label_row("Manner of payment", layout, buttons),
            &["manner"],
        )];
        if manner == Form2200CManner::Other {
            children.push(self.field("other", TEXT_INPUTS[7].1, layout, !editable));
        }
        self.section("Part II — Manner of Payment", children, cx)
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..FORM_2200C_ROWS {
            let entry = self.draft.schedule.get(row).cloned().unwrap_or_default();
            let mut fields: Vec<AnyElement> = ROW_INPUTS
                .iter()
                .map(|(prefix, label)| self.field(&row_key(prefix, row), label, layout, !editable))
                .collect();
            fields.push(self.computed("(H) Excise tax = (E + F) × 5%", entry.excise_tax, cx));
            fields.push(self.computed("(I) VAT = (D + E + G + H) × 12%", entry.vat, cx));
            fields.push(self.computed("(J) Total amount billed", entry.total_billed, cx));
            let row_error = format!("schedule[{row}]");
            children.push(
                self.with_errors(
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
                                .font_weight(FontWeight::BOLD)
                                .child(format!("Row {}", row + 1)),
                        )
                        .child(self.grid(layout, fields))
                        .into_any_element(),
                    &[row_error.as_str()],
                ),
            );
        }
        children.push(self.with_errors(
            self.computed("Total excise tax (to Item 16)", self.draft.excise_total, cx),
            &["schedule", "amounts"],
        ));
        self.section(
            "Part V — Schedule 1 – Summary of Cosmetic Procedures Performed",
            children,
            cx,
        )
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let children = vec![
            self.computed("16 — Excise tax due", d.excise_tax_due, cx),
            self.field("bal", MONEY_INPUTS[0].1, layout, !editable),
            self.field("cred", MONEY_INPUTS[1].1, layout, !editable),
            self.computed("17C — Total", d.total_credits, cx),
            self.computed("18 — Net tax due/(overpayment)", d.net_tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed("20 — Tax still due/(overpayment)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[3].1, layout, !editable),
            self.field("int", MONEY_INPUTS[4].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[5].1, layout, !editable),
            self.computed("21D — Total penalties", d.total_penalties, cx),
            self.computed("22 — Amount payable", d.amount_payable, cx),
            self.field("pay", MONEY_INPUTS[6].1, layout, !editable),
            self.computed("23B — Penalties", d.penalties_paid, cx),
            self.computed("23C — Total payment made", d.total_payment, cx),
            self.with_errors(
                self.computed(
                    "24 — Balance to be carried over to next return",
                    d.balance_carried_forward,
                    cx,
                ),
                &["balance_carried_forward"],
            ),
        ];
        self.section(
            "Part III — Payments and Application",
            vec![self.with_errors(self.grid(layout, children), &["amounts"])],
            cx,
        )
    }
}

impl QueueableFormView for Form2200CView {
    type Draft = Form2200CDraft;

    fn new(
        draft: Form2200CDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0".into()));
        }
        for row in 0..FORM_2200C_ROWS {
            for (prefix, _) in ROW_INPUTS {
                keys.push((row_key(prefix, row), "0".into()));
            }
        }
        for (key, placeholder) in keys {
            let value = Self::initial(&draft, &key);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
            inputs.insert(key, input);
        }
        let place = Self::new_place_selects(&draft.place, window, cx, &mut subscriptions);
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            place,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            header: FormHeaderState::new(),
            validated: false,
            _subscriptions: subscriptions,
        }
    }

    /// The library tile's month (`period`) sets Item 1; Month stays locked.
    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2200CDraft {
        Form2200CDraft::new_for_month(profile, year, period)
    }

    /// One return per Item 1 date (`MMDD`). Reopen the latest saved return
    /// in the tile's month (any month when the tile has none), else start a
    /// new one in that month.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2200CDraft {
        let tin = profile.tin.full();
        let months = if (1..=12).contains(&period) {
            i64::from(period)..=i64::from(period)
        } else {
            1..=12i64
        };
        months
            .flat_map(|month| (1..=31i64).map(move |day| month * 100 + day))
            .filter_map(|key| {
                db.get_queueable_draft::<Form2200CDraft>(&tin, year, key)
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

impl FormViewTrait for Form2200CView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2200-C"
    }
    fn form_subtitle(&self) -> &'static str {
        "Excise Tax Return for Cosmetic Procedures"
    }
    fn form_version(&self) -> &'static str {
        "January 2018 (ENCS)"
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
                    "2200-C draft saved.".into(),
                    window,
                    cx,
                );
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
        if !can_queue_for_submission(Form2200CDraft::FORM_CODE) {
            self.status_message =
                Some("2200-C is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 2200-C. No submission was started: {error}"
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 2200-C queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2200-C payment status needs a verified confirmation workflow.".into());
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
                    "Form 2200-C reverted to Draft.".into(),
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
        let fields = self.draft.to_print_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "2200c-2018",
            &fields,
            "2200-C — Print Preview",
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

impl Render for Form2200CView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        self.sync_controls(window, cx);
        let open_issues = self.validation_errors.len() + self.parse_errors.len();
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let mut issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
            .take(20)
            .map(|(_, message)| {
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.clone()}</div> }
                    .into_any_element()
            })
            .collect();
        if open_issues > 20 {
            let more = format!("+{} more", open_issues - 20);
            issues.push(
                rsx! { <div text_sm text_color={cx.theme().danger}>{more}</div> }
                    .into_any_element(),
            );
        }

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
                .child(Button::new("2200c_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(header_buttons);

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
                            Button::new("2200c_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2200c_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_manner(layout, cx))
            .child(self.render_totals(layout, cx));
        if matches!(
            self.draft.manner,
            Form2200CManner::ActualRemoval | Form2200CManner::Other
        ) {
            body = body.child(self.render_schedule(layout, cx));
        }
        body = body.children(crate::views::event_form_kit::declaration_and_payment(
            layout == Layout::Phone,
            "Part IV – Details of Payment",
            crate::views::event_form_kit::PAYMENT_ROWS_2200,
            crate::views::event_form_kit::PAYMENT_COLUMNS_2200,
            cx,
        ));
        if !issues.is_empty() {
            body = body.child(self.section("Needs review", issues, cx));
        }

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
                    .id("2200c_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form2200CView {
    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "2200c",
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
    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200CDraft>(
                        &self.draft.tin,
                        self.draft.year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "2200-C marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form2200CView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form2200CView::blocking_errors,
    on_validate: Form2200CView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form2200CView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_2200c_view.rs");
        assert!(src.contains(concat!("render_header_", "buttons(")));
        assert!(src.contains(concat!("render_status_", "box(")));
        assert!(src.contains(concat!("revert_for_", "amendment(")));
        assert!(!src.contains(concat!("Queued for background ", "submission")));
        assert!(!src.contains(concat!("\"Queue for ", "submission\"")));
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
