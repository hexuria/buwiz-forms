//! Editor for BIR Form 1604-C, Annual Information Return of Income Taxes
//! Withheld on Compensation (January 2018). Rust owns every calculation,
//! validation and the official submit plaintext
//! (`bir_core::forms::form_1604c`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1604c::{
    FORM_1604C_MONTH_NAMES, FORM_1604C_MONTHS, Form1604cAgentCategory, Form1604cDraft,
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
use crate::views::form_controls::{self, messages_for, select_field, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form1604cView {}

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

/// Text inputs outside Part II: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 1 — For the year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("name", "Item 6 — Withholding agent's name", ""),
    ("address", "Item 7 — Registered address", ""),
    ("zip", "Item 7A — ZIP code", ""),
    ("phone", "Item 9 — Contact number", ""),
    ("email", "Item 10 — Email address (BIR confirmation)", ""),
    ("refund_day", "Item 11A — Day of refund", "DD"),
    ("refund_year", "Item 11A — Year of refund", "YYYY"),
    (
        "overremittance",
        "Item 12 — Total amount of overremittance",
        "0.00",
    ),
];

/// The editable cells of a Part II row: (key, label, placeholder).
const ROW_FIELDS: [(&str, &str, &str); 6] = [
    ("date", "Date of remittance", "MM/DD/YYYY"),
    ("bank", "Drawee bank / bank code / agency", ""),
    ("ref", "TRA/eROR/eAR number", ""),
    ("tax", "Taxes withheld", "0.00"),
    ("adj", "Adjustment", "0.00"),
    ("pen", "Penalties", "0.00"),
];

/// Item 13 options: (stored month number, label).
const MONTH_OPTIONS: &[(&str, &str)] = &[
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

/// Item 11A month (`txtRefMonth`): (stored MM, label).
const REFUND_MONTH_OPTIONS: &[(&str, &str)] = &[
    ("01", "01"),
    ("02", "02"),
    ("03", "03"),
    ("04", "04"),
    ("05", "05"),
    ("06", "06"),
    ("07", "07"),
    ("08", "08"),
    ("09", "09"),
    ("10", "10"),
    ("11", "11"),
    ("12", "12"),
];

/// Item 11A `MM/DD/YYYY` split into month, day and year (blank parts empty).
fn refund_parts(date: &str) -> (String, String, String) {
    let mut parts = date.trim().splitn(3, '/');
    let mut next = || parts.next().unwrap_or("").trim().to_string();
    let month = next();
    let day = next();
    let year = next();
    let month = if REFUND_MONTH_OPTIONS.iter().any(|(v, _)| *v == month) {
        month
    } else {
        String::new()
    };
    (month, day, year)
}

/// Join Item 11A's parts the way the official page does
/// (`mm + "/" + dd + "/" + yyyy`, an unpicked month being "00").
fn refund_join(month: &str, day: &str, year: &str) -> String {
    if month.is_empty() && day.is_empty() && year.is_empty() {
        return String::new();
    }
    let month = if month.is_empty() { "00" } else { month };
    format!("{month}/{day}/{year}")
}

fn row_key(month: usize, field: &str) -> String {
    format!("m{month}_{field}")
}

/// Validation field of a row cell (`months[m].<field>`).
fn row_error_key(month: usize, field: &str) -> String {
    let field = match field {
        "date" => "date",
        "bank" => "bank",
        "ref" => "reference",
        "tax" => "taxes_withheld",
        "adj" => "adjustment",
        _ => "penalties",
    };
    format!("months[{month}].{field}")
}

/// Accepts `1,234.56`, `-1234.5`, blank (zero).
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

pub struct Form1604cView {
    draft: Form1604cDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    month_select: Entity<ComboboxState>,
    refund_month_select: Entity<ComboboxState>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    header: FormHeaderState,
    _subscriptions: Vec<Subscription>,
}

impl Form1604cView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1604cDraft, key: &str) -> String {
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
            "refund_day" => refund_parts(&draft.refund_date).1,
            "refund_year" => refund_parts(&draft.refund_date).2,
            "overremittance" => money(draft.overremittance),
            _ => {
                for (m, row) in draft.months.iter().enumerate() {
                    if key == row_key(m, "date") {
                        return row.date.clone();
                    } else if key == row_key(m, "bank") {
                        return row.bank.clone();
                    } else if key == row_key(m, "ref") {
                        return row.reference.clone();
                    } else if key == row_key(m, "tax") {
                        return money(row.taxes_withheld);
                    } else if key == row_key(m, "adj") {
                        return money(row.adjustment);
                    } else if key == row_key(m, "pen") {
                        return money(row.penalties);
                    }
                }
                String::new()
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
        if let Some(sheets) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        if draft.refunds_released
            && let Some(value) = number("overremittance", "Item 12", cx)
        {
            draft.overremittance = value;
        }
        for (m, name) in FORM_1604C_MONTH_NAMES.iter().enumerate() {
            if let Some(value) = number(&row_key(m, "tax"), &format!("{name} taxes withheld"), cx) {
                draft.months[m].taxes_withheld = value;
            }
            if let Some(value) = number(&row_key(m, "adj"), &format!("{name} adjustment"), cx) {
                draft.months[m].adjustment = value;
            }
            if let Some(value) = number(&row_key(m, "pen"), &format!("{name} penalties"), cx) {
                draft.months[m].penalties = value;
            }
        }
        for m in 0..FORM_1604C_MONTHS {
            draft.months[m].date = self.input_text(&row_key(m, "date"), cx).trim().to_string();
            draft.months[m].bank = self.input_text(&row_key(m, "bank"), cx);
            draft.months[m].reference = self.input_text(&row_key(m, "ref"), cx);
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        if draft.refunds_released {
            let (month, _, _) = refund_parts(&self.draft.refund_date);
            draft.refund_date = refund_join(
                &month,
                self.input_text("refund_day", cx).trim(),
                self.input_text("refund_year", cx).trim(),
            );
        }
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1604cDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// Item 11: "No" clears Items 11A–13, like `changeRefund`; "Yes" reads
    /// whatever the editors hold.
    fn set_refunds_released(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        self.draft.refunds_released = on;
        if !on {
            for key in ["refund_day", "refund_year", "overremittance"] {
                if let Some(input) = self.inputs.get(key) {
                    input.update(cx, |state, cx| state.set_value("", window, cx));
                }
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
                .release_abandoned_claimed_queueable::<Form1604cDraft>(
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

    fn has_error(&self, key: &str) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| field == key || field.starts_with(&format!("{key}[")))
    }

    /// A labelled input. On phones the label sits above the field.
    fn field(
        &self,
        key: &str,
        label: &str,
        error_key: &str,
        layout: Layout,
        disabled: bool,
    ) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let message = self.error_for(&[key, error_key]);
        let label = if self.has_error(key) || self.has_error(error_key) {
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

    fn readonly(label: &str, value: String) -> AnyElement {
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

    fn choice_row(layout: Layout, label: &str, error: bool, buttons: Vec<Button>) -> AnyElement {
        let caption = div()
            .min_w_0()
            .flex_1()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let caption = if error {
            caption.text_color(gpui::red())
        } else {
            caption
        };
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w_full()
                .child(caption.w_full().flex_none())
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .children(buttons),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(caption)
                .children(buttons)
                .into_any_element()
        }
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let children = vec![
            Self::readonly(TEXT_INPUTS[0].1, self.draft.taxable_year.to_string()),
            Self::choice_row(
                layout,
                "Item 2 — Amended return?",
                false,
                vec![
                    Self::choice("1604c_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("1604c_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
            ),
            self.field(
                "sheets",
                TEXT_INPUTS[1].1,
                "number_of_attached_sheets",
                layout,
                !editable,
            ),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut fields = vec![
            self.with_errors(Self::readonly("Item 4 — TIN", format_tin(&d.tin)), &["tin"]),
            self.with_errors(
                Self::readonly("Item 5 — RDO Code", d.rdo_code.clone()),
                &["rdo_code"],
            ),
        ];
        for (key, label, error_key) in [
            ("name", TEXT_INPUTS[2].1, "taxpayer_name"),
            ("address", TEXT_INPUTS[3].1, "registered_address"),
            ("zip", TEXT_INPUTS[4].1, "zip_code"),
            ("phone", TEXT_INPUTS[5].1, "contact_number"),
            ("email", TEXT_INPUTS[6].1, "email"),
        ] {
            fields.push(self.field(key, label, error_key, layout, !editable));
        }
        let category = d.agent_category;
        let category_row = self.with_errors(
            Self::choice_row(
                layout,
                "Item 8 — Category of withholding agent",
                self.has_error("agent_category"),
                vec![
                    Self::choice(
                        "1604c_private",
                        "Private",
                        category == Form1604cAgentCategory::Private,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| d.agent_category = Form1604cAgentCategory::Private)
                    })),
                    Self::choice(
                        "1604c_government",
                        "Government",
                        category == Form1604cAgentCategory::Government,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.agent_category = Form1604cAgentCategory::Government
                        })
                    })),
                ],
            ),
            &["agent_category"],
        );
        let top = d.top_withholding_agent;
        let top_row =
            Self::choice_row(
                layout,
                "Item 8A — If private, top withholding agent?",
                false,
                vec![
                    Self::choice("1604c_top_yes", "Yes", top, !editable).on_click(cx.listener(
                        |this, _, _, cx| this.edit(cx, |d| d.top_withholding_agent = true),
                    )),
                    Self::choice("1604c_top_no", "No", !top, !editable).on_click(cx.listener(
                        |this, _, _, cx| this.edit(cx, |d| d.top_withholding_agent = false),
                    )),
                ],
            );
        let released = d.refunds_released;
        let refund_row = Self::choice_row(
            layout,
            "Item 11 — Refunds of overwithholding/overremittance released to employees?",
            false,
            vec![
                Self::choice("1604c_refund_yes", "Yes", released, !editable).on_click(
                    cx.listener(|this, _, window, cx| this.set_refunds_released(true, window, cx)),
                ),
                Self::choice("1604c_refund_no", "No", !released, !editable).on_click(
                    cx.listener(|this, _, window, cx| this.set_refunds_released(false, window, cx)),
                ),
            ],
        );
        let mut children = vec![self.grid(layout, fields), category_row, top_row, refund_row];
        if released {
            let shown_month = d.first_crediting_month.to_string();
            let shown_month = (d.first_crediting_month != 0).then_some(shown_month.as_str());
            children.push(self.grid(
                layout,
                vec![
                    self.render_refund_date(layout, cx),
                    self.field(
                        "overremittance",
                        TEXT_INPUTS[9].1,
                        "overremittance",
                        layout,
                        !editable,
                    ),
                ],
            ));
            children.push(select_field(
                "Item 13 — Month of first crediting of overremittance",
                &self.month_select,
                MONTH_OPTIONS,
                shown_month,
                !editable,
                self.error_for(&["first_crediting_month"]).as_deref(),
                cx,
            ));
        }
        self.section("Part I — Background Information", children, cx)
    }

    /// Item 11A as the official month select plus day and year boxes
    /// (`txtRefMonth`, `txtRefDate`, `txtRefYear`).
    fn render_refund_date(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let (month, _, _) = refund_parts(&self.draft.refund_date);
        let month = (!month.is_empty()).then_some(month.as_str());
        let input = |key: &str| {
            let state = self
                .inputs
                .get(key)
                .expect("editor input registry is complete");
            div()
                .flex_1()
                .child(Input::new(state).disabled(!editable))
                .into_any_element()
        };
        let parts = div()
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .child(div().flex_1().child(select_field(
                "Month",
                &self.refund_month_select,
                REFUND_MONTH_OPTIONS,
                month,
                !editable,
                None,
                cx,
            )))
            .child(input("refund_day"))
            .child(input("refund_year"));
        let mut label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child("Item 11A — Date of refund (MM/DD/YYYY)");
        if self.has_error("refund_date") {
            label = label.text_color(gpui::red());
        }
        let row = match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(parts),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.45)))
                .child(parts.w(relative(0.55))),
        };
        self.with_errors(
            row.into_any_element(),
            &["refund_date", "refund_day", "refund_year"],
        )
    }

    fn render_row(&self, m: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let row = &self.draft.months[m];
        let cells: Vec<AnyElement> = ROW_FIELDS
            .iter()
            .map(|(field, label, _)| {
                self.field(
                    &row_key(m, field),
                    label,
                    &row_error_key(m, field),
                    layout,
                    !editable,
                )
            })
            .collect();
        let total = self.computed("Total amount remitted", row.total_remitted, cx);
        let body = if layout == Layout::Phone {
            div().flex().flex_col().gap_2().children(cells).child(total)
        } else {
            let mut iter = cells.into_iter();
            let mut grid = div().flex().flex_col().gap_2();
            while let Some(left) = iter.next() {
                let mut line = div().flex().gap_4().child(div().flex_1().child(left));
                if let Some(right) = iter.next() {
                    line = line.child(div().flex_1().child(right));
                }
                grid = grid.child(line);
            }
            grid.child(total)
        };
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
                    .child(FORM_1604C_MONTH_NAMES[m].to_string()),
            )
            .child(body)
            .into_any_element()
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children: Vec<AnyElement> = (0..FORM_1604C_MONTHS)
            .map(|m| self.render_row(m, layout, cx))
            .collect();
        children.push(self.grid(
            layout,
            vec![
                self.computed("Total taxes withheld", d.total_taxes_withheld, cx),
                self.computed("Total adjustment", d.total_adjustment, cx),
                self.computed("Total penalties", d.total_penalties, cx),
                self.with_errors(
                    self.computed(
                        "Total amount remitted (positive months)",
                        d.total_remitted,
                        cx,
                    ),
                    &["total_remitted"],
                ),
            ],
        ));
        self.section(
            "Part II — Summary of Remittances per BIR Form No. 1601-C",
            children,
            cx,
        )
    }
}

impl QueueableFormView for Form1604cView {
    type Draft = Form1604cDraft;

    const PRINT_SLUG: &'static str = "1604c-2018";

    fn print_fields(draft: &Form1604cDraft) -> std::collections::BTreeMap<String, String> {
        draft.to_print_field_map()
    }

    fn agent_draft(&self) -> &Form1604cDraft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        HEADER_ACTIONS
    }

    fn new(
        draft: Form1604cDraft,
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
        for m in 0..FORM_1604C_MONTHS {
            for (field, _, placeholder) in ROW_FIELDS {
                let key = row_key(m, field);
                let value = Self::initial(&draft, &key);
                keys.push((key, placeholder.to_string(), value));
            }
        }
        for (key, placeholder, value) in keys {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.validated = false;
                        this.sync_from_inputs(cx);
                    }
                },
            ));
            inputs.insert(key, input);
        }
        let month_select = form_controls::new_select_state(MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &month_select,
            MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                if let Ok(month) = value.parse::<u8>() {
                    this.edit(cx, |d| d.first_crediting_month = month);
                }
            },
        ));
        let refund_month_select = form_controls::new_select_state(REFUND_MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &refund_month_select,
            REFUND_MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                let day = this.input_text("refund_day", cx).trim().to_string();
                let year = this.input_text("refund_year", cx).trim().to_string();
                this.edit(cx, |d| {
                    if d.refunds_released {
                        d.refund_date = refund_join(&value, &day, &year);
                    }
                });
            },
        ));
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            month_select,
            refund_month_select,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Annual return: the dashboard period is 0 and the draft is keyed by
    /// its year (`FilingPeriod::Annual`).
    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1604cDraft {
        Form1604cDraft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1604cView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1604-C"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Information Return of Income Taxes Withheld on Compensation"
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
                    "1604-C draft saved.".into(),
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
        if !can_queue_for_submission(Form1604cDraft::FORM_CODE) {
            self.status_message =
                Some("1604-C is not enabled for in-app submission in this build.".into());
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
                "Form 1604-C",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.validated = false;
        self.notify(
            notification::NotificationType::Success,
            "Form 1604-C queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1604-C is an information return; there is no payment to record.".into());
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
                    "Form 1604-C reverted to Draft.".into(),
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
        let fields = Self::print_fields(&self.draft);
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            Self::PRINT_SLUG,
            &fields,
            "1604-C — Print Preview",
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

impl Render for Form1604cView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let shown_month = self.draft.first_crediting_month.to_string();
        let shown_month = (self.draft.first_crediting_month != 0).then_some(shown_month.as_str());
        form_controls::sync_select(&self.month_select, MONTH_OPTIONS, shown_month, window, cx);
        let (refund_month, _, _) = refund_parts(&self.draft.refund_date);
        let refund_month = (!refund_month.is_empty()).then_some(refund_month.as_str());
        form_controls::sync_select(
            &self.refund_month_select,
            REFUND_MONTH_OPTIONS,
            refund_month,
            window,
            cx,
        );

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
                Button::new("1604c_back")
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
                            Button::new("1604c_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1604c_release_cancel")
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
            .child(self.render_part_one(layout, cx));
        body = body.child(self.render_part_two(layout, cx));

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
                    .id("1604c_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form1604cView {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1604cDraft>(
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
            slug: "1604c",
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1604cDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1604-C marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1604cView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1604cView::blocking_errors,
    on_validate: Form1604cView::header_validate,
    on_submit: Form1604cView::mark_submitted,
    on_save: Form1604cView::save_draft,
    on_print: Form1604cView::preview_pdf,
    on_revert: Form1604cView::revert_to_draft,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1604cView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount, row_error_key, row_key};
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
        assert_eq!(parse_amount("-500.25"), Some(-500.25));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }

    #[test]
    fn row_keys_match_validation_fields() {
        assert_eq!(row_key(11, "adj"), "m11_adj");
        assert_eq!(row_error_key(11, "ref"), "months[11].reference");
        assert_eq!(row_error_key(0, "bank"), "months[0].bank");
    }
}
