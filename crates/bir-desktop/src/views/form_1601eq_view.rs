//! Editor for BIR Form 1601-EQ, Quarterly Remittance Return of Creditable
//! Income Taxes Withheld (Expanded), January 2018. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1601eq`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1601eq::{
    FORM_1601EQ_ATC_OPTIONS, FORM_1601EQ_FIRST_ITEM, FORM_1601EQ_MAIN_ROWS, Form1601EqCategory,
    Form1601EqDraft, Form1601EqOverRemittance, form_1601eq_atc_option,
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

impl EventEmitter<QueueableFormEvent> for Form1601EqView {}

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

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 1 — For the Year", "YYYY"),
    ("sheets", "Item 5 — No. of sheets attached", "0"),
    ("name", "Item 8 — Withholding Agent's Name", ""),
    ("address", "Item 9 — Registered Address", ""),
    ("zip", "Item 9A — Zip Code", ""),
    ("phone", "Item 10 — Contact Number", "digits only"),
    ("email", "Item 12 — Email Address", ""),
    (
        "lob",
        "Line of business (kept with the return, not printed)",
        "",
    ),
];

/// Money inputs outside the ATC rows: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("rem1", "20 — Remittances made: 1st month of the quarter"),
    ("rem2", "21 — Remittances made: 2nd month of the quarter"),
    (
        "prev",
        "22 — Tax remitted in return previously filed (amended only)",
    ),
    (
        "over",
        "23 — Over-remittance from previous quarter of the same year",
    ),
    ("sur", "26 — Surcharge"),
    ("int", "27 — Interest"),
    ("comp", "28 — Compromise"),
];

/// How many popup matches the ATC finder shows at once.
const ATC_MATCHES_SHOWN: usize = 12;

fn base_key(code: &str) -> String {
    format!("base_{code}")
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

/// Popup options matching a filter on code or description, in popup order.
fn atc_matches(filter: &str) -> Vec<usize> {
    let needle = filter.trim().to_uppercase();
    FORM_1601EQ_ATC_OPTIONS
        .iter()
        .enumerate()
        .filter(|(_, option)| {
            needle.is_empty()
                || option.code.contains(&needle)
                || option.description.contains(&needle)
        })
        .map(|(index, _)| index)
        .collect()
}

pub struct Form1601EqView {
    draft: Form1601EqDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1601EqView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1601EqDraft, key: &str) -> String {
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
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "lob" => draft.line_of_business.clone(),
            "rem1" => money(draft.remittance_first_month),
            "rem2" => money(draft.remittance_second_month),
            "prev" => money(draft.tax_remitted_previous),
            "over" => money(draft.over_remittance_previous_quarter),
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            "atc_filter" => String::new(),
            _ => key
                .strip_prefix("base_")
                .and_then(|code| draft.schedule.iter().find(|row| row.atc_code == code))
                .map(|row| money(row.tax_base))
                .unwrap_or_default(),
        }
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Create the editor for `key` if it does not exist yet.
    fn ensure_input(
        &mut self,
        key: String,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inputs.contains_key(&key) {
            return;
        }
        let value = Self::initial(&self.draft, &key);
        let placeholder = placeholder.to_string();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |state, cx| state.set_value(value, window, cx));
        let filter = key == "atc_filter";
        self._subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    if filter {
                        cx.notify();
                    } else {
                        this.sync_from_inputs(cx);
                    }
                }
            },
        ));
        self.inputs.insert(key, input);
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
        if let Some(year) = number("year", "Item 1 year", cx) {
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
                    "rem1" => draft.remittance_first_month = value,
                    "rem2" => draft.remittance_second_month = value,
                    "prev" => draft.tax_remitted_previous = value,
                    "over" => draft.over_remittance_previous_quarter = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        for row in &mut draft.schedule {
            let label = format!("Tax base for {}", row.atc_code);
            if let Some(value) = number(&base_key(&row.atc_code), &label, cx) {
                row.tax_base = value;
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.line_of_business = self.input_text("lob", cx);
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1601EqDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    fn add_atc(&mut self, option: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(option) = FORM_1601EQ_ATC_OPTIONS.get(option) else {
            return;
        };
        let mut draft = self.draft.clone();
        match draft.add_atc(option.code) {
            Ok(()) => {
                self.draft = draft;
                self.ensure_input(base_key(option.code), "0.00", window, cx);
                self.status_message = None;
                self.sync_from_inputs(cx);
            }
            Err(message) => {
                self.status_message = Some(message);
                cx.notify();
            }
        }
    }

    fn remove_atc(&mut self, code: String, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.remove_atc(&code);
        if let Some(input) = self.inputs.get(&base_key(&code)) {
            input.update(cx, |state, cx| state.set_value("", window, cx));
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
                .release_abandoned_claimed_queueable::<Form1601EqDraft>(
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
        let Some(input) = self.inputs.get(key) else {
            return div().into_any_element();
        };
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

    fn labelled_row(label: &str, choices: Vec<AnyElement>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .children(choices)
            .into_any_element()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let quarters = (1..=4u8)
            .map(|quarter| {
                Self::choice(
                    ("1601eq_qtr", quarter as usize),
                    format!("Q{quarter}"),
                    self.draft.quarter == quarter,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.quarter = quarter)))
                .into_any_element()
            })
            .collect();
        let amended = self.draft.is_amended;
        let withheld = self.draft.any_tax_withheld;
        let children = vec![
            self.field("year", "Item 1 — For the Year", layout, !editable),
            Self::labelled_row("Item 2 — Quarter", quarters),
            Self::labelled_row(
                "Item 3 — Amended return?",
                vec![
                    Self::choice("1601eq_amended_yes", "Yes", amended, !editable)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                        )
                        .into_any_element(),
                    Self::choice("1601eq_amended_no", "No", !amended, !editable)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                        )
                        .into_any_element(),
                ],
            ),
            Self::labelled_row(
                "Item 4 — Any taxes withheld?",
                vec![
                    Self::choice(
                        "1601eq_withheld_yes",
                        "Yes",
                        withheld == Some(true),
                        !editable,
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.set_any_tax_withheld(true))
                        }),
                    )
                    .into_any_element(),
                    Self::choice(
                        "1601eq_withheld_no",
                        "No",
                        withheld == Some(false),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| d.set_any_tax_withheld(false))
                    }))
                    .into_any_element(),
                ],
            ),
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
        let category = self.draft.category;
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
        let category_row = Self::labelled_row(
            "Item 11 — Category of withholding agent",
            vec![
                Self::choice(
                    "1601eq_category_private",
                    "Private",
                    category == Some(Form1601EqCategory::Private),
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| d.set_category(Form1601EqCategory::Private))
                }))
                .into_any_element(),
                Self::choice(
                    "1601eq_category_government",
                    "Government",
                    category == Some(Form1601EqCategory::Government),
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| d.set_category(Form1601EqCategory::Government))
                }))
                .into_any_element(),
            ],
        );
        self.section(
            "Part I — Background Information",
            vec![self.grid(layout, fields), category_row],
            cx,
        )
    }

    fn render_atc_row(&self, index: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let entry = &self.draft.schedule[index];
        let code = entry.atc_code.clone();
        let item = if index < FORM_1601EQ_MAIN_ROWS {
            format!("Item {}", FORM_1601EQ_FIRST_ITEM + index)
        } else {
            format!("Other ATC {}", index + 1 - FORM_1601EQ_MAIN_ROWS)
        };
        let option = form_1601eq_atc_option(&code).map(|(_, option)| option);
        let rate = option
            .map(|o| format!("{}%", o.rate_text_for_year(self.draft.taxable_year)))
            .unwrap_or_else(|| "?".into());
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
                            .child(format!("{item} — {code} at {rate}")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(option.map(|o| o.description).unwrap_or("").to_string()),
                    ),
            )
            .child(
                Button::new(("1601eq_remove_atc", index))
                    .label("Remove")
                    .outline()
                    .small()
                    .disabled(!editable)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_atc(code.clone(), window, cx)
                    })),
            );
        let base = self.field(
            &base_key(&entry.atc_code),
            "Tax base (consolidated for the quarter)",
            layout,
            !editable,
        );
        let withheld = self.computed("Tax withheld", entry.tax_withheld, cx);
        let body = if layout == Layout::Phone {
            div().flex().flex_col().gap_2().child(base).child(withheld)
        } else {
            div()
                .flex()
                .gap_4()
                .items_center()
                .child(div().flex_1().child(base))
                .child(div().flex_1().child(withheld))
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

    fn render_schedule(&self, layout: Layout, view: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = (0..self.draft.schedule.len())
            .map(|row| self.render_atc_row(row, layout, view))
            .collect();
        let can_add =
            editable && self.draft.any_tax_withheld == Some(true) && self.draft.category.is_some();
        if can_add {
            let filter = self.input_text("atc_filter", view);
            let matches = atc_matches(&filter);
            let mut picks = div().flex().flex_col().gap_1();
            for index in matches.iter().copied().take(ATC_MATCHES_SHOWN) {
                let option = &FORM_1601EQ_ATC_OPTIONS[index];
                let taken = self
                    .draft
                    .schedule
                    .iter()
                    .any(|row| row.atc_code == option.code);
                picks =
                    picks.child(
                        Button::new(("1601eq_add_atc", index))
                            .label(format!(
                                "{} ({}%) — {}",
                                option.code,
                                option.rate_text_for_year(self.draft.taxable_year),
                                option.description
                            ))
                            .outline()
                            .small()
                            .disabled(taken)
                            .on_click(view.listener(move |this, _, window, cx| {
                                this.add_atc(index, window, cx)
                            })),
                    );
            }
            let more = matches.len().saturating_sub(ATC_MATCHES_SHOWN);
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Add an ATC (search by code or nature of payment)"),
                    )
                    .child(self.field("atc_filter", "Search ATC", layout, false))
                    .child(picks)
                    .when(more > 0, |col| {
                        col.child(
                            div()
                                .text_sm()
                                .child(format!("{more} more — refine the search to see them.")),
                        )
                    })
                    .into_any_element(),
            );
        } else if editable && self.draft.any_tax_withheld != Some(true) {
            children.push(
                div()
                    .text_sm()
                    .child("Answer Item 4 \"Yes\" and Item 11 to add ATCs.")
                    .into_any_element(),
            );
        }
        if self.draft.schedule.len() > FORM_1601EQ_MAIN_ROWS {
            children.push(self.computed(
                "Total of other selected ATCs",
                self.draft.total_other_tax_withheld,
                view,
            ));
        }
        children.push(self.computed(
            "19 — Total taxes withheld for the quarter",
            self.draft.total_taxes_withheld,
            view,
        ));
        self.section("Part II — Computation of Tax (Items 13–19)", children, view)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![
            self.field("rem1", MONEY_INPUTS[0].1, layout, !editable),
            self.field("rem2", MONEY_INPUTS[1].1, layout, !editable),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.field("over", MONEY_INPUTS[3].1, layout, !editable),
            self.computed("24 — Total remittances made", d.total_remittances, cx),
            self.computed("25 — Tax still due/(over-remittance)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[4].1, layout, !editable),
            self.field("int", MONEY_INPUTS[5].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[6].1, layout, !editable),
            self.computed("29 — Total penalties", d.total_penalties, cx),
            self.computed(
                "30 — Total amount still due/(over-remittance)",
                d.total_amount_due,
                cx,
            ),
        ];
        if d.total_amount_due < 0.0 {
            let over = d.over_remittance;
            let pick = |id: &'static str, label: &'static str, value: Form1601EqOverRemittance| {
                Self::choice(id, label, over == value, !editable)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(cx, |d| d.over_remittance = value)
                    }))
                    .into_any_element()
            };
            children.push(Self::labelled_row(
                "If over-remittance, mark one:",
                vec![
                    pick(
                        "1601eq_refund",
                        "To be refunded",
                        Form1601EqOverRemittance::Refund,
                    ),
                    pick(
                        "1601eq_tcc",
                        "To be issued a Tax Credit Certificate",
                        Form1601EqOverRemittance::TaxCreditCertificate,
                    ),
                    pick(
                        "1601eq_carry",
                        "To be carried over to the next quarter",
                        Form1601EqOverRemittance::CarriedOver,
                    ),
                ],
            ));
        }
        self.section(
            "Part II — Remittances, Penalties and Amount Still Due (Items 20–30)",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form1601EqView {
    type Draft = Form1601EqDraft;

    fn new(
        draft: Form1601EqDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let validation_errors = draft.validate();
        let codes: Vec<String> = draft
            .schedule
            .iter()
            .map(|row| row.atc_code.clone())
            .collect();
        let mut view = Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs: BTreeMap::new(),
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            _subscriptions: Vec::new(),
        };
        for (key, _, placeholder) in TEXT_INPUTS {
            view.ensure_input(key.to_string(), placeholder, window, cx);
        }
        for (key, _) in MONEY_INPUTS {
            view.ensure_input(key.to_string(), "0.00", window, cx);
        }
        view.ensure_input("atc_filter".into(), "e.g. WI010 or RENTALS", window, cx);
        for code in codes {
            view.ensure_input(base_key(&code), "0.00", window, cx);
        }
        view
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1601EqDraft {
        let quarter = if (1..=4).contains(&period) {
            period
        } else {
            let today = chrono::Local::now().date_naive();
            if i32::from(year) == chrono::Datelike::year(&today) {
                ((chrono::Datelike::month(&today) - 1) / 3 + 1) as u8
            } else {
                4
            }
        };
        Form1601EqDraft::new_from_profile(profile, year, quarter)
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

impl FormViewTrait for Form1601EqView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1601-EQ"
    }
    fn form_subtitle(&self) -> &'static str {
        "Quarterly Remittance Return of Creditable Income Taxes Withheld (Expanded)"
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
                    "1601-EQ draft saved.".into(),
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
        if !can_queue_for_submission(Form1601EqDraft::FORM_CODE) {
            self.status_message =
                Some("1601-EQ is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1601-EQ. No submission was started: {error}"
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
            "Form 1601-EQ queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1601-EQ payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1601EqDraft>(
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
            "1601eq-2019",
            &fields,
            "1601-EQ — Print Preview",
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

impl Render for Form1601EqView {
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
                .child(Button::new("1601eq_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1601eq_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1601eq_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1601eq_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1601eq_submit")
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

        let schedule = self.render_schedule(layout, cx);
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
                            Button::new("1601eq_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1601eq_release_cancel")
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
            .child(schedule);
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
                    .id("1601eq_scroll")
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
    use super::{Layout, atc_matches, format_tin, parse_amount};
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

    #[test]
    fn atc_finder_matches_code_and_description() {
        assert_eq!(atc_matches("wi010"), vec![0]);
        assert!(atc_matches("rentals").contains(&18));
        assert_eq!(atc_matches("").len(), 111);
    }
}
