//! Editor for BIR Form 2553, Return of Percentage Tax Payable Under Special
//! Laws (v1999). Rust owns every calculation, validation and the official
//! submit plaintext (`bir_core::forms::form_2553`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2551q::TaxPeriodBasis;
use bir_core::forms::form_2553::{
    FORM_2553_ATC_OPTIONS, FORM_2553_ATC_ROWS, Form2553Draft, Form2553Overpayment,
    Form2553TaxTreaty,
};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{
    self, field_error, messages_for, select_field, with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form2553View {}

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

/// Item 2 month choices: (month number, label).
const MONTH_OPTIONS: [(&str, &str); 12] = [
    ("1", "January"),
    ("2", "February"),
    ("3", "March"),
    ("4", "April"),
    ("5", "May"),
    ("6", "June"),
    ("7", "July"),
    ("8", "August"),
    ("9", "September"),
    ("10", "October"),
    ("11", "November"),
    ("12", "December"),
];

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 2 — Year ended: year", "YYYY"),
    ("sheets", "Item 5 — Number of sheets attached", "0"),
    ("lob", "Item 8 — Line of Business/Occupation", ""),
    (
        "name",
        "Item 9 — Taxpayer's Name (For Individual) / (For Non-individual)",
        "",
    ),
    ("phone", "Item 10 — Telephone Number", "digits only"),
    ("address", "Item 11 — Registered Address", ""),
    ("zip", "Item 12 — Zip Code", ""),
    ("email", "Email address for the BIR confirmation", ""),
];

/// Money inputs outside the ATC rows: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "prev",
        "20A — Tax paid in return previously filed if this is an Amended Return",
    ),
    ("cwt", "20B — Creditable tax withheld per BIR Form 2307"),
    ("sur", "22A — Surcharge"),
    ("int", "22B — Interest"),
    ("comp", "22C — Compromise"),
];

fn amount_key(row: usize) -> String {
    format!("amt{row}")
}

fn rate_key(row: usize) -> String {
    format!("rate{row}")
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

const CONTENT_MAX_W: Pixels = px(1180.);

pub struct Form2553View {
    draft: Form2553Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    /// Item 2 "Month the year ends" combobox.
    year_end_month: Entity<ComboboxState>,
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

impl Form2553View {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2553Draft, key: &str) -> String {
        let money = |value: f64| {
            if value == 0.0 {
                String::new()
            } else {
                official_amount(value)
            }
        };
        match key {
            "year" => draft.taxable_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "lob" => draft.line_of_business.clone(),
            "name" => draft.taxpayer_name.clone(),
            "phone" => draft.contact_number.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "email" => draft.email.clone(),
            "prev" => money(draft.tax_paid_previous),
            "cwt" => money(draft.creditable_tax_withheld),
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            _ => {
                let row = |prefix: &str| {
                    key.strip_prefix(prefix)
                        .and_then(|n| n.parse::<usize>().ok())
                };
                if let Some(index) = row("amt") {
                    draft
                        .schedule
                        .get(index)
                        .map(|r| money(r.taxable_amount))
                        .unwrap_or_default()
                } else if let Some(index) = row("rate") {
                    draft
                        .schedule
                        .get(index)
                        .filter(|r| r.tax_rate != 0.0)
                        .map(|r| format!("{:.2}", r.tax_rate))
                        .unwrap_or_default()
                } else {
                    String::new()
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
    /// A malformed number is reported instead of becoming zero.
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
        if let Some(year) = number("year", "Item 2 year", cx) {
            draft.taxable_year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
        }
        if let Some(sheets) = number("sheets", "Item 5", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        for (key, label) in MONEY_INPUTS {
            if let Some(value) = number(key, label, cx) {
                match *key {
                    "prev" => draft.tax_paid_previous = value,
                    "cwt" => draft.creditable_tax_withheld = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        for row in 0..draft.schedule.len().min(FORM_2553_ATC_ROWS) {
            let item = 14 + row;
            if let Some(value) = number(&amount_key(row), &format!("{item}C"), cx) {
                draft.schedule[row].taxable_amount = value;
            }
            let editable_rate = FORM_2553_ATC_OPTIONS.iter().any(|option| {
                option.rate_is_editable
                    && option.code == draft.schedule[row].atc_code
                    && option.description == draft.schedule[row].description
            });
            if editable_rate && let Some(value) = number(&rate_key(row), &format!("{item}D"), cx) {
                draft.schedule[row].tax_rate = value;
            }
        }
        draft.line_of_business = self.input_text("lob", cx);
        draft.taxpayer_name = self.input_text("name", cx);
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.recompute();
        draft.lifecycle.updated_at = self.draft.lifecycle.updated_at.clone();
        let changed = draft != self.draft;
        if changed {
            draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        }
        let validation_errors = draft.validate();
        if changed
            || parse_errors != self.parse_errors
            || validation_errors != self.validation_errors
        {
            self.validated = false;
        }
        self.validation_errors = validation_errors;
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2553Draft)) {
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

    fn pick_atc(&mut self, row: usize, option: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(option) = FORM_2553_ATC_OPTIONS.get(option) else {
            return;
        };
        let mut draft = self.draft.clone();
        match draft.set_atc(row, option) {
            Ok(()) => {
                self.draft = draft;
                if let Some(input) = self.inputs.get(&rate_key(row)) {
                    let rate = Self::initial(&self.draft, &rate_key(row));
                    input.update(cx, |state, cx| state.set_value(rate, window, cx));
                }
                self.sync_from_inputs(cx);
            }
            Err(message) => {
                self.status_message = Some(message);
                cx.notify();
            }
        }
    }

    fn clear_atc(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        // Later rows shift up so no empty card is left in the middle.
        if row < self.draft.schedule.len() {
            self.draft.schedule.remove(row);
        }
        self.draft.clear_atc(FORM_2553_ATC_ROWS);
        // Rows after the removed one moved up; reload their editors.
        for index in 0..FORM_2553_ATC_ROWS {
            for key in [amount_key(index), rate_key(index)] {
                if let Some(input) = self.inputs.get(&key) {
                    let value = Self::initial(&self.draft, &key);
                    input.update(cx, |state, cx| state.set_value(value, window, cx));
                }
            }
        }
        self.sync_from_inputs(cx);
    }

    /// On blur, write the official currency format back into a money box,
    /// like the official page's `formatCurrency`. A blank or unparsable box is
    /// left as typed.
    fn reformat_money(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let text = self.input_text(key, cx);
        let Some(value) = parse_amount(&text) else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        let formatted = official_amount(value);
        if formatted == text {
            return;
        }
        if let Some(input) = self.inputs.get(key) {
            input.update(cx, |state, cx| state.set_value(formatted, window, cx));
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

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form2553Draft>(
                    &draft.tin,
                    draft.taxable_year,
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

    /// The validation field names an editor key feeds (for inline errors).
    fn draft_fields(key: &str) -> Vec<&str> {
        let fields: &[&str] = match key {
            "year" => &["taxable_year"],
            "sheets" => &["number_of_attached_sheets"],
            "lob" => &["line_of_business"],
            "name" => &["taxpayer_name"],
            "phone" => &["contact_number"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "prev" => &["tax_paid_previous"],
            "cwt" => &["creditable_tax_withheld"],
            "sur" => &["surcharge"],
            "int" => &["interest"],
            "comp" => &["compromise"],
            _ => &[],
        };
        fields.to_vec()
    }

    /// Validation and parse messages for the given field names, one line.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors.iter().chain(&self.validation_errors),
            keys,
        )
    }

    /// Messages for one editor input (its own key plus the draft fields it feeds).
    fn input_error(&self, key: &str) -> Option<String> {
        let mut keys = vec![key];
        keys.extend(Self::draft_fields(key));
        let row = |prefix: &str| {
            key.strip_prefix(prefix)
                .and_then(|n| n.parse::<usize>().ok())
        };
        let dynamic = if let Some(index) = row("amt") {
            Some(format!("schedule[{index}].taxable_amount"))
        } else {
            row("rate").map(|index| format!("schedule[{index}].tax_rate"))
        };
        if let Some(dynamic) = &dynamic {
            keys.push(dynamic);
        }
        self.error_for(&keys)
    }

    /// A labelled input. On phones the label sits above the field; a
    /// validation message for it shows directly underneath.
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let message = self.input_error(key);
        let label = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if message.is_some() {
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
        };
        match message {
            Some(message) => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(row)
                .child(field_error(message))
                .into_any_element(),
            None => row.into_any_element(),
        }
    }

    /// Two columns on desktop, one otherwise.
    fn grid(&self, layout: Layout, children: Vec<AnyElement>) -> AnyElement {
        if layout == Layout::Desktop {
            let mut rows = div().flex().flex_col().gap_3().w_full();
            let mut iter = children.into_iter();
            while let Some(left) = iter.next() {
                let mut row = div()
                    .flex()
                    .gap_6()
                    .w_full()
                    .child(div().flex_1().min_w_0().max_w_full().child(left));
                row = match iter.next() {
                    Some(right) => row.child(div().flex_1().min_w_0().max_w_full().child(right)),
                    None => row.child(div().flex_1().min_w_0()),
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

    /// A caption and its choice buttons. On phones the caption sits on its own
    /// line with the buttons in a full-width wrapping row below it.
    fn choice_row(layout: Layout, label: &str, buttons: Vec<Button>) -> AnyElement {
        let caption = div()
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
                .child(caption)
                .child(div().flex().flex_wrap().w_full().gap_2().children(buttons))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(caption.flex_1().min_w(px(220.)))
                .children(buttons)
                .into_any_element()
        }
    }

    /// True when the Item 2 year-ended month/year is after the current local
    /// month/year. Display only; never feeds validation.
    fn item_2_in_future(&self) -> bool {
        let today = chrono::Local::now().date_naive();
        i32::from(self.draft.taxable_year) > chrono::Datelike::year(&today)
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let basis = self.draft.tax_period_basis;
        let fiscal = basis == TaxPeriodBasis::Fiscal;
        // Calendar years always end in December, so the picker is read-only.
        let month_value = self.draft.year_end_month.to_string();
        let month = select_field(
            "Item 2 — Year ended: month",
            &self.year_end_month,
            &MONTH_OPTIONS,
            Some(month_value.as_str()),
            !editable || !fiscal,
            self.error_for(&["year_end_month"]).as_deref(),
            cx,
        );
        let quarters: Vec<Button> = (1..=4u8)
            .map(|quarter| {
                Self::choice(
                    ("2553_qtr", quarter as usize),
                    match quarter {
                        1 => "1st",
                        2 => "2nd",
                        3 => "3rd",
                        _ => "4th",
                    },
                    self.draft.quarter == quarter,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.quarter = quarter)))
            })
            .collect();
        let amended = self.draft.is_amended;
        let mut children = vec![
            Self::choice_row(
                layout,
                "Item 1 — For the (Calendar or Fiscal)",
                vec![
                    Self::choice("2553_calendar", "Calendar", !fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.set_tax_period_basis(TaxPeriodBasis::Calendar);
                            })
                        }),
                    ),
                    Self::choice("2553_fiscal", "Fiscal", fiscal, !editable).on_click(cx.listener(
                        |this, _, _, cx| {
                            let mut notice = None;
                            this.edit(cx, |d| {
                                notice = d.set_tax_period_basis(TaxPeriodBasis::Fiscal);
                            });
                            if let Some(notice) = notice {
                                this.status_message = Some(notice.to_string());
                            }
                        },
                    )),
                ],
            ),
            month,
            self.field("year", "Item 2 — Year ended: year", layout, !editable),
            with_field_error(
                Self::choice_row(layout, "Item 3 — Quarter", quarters),
                self.error_for(&["quarter"]),
            ),
            Self::choice_row(
                layout,
                "Item 4 — Amended return?",
                vec![
                    Self::choice("2553_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("2553_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(cx, |d| d.is_amended = false);
                            // recompute() zeroes 20A; clear the greyed box too.
                            if let Some(input) = this.inputs.get("prev") {
                                input.update(cx, |state, cx| {
                                    state.set_value(String::new(), window, cx)
                                });
                            }
                        }),
                    ),
                ],
            ),
            self.field(
                "sheets",
                "Item 5 — Number of sheets attached",
                layout,
                !editable,
            ),
        ];
        if self.item_2_in_future() {
            // Non-blocking: not part of `validate()` or the parse errors.
            children.insert(
                3,
                div()
                    .text_sm()
                    .text_color(cx.theme().warning)
                    .child("This year is in the future. Check Item 2 before filing.")
                    .into_any_element(),
            );
        }
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let treaty = self.draft.tax_treaty;
        let mut fields = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 6 — TIN"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format_tin(&self.draft.tin)),
                )
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 7 — RDO Code"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(self.draft.rdo_code.clone()),
                )
                .into_any_element(),
        ];
        for (key, label, _) in TEXT_INPUTS.iter().skip(2).filter(|(k, _, _)| *k != "email") {
            fields.push(self.field(key, label, layout, !editable));
        }
        let email = TEXT_INPUTS
            .iter()
            .find(|(k, _, _)| *k == "email")
            .map(|(key, label, _)| self.field(key, label, layout, !editable));
        let treaty_error = self.error_for(&["tax_treaty"]);
        let treaty_row = Self::choice_row(
            layout,
            "Item 13 — Are you availing of tax relief under Special Law or International Tax Treaty?",
            vec![
                Self::choice(
                    "2553_treaty_no",
                    "No",
                    treaty == Form2553TaxTreaty::No,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| d.tax_treaty = Form2553TaxTreaty::No)
                })),
                Self::choice(
                    "2553_treaty_special",
                    "Yes — Special Rate",
                    treaty == Form2553TaxTreaty::SpecialRate,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| d.tax_treaty = Form2553TaxTreaty::SpecialRate)
                })),
                Self::choice(
                    "2553_treaty_intl",
                    "Yes — International Tax Treaty",
                    treaty == Form2553TaxTreaty::InternationalTaxTreaty,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| {
                        d.tax_treaty = Form2553TaxTreaty::InternationalTaxTreaty
                    })
                })),
            ],
        );
        self.section(
            "Part I — Background Information",
            std::iter::once(self.grid(layout, fields))
                .chain(std::iter::once(with_field_error(treaty_row, treaty_error)))
                .chain(email)
                .collect(),
            cx,
        )
    }

    fn render_atc_row(&self, row: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let item = 14 + row;
        let entry = &self.draft.schedule[row];
        let option = FORM_2553_ATC_OPTIONS
            .iter()
            .find(|o| o.code == entry.atc_code && o.description == entry.description);
        let rate_editable = option.is_some_and(|o| o.rate_is_editable);
        let heading = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Item {item} — {}", entry.atc_code)),
                    )
                    .child(div().text_sm().child(entry.description.clone())),
            )
            .child(
                Button::new(("2553_clear_atc", row))
                    .label("Remove")
                    .outline()
                    .small()
                    .disabled(!editable)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.clear_atc(row, window, cx)),
                    ),
            );
        let rate: AnyElement = if rate_editable {
            self.field(
                &rate_key(row),
                &format!("{item}D — Tax rate (%)"),
                layout,
                !editable,
            )
        } else {
            div()
                .flex()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("{item}D — Tax rate")),
                )
                .child(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::BOLD)
                        .child(format!("{}%", option.map(|o| o.rate_text).unwrap_or("?"))),
                )
                .into_any_element()
        };
        let amount = self.field(
            &amount_key(row),
            &format!("{item}C — Taxable amount"),
            layout,
            !editable,
        );
        let due = self.computed(&format!("{item}E — Tax due"), entry.tax_due, cx);
        let body = if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(amount)
                .child(rate)
                .child(due)
        } else {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(div().flex_1().child(amount))
                        .child(div().flex_1().child(rate)),
                )
                .child(due)
        };
        let atc_key = format!("schedule[{row}].atc_code");
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading)
            .children(self.error_for(&[atc_key.as_str()]).map(field_error))
            .child(body)
            .into_any_element()
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = (0..self.draft.schedule.len().min(FORM_2553_ATC_ROWS))
            .filter(|row| !self.draft.schedule[*row].is_empty())
            .map(|row| self.render_atc_row(row, layout, cx))
            .collect();
        let next = self
            .draft
            .schedule
            .iter()
            .position(|row| row.is_empty())
            .unwrap_or(self.draft.schedule.len());
        if next < FORM_2553_ATC_ROWS && editable {
            let mut picks = div().flex().flex_wrap().gap_2();
            for (index, option) in FORM_2553_ATC_OPTIONS.iter().enumerate() {
                let taken = self
                    .draft
                    .schedule
                    .iter()
                    .any(|r| r.atc_code == option.code && r.description == option.description);
                if taken {
                    continue;
                }
                picks = picks.child(
                    Button::new(("2553_add_atc", index))
                        .label(format!("{} — {}", option.code, option.description))
                        .outline()
                        .small()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick_atc(next, index, window, cx)
                        })),
                );
            }
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .max_w_full()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("Add an ATC (Item {})", 14 + next)),
                    )
                    .child(picks)
                    .into_any_element(),
            );
        }
        children.extend(
            self.error_for(&["schedule"])
                .map(field_error)
                .map(IntoElement::into_any_element),
        );
        children.push(self.computed("19 — Total tax due", self.draft.total_tax_due, cx));
        children.extend(self.render_totals(layout, cx));
        self.section("Part II — Computation of Tax", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> Vec<AnyElement> {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let credits = vec![
            self.field(
                "prev",
                MONEY_INPUTS[0].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.field("cwt", MONEY_INPUTS[1].1, layout, !editable),
            self.computed("20C — Total tax credits/payments", d.total_tax_credits, cx),
            self.computed("21 — Tax payable (overpayment)", d.tax_payable, cx),
        ];
        let penalties = vec![
            self.field("sur", MONEY_INPUTS[2].1, layout, !editable),
            self.field("int", MONEY_INPUTS[3].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[4].1, layout, !editable),
            self.computed("22D — Total penalties", d.total_penalties, cx),
        ];
        let total = self.computed(
            "23 — Total amount payable (overpayment)",
            d.total_amount_payable,
            cx,
        );
        let mut overpayment = Vec::new();
        let mut overpayment_error = None;
        if d.total_amount_payable < 0.0 {
            let over = d.overpayment;
            overpayment_error = self.error_for(&["overpayment"]);
            overpayment.push(Self::choice_row(
                layout,
                "If overpayment, mark one box only:",
                vec![
                    Self::choice(
                        "2553_refund",
                        "To be Refunded",
                        over == Form2553Overpayment::Refund,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| d.overpayment = Form2553Overpayment::Refund)
                    })),
                    Self::choice(
                        "2553_tcc",
                        "To be issued a Tax Credit Certificate",
                        over == Form2553Overpayment::TaxCreditCertificate,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.overpayment = Form2553Overpayment::TaxCreditCertificate
                        })
                    })),
                ],
            ));
        }
        let caption = |text: &str| {
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(text.to_string())
                .into_any_element()
        };
        let mut out = vec![
            caption("20 Less: Tax Credits/Payments"),
            self.grid(layout, credits),
            caption("22 Add: Penalties"),
            self.grid(layout, penalties),
            total,
        ];
        out.extend(
            overpayment
                .into_iter()
                .map(|row| with_field_error(row, overpayment_error.clone())),
        );
        out
    }
}

impl QueueableFormView for Form2553View {
    type Draft = Form2553Draft;

    const PRINT_SLUG: &'static str = "2553-1999";

    fn print_fields(draft: &Form2553Draft) -> std::collections::BTreeMap<String, String> {
        draft.to_bir_field_map()
    }

    fn agent_draft(&self) -> &Form2553Draft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        HEADER_ACTIONS
    }

    fn new(
        draft: Form2553Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| {
                (
                    key.to_string(),
                    placeholder.to_string(),
                    Self::initial(&draft, key),
                )
            })
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into(), Self::initial(&draft, key)));
        }
        for row in 0..FORM_2553_ATC_ROWS {
            keys.push((
                amount_key(row),
                "0.00".into(),
                Self::initial(&draft, &amount_key(row)),
            ));
            keys.push((
                rate_key(row),
                "rate %".into(),
                Self::initial(&draft, &rate_key(row)),
            ));
        }
        for (key, placeholder, value) in keys {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            let money_key = key.starts_with("amt") || MONEY_INPUTS.iter().any(|(k, _)| *k == key);
            let blur_key = key.clone();
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => this.sync_from_inputs(cx),
                    InputEvent::Blur if money_key => this.reformat_money(&blur_key, window, cx),
                    _ => {}
                },
            ));
            inputs.insert(key, input);
        }
        let year_end_month = form_controls::new_select_state(&MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &year_end_month,
            &MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                let Ok(month) = value.parse::<u8>() else {
                    return;
                };
                // Fiscal years cannot end in December (`dateyear`); a refused
                // pick re-renders the draft's month back into the box.
                let fiscal = this.draft.tax_period_basis == TaxPeriodBasis::Fiscal;
                if !fiscal {
                    cx.notify();
                    return;
                }
                if month == 12 {
                    // Show the official input-019 alert that core returns.
                    let mut probe = this.draft.clone();
                    probe.year_end_month = 12;
                    this.status_message = probe
                        .set_tax_period_basis(TaxPeriodBasis::Fiscal)
                        .map(str::to_string);
                    cx.notify();
                    return;
                }
                let mut notice = None;
                this.edit(cx, |d| {
                    notice = d.set_year_end_month(month);
                });
                if let Some(notice) = notice {
                    this.status_message = Some(notice.to_string());
                }
            },
        ));
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            year_end_month,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            header: FormHeaderState::new(),
            validated: false,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2553Draft {
        // The library tile names the quarter; the period lock stays. Anything
        // else leaves Item 3 blank so validation asks for it.
        let quarter = if (1..=4).contains(&period) { period } else { 0 };
        Form2553Draft::new_from_profile(profile, year, quarter)
    }

    /// 2553 is event-based on the dashboard; the return names its own
    /// quarter (Item 3). Reopen this year's latest saved return.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2553Draft {
        let tin = profile.tin.full();
        (1..=4i64)
            .filter_map(|q| {
                db.get_queueable_draft::<Form2553Draft>(&tin, year, q)
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

impl FormViewTrait for Form2553View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2553"
    }
    fn form_subtitle(&self) -> &'static str {
        "Return of Percentage Tax Payable Under Special Laws"
    }
    fn form_version(&self) -> &'static str {
        "1999 (ENCS)"
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
                    "2553 draft saved.".into(),
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
        if !can_queue_for_submission(Form2553Draft::FORM_CODE) {
            self.status_message =
                Some("2553 is not enabled for in-app submission in this build.".into());
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
            self.status_message = Some("Fix the items marked below before submitting.".into());
            cx.notify();
            return;
        }
        let before = self.draft.clone();
        if let Err(errors) = self.draft.queue(QueueAuthSource::Gui) {
            self.validation_errors = errors;
            self.status_message = Some("Fix the items marked below before submitting.".into());
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
            self.status_message = Some(bir_core::forms::queueable::queue_failure_message(
                "Form 2553",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 2553 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2553 payment status needs a verified confirmation workflow.".into());
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
                    "Form 2553 reverted to Draft.".into(),
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
        // Reload the editors from the reverted draft (Amended Return = Yes
        // enables Item 20A).
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
            "2553 — Print Preview",
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

impl Form2553View {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2553Draft>(
                        &queued.tin,
                        queued.taxable_year,
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
}

impl Render for Form2553View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let month_value = self.draft.year_end_month.to_string();
        form_controls::sync_select(
            &self.year_end_month,
            &MONTH_OPTIONS,
            Some(month_value.as_str()),
            window,
            cx,
        );
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };

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
                .child(Button::new("2553_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(header_buttons);

        let mut body = div()
            .w_full()
            .max_w(CONTENT_MAX_W)
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
                            Button::new("2553_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2553_release_cancel")
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
            .child(self.render_schedule(layout, cx));
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
                    .id("2553_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form2553View {
    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "2553",
            status: self.draft.lifecycle.status.clone(),
            editor_safe: true,
            validated: self.validated,
            errors: self
                .parse_errors
                .iter()
                .chain(self.validation_errors.iter())
                .map(|(_, message)| message.replace('\n', " "))
                .collect(),
            content_max_w: CONTENT_MAX_W,
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2553Draft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "2553 marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form2553View> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form2553View::blocking_errors,
    on_validate: Form2553View::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form2553View::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_2553_view.rs");
        assert!(src.contains(concat!("render_header_", "buttons(")));
        assert!(src.contains(concat!("render_status_", "box(")));
        assert!(src.contains(concat!("revert_for_", "amendment(")));
        assert!(!src.contains(concat!("Queued for background ", "submission")));
        assert!(!src.contains(concat!("\"Queue for ", "submission\"")));
        assert!(!src.contains(concat!("Queue is off: ", "{issue_count}")));
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
