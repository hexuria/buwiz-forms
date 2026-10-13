//! Editor for BIR Form 1603-Q, Quarterly Remittance Return of Final Income
//! Taxes Withheld on Fringe Benefits, January 2018. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1603q`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1603q::{
    FORM_1603Q_SCHEDULE_LINES, Form1603QCategory, Form1603QDraft, Form1603QTaxRelief,
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

use crate::components::form_engine::FormViewTrait;
use crate::components::penalty_section::{
    self, PenaltyPartyView, PenaltySection, PenaltySectionEvent,
};
use crate::views::form_controls::{messages_for, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::penalties::PenaltyComponent;
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form1603QView {}

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
    ("other_specify", "16 — Other remittances made (specify)", ""),
];

/// Money inputs: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("value0", "Schedule 1 line 1 (WF360) — monetary value"),
    ("value1", "Schedule 1 line 2 (WF330) — monetary value"),
    (
        "prev",
        "15 — Tax remitted in return previously filed (amended only)",
    ),
    ("other", "16 — Other remittances made"),
    ("sur", "19 — Surcharge"),
    ("int", "20 — Interest"),
    ("comp", "21 — Compromise"),
];

/// Model validation keys (`Form1603QDraft::validate`) behind one input key.
fn model_error_keys(key: &str) -> &'static [&'static str] {
    match key {
        "year" => &["taxable_year"],
        "sheets" => &["number_of_attached_sheets"],
        "name" => &["taxpayer_name"],
        "address" => &["registered_address"],
        "zip" => &["zip_code"],
        "phone" => &["contact_number"],
        "other_specify" => &["other_remittances_specify"],
        "value0" => &["monetary_value[0]", "monetary_value"],
        "value1" => &["monetary_value[1]", "monetary_value"],
        "prev" => &["tax_remitted_previous"],
        "other" => &["other_remittances"],
        "sur" => &["surcharge"],
        "int" => &["interest"],
        "comp" => &["compromise"],
        _ => &[],
    }
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

pub struct Form1603QView {
    draft: Form1603QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    header: FormHeaderState,
    /// The shared penalty section (computed values, overrides, note).
    penalty: Entity<PenaltySection>,
    _subscriptions: Vec<Subscription>,
}

impl Form1603QView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1603QDraft, key: &str) -> String {
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
            "other_specify" => draft.other_remittances_specify.clone(),
            "value0" => money(draft.monetary_value[0]),
            "value1" => money(draft.monetary_value[1]),
            "prev" => money(draft.tax_remitted_previous),
            "other" => money(draft.other_remittances),
            "sur" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Surcharge),
            "int" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Interest),
            "comp" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Compromise),
            _ => String::new(),
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
                    this.validated = false;
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
                    "value0" => draft.monetary_value[0] = value,
                    "value1" => draft.monetary_value[1] = value,
                    "prev" => draft.tax_remitted_previous = value,
                    "other" => draft.other_remittances = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.line_of_business = self.input_text("lob", cx);
        draft.other_remittances_specify = self.input_text("other_specify", cx);
        // An empty penalty box means "use computed"; any amount overrides it.
        parse_errors.extend(penalty_section::apply_overrides(
            &self.penalty,
            &Form1603QDraft::penalty_items(),
            &mut draft,
            cx,
        ));
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1603QDraft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        change(&mut self.draft);
        self.draft.recompute();
        self.write_back_amounts(window, cx);
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// Put each recomputed amount back into its editor when the editor no
    /// longer holds it (an Item 3 toggle zeroed a now-disabled box).
    fn write_back_amounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (key, _) in MONEY_INPUTS {
            let value = Self::initial(&self.draft, key);
            let current = self.input_text(key, cx);
            if parse_amount(&current) != parse_amount(&value) {
                if let Some(input) = self.inputs.get(*key) {
                    input.update(cx, |state, cx| state.set_value(value, window, cx));
                }
            }
        }
    }

    /// Item 4: like `changeTaxWitheld`, a new answer clears every amount, so
    /// the editors reload from the draft.
    fn set_withheld(&mut self, withheld: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        self.draft.set_any_tax_withheld(withheld);
        for (key, _) in MONEY_INPUTS {
            if let Some(input) = self.inputs.get(*key) {
                let value = Self::initial(&self.draft, key);
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.status_message = None;
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
                .release_abandoned_claimed_queueable::<Form1603QDraft>(
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
        let mut keys: Vec<&str> = vec![key];
        keys.extend_from_slice(model_error_keys(key));
        let message = self.error_for(&keys);
        let label = div()
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
        with_field_error(row.into_any_element(), message)
    }

    /// Messages (input parse errors and model validation) recorded for any of
    /// `keys`, joined into one line.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter()),
            keys,
        )
    }

    /// Put the messages for `keys` directly under an already-built row.
    fn with_errors(&self, row: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(row, self.error_for(keys))
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
                <div min_w_0 flex_1 max_w_full text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
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

    fn labelled_row(layout: Layout, label: &str, choices: Vec<AnyElement>) -> AnyElement {
        let label = div()
            .min_w_0()
            .flex_1()
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
                .child(label.w_full().flex_none())
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .children(choices),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(label)
                .children(choices)
                .into_any_element()
        }
    }

    /// A label with a fixed value; the library tile decided it.
    fn read_only_row(layout: Layout, label: &str, value: String) -> AnyElement {
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let value = div().font_weight(FontWeight::BOLD).child(value);
        match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(value)
                .into_any_element(),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.45)))
                .child(value)
                .into_any_element(),
        }
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let withheld = self.draft.any_tax_withheld;
        let children = vec![
            self.with_errors(
                Self::read_only_row(
                    layout,
                    "Item 1 — For the Year",
                    self.draft.taxable_year.to_string(),
                ),
                &["taxable_year", "year"],
            ),
            self.with_errors(
                Self::read_only_row(
                    layout,
                    "Item 2 — Quarter",
                    format!("Q{}", self.draft.quarter),
                ),
                &["quarter"],
            ),
            Self::labelled_row(
                layout,
                "Item 3 — Amended return?",
                vec![
                    Self::choice("1603q_amended_yes", "Yes", amended, !editable)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = true)
                        }))
                        .into_any_element(),
                    Self::choice("1603q_amended_no", "No", !amended, !editable)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = false)
                        }))
                        .into_any_element(),
                ],
            ),
            self.with_errors(
                Self::labelled_row(
                    layout,
                    "Item 4 — Any taxes withheld?",
                    vec![
                        Self::choice(
                            "1603q_withheld_yes",
                            "Yes",
                            withheld == Some(true),
                            !editable,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.set_withheld(true, window, cx)),
                        )
                        .into_any_element(),
                        Self::choice(
                            "1603q_withheld_no",
                            "No",
                            withheld == Some(false),
                            !editable,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.set_withheld(false, window, cx)),
                        )
                        .into_any_element(),
                    ],
                ),
                &["any_tax_withheld"],
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
            self.with_errors(
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
                &["tin"],
            ),
            self.with_errors(
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
                &["rdo_code"],
            ),
        ];
        for (key, label, _) in TEXT_INPUTS.iter().skip(2).take(6) {
            fields.push(self.field(key, label, layout, !editable));
        }
        let category_row = self.with_errors(
            Self::labelled_row(
                layout,
                "Item 11 — Category of withholding agent",
                vec![
                    Self::choice(
                        "1603q_category_private",
                        "Private",
                        category == Some(Form1603QCategory::Private),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.category = Some(Form1603QCategory::Private)
                        })
                    }))
                    .into_any_element(),
                    Self::choice(
                        "1603q_category_government",
                        "Government",
                        category == Some(Form1603QCategory::Government),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.category = Some(Form1603QCategory::Government)
                        })
                    }))
                    .into_any_element(),
                ],
            ),
            &["category"],
        );
        let relief = self.draft.tax_relief;
        let pick = |id: &'static str, label: &'static str, value: Option<Form1603QTaxRelief>| {
            Self::choice(id, label, relief == value, !editable)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.tax_relief = value)
                }))
                .into_any_element()
        };
        let mut relief_rows = vec![Self::labelled_row(
            layout,
            "Item 13 — Availing of tax relief under a Special Law or Tax Treaty?",
            vec![
                pick("1603q_relief_yes", "Yes", Some(relief.unwrap_or_default())),
                pick("1603q_relief_no", "No", None),
            ],
        )];
        if relief.is_some() {
            relief_rows.push(self.with_errors(
                Self::labelled_row(
                    layout,
                    "Item 13A — If yes, specify",
                    vec![
                        pick(
                            "1603q_relief_special",
                            "Special Rate",
                            Some(Form1603QTaxRelief::SpecialRate),
                        ),
                        pick(
                            "1603q_relief_treaty",
                            "International Tax Treaty",
                            Some(Form1603QTaxRelief::InternationalTaxTreaty),
                        ),
                        pick("1603q_relief_both", "Both", Some(Form1603QTaxRelief::Both)),
                    ],
                ),
                &["tax_relief"],
            ));
        }
        let mut children = vec![self.grid(layout, fields), category_row];
        children.extend(relief_rows);
        self.section("Part I — Background Information", children, cx)
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable =
            self.draft.lifecycle.is_editable() && self.draft.any_tax_withheld == Some(true);
        let d = &self.draft;
        let mut children = Vec::new();
        for (index, line) in FORM_1603Q_SCHEDULE_LINES.iter().enumerate() {
            let key = format!("value{index}");
            let body = vec![
                self.field(&key, MONEY_INPUTS[index].1, layout, !editable),
                self.computed(
                    &format!("Grossed-up monetary value (÷ {})", line.divisor_text),
                    d.grossed_up_value[index],
                    cx,
                ),
                self.computed(
                    &format!("Fringe benefit tax ({})", line.rate_text),
                    d.fringe_benefit_tax[index],
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
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{} — {}", line.atc_code, line.description)),
                    )
                    .child(self.grid(layout, body))
                    .into_any_element(),
            );
        }
        if self.draft.any_tax_withheld != Some(true) {
            children.push(
                div()
                    .text_sm()
                    .child("Answer Item 4 \"Yes\" to fill up Schedule 1.")
                    .into_any_element(),
            );
        }
        children.push(self.computed(
            "14 — Total taxes withheld (Schedule 1 total)",
            d.total_taxes_withheld,
            cx,
        ));
        self.section("Part IV — Schedule 1 (fringe benefits)", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![self.field(
            "prev",
            MONEY_INPUTS[2].1,
            layout,
            !editable || !d.is_amended,
        )];
        if layout == Layout::Desktop {
            // Keep item 15 on its own row so item 16's "(specify)" pairs with its amount.
            children.push(div().into_any_element());
        }
        children.extend([
            self.field("other_specify", TEXT_INPUTS[8].1, layout, !editable),
            self.field("other", MONEY_INPUTS[3].1, layout, !editable),
            self.computed("17 — Total remittances made", d.total_remittances, cx),
            self.computed("18 — Tax still due/(over-remittance)", d.tax_still_due, cx),
            penalty_section::render(
                &self.penalty,
                &PenaltyPartyView::all(&self.draft),
                &self
                    .validation_errors
                    .iter()
                    .chain(&self.parse_errors)
                    .cloned()
                    .collect::<Vec<_>>(),
                cx,
            ),
            self.computed("22 — Total penalties", d.total_penalties, cx),
            self.with_errors(
                self.computed("23 — Total amount still due", d.total_amount_due, cx),
                &["total_amount_due"],
            ),
        ]);
        self.section(
            "Part II — Computation of Tax (Items 14–23)",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form1603QView {
    type Draft = Form1603QDraft;

    fn new(
        draft: Form1603QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let validation_errors = draft.validate();
        let mut view = Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs: BTreeMap::new(),
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
            penalty: cx.new(|cx| PenaltySection::wrap(&[], Vec::new(), window, cx)),
            _subscriptions: Vec::new(),
        };
        for (key, _, placeholder) in TEXT_INPUTS {
            view.ensure_input(key.to_string(), placeholder, window, cx);
        }
        for (key, _) in MONEY_INPUTS {
            view.ensure_input(key.to_string(), "0.00", window, cx);
        }
        // The penalty boxes are overrides (empty means "use computed").
        let penalty_views = PenaltyPartyView::all(&view.draft);
        let penalty_inputs = vec![[
            view.inputs["sur"].clone(),
            view.inputs["int"].clone(),
            view.inputs["comp"].clone(),
        ]];
        view.penalty =
            cx.new(|cx| PenaltySection::wrap(&penalty_views, penalty_inputs, window, cx));
        let subscription = cx.subscribe_in(
            &view.penalty,
            window,
            |this: &mut Self, _, _: &PenaltySectionEvent, _, cx| {
                this.validated = false;
                this.sync_from_inputs(cx);
            },
        );
        view._subscriptions.push(subscription);
        view
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1603QDraft {
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
        Form1603QDraft::new_from_profile(profile, year, quarter)
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

impl FormViewTrait for Form1603QView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1603-Q"
    }
    fn form_subtitle(&self) -> &'static str {
        "Quarterly Remittance Return of Final Income Taxes Withheld on Fringe Benefits Paid to Employees Other Than Rank and File"
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
                    "1603-Q draft saved.".into(),
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
        if !can_queue_for_submission(Form1603QDraft::FORM_CODE) {
            self.status_message =
                Some("1603-Q is not enabled for in-app submission in this build.".into());
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
                "Form 1603-Q",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.validated = false;
        self.notify(
            notification::NotificationType::Success,
            "Form 1603-Q queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1603-Q payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    /// D7A + D8A: Queued reverts plainly (a claimed submission keeps the
    /// release confirm); Submitted/Confirmed keeps a snapshot of what BIR
    /// received and marks the return amended.
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let status = self.draft.lifecycle.status.clone();
        if !can_user_revert(&status) {
            return;
        }
        if matches!(status, FilingStatus::Queued) && !self.draft.lifecycle.is_unclaimed() {
            self.cancel_queue(window, cx);
            return;
        }
        match revert_for_amendment(&self.db, &self.draft) {
            Ok(reverted) => {
                self.draft = reverted.draft;
                self.validated = false;
                self.validation_errors = self.draft.validate();
                self.status_message = reverted
                    .snapshot_id
                    .map(|_| AMENDED_REVERT_MESSAGE.to_string());
                if reverted.snapshot_id.is_some() {
                    self.header.load_sent_snapshot(&self.db, &self.draft);
                }
                self.notify(
                    notification::NotificationType::Success,
                    "Form 1603-Q reverted to Draft.".into(),
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
            "1603q-2018",
            &fields,
            "1603-Q — Print Preview",
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

impl Render for Form1603QView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
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
        // Phone: no stepper, so the box sits at the top of the body.
        let (status_box, phone_status_box) = if phone {
            (None, status_box)
        } else {
            (status_box, None)
        };

        let toolbar = div()
            .flex()
            .items_center()
            .justify_between()
            .when(phone, |d| d.gap_1().py_2())
            .when(!phone, |d| d.gap_2().py_3())
            .px(pad)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("1603q_back")
                    .label("← Back")
                    .when(phone, |b| b.small())
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                    ),
            )
            .child(header_buttons);

        let mut body = div()
            .w_full()
            .max_w(CONTENT_MAX_W)
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .children(phone_status_box)
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("1603q_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1603q_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            });
        body = body
            .child(self.render_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_totals(layout, cx));
        body = body.child(self.render_schedule(layout, cx));

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
                    })
                    .children(status_box.map(|b| div().mt_3().child(b))),
            )
            .child(
                div()
                    .id("1603q_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form1603QView {
    /// The pre-header queue cancel, kept for a claimed submission.
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1603QDraft>(
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

    fn blocking_errors(&self) -> usize {
        self.parse_errors.len() + self.validation_errors.len()
    }

    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1603q",
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

    /// Validate: re-read the entries, run the official checks and mark the
    /// current entries validated.
    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
        self.validation_errors = self.draft.validate();
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1603QDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1603-Q marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1603QView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1603QView::blocking_errors,
    on_validate: Form1603QView::header_validate,
    on_submit: Form1603QView::mark_submitted,
    on_save: Form1603QView::save_draft,
    on_print: Form1603QView::preview_pdf,
    on_revert: Form1603QView::revert_to_draft,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1603QView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

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
