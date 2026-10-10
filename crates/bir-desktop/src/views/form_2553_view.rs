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
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::form_engine::FormViewTrait;
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

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 2 — Year ended", "YYYY"),
    ("sheets", "Item 5 — No. of sheets attached", "0"),
    ("lob", "Item 8 — Line of Business/Occupation", ""),
    ("name", "Item 9 — Taxpayer's Name", ""),
    ("phone", "Item 10 — Telephone Number", "digits only"),
    ("address", "Item 11 — Registered Address", ""),
    ("zip", "Item 12 — Zip Code", ""),
    ("email", "Email address for the BIR confirmation", ""),
];

/// Money inputs outside the ATC rows: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "prev",
        "20A — Tax paid in return previously filed (amended only)",
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

pub struct Form2553View {
    draft: Form2553Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
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
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
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
        self.draft.clear_atc(row);
        // Rows after the cleared one keep their place; reload their editors.
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

    /// A labelled input. On phones the label sits above the field.
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let error = self
            .validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| field == key || field.starts_with(&format!("{key}[")));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if error {
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
                <div text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div text_right font_weight={FontWeight::BOLD}>{official_amount(value)}</div>
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

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let basis = self.draft.tax_period_basis;
        let fiscal = basis == TaxPeriodBasis::Fiscal;
        let mut months = div().flex().flex_wrap().gap_1();
        for (index, name) in MONTHS.iter().enumerate() {
            let month = index as u8 + 1;
            // Fiscal years cannot end in December (`dateyear`).
            let unavailable = !fiscal || month == 12;
            months = months.child(
                Self::choice(
                    ("2553_month", index),
                    *name,
                    self.draft.year_end_month == month,
                    !editable || unavailable,
                )
                .small()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.year_end_month = month)),
                ),
            );
        }
        let mut quarters = div().flex().flex_wrap().gap_2();
        for quarter in 1..=4u8 {
            quarters = quarters.child(
                Self::choice(
                    ("2553_qtr", quarter as usize),
                    format!("Q{quarter}"),
                    self.draft.quarter == quarter,
                    !editable,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.quarter = quarter)),
                ),
            );
        }
        let amended = self.draft.is_amended;
        let children = vec![
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 1 —"),
                )
                .child(
                    Self::choice("2553_calendar", "Calendar", !fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.set_tax_period_basis(TaxPeriodBasis::Calendar))
                        }),
                    ),
                )
                .child(
                    Self::choice("2553_fiscal", "Fiscal", fiscal, !editable).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.set_tax_period_basis(TaxPeriodBasis::Fiscal);
                                if d.year_end_month == 12 {
                                    d.year_end_month = 6;
                                }
                            })
                        },
                    )),
                )
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 2 — Month the year ends"),
                )
                .child(months)
                .into_any_element(),
            self.field("year", "Item 2 — Year ended", layout, !editable),
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 3 — Quarter"),
                )
                .child(quarters)
                .into_any_element(),
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 4 — Amended return?"),
                )
                .child(
                    Self::choice("2553_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                )
                .child(
                    Self::choice("2553_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                )
                .into_any_element(),
            self.field(
                "sheets",
                "Item 5 — No. of sheets attached",
                layout,
                !editable,
            ),
        ];
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
        for (key, label, _) in TEXT_INPUTS.iter().skip(2) {
            fields.push(self.field(key, label, layout, !editable));
        }
        let treaty_row = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(
                "Item 13 — Availing of tax relief under a Special Law or International Tax Treaty?",
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Self::choice(
                            "2553_treaty_no",
                            "No",
                            treaty == Form2553TaxTreaty::No,
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_treaty = Form2553TaxTreaty::No)
                        })),
                    )
                    .child(
                        Self::choice(
                            "2553_treaty_special",
                            "Yes — Special Rate",
                            treaty == Form2553TaxTreaty::SpecialRate,
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_treaty = Form2553TaxTreaty::SpecialRate)
                        })),
                    )
                    .child(
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
                    ),
            )
            .into_any_element();
        self.section(
            "Part I — Background Information",
            vec![self.grid(layout, fields), treaty_row],
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
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("{item}D — Tax rate")),
                )
                .child(
                    div()
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
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading)
            .child(body)
            .into_any_element()
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = (0..self.draft.schedule.len().min(FORM_2553_ATC_ROWS))
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
                picks = picks.child(
                    Button::new(("2553_add_atc", index))
                        .label(format!("{} — {}", option.code, option.description))
                        .outline()
                        .small()
                        .disabled(taken)
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
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("Add an ATC (Item {})", 14 + next)),
                    )
                    .child(picks)
                    .into_any_element(),
            );
        }
        children.push(self.computed("19 — Total tax due", self.draft.total_tax_due, cx));
        self.section("Part II — Computation of Tax (Items 14–19)", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![
            self.field(
                "prev",
                MONEY_INPUTS[0].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.field("cwt", MONEY_INPUTS[1].1, layout, !editable),
            self.computed("20C — Total tax credits/payments", d.total_tax_credits, cx),
            self.computed("21 — Tax payable (overpayment)", d.tax_payable, cx),
            self.field("sur", MONEY_INPUTS[2].1, layout, !editable),
            self.field("int", MONEY_INPUTS[3].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[4].1, layout, !editable),
            self.computed("22D — Total penalties", d.total_penalties, cx),
            self.computed(
                "23 — Total amount payable (overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        if d.total_amount_payable < 0.0 {
            let over = d.overpayment;
            children.push(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("If overpayment, mark one:"),
                    )
                    .child(
                        Self::choice(
                            "2553_refund",
                            "To be refunded",
                            over == Form2553Overpayment::Refund,
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.overpayment = Form2553Overpayment::Refund)
                        })),
                    )
                    .child(
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
                    )
                    .into_any_element(),
            );
        }
        self.section(
            "Part II — Tax Credits, Penalties and Amount Payable",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2553View {
    type Draft = Form2553Draft;

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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form2553Draft {
        let today = chrono::Local::now().date_naive();
        let quarter = if i32::from(year) == chrono::Datelike::year(&today) {
            ((chrono::Datelike::month(&today) - 1) / 3 + 1) as u8
        } else {
            4
        };
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
                "Could not queue Form 2553. No submission was started: {error}"
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
        let fields = self.draft.to_bir_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "2553-1999",
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

impl Render for Form2553View {
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
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.clone()}</div> }
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
                .child(Button::new("2553_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2553_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2553_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2553_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2553_submit")
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
        body = body.child(self.render_totals(layout, cx));
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
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }
}
