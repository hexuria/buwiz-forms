//! Editor for BIR Form 1701Q, Quarterly Income Tax Return for Individuals,
//! Estates and Trusts (January 2018). Rust owns every calculation,
//! validation and the official submit plaintext
//! (`bir_core::forms::form_1701q_official`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QDeductionMethod, Form1701QDraft, Form1701QFilerType, Form1701QParty,
    Form1701QSpouseType, Form1701QTaxRate,
};
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

use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{
    field_error, messages_for, new_select_state, on_select, select_field, sync_select,
    with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, FormHeaderActions, FormHeaderProps, FormHeaderState,
    render_header_buttons, render_status_box, revert_for_amendment, separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form1701QView {}

/// ATC dropdown options as owned `(code, "code label")` pairs.
fn atc_options(choices: &[Form1701QAtc]) -> Vec<(String, String)> {
    choices
        .iter()
        .map(|atc| {
            (
                atc.code().to_string(),
                format!("{} {}", atc.code(), atc.label()),
            )
        })
        .collect()
}

/// Borrow owned option pairs in the form the shared controls take.
fn option_refs(options: &[(String, String)]) -> Vec<(&str, &str)> {
    options
        .iter()
        .map(|(value, label)| (value.as_str(), label.as_str()))
        .collect()
}

/// The ATCs Item 8 offers: an estate or trust only has the graduated code.
fn taxpayer_atc_choices(estate: bool) -> Vec<Form1701QAtc> {
    if estate {
        vec![Form1701QAtc::Ii012]
    } else {
        Form1701QAtc::TAXPAYER_CHOICES.to_vec()
    }
}

/// Validation keys behind an input key, plus the parse-error label it uses.
fn error_keys(key: &str) -> Vec<String> {
    let model = match key {
        "sheets" => "number_of_sheets".to_string(),
        "name" => "taxpayer_name".to_string(),
        "address" => "registered_address".to_string(),
        "zip" => "zip_code".to_string(),
        "birth" => "date_of_birth".to_string(),
        "ftn" => "foreign_tax_number".to_string(),
        "spouse_rdo" => "spouse_rdo_code".to_string(),
        "spouse_ftn" => "spouse_foreign_tax_number".to_string(),
        "d43" => "item_43_description".to_string(),
        "d48" => "item_48_description".to_string(),
        "d61" => "item_61_description".to_string(),
        other => other.to_string(),
    };
    let mut keys = vec![model];
    if key == "sheets" {
        keys.push("Item 4".to_string());
    } else if let Some(item) = key
        .get(1..)
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|_| key.starts_with(['a', 'b']))
    {
        let (who, suffix) = if key.starts_with('a') {
            ("taxpayer", "A")
        } else {
            ("spouse", "B")
        };
        keys.push(format!("item_{item}_{who}"));
        if item == 59 {
            keys.push("item_59".to_string());
        }
        if let Some((_, label)) = AMOUNT_INPUTS.iter().find(|(i, _)| *i == item) {
            keys.push(format!("{item}{suffix} {label}"));
        }
    }
    keys
}

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

/// Free-text inputs: (key, label).
const TEXT_INPUTS: &[(&str, &str)] = &[
    ("sheets", "Item 4 — Number of sheets attached"),
    ("name", "Item 9 — Taxpayer/Filer's Name"),
    ("address", "Item 10 — Registered Address"),
    ("zip", "Item 10A — ZIP Code"),
    ("birth", "Item 11 — Date of Birth (MM/DD/YYYY)"),
    ("email", "Item 12 — Email Address"),
    ("citizenship", "Item 13 — Citizenship"),
    ("ftn", "Item 14 — Foreign Tax Number"),
    ("spouse_tin", "Item 17 — Spouse's TIN"),
    ("spouse_rdo", "Item 18 — Spouse's RDO Code"),
    ("spouse_name", "Item 21 — Spouse's Name"),
    ("spouse_citizenship", "Item 22 — Spouse's Citizenship"),
    ("spouse_ftn", "Item 23 — Spouse's Foreign Tax Number"),
    ("d43", "Item 43 — Other non-operating income (specify)"),
    ("d48", "Item 48 — Other non-operating income (specify)"),
    ("d61", "Item 61 — Other tax credits/payments (specify)"),
];

/// Amount items the filer types, with their labels.
const AMOUNT_INPUTS: &[(u8, &str)] = &[
    (36, "Sales/Revenues/Receipts/Fees"),
    (37, "Less: Cost of Sales/Services (itemized only)"),
    (39, "Less: Allowable Itemized Deductions"),
    (42, "Add: Taxable income/(loss) previous quarter"),
    (43, "Add: Other non-operating income"),
    (44, "Add: Amount received/share in income by partner"),
    (47, "Sales/Revenues/Receipts/Fees"),
    (48, "Add: Other non-operating income"),
    (50, "Add: Total taxable income/(loss) previous quarter"),
    (52, "Less: Allowable reduction (₱250,000)"),
    (55, "Prior year's excess credits"),
    (56, "Tax payment(s) for previous quarter(s)"),
    (57, "Creditable tax withheld for previous quarter(s)"),
    (58, "Creditable tax withheld per BIR Form 2307 this quarter"),
    (59, "Tax paid in return previously filed (amended)"),
    (60, "Foreign tax credits, if applicable"),
    (61, "Other tax credits/payments"),
    (64, "Surcharge"),
    (65, "Interest"),
    (66, "Compromise"),
];

fn amount_key(item: u8, party: Form1701QParty) -> String {
    match party {
        Form1701QParty::Taxpayer => format!("a{item}"),
        Form1701QParty::Spouse => format!("b{item}"),
    }
}

/// Accepts `1,234.56`, `1234.5`, `-50`, blank (zero).
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

fn money_text(value: Option<f64>) -> String {
    match value {
        Some(v) if v != 0.0 => official_amount(v),
        _ => String::new(),
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

const CONTENT_MAX_W: Pixels = px(1180.);

pub struct Form1701QView {
    draft: Form1701QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    atc_select: Entity<ComboboxState>,
    spouse_atc_select: Entity<ComboboxState>,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1701QView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1701QDraft, key: &str) -> String {
        match key {
            "sheets" => draft.number_of_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => format!("{}{}", draft.registered_address, draft.registered_address_2),
            "zip" => draft.zip_code.clone(),
            "birth" => draft.date_of_birth.clone(),
            "email" => draft.email.clone(),
            "citizenship" => draft.citizenship.clone(),
            "ftn" => draft.foreign_tax_number.clone(),
            "spouse_tin" => draft.spouse_tin.clone(),
            "spouse_rdo" => draft.spouse_rdo_code.clone(),
            "spouse_name" => draft.spouse_name.clone(),
            "spouse_citizenship" => draft.spouse_citizenship.clone(),
            "spouse_ftn" => draft.spouse_foreign_tax_number.clone(),
            "d43" => draft.item_43_non_operating_income_description.clone(),
            "d48" => draft.item_48_non_operating_income_description.clone(),
            "d61" => draft.item_61_other_tax_credit_description.clone(),
            _ => {
                let party = if key.starts_with('a') {
                    Form1701QParty::Taxpayer
                } else {
                    Form1701QParty::Spouse
                };
                key.get(1..)
                    .and_then(|n| n.parse::<u8>().ok())
                    .map(|item| money_text(draft.amount(item, party)))
                    .unwrap_or_default()
            }
        }
    }

    fn all_keys() -> Vec<String> {
        let mut keys: Vec<String> = TEXT_INPUTS.iter().map(|(k, _)| k.to_string()).collect();
        for (item, _) in AMOUNT_INPUTS {
            for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
                keys.push(amount_key(*item, party));
            }
        }
        keys
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
        let mut number = |text: String, label: &str| -> Option<f64> {
            let parsed = parse_amount(&text);
            if parsed.is_none() {
                parse_errors.push((
                    label.to_string(),
                    format!("{label}: \"{text}\" is not a number."),
                ));
            }
            parsed
        };
        let mut draft = self.draft.clone();
        if let Some(sheets) = number(self.input_text("sheets", cx), "Item 4") {
            draft.number_of_sheets = if (0.0..=99.0).contains(&sheets) {
                sheets as u8
            } else {
                u8::MAX
            };
        }
        for (item, label) in AMOUNT_INPUTS {
            for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
                let suffix = if party == Form1701QParty::Taxpayer {
                    "A"
                } else {
                    "B"
                };
                if let Some(value) = number(
                    self.input_text(&amount_key(*item, party), cx),
                    &format!("{item}{suffix} {label}"),
                ) {
                    draft.set_amount(*item, party, Some(value));
                }
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.registered_address_2.clear();
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.date_of_birth = self.input_text("birth", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.citizenship = self.input_text("citizenship", cx);
        draft.foreign_tax_number = self.input_text("ftn", cx).trim().to_string();
        // Line of business and telephone are not rendered; they keep the
        // profile values already in the draft.
        draft.spouse_tin = self.input_text("spouse_tin", cx).trim().to_string();
        draft.spouse_rdo_code = self.input_text("spouse_rdo", cx).trim().to_string();
        draft.spouse_name = self.input_text("spouse_name", cx);
        draft.spouse_citizenship = self.input_text("spouse_citizenship", cx);
        draft.spouse_foreign_tax_number = self.input_text("spouse_ftn", cx).trim().to_string();
        draft.item_43_non_operating_income_description = self.input_text("d43", cx);
        draft.item_48_non_operating_income_description = self.input_text("d48", cx);
        draft.item_61_other_tax_credit_description = self.input_text("d61", cx);
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

    /// Apply a choice: read the editors first, apply it, recompute, then put
    /// every recomputed amount back into its editor so a box a radio rule
    /// disabled or cleared never shows a figure the draft does not hold.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1701QDraft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        change(&mut self.draft);
        self.draft.recompute();
        self.write_back_amounts(window, cx);
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        self.validated = false;
        cx.notify();
    }

    /// Put each recomputed amount back into its editor when the editor no
    /// longer holds it.
    fn write_back_amounts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (item, _) in AMOUNT_INPUTS {
            for party in [Form1701QParty::Taxpayer, Form1701QParty::Spouse] {
                let key = amount_key(*item, party);
                let value = Self::initial(&self.draft, &key);
                let current = self.input_text(&key, cx);
                if parse_amount(&current) != parse_amount(&value) {
                    if let Some(input) = self.inputs.get(&key) {
                        input.update(cx, |state, cx| state.set_value(value, window, cx));
                    }
                }
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
                .release_abandoned_claimed_queueable::<Form1701QDraft>(
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
        let editable = self.draft.lifecycle.is_editable();
        let body = div().child(Input::new(input).disabled(disabled || !editable));
        let label = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let keys = error_keys(key);
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let label = if self.error_for(&keys).is_some() {
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
                .child(label.w(relative(0.5)))
                .child(body.w(relative(0.5))),
        }
        .into_any_element();
        self.with_err(row, &keys)
    }

    /// The messages recorded for any of `keys`, if any.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.validation_errors
                .iter()
                .chain(self.parse_errors.iter()),
            keys,
        )
    }

    /// `element` with the messages for `keys` directly under it.
    fn with_err(&self, element: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(element, self.error_for(keys))
    }

    fn text(&self, key: &str, layout: Layout) -> AnyElement {
        let label = TEXT_INPUTS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, l)| *l)
            .unwrap_or("");
        self.field(key, label, layout, false)
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

    fn computed(&self, label: &str, value: Option<f64>, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{official_amount(value.unwrap_or(0.0))}</div>
            </div>
        }
        .into_any_element()
    }

    /// Read-only item: the label, then the value in bold.
    fn read_only_row(layout: Layout, label: &str, value: String) -> AnyElement {
        let label = div()
            .min_w_0()
            .max_w_full()
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
                .child(value),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.5)))
                .child(value.w(relative(0.5))),
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

    fn choice_row(layout: Layout, title: &str, buttons: Vec<Button>) -> AnyElement {
        if layout == Layout::Phone {
            return div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(
                    div()
                        .min_w_0()
                        .max_w_full()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(title.to_string()),
                )
                .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
                .into_any_element();
        }
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
                    .child(title.to_string()),
            )
            .children(buttons)
            .into_any_element()
    }

    fn spouse_active(&self) -> bool {
        self.draft.has_spouse
            && !matches!(
                self.draft.filer_type,
                Some(Form1701QFilerType::Estate | Form1701QFilerType::Trust)
            )
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = d.is_amended;
        let children = vec![
            self.with_err(
                Self::read_only_row(layout, "Item 1 — For the year", d.taxable_year.to_string()),
                &["taxable_year"],
            ),
            self.with_err(
                Self::read_only_row(layout, "Item 2 — Quarter", format!("Q{}", d.quarter)),
                &["quarter"],
            ),
            Self::choice_row(
                layout,
                "Item 3 — Amended return?",
                vec![
                    Self::choice("1701q_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = true)
                        }),
                    ),
                    Self::choice("1701q_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = false)
                        }),
                    ),
                ],
            ),
            self.text("sheets", layout),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_taxpayer(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fixed = |label: &str, value: String| {
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
        };
        let head_fields = vec![
            self.with_err(fixed("Item 5 — TIN", format_tin(&d.tin)), &["tin"]),
            self.with_err(
                fixed("Item 6 — RDO Code", d.rdo_code.clone()),
                &["rdo_code"],
            ),
        ];
        let fields = vec![
            self.text("name", layout),
            self.text("address", layout),
            self.text("zip", layout),
            self.text("birth", layout),
            self.text("email", layout),
            self.text("citizenship", layout),
            self.text("ftn", layout),
        ];
        let filer: Vec<Button> = Form1701QFilerType::ALL
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                Self::choice(
                    ("1701q_filer", index),
                    kind.label(),
                    d.filer_type == Some(kind),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.filer_type = Some(kind))
                }))
            })
            .collect();
        let estate = matches!(
            d.filer_type,
            Some(Form1701QFilerType::Estate | Form1701QFilerType::Trust)
        );
        let atc_options = atc_options(&taxpayer_atc_choices(estate));
        let atc_field = select_field(
            "Item 8 — ATC",
            &self.atc_select,
            &option_refs(&atc_options),
            d.atc.map(Form1701QAtc::code),
            !editable,
            self.error_for(&["atc"]).as_deref(),
            cx,
        );
        let foreign = d.claims_foreign_tax_credits;
        let mut rows = vec![
            self.grid(layout, head_fields),
            self.with_err(
                Self::choice_row(layout, "Item 7 — Taxpayer/Filer type", filer),
                &["filer_type"],
            ),
            atc_field,
            self.grid(layout, fields),
            Self::choice_row(
                layout,
                "Item 15 — Claiming foreign tax credits?",
                vec![
                    Self::choice("1701q_ftc_yes", "Yes", foreign == Some(true), !editable)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.claims_foreign_tax_credits = Some(true))
                        })),
                    Self::choice("1701q_ftc_no", "No", foreign == Some(false), !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.claims_foreign_tax_credits = Some(false))
                        }),
                    ),
                ],
            ),
            self.with_err(
                div()
                    .text_sm()
                    .child(format!(
                        "Item 16 — Tax rate: {}",
                        d.tax_rate
                            .map(Form1701QTaxRate::label)
                            .unwrap_or("follows the ATC")
                    ))
                    .into_any_element(),
                &["tax_rate"],
            ),
        ];
        if d.tax_rate == Some(Form1701QTaxRate::Graduated) {
            let methods: Vec<Button> = Form1701QDeductionMethod::ALL
                .into_iter()
                .enumerate()
                .map(|(index, method)| {
                    Self::choice(
                        ("1701q_method", index),
                        method.label(),
                        d.deduction_method == Some(method),
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.deduction_method = Some(method))
                    }))
                })
                .collect();
            rows.push(self.with_err(
                Self::choice_row(layout, "Item 16A — Method of deduction", methods),
                &["deduction_method"],
            ));
        }
        rows.extend(self.render_profile_errors(cx));
        self.section(
            "Part I — Background Information on Taxpayer/Filer",
            rows,
            cx,
        )
    }

    /// Messages for the hidden profile values (Telephone, Line of business):
    /// they have no field here, so they are listed once with a pointer to
    /// Profile Settings. `None` when there are none.
    fn render_profile_errors(&self, cx: &Context<Self>) -> Option<AnyElement> {
        const LINES: [(&str, &[&str]); 2] = [
            ("Telephone", &["contact_number"]),
            ("Line of business", &["line_of_business"]),
        ];
        let lines: Vec<AnyElement> = LINES
            .iter()
            .filter_map(|(label, keys)| {
                self.error_for(keys)
                    .map(|message| field_error(format!("{label}: {message}")).into_any_element())
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(lines)
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            "These come from your taxpayer profile. Edit them in Profile Settings.",
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_spouse(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let estate = matches!(
            d.filer_type,
            Some(Form1701QFilerType::Estate | Form1701QFilerType::Trust)
        );
        let active = self.spouse_active();
        let mut rows = vec![Self::choice_row(
            layout,
            "Part II — Spouse information?",
            vec![
                Self::choice("1701q_spouse_yes", "Yes", active, !editable || estate).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.has_spouse = true)
                    }),
                ),
                Self::choice("1701q_spouse_no", "No", !active, !editable).on_click(cx.listener(
                    |this, _, window, cx| this.edit(window, cx, |d| d.has_spouse = false),
                )),
            ],
        )];
        if active {
            let kinds: Vec<Button> = Form1701QSpouseType::ALL
                .into_iter()
                .enumerate()
                .map(|(index, kind)| {
                    Self::choice(
                        ("1701q_spouse_type", index),
                        kind.label(),
                        d.spouse_type == Some(kind),
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.spouse_type = Some(kind))
                    }))
                })
                .collect();
            let compensation = d.spouse_type == Some(Form1701QSpouseType::CompensationEarner);
            let spouse_options = atc_options(&Form1701QAtc::SPOUSE_CHOICES);
            let spouse_atc_field = select_field(
                "Item 20 — Spouse ATC",
                &self.spouse_atc_select,
                &option_refs(&spouse_options),
                d.spouse_atc.map(Form1701QAtc::code),
                !editable || compensation,
                self.error_for(&["spouse_atc"]).as_deref(),
                cx,
            );
            let foreign = d.spouse_claims_foreign_tax_credits;
            rows.push(self.with_err(
                Self::choice_row(layout, "Item 19 — Spouse type", kinds),
                &["spouse_type"],
            ));
            rows.push(spouse_atc_field);
            rows.push(self.grid(
                layout,
                vec![
                    self.text("spouse_tin", layout),
                    self.text("spouse_rdo", layout),
                    self.text("spouse_name", layout),
                    self.text("spouse_citizenship", layout),
                    self.text("spouse_ftn", layout),
                ],
            ));
            rows.push(Self::choice_row(
                layout,
                "Item 24 — Claiming foreign tax credits?",
                vec![
                    Self::choice("1701q_sftc_yes", "Yes", foreign == Some(true), !editable)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| {
                                d.spouse_claims_foreign_tax_credits = Some(true)
                            })
                        })),
                    Self::choice("1701q_sftc_no", "No", foreign == Some(false), !editable)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| {
                                d.spouse_claims_foreign_tax_credits = Some(false)
                            })
                        })),
                ],
            ));
            if d.spouse_tax_rate == Some(Form1701QTaxRate::Graduated) {
                let methods: Vec<Button> = Form1701QDeductionMethod::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, method)| {
                        Self::choice(
                            ("1701q_spouse_method", index),
                            method.label(),
                            d.spouse_deduction_method == Some(method),
                            !editable,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit(window, cx, |d| d.spouse_deduction_method = Some(method))
                            },
                        ))
                    })
                    .collect();
                rows.push(self.with_err(
                    Self::choice_row(layout, "Item 25A — Method of deduction", methods),
                    &["spouse_deduction_method"],
                ));
            }
        }
        self.section("Part II — Background Information on Spouse", rows, cx)
    }

    /// One amount line: the taxpayer's column and, with a spouse, the
    /// spouse's column.
    fn amount_line(&self, item: u8, layout: Layout, enabled: [bool; 2]) -> AnyElement {
        let label = AMOUNT_INPUTS
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, l)| *l)
            .unwrap_or("");
        let mut cells = vec![self.field(
            &amount_key(item, Form1701QParty::Taxpayer),
            &format!("{item}A — {label}"),
            layout,
            !enabled[0],
        )];
        if self.spouse_active() {
            cells.push(self.field(
                &amount_key(item, Form1701QParty::Spouse),
                &format!("{item}B — {label}"),
                layout,
                !enabled[1],
            ));
        }
        self.grid(layout, cells)
    }

    fn computed_line(
        &self,
        item: u8,
        label: &str,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut cells = vec![self.computed(
            &format!("{item}A — {label}"),
            self.draft.amount(item, Form1701QParty::Taxpayer),
            cx,
        )];
        if self.spouse_active() {
            cells.push(self.computed(
                &format!("{item}B — {label}"),
                self.draft.amount(item, Form1701QParty::Spouse),
                cx,
            ));
        }
        self.grid(layout, cells)
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let rate = |party: Form1701QParty| match party {
            Form1701QParty::Taxpayer => d.tax_rate,
            Form1701QParty::Spouse => d.spouse_tax_rate,
        };
        let method = |party: Form1701QParty| match party {
            Form1701QParty::Taxpayer => d.deduction_method,
            Form1701QParty::Spouse => d.spouse_deduction_method,
        };
        let both = |f: &dyn Fn(Form1701QParty) -> bool| {
            [f(Form1701QParty::Taxpayer), f(Form1701QParty::Spouse)]
        };
        let graduated = both(&|p| rate(p) == Some(Form1701QTaxRate::Graduated));
        let itemized = both(&|p| {
            rate(p) == Some(Form1701QTaxRate::Graduated)
                && method(p) == Some(Form1701QDeductionMethod::Itemized)
        });
        let eight = both(&|p| rate(p) == Some(Form1701QTaxRate::EightPercent));
        let item52 = [
            eight[0] && d.atc != Some(Form1701QAtc::Ii016),
            eight[1] && d.spouse_atc != Some(Form1701QAtc::Ii016),
        ];
        let any = |flags: [bool; 2]| flags[0] || (self.spouse_active() && flags[1]);

        let mut children = Vec::new();
        if any(graduated) {
            children.push(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("Schedule I — Graduated rates")
                    .into_any_element(),
            );
            children.push(self.amount_line(36, layout, graduated));
            children.push(self.amount_line(37, layout, itemized));
            children.push(self.computed_line(38, "Gross income/(loss)", layout, cx));
            children.push(self.amount_line(39, layout, itemized));
            children.push(self.computed_line(40, "Optional standard deduction (40%)", layout, cx));
            children.push(self.computed_line(41, "Net income/(loss) this quarter", layout, cx));
            children.push(self.amount_line(42, layout, graduated));
            children.push(self.text("d43", layout));
            children.push(self.amount_line(43, layout, graduated));
            children.push(self.amount_line(44, layout, graduated));
            children.push(self.computed_line(45, "Total taxable income to date", layout, cx));
            children.push(self.computed_line(46, "Tax due", layout, cx));
        }
        if any(eight) {
            children.push(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("Schedule II — 8% rate")
                    .into_any_element(),
            );
            children.push(self.amount_line(47, layout, eight));
            children.push(self.text("d48", layout));
            children.push(self.amount_line(48, layout, eight));
            children.push(self.computed_line(49, "Total income this quarter", layout, cx));
            children.push(self.amount_line(50, layout, eight));
            children.push(self.computed_line(51, "Cumulative taxable income", layout, cx));
            children.push(self.amount_line(52, layout, item52));
            children.push(self.computed_line(53, "Taxable income to date", layout, cx));
            children.push(self.computed_line(54, "Tax due (8%)", layout, cx));
        }
        let open = [true, self.spouse_active()];
        let amended = [d.is_amended, d.is_amended];
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule III — Tax credits/payments")
                .into_any_element(),
        );
        for item in [55, 56, 57, 58] {
            children.push(self.amount_line(item, layout, open));
        }
        children.push(self.amount_line(59, layout, amended));
        children.push(self.amount_line(60, layout, open));
        children.push(self.text("d61", layout));
        children.push(self.amount_line(61, layout, open));
        children.push(self.computed_line(62, "Total tax credits/payments", layout, cx));
        children.push(self.computed_line(63, "Tax payable/(overpayment)", layout, cx));
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule IV — Penalties")
                .into_any_element(),
        );
        for item in [64, 65, 66] {
            children.push(self.amount_line(item, layout, open));
        }
        children.push(self.computed_line(67, "Total penalties", layout, cx));
        children.push(self.computed_line(68, "Total amount payable/(overpayment)", layout, cx));
        self.section("Part V — Computation of Tax", children, cx)
    }

    fn render_part_three(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let children = vec![
            self.computed_line(26, "Tax due", layout, cx),
            self.computed_line(27, "Less: Tax credits/payments", layout, cx),
            self.computed_line(28, "Tax payable/(overpayment)", layout, cx),
            self.computed_line(29, "Add: Total penalties", layout, cx),
            self.computed_line(30, "Total amount payable/(overpayment)", layout, cx),
            self.with_err(
                self.computed(
                    "31 — Aggregate amount payable/(overpayment)",
                    self.draft.item_31_aggregate_amount_payable,
                    cx,
                ),
                &["item_31"],
            ),
        ];
        self.section("Part III — Total Tax Payable", children, cx)
    }
}

impl QueueableFormView for Form1701QView {
    type Draft = Form1701QDraft;

    fn new(
        draft: Form1701QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for key in Self::all_keys() {
            let placeholder = if key.get(1..).is_some_and(|n| n.parse::<u8>().is_ok()) {
                "0.00"
            } else {
                ""
            };
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
        let all_taxpayer = atc_options(&Form1701QAtc::TAXPAYER_CHOICES);
        let atc_select = new_select_state(&option_refs(&all_taxpayer), window, cx);
        subscriptions.push(on_select(
            &atc_select,
            &option_refs(&all_taxpayer),
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                if let Some(atc) = Form1701QAtc::TAXPAYER_CHOICES
                    .into_iter()
                    .find(|a| a.code() == value)
                {
                    this.edit(window, cx, |d| d.atc = Some(atc));
                }
            },
        ));
        let all_spouse = atc_options(&Form1701QAtc::SPOUSE_CHOICES);
        let spouse_atc_select = new_select_state(&option_refs(&all_spouse), window, cx);
        subscriptions.push(on_select(
            &spouse_atc_select,
            &option_refs(&all_spouse),
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                if let Some(atc) = Form1701QAtc::SPOUSE_CHOICES
                    .into_iter()
                    .find(|a| a.code() == value)
                {
                    this.edit(window, cx, |d| d.spouse_atc = Some(atc));
                }
            },
        ));
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            header: FormHeaderState::new(),
            validated: false,
            atc_select,
            spouse_atc_select,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1701QDraft {
        let mut draft = Form1701QDraft::new_from_profile(profile, year, period.clamp(1, 3));
        draft.recompute();
        draft
    }
}

impl FormViewTrait for Form1701QView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701Q"
    }
    fn form_subtitle(&self) -> &'static str {
        "Quarterly Income Tax Return for Individuals, Estates and Trusts"
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
                    "1701Q draft saved.".into(),
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
        if !can_queue_for_submission(<Form1701QDraft as QueueableForm>::FORM_CODE) {
            self.status_message =
                Some("1701Q is not enabled for in-app submission in this build.".into());
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
                "Form 1701Q",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 1701Q queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1701Q payment status needs a verified confirmation workflow.".into());
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
                self.parse_errors.clear();
                if reverted.marked_amended {
                    // Show Item "Amended return?" = Yes and refresh every
                    // editor from the reverted draft.
                    self.edit(window, cx, |d| d.is_amended = true);
                }
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
                    "Form 1701Q reverted to Draft.".into(),
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
            "1701q-2018",
            &fields,
            "1701Q — Print Preview",
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

impl Render for Form1701QView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let estate = matches!(
            self.draft.filer_type,
            Some(Form1701QFilerType::Estate | Form1701QFilerType::Trust)
        );
        let taxpayer_options = atc_options(&taxpayer_atc_choices(estate));
        sync_select(
            &self.atc_select,
            &option_refs(&taxpayer_options),
            self.draft.atc.map(Form1701QAtc::code),
            window,
            cx,
        );
        let spouse_options = atc_options(&Form1701QAtc::SPOUSE_CHOICES);
        sync_select(
            &self.spouse_atc_select,
            &option_refs(&spouse_options),
            self.draft.spouse_atc.map(Form1701QAtc::code),
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
                .child(Button::new("1701q_back").label("← Back").on_click(
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
                            Button::new("1701q_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1701q_release_cancel")
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
            .child(self.render_taxpayer(layout, cx))
            .child(self.render_spouse(layout, cx))
            .child(self.render_part_three(layout, cx))
            .child(self.render_schedules(layout, cx));
        // Decision 4B: the printed declaration and Items 32-35 "Details of
        // Payment" are shown read-only; they are filled in at payment.
        body = body.children(
            crate::views::event_form_kit::declaration_and_payment_with_text(
                layout == Layout::Phone,
                "I declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me, and to the best of my knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I give my consent to the processing of my information as contemplated under the **Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes.",
                "Part IV – Details of Payment",
                &[
                    "32 Cash/Bank Debit Memo",
                    "33 Check",
                    "34 Tax Debit Memo",
                    "35 Others (specify)",
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
                    .id("1701q_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

/// The official success alert after a passing validation.
const VALIDATION_SUCCESS: &str =
    "Validation successful. Click on Edit if you wish to modify your entries.";

impl Form1701QView {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1701QDraft>(
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

    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1701q",
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
            success_text: VALIDATION_SUCCESS,
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1701QDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1701Q marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1701QView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1701QView::blocking_errors,
    on_validate: Form1701QView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1701QView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, amount_key, format_tin, parse_amount};
    use bir_core::forms::form_1701q::Form1701QParty;
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_1701q_view.rs");
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
    fn entries_parse_like_the_official_page() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount("-50"), Some(-50.0));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
        assert_eq!(amount_key(36, Form1701QParty::Spouse), "b36");
    }
}
