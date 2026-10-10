//! Editor for BIR Form 1600-VT, Monthly Remittance Return of Value-Added Tax
//! Withheld (v2018). Rust owns every calculation, validation and the official
//! submit plaintext (`bir_core::forms::form_1600vt`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1600vt::{
    FORM_1600VT_ATC_ROWS, FORM_1600VT_SCHEDULE_ROWS, Form1600VtAgentCategory, Form1600VtDraft,
    Form1600VtScheduleRow, form_1600vt_atc_index, form_1600vt_popup,
};
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form1600VtView {}

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
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 4 — No. of sheets attached", "0"),
    ("name", "Item 7 — Withholding Agent's Name", ""),
    ("address", "Item 8 — Registered Address", ""),
    ("zip", "Item 8A — Zip Code", ""),
    ("phone", "Item 9 — Contact Number", "digits only"),
    ("email", "Item 11 — Email Address", ""),
    ("relief_spec", "Item 12A — If yes, specify", ""),
];

/// Money inputs: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "prev",
        "19 — Tax remitted in return previously filed (amended only)",
    ),
    ("other", "20 — Other remittances made"),
    ("sur", "23 — Surcharge"),
    ("int", "24 — Interest"),
    ("comp", "25 — Compromise"),
];

/// Per Schedule 1 row (Page 2): (suffix, label).
const SCHEDULE_COLUMNS: &[(&str, &str)] = &[
    ("tin", "TIN (12–14 digits)"),
    ("name", "Name of payee"),
    ("atc", "ATC"),
    ("amt", "Amount of income payment"),
    ("rate", "Tax rate (%)"),
];

fn base_key(row: usize) -> String {
    format!("base{row}")
}

fn schedule_key(row: usize, column: &str) -> String {
    format!("s{row}_{column}")
}

fn label_for(key: &str) -> String {
    if let Some((_, label, _)) = TEXT_INPUTS.iter().find(|(k, _, _)| *k == key) {
        return label.to_string();
    }
    if let Some((_, label)) = MONEY_INPUTS.iter().find(|(k, _)| *k == key) {
        return label.to_string();
    }
    if let Some(row) = key
        .strip_prefix("base")
        .and_then(|n| n.parse::<usize>().ok())
    {
        return format!("{} — Tax base", 13 + row);
    }
    if let Some((row, column)) = key
        .strip_prefix('s')
        .and_then(|rest| rest.split_once('_'))
        .and_then(|(row, column)| Some((row.parse::<usize>().ok()?, column)))
        && let Some((_, label)) = SCHEDULE_COLUMNS.iter().find(|(c, _)| *c == column)
    {
        return format!("Row {} — {label}", row + 1);
    }
    key.to_string()
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

pub struct Form1600VtView {
    draft: Form1600VtDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1600VtView {
    fn all_keys() -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        for row in 0..FORM_1600VT_ATC_ROWS {
            keys.push((base_key(row), "0.00".into()));
        }
        for row in 0..FORM_1600VT_SCHEDULE_ROWS {
            for (column, _) in SCHEDULE_COLUMNS {
                let placeholder = if matches!(*column, "amt" | "rate") {
                    "0.00"
                } else {
                    ""
                };
                keys.push((schedule_key(row, column), placeholder.into()));
            }
        }
        keys
    }

    /// Editor text for one input from the draft.
    fn initial(draft: &Form1600VtDraft, key: &str) -> String {
        let money = |value: f64| {
            if value == 0.0 {
                String::new()
            } else {
                official_amount(value)
            }
        };
        match key {
            "year" => draft.year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.agent_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "relief_spec" => draft.tax_relief_specify.clone(),
            "prev" => money(draft.tax_remitted_previous),
            "other" => money(draft.other_payments),
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            _ => {
                if let Some(row) = key
                    .strip_prefix("base")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    return draft
                        .atc_rows
                        .get(row)
                        .map(|r| money(r.tax_base))
                        .unwrap_or_default();
                }
                let Some((row, column)) = key
                    .strip_prefix('s')
                    .and_then(|rest| rest.split_once('_'))
                    .and_then(|(row, column)| Some((row.parse::<usize>().ok()?, column)))
                else {
                    return String::new();
                };
                let Some(entry) = draft.schedule.get(row) else {
                    return String::new();
                };
                match column {
                    "tin" => entry.tin.clone(),
                    "name" => entry.payee_name.clone(),
                    "atc" => entry.atc.clone(),
                    "amt" => money(entry.income_payment),
                    "rate" => money(entry.tax_rate),
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
    /// A malformed number is reported instead of becoming zero.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let mut parse_errors = Vec::new();
        let mut number = |key: &str, cx: &App| -> Option<f64> {
            let text = self.input_text(key, cx);
            let parsed = parse_amount(&text);
            if parsed.is_none() {
                parse_errors.push((
                    key.to_string(),
                    format!("{}: \"{text}\" is not a number.", label_for(key)),
                ));
            }
            parsed
        };
        let mut draft = self.draft.clone();
        if let Some(year) = number("year", cx) {
            draft.year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
        }
        if let Some(sheets) = number("sheets", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        for (key, _) in MONEY_INPUTS {
            if let Some(value) = number(key, cx) {
                match *key {
                    "prev" => draft.tax_remitted_previous = value,
                    "other" => draft.other_payments = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        for row in 0..draft.atc_rows.len().min(FORM_1600VT_ATC_ROWS) {
            if let Some(value) = number(&base_key(row), cx) {
                draft.atc_rows[row].tax_base = value;
            }
        }
        let mut schedule = Vec::new();
        for row in 0..FORM_1600VT_SCHEDULE_ROWS {
            let text = |column: &str| self.input_text(&schedule_key(row, column), cx);
            let entry = Form1600VtScheduleRow {
                tin: text("tin").trim().to_string(),
                payee_name: text("name"),
                atc: text("atc").trim().to_string(),
                income_payment: number(&schedule_key(row, "amt"), cx).unwrap_or(0.0),
                tax_rate: number(&schedule_key(row, "rate"), cx).unwrap_or(0.0),
                tax_withheld: 0.0,
            };
            schedule.push(entry);
        }
        while schedule.last().is_some_and(Form1600VtScheduleRow::is_blank) {
            schedule.pop();
        }
        draft.schedule = schedule;
        draft.agent_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.tax_relief_specify = self.input_text("relief_spec", cx);
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// Reload every editor from the draft (after ATC rows moved).
    fn reload_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.sync_from_inputs(cx);
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1600VtDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// A change that can add, drop or reorder ATC rows.
    fn edit_rows(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1600VtDraft) -> Result<(), String>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        match change(&mut self.draft) {
            Ok(()) => {
                self.status_message = None;
                self.reload_inputs(window, cx);
            }
            Err(message) => {
                self.status_message = Some(message);
                cx.notify();
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

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form1600VtDraft>(
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
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child(title.to_string()),
            )
            .children(children)
            .into_any_element()
    }

    fn has_error(&self, keys: &[&str]) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| keys.iter().any(|key| field == key))
    }

    /// The draft fields an editor key feeds (for highlighting).
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let fields = Self::draft_fields(key);
        let mut keys = vec![key];
        keys.extend(fields.iter().map(String::as_str));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if self.has_error(&keys) {
            label.text_color(gpui::red())
        } else {
            label
        };
        match layout {
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
        .into_any_element()
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
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_right()
                    .font_weight(FontWeight::BOLD)
                    .child(official_amount(value)),
            )
            .into_any_element()
    }

    fn readonly(&self, label: &str, value: String) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(div().font_weight(FontWeight::BOLD).child(value))
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

    /// A labelled row of choice buttons.
    fn choice_row(&self, label: &str, error_keys: &[&str], buttons: Vec<Button>) -> AnyElement {
        let title = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let title = if self.has_error(error_keys) {
            title.text_color(gpui::red())
        } else {
            title
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(title)
            .child(div().flex().flex_wrap().gap_2().children(buttons))
            .into_any_element()
    }

    fn yes_no(
        &self,
        id: &'static str,
        label: &str,
        error_key: &str,
        value: Option<bool>,
        set: fn(&mut Form1600VtDraft, bool),
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let buttons = [(true, "Yes"), (false, "No")]
            .into_iter()
            .map(|(answer, text)| {
                Self::choice(
                    (id, usize::from(answer)),
                    text,
                    value == Some(answer),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| set(d, answer))))
            })
            .collect();
        self.choice_row(label, &[error_key], buttons)
    }

    /// The draft fields an editor key feeds (for highlighting).
    fn draft_fields(key: &str) -> Vec<String> {
        let direct = match key {
            "year" => Some("year"),
            "sheets" => Some("number_of_attached_sheets"),
            "name" => Some("agent_name"),
            "address" => Some("registered_address"),
            "zip" => Some("zip_code"),
            "phone" => Some("contact_number"),
            "email" => Some("email"),
            "relief_spec" => Some("tax_relief_specify"),
            "prev" => Some("tax_remitted_previous"),
            "other" => Some("other_payments"),
            "sur" => Some("surcharge"),
            "int" => Some("interest"),
            "comp" => Some("compromise"),
            _ => None,
        };
        if let Some(field) = direct {
            return vec![field.to_string()];
        }
        if let Some(row) = key
            .strip_prefix("base")
            .and_then(|n| n.parse::<usize>().ok())
        {
            return vec![format!("atc_rows[{row}].tax_base")];
        }
        if let Some((row, column)) = key
            .strip_prefix('s')
            .and_then(|rest| rest.split_once('_'))
            .and_then(|(row, column)| Some((row.parse::<usize>().ok()?, column)))
        {
            let field = match column {
                "tin" => "tin",
                "name" => "payee_name",
                "atc" => "atc",
                "amt" => "income_payment",
                _ => "tax_rate",
            };
            return vec![
                format!("schedule[{row}]"),
                format!("schedule[{row}].{field}"),
            ];
        }
        Vec::new()
    }

    fn text_field(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        self.field(key, &label_for(key), layout, disabled)
    }

    fn render_return_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let months = MONTHS
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let month = index as u8 + 1;
                Self::choice(
                    ("1600vt_month", index),
                    *name,
                    self.draft.month == month,
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.month = month)))
            })
            .collect();
        let withheld = self.draft.taxes_withheld;
        let children = vec![
            self.choice_row("Item 1 — Month", &["month"], months),
            self.text_field("year", layout, !editable),
            self.yes_no(
                "1600vt_amended",
                "Item 2 — Amended return?",
                "is_amended",
                Some(self.draft.is_amended),
                |d, v| d.is_amended = v,
                cx,
            ),
            self.choice_row(
                "Item 3 — Any taxes withheld?",
                &["taxes_withheld"],
                vec![
                    Self::choice("1600vt_withheld_yes", "Yes", withheld, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit_rows(window, cx, |d| {
                                d.set_taxes_withheld(true);
                                Ok(())
                            })
                        }),
                    ),
                    Self::choice("1600vt_withheld_no", "No", !withheld, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit_rows(window, cx, |d| {
                                d.set_taxes_withheld(false);
                                Ok(())
                            })
                        }),
                    ),
                ],
            ),
            self.text_field("sheets", layout, !editable),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let category = d.agent_category;
        let category_buttons = [
            (
                "1600vt_private",
                "Private",
                Form1600VtAgentCategory::Private,
            ),
            (
                "1600vt_government",
                "Government",
                Form1600VtAgentCategory::Government,
            ),
        ]
        .into_iter()
        .map(|(id, text, value)| {
            Self::choice(id, text, category == Some(value), !editable).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.edit_rows(window, cx, |d| {
                        d.set_agent_category(value);
                        Ok(())
                    })
                },
            ))
        })
        .collect();
        let children = vec![
            self.readonly("Item 5 — TIN", format_tin(&d.tin)),
            self.readonly("Item 6 — RDO Code", d.rdo_code.clone()),
            self.text_field("name", layout, !editable),
            self.text_field("address", layout, !editable),
            self.text_field("zip", layout, !editable),
            self.text_field("phone", layout, !editable),
            self.choice_row(
                "Item 10 — Category of Withholding Agent",
                &["agent_category"],
                category_buttons,
            ),
            self.text_field("email", layout, !editable),
            self.yes_no(
                "1600vt_relief",
                "Item 12 — Availing of tax relief under a Special Law or International Tax Treaty?",
                "tax_relief",
                Some(d.tax_relief),
                |d, v| d.tax_relief = v,
                cx,
            ),
            self.text_field("relief_spec", layout, !editable || !d.tax_relief),
        ];
        self.section(
            "Part I — Background Information",
            vec![self.grid(layout, children)],
            cx,
        )
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = Vec::new();
        if let Some(category) = d.agent_category.filter(|_| d.taxes_withheld) {
            let picks = form_1600vt_popup(category)
                .map(|(index, option)| {
                    let ticked = d
                        .atc_rows
                        .iter()
                        .any(|r| r.atc_code == option.code && r.rate_text == option.rate_text);
                    Self::choice(
                        ("1600vt_atc", index),
                        format!("{} ({}%)", option.code, option.rate_text),
                        ticked,
                        !editable,
                    )
                    .small()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_rows(window, cx, |d| d.toggle_atc(index))
                    }))
                })
                .collect();
            children.push(self.choice_row(
                "ATCs (tap to add or remove, at most five)",
                &["atc_rows"],
                picks,
            ));
        } else {
            children.push(
                div()
                    .text_sm()
                    .child("Answer Item 3 Yes and Item 10 to pick ATCs.")
                    .into_any_element(),
            );
        }
        for (row, entry) in d.atc_rows.iter().enumerate().take(FORM_1600VT_ATC_ROWS) {
            let description = form_1600vt_atc_index(&entry.atc_code, &entry.rate_text)
                .map(|i| {
                    bir_core::forms::form_1600vt::FORM_1600VT_ATC_OPTIONS[i]
                        .description
                        .to_string()
                })
                .unwrap_or_default();
            let body = vec![
                self.text_field(&base_key(row), layout, !editable),
                self.computed(
                    &format!("{} — Tax withheld at {}%", 13 + row, entry.rate_text),
                    entry.tax_withheld,
                    cx,
                ),
            ];
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(div().font_weight(FontWeight::BOLD).child(format!(
                        "Item {} — {}",
                        13 + row,
                        entry.atc_code
                    )))
                    .child(div().text_sm().child(description))
                    .child(self.grid(layout, body))
                    .into_any_element(),
            );
        }
        let totals = vec![
            self.computed("18 — Total taxes withheld", d.total_tax_withheld, cx),
            self.text_field("prev", layout, !editable || !d.is_amended),
            self.text_field("other", layout, !editable),
            self.computed("21 — Total remittances made", d.total_payments, cx),
            self.computed("22 — Tax still due/(overremittance)", d.tax_still_due, cx),
            self.text_field("sur", layout, !editable),
            self.text_field("int", layout, !editable),
            self.text_field("comp", layout, !editable),
            self.computed("26 — Total penalties", d.total_penalties, cx),
            self.computed(
                "27 — Total amount still due/(overremittance)",
                d.total_amount_due,
                cx,
            ),
        ];
        children.push(self.grid(layout, totals));
        self.section("Part II — Computation of Tax", children, cx)
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..FORM_1600VT_SCHEDULE_ROWS {
            let fields: Vec<AnyElement> = SCHEDULE_COLUMNS
                .iter()
                .map(|(column, _)| self.text_field(&schedule_key(row, column), layout, !editable))
                .collect();
            let withheld = self
                .draft
                .schedule
                .get(row)
                .map(|r| r.tax_withheld)
                .unwrap_or(0.0);
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(self.grid(layout, fields))
                    .child(self.computed(
                        &format!("Row {} — Amount of tax withheld", row + 1),
                        withheld,
                        cx,
                    ))
                    .into_any_element(),
            );
        }
        children.push(self.computed(
            "Total tax withheld and remitted",
            self.draft.schedule_total,
            cx,
        ));
        self.section("Page 2 — Schedule 1 (optional)", children, cx)
    }
}

impl QueueableFormView for Form1600VtView {
    type Draft = Form1600VtDraft;

    fn new(
        draft: Form1600VtDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (key, placeholder) in Self::all_keys() {
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
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1600VtDraft {
        let month = if (1..=12).contains(&period) {
            period
        } else {
            chrono::Datelike::month(&chrono::Local::now().date_naive()) as u8
        };
        Form1600VtDraft::new_from_profile(profile, year, month)
    }
}

impl FormViewTrait for Form1600VtView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1600-VT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Monthly Remittance Return of Value-Added Tax Withheld"
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
                    "1600VT draft saved.".into(),
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
        if !can_queue_for_submission(Form1600VtDraft::FORM_CODE) {
            self.status_message =
                Some("1600VT is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1600VT. No submission was started: {error}"
            ));
            cx.notify();
            return;
        }
        self.status_message = Some(format!(
            "Queued for background submission as {}.",
            self.draft.submission_filename()
        ));
        self.notify(
            notification::NotificationType::Success,
            "Form 1600VT queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1600VT payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1600VtDraft>(
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
            "1600-vt-2018",
            &fields,
            "1600VT — Print Preview",
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

impl Render for Form1600VtView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let is_draft = self.draft.lifecycle.is_editable();
        let is_queued = matches!(self.draft.lifecycle.status, FilingStatus::Queued);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
            .take(20)
            .map(|(_, message)| {
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(message.clone())
                    .into_any_element()
            })
            .collect();

        let toolbar =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .px(pad)
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Button::new("1600vt_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1600vt_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1600vt_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1600vt_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1600vt_submit")
                                .label("Queue for submission")
                                .primary()
                                .disabled(
                                    !is_draft
                                        || !self.validation_errors.is_empty()
                                        || !self.parse_errors.is_empty(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.mark_submitted(window, cx)
                                })),
                        ),
                );

        let mut body = div()
            .w_full()
            .max_w(px(1180.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .when_some(self.status_message.clone(), |col, message| {
                col.child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .text_sm()
                        .child(message),
                )
            })
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("1600vt_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1600vt_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_return_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_part_two(layout, cx))
            .child(self.render_schedule(layout, cx));
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
                    .when(layout != Layout::Phone, |d| {
                        d.child(div().mt_4().child(self.render_status_pipeline(cx)))
                    }),
            )
            .child(
                div()
                    .id("1600vt_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount};
    use gpui::px;

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
        assert_eq!(format_tin("98765432100000"), "987-654-321-00000");
    }
}
