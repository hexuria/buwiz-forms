//! Editor for BIR Form 1604-F, Annual Information Return of Income Payments
//! Subjected to Final Withholding Taxes (January 2018). Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1604f`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1604f::{
    FORM_1604F_QUARTERS, FORM_1604F_SCHEDULE_TITLES, FORM_1604F_SCHEDULES, Form1604fAgentCategory,
    Form1604fDraft,
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
use crate::views::form_controls::{messages_for, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form1604fView {}

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

const QUARTER_LABELS: [&str; FORM_1604F_QUARTERS] =
    ["1st Quarter", "2nd Quarter", "3rd Quarter", "4th Quarter"];

/// Text inputs outside Part II: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 1 — For the year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("name", "Item 6 — Withholding agent's name", ""),
    ("address", "Item 7 — Registered address", ""),
    ("zip", "Item 7A — ZIP code", ""),
    ("phone", "Item 9 — Contact number", ""),
    ("email", "Item 10 — Email (BIR confirmation)", ""),
    ("lob", "Line of business", ""),
    ("relief", "Item 11A — Special law or tax treaty", ""),
];

/// The four editable cells of a schedule row.
const ROW_FIELDS: [(&str, &str, &str); 4] = [
    ("date", "Date of remittance", "MM/DD/YYYY"),
    ("ref", "TRA/eROR/eAR number", ""),
    ("tax", "Taxes withheld", "0.00"),
    ("pen", "Penalties", "0.00"),
];

fn row_key(schedule: usize, quarter: usize, field: &str) -> String {
    format!("s{schedule}q{quarter}_{field}")
}

/// Validation field of a row cell (`schedules[s][q].<field>`).
fn row_error_key(schedule: usize, quarter: usize, field: &str) -> String {
    let field = match field {
        "date" => "date",
        "ref" => "reference",
        "tax" => "taxes_withheld",
        _ => "penalties",
    };
    format!("schedules[{schedule}][{quarter}].{field}")
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

pub struct Form1604fView {
    draft: Form1604fDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
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

impl Form1604fView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1604fDraft, key: &str) -> String {
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
            "relief" => draft.tax_relief_details.clone(),
            _ => {
                for s in 0..FORM_1604F_SCHEDULES {
                    for q in 0..FORM_1604F_QUARTERS {
                        let row = &draft.schedules[s][q];
                        if key == row_key(s, q, "date") {
                            return row.date.clone();
                        } else if key == row_key(s, q, "ref") {
                            return row.reference.clone();
                        } else if key == row_key(s, q, "tax") {
                            return money(row.taxes_withheld);
                        } else if key == row_key(s, q, "pen") {
                            return money(row.penalties);
                        }
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
        // Item 1 year is chosen by the library tile and shown read-only; keep the draft's value.
        if let Some(sheets) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        for s in 0..FORM_1604F_SCHEDULES {
            for (q, quarter) in QUARTER_LABELS.iter().enumerate() {
                let label = format!("Schedule {} {quarter}", s + 1);
                if let Some(value) = number(
                    &row_key(s, q, "tax"),
                    &format!("{label} taxes withheld"),
                    cx,
                ) {
                    draft.schedules[s][q].taxes_withheld = value;
                }
                if let Some(value) =
                    number(&row_key(s, q, "pen"), &format!("{label} penalties"), cx)
                {
                    draft.schedules[s][q].penalties = value;
                }
            }
        }
        for s in 0..FORM_1604F_SCHEDULES {
            for q in 0..FORM_1604F_QUARTERS {
                draft.schedules[s][q].date = self
                    .input_text(&row_key(s, q, "date"), cx)
                    .trim()
                    .to_string();
                draft.schedules[s][q].reference = self.input_text(&row_key(s, q, "ref"), cx);
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.line_of_business = self.input_text("lob", cx);
        draft.tax_relief_details = self.input_text("relief", cx);
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

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1604fDraft)) {
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

    /// Item 11: "No" clears Item 11A, like `TaxReliefEnable`.
    fn set_tax_relief(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |d| d.tax_relief = on);
        if !on && let Some(input) = self.inputs.get("relief") {
            input.update(cx, |state, cx| state.set_value("", window, cx));
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1604fDraft>(
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

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form1604fDraft>(
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
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
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
        }
        .into_any_element();
        with_field_error(
            row,
            messages_for(
                self.parse_errors.iter().chain(&self.validation_errors),
                &[key, error_key],
            ),
        )
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
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
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

    fn choice_row(
        layout: Layout,
        label: &str,
        error: Option<String>,
        buttons: Vec<Button>,
    ) -> AnyElement {
        let row = Self::choice_row_inner(layout, label, error.is_some(), buttons);
        with_field_error(row, error)
    }

    fn choice_row_inner(
        layout: Layout,
        label: &str,
        error: bool,
        buttons: Vec<Button>,
    ) -> AnyElement {
        let caption = div()
            .min_w_0()
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
            return div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(caption)
                .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
                .into_any_element();
        }
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(caption.flex_1())
            .children(buttons)
            .into_any_element()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let children = vec![
            Self::readonly(TEXT_INPUTS[0].1, self.draft.taxable_year.to_string()),
            Self::choice_row(
                layout,
                "Item 2 — Amended return?",
                None,
                vec![
                    Self::choice("1604f_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("1604f_amended_no", "No", !amended, !editable).on_click(
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
            Self::readonly("Item 4 — TIN", format_tin(&d.tin)),
            Self::readonly("Item 5 — RDO Code", d.rdo_code.clone()),
        ];
        for (key, label, error_key) in [
            ("name", TEXT_INPUTS[2].1, "taxpayer_name"),
            ("address", TEXT_INPUTS[3].1, "registered_address"),
            ("zip", TEXT_INPUTS[4].1, "zip_code"),
        ] {
            fields.push(self.field(key, label, error_key, layout, !editable));
        }
        // Line of business is Item 6 on the (commented-out) HTA block, so it
        // sits right after the name.
        fields.insert(
            3,
            self.field(
                "lob",
                TEXT_INPUTS[7].1,
                "line_of_business",
                layout,
                !editable,
            ),
        );
        // Items 9 and 10 come after Items 8 and 8A.
        let contact_fields = vec![
            self.field(
                "phone",
                TEXT_INPUTS[5].1,
                "contact_number",
                layout,
                !editable,
            ),
            self.field("email", TEXT_INPUTS[6].1, "email", layout, !editable),
        ];
        let category = d.agent_category;
        let category_row = Self::choice_row(
            layout,
            "Item 8 — Category of withholding agent",
            messages_for(&self.validation_errors, &["agent_category"]),
            vec![
                Self::choice(
                    "1604f_private",
                    "Private",
                    category == Form1604fAgentCategory::Private,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| d.agent_category = Form1604fAgentCategory::Private)
                })),
                Self::choice(
                    "1604f_government",
                    "Government",
                    category == Form1604fAgentCategory::Government,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| {
                        d.agent_category = Form1604fAgentCategory::Government
                    })
                })),
            ],
        );
        let top = d.top_withholding_agent;
        let top_row =
            Self::choice_row(
                layout,
                "Item 8A — If private, top withholding agent?",
                None,
                vec![
                    Self::choice("1604f_top_yes", "Yes", top, !editable).on_click(cx.listener(
                        |this, _, _, cx| this.edit(cx, |d| d.top_withholding_agent = true),
                    )),
                    Self::choice("1604f_top_no", "No", !top, !editable).on_click(cx.listener(
                        |this, _, _, cx| this.edit(cx, |d| d.top_withholding_agent = false),
                    )),
                ],
            );
        let relief = d.tax_relief;
        let relief_row = Self::choice_row(
            layout,
            "Item 11 — Payees availing of tax relief under a Special Law or International Tax Treaty?",
            None,
            vec![
                Self::choice("1604f_relief_yes", "Yes", relief, !editable).on_click(
                    cx.listener(|this, _, window, cx| this.set_tax_relief(true, window, cx)),
                ),
                Self::choice("1604f_relief_no", "No", !relief, !editable).on_click(
                    cx.listener(|this, _, window, cx| this.set_tax_relief(false, window, cx)),
                ),
            ],
        );
        let mut children = vec![
            self.grid(layout, fields),
            category_row,
            top_row,
            self.grid(layout, contact_fields),
            relief_row,
        ];
        if relief {
            children.push(self.field(
                "relief",
                TEXT_INPUTS[8].1,
                "tax_relief_details",
                layout,
                !editable,
            ));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn render_row(&self, s: usize, q: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let row = &self.draft.schedules[s][q];
        let cells: Vec<AnyElement> = ROW_FIELDS
            .iter()
            .map(|(field, label, _)| {
                self.field(
                    &row_key(s, q, field),
                    label,
                    &row_error_key(s, q, field),
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
                    .child(QUARTER_LABELS[q].to_string()),
            )
            .child(body)
            .into_any_element()
    }

    fn render_schedule(&self, s: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let totals = &self.draft.totals[s];
        let mut children: Vec<AnyElement> = (0..FORM_1604F_QUARTERS)
            .map(|q| self.render_row(s, q, layout, cx))
            .collect();
        children.push(self.grid(
            layout,
            vec![
                self.computed("Total taxes withheld", totals.taxes_withheld, cx),
                self.computed("Total penalties", totals.penalties, cx),
                self.computed("Total amount remitted", totals.total_remitted, cx),
            ],
        ));
        self.section(FORM_1604F_SCHEDULE_TITLES[s], children, cx)
    }
}

impl QueueableFormView for Form1604fView {
    type Draft = Form1604fDraft;

    fn new(
        draft: Form1604fDraft,
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
        for s in 0..FORM_1604F_SCHEDULES {
            for q in 0..FORM_1604F_QUARTERS {
                for (field, _, placeholder) in ROW_FIELDS {
                    let key = row_key(s, q, field);
                    let value = Self::initial(&draft, &key);
                    keys.push((key, placeholder.to_string(), value));
                }
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
            header: FormHeaderState::new(),
            validated: false,
            _subscriptions: subscriptions,
        }
    }

    /// Annual return: the dashboard period is 0 and the draft is keyed by
    /// its year (`FilingPeriod::Annual`).
    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1604fDraft {
        Form1604fDraft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1604fView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1604-F"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Information Return of Income Payments Subjected to Final Withholding Taxes"
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
                    "1604-F draft saved.".into(),
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
        if !can_queue_for_submission(Form1604fDraft::FORM_CODE) {
            self.status_message =
                Some("1604-F is not enabled for in-app submission in this build.".into());
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
            self.status_message = Some(bir_core::forms::queueable::queue_failure_message(
                "Form 1604-F",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 1604-F queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1604-F is an information return; there is no payment to record.".into());
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
                    "Form 1604-F reverted to Draft.".into(),
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
        let fields = self.draft.to_print_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "1604f-2018",
            &fields,
            "1604-F — Print Preview",
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

impl Render for Form1604fView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
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

        let error_count = self.parse_errors.len() + self.validation_errors.len();
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
                .child(Button::new("1604f_back").label("← Back").on_click(
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
                            Button::new("1604f_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1604f_release_cancel")
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
            .child(self.render_part_one(layout, cx));
        for s in 0..FORM_1604F_SCHEDULES {
            body = body.child(self.render_schedule(s, layout, cx));
        }
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
                    .id("1604f_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form1604fView {
    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1604f",
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1604fDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1604-F marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1604fView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1604fView::blocking_errors,
    on_validate: Form1604fView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1604fView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, format_tin, parse_amount, row_error_key, row_key};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_1604f_view.rs");
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

    #[test]
    fn row_keys_match_validation_fields() {
        assert_eq!(row_key(1, 3, "tax"), "s1q3_tax");
        assert_eq!(row_error_key(1, 3, "ref"), "schedules[1][3].reference");
        assert_eq!(row_error_key(0, 0, "pen"), "schedules[0][0].penalties");
    }
}
