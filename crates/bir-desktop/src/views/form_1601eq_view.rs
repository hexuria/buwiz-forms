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
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;
use std::rc::Rc;

use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::components::penalty_section::{
    self, PenaltyPartyView, PenaltySection, PenaltySectionEvent,
};
use crate::views::form_controls::{
    self, MultiChange, messages_for, multi_select_field, with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::penalties::PenaltyComponent;
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

/// Every ATC of the official popup as `(code, "CODE (rate%) — description")`,
/// in popup order. The picker searches the whole label.
fn atc_pairs(year: u16) -> Vec<(String, String)> {
    FORM_1601EQ_ATC_OPTIONS
        .iter()
        .map(|option| {
            (
                option.code.to_string(),
                format!(
                    "{} ({}%) — {}",
                    option.code,
                    option.rate_text_for_year(year),
                    option.description
                ),
            )
        })
        .collect()
}

fn as_refs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(v, l)| (v.as_str(), l.as_str()))
        .collect()
}

/// The ticked ATC codes, in schedule order.
fn selected_atc_values(draft: &Form1601EqDraft) -> Vec<String> {
    draft
        .schedule
        .iter()
        .map(|row| row.atc_code.clone())
        .collect()
}

/// A change that would remove entered ATC rows and waits for confirmation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingWipe {
    /// Item 4 set to "No".
    WithheldNo,
    /// Item 11 switched to another category.
    Category(Form1601EqCategory),
}

impl PendingWipe {
    /// Official confirm text from the BIR page (`changeTaxWithheldNO`,
    /// `changeCategory`).
    fn question(self) -> &'static str {
        match self {
            Self::WithheldNo => {
                "You are about to change the value . Doing this will clear all \n computation fields Do you wish to proceed?"
            }
            Self::Category(_) => "By changing Category your selected ATC will be remove.",
        }
    }
}

const CONTENT_MAX_W: Pixels = px(1180.);

pub struct Form1601EqView {
    draft: Form1601EqDraft,
    /// Change waiting for the user's answer in the inline confirmation.
    pending_wipe: Option<PendingWipe>,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// The official submit confirm is open (OK queues, Cancel changes nothing).
    submit_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    /// Shared header: ⋯ menu, status box and "View what was sent".
    header: FormHeaderState,
    /// Searchable multi-select for the Part II ATCs (values are ATC codes).
    atc_select: Entity<ComboboxState>,
    /// The shared penalty section (computed values, overrides, note).
    penalty: Entity<PenaltySection>,
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
            "sur" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Surcharge),
            "int" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Interest),
            "comp" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Compromise),
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
        self._subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.validated = false;
                    this.sync_from_inputs(cx);
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
        // An empty penalty box means "use computed"; any amount overrides it.
        parse_errors.extend(penalty_section::apply_overrides(
            &self.penalty,
            &Form1601EqDraft::penalty_items(),
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

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1601EqDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.validated = false;
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// Apply `wipe` now when no entered row would be lost, else ask first.
    fn request_wipe(&mut self, wipe: PendingWipe, cx: &mut Context<Self>) {
        let loses_rows = match wipe {
            PendingWipe::WithheldNo => !self.draft.schedule.is_empty(),
            PendingWipe::Category(category) => {
                self.draft.category != Some(category) && !self.draft.schedule.is_empty()
            }
        };
        if loses_rows && self.draft.lifecycle.is_editable() {
            self.pending_wipe = Some(wipe);
            cx.notify();
        } else {
            self.apply_wipe(wipe, cx);
        }
    }

    fn apply_wipe(&mut self, wipe: PendingWipe, cx: &mut Context<Self>) {
        self.pending_wipe = None;
        self.edit(cx, |d| match wipe {
            PendingWipe::WithheldNo => d.set_any_tax_withheld(false),
            PendingWipe::Category(category) => d.set_category(category),
        });
    }

    /// Inline "official question + Yes/Cancel" box, shown only for `matches`.
    fn wipe_confirm(
        &self,
        matches: fn(PendingWipe) -> bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let wipe = self.pending_wipe.filter(|wipe| matches(*wipe))?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(cx.theme().muted)
                .child(div().text_sm().child(wipe.question()))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("1601eq_wipe_yes")
                                .label("Yes, remove")
                                .danger()
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.apply_wipe(wipe, cx)),
                                ),
                        )
                        .child(
                            Button::new("1601eq_wipe_cancel")
                                .label("Cancel")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pending_wipe = None;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Apply a new ATC list from the picker: untick what left, tick what
    /// arrived, through the model's own `remove_atc` / `add_atc`.
    fn apply_atc_selection(
        &mut self,
        next: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let current = selected_atc_values(&self.draft);
        for code in current.iter().filter(|c| !next.contains(c)) {
            self.remove_atc(code.clone(), window, cx);
        }
        for code in next.iter().filter(|c| !current.contains(c)) {
            self.add_atc(code, window, cx);
        }
    }

    fn add_atc(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut draft = self.draft.clone();
        match draft.add_atc(code) {
            Ok(()) => {
                self.draft = draft;
                self.validated = false;
                self.ensure_input(base_key(code), "0.00", window, cx);
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
        self.validated = false;
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
        let keys = self.error_keys(key);
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let error = self.error_for(&key_refs);
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if error.is_some() {
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
        with_field_error(row, error)
    }

    /// The model error keys an editor key feeds (plus the key itself, which
    /// carries the "not a number" parse error).
    fn error_keys(&self, key: &str) -> Vec<String> {
        let model = match key {
            "year" => "taxable_year",
            "sheets" => "number_of_attached_sheets",
            "name" => "taxpayer_name",
            "address" => "registered_address",
            "zip" => "zip_code",
            "phone" => "contact_number",
            "email" => "email",
            "lob" => "line_of_business",
            "rem1" => "remittance_first_month",
            "rem2" => "remittance_second_month",
            "prev" => "tax_remitted_previous",
            "over" => "over_remittance_previous_quarter",
            "sur" => "surcharge",
            "int" => "interest",
            "comp" => "compromise",
            _ => "",
        };
        let mut keys = vec![key.to_string()];
        if !model.is_empty() {
            keys.push(model.to_string());
        } else if let Some(code) = key.strip_prefix("base_")
            && let Some(index) = self.draft.schedule.iter().position(|r| r.atc_code == code)
        {
            keys.push(format!("schedule[{index}].tax_base"));
        }
        keys
    }

    /// Messages recorded for any of `keys` (validation and parse errors),
    /// joined into one line.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.validation_errors
                .iter()
                .chain(self.parse_errors.iter()),
            keys,
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

    /// A labelled row of choice buttons with its validation message (for
    /// `error_keys`) directly underneath.
    fn labelled_row(
        &self,
        label: &str,
        error_keys: &[&str],
        choices: Vec<AnyElement>,
    ) -> AnyElement {
        let row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .children(choices)
            .into_any_element();
        with_field_error(row, self.error_for(error_keys))
    }

    /// Plain label + value for an item the library tile decides, with its
    /// validation message (for `error_keys`) underneath.
    fn read_only_row(
        &self,
        layout: Layout,
        label: &str,
        value: String,
        error_keys: &[&str],
    ) -> AnyElement {
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let value = div().font_weight(FontWeight::BOLD).child(value);
        let row = match layout {
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
                .child(label.w(relative(0.45)))
                .child(value.w(relative(0.55))),
        }
        .into_any_element();
        with_field_error(row, self.error_for(error_keys))
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let withheld = self.draft.any_tax_withheld;
        let children = vec![
            self.read_only_row(
                layout,
                "Item 1 — For the Year",
                self.draft.taxable_year.to_string(),
                &["taxable_year"],
            ),
            self.read_only_row(
                layout,
                "Item 2 — Quarter",
                format!("Q{}", self.draft.quarter),
                &["quarter"],
            ),
            self.labelled_row(
                "Item 3 — Amended return?",
                &["is_amended"],
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
            self.labelled_row(
                "Item 4 — Any taxes withheld?",
                &["any_tax_withheld"],
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
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.request_wipe(PendingWipe::WithheldNo, cx)
                        }),
                    )
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
        let mut body = vec![self.grid(layout, children)];
        body.extend(self.wipe_confirm(|w| w == PendingWipe::WithheldNo, cx));
        self.section("Return period", body, cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let category = self.draft.category;
        let tile = |label: &str, value: String, keys: &[&str]| {
            let row = div()
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
                .into_any_element();
            with_field_error(row, self.error_for(keys))
        };
        let mut fields = vec![
            tile("Item 6 — TIN", format_tin(&self.draft.tin), &["tin"]),
            tile(
                "Item 7 — RDO Code",
                self.draft.rdo_code.clone(),
                &["rdo_code"],
            ),
        ];
        // Items 8-10, then Item 11 (category), then Item 12 (email) and the
        // line of business, in the official sheet order (CatAgent_P comes
        // before txtEmail).
        let mut after_category = Vec::new();
        for (key, label, _) in TEXT_INPUTS.iter().skip(2) {
            let field = self.field(key, label, layout, !editable);
            if matches!(*key, "email" | "lob") {
                after_category.push(field);
            } else {
                fields.push(field);
            }
        }
        let category_row = self.labelled_row(
            "Item 11 — Category of withholding agent",
            &["category"],
            vec![
                Self::choice(
                    "1601eq_category_private",
                    "Private",
                    category == Some(Form1601EqCategory::Private),
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_wipe(PendingWipe::Category(Form1601EqCategory::Private), cx)
                }))
                .into_any_element(),
                Self::choice(
                    "1601eq_category_government",
                    "Government",
                    category == Some(Form1601EqCategory::Government),
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_wipe(PendingWipe::Category(Form1601EqCategory::Government), cx)
                }))
                .into_any_element(),
            ],
        );
        let mut body = vec![self.grid(layout, fields), category_row];
        body.extend(self.wipe_confirm(|w| matches!(w, PendingWipe::Category(_)), cx));
        body.push(self.grid(layout, after_category));
        self.section("Part I — Background Information", body, cx)
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
            )
            .into_any_element();
        let heading = with_field_error(
            heading,
            self.error_for(&[format!("schedule[{index}].atc_code").as_str()]),
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

    fn render_schedule(&self, layout: Layout, view: &mut Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = (0..self.draft.schedule.len())
            .map(|row| self.render_atc_row(row, layout, view))
            .collect();
        let can_add =
            editable && self.draft.any_tax_withheld == Some(true) && self.draft.category.is_some();
        let atc_error = self.error_for(&["schedule"]);
        if can_add {
            let pairs = atc_pairs(self.draft.taxable_year);
            let selected = selected_atc_values(&self.draft);
            let on_change: MultiChange<Self> = Rc::new(Self::apply_atc_selection);
            children.push(multi_select_field(
                "1601eq_atc",
                "Add ATCs (search by code or nature of payment)",
                &self.atc_select,
                &as_refs(&pairs),
                &selected,
                pairs.len(),
                !editable,
                atc_error.as_deref(),
                on_change,
                view,
            ));
        } else if editable && self.draft.any_tax_withheld != Some(true) {
            children.push(with_field_error(
                div()
                    .text_sm()
                    .child("Answer Item 4 \"Yes\" and Item 11 to add ATCs.")
                    .into_any_element(),
                atc_error,
            ));
        } else if atc_error.is_some() {
            children.push(with_field_error(div().into_any_element(), atc_error));
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
        let total_keys: &[&str] = if d.total_amount_due < 0.0 {
            &["total_amount_due"]
        } else {
            &["total_amount_due", "over_remittance"]
        };
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
            self.computed("29 — Total penalties", d.total_penalties, cx),
            with_field_error(
                self.computed(
                    "30 — Total amount still due/(over-remittance)",
                    d.total_amount_due,
                    cx,
                ),
                // The over-remittance marks are only shown below zero; their
                // message belongs here otherwise.
                self.error_for(total_keys),
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
            children.push(self.labelled_row(
                "If over-remittance, mark one:",
                &["over_remittance"],
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
        let pairs = atc_pairs(draft.taxable_year);
        let refs = as_refs(&pairs);
        let atc_select = form_controls::new_select_state(&refs, window, cx);
        let on_change: MultiChange<Self> = Rc::new(Self::apply_atc_selection);
        let atc_subscription = form_controls::on_multi_select(
            &atc_select,
            &refs,
            pairs.len(),
            window,
            cx,
            |this: &Self| selected_atc_values(&this.draft),
            on_change,
        );
        let mut view = Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs: BTreeMap::new(),
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            submit_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
            pending_wipe: None,
            atc_select,
            penalty: cx.new(|cx| PenaltySection::wrap(&[], Vec::new(), window, cx)),
            _subscriptions: vec![atc_subscription],
        };
        for (key, _, placeholder) in TEXT_INPUTS {
            view.ensure_input(key.to_string(), placeholder, window, cx);
        }
        for (key, _) in MONEY_INPUTS {
            view.ensure_input(key.to_string(), "0.00", window, cx);
        }
        for code in codes {
            view.ensure_input(base_key(&code), "0.00", window, cx);
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
        self.submit_confirm_open = false;
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
                "Form 1601-EQ",
                &error,
            ));
            cx.notify();
            return;
        }
        // D5C: the toast and the stepper are the only confirmation.
        self.status_message = None;
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

    /// D7A + D8A: a claimed queue keeps the "nothing reached BIR" release;
    /// otherwise the generic revert (Submitted/Confirmed keep a snapshot of
    /// what BIR received and mark the return amended).
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let status = self.draft.lifecycle.status.clone();
        if !can_user_revert(&status) {
            return;
        }
        if matches!(status, FilingStatus::Queued) && !self.draft.lifecycle.is_unclaimed() {
            self.release_claim_confirm_open = true;
            self.status_message = Some(
                "Submission was claimed. Confirm nothing reached BIR to return it to an editable Draft. This does not file.".into(),
            );
            cx.notify();
            return;
        }
        match revert_for_amendment(&self.db, &self.draft) {
            Ok(reverted) => {
                self.draft = reverted.draft;
                self.validated = false;
                self.validation_errors = self.draft.validate();
                self.parse_errors.clear();
                self.status_message = reverted
                    .snapshot_id
                    .map(|_| AMENDED_REVERT_MESSAGE.to_string());
                if reverted.snapshot_id.is_some() {
                    self.header.load_sent_snapshot(&self.db, &self.draft);
                }
                self.notify(
                    notification::NotificationType::Success,
                    "Form 1601-EQ reverted to Draft.".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1601EqDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Error,
                    format!("Could not revert to Draft: {error}"),
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
                format!(
                    "{} This preview uses the January 2019 printed sheet (no January 2018 sheet is available). No filing state was changed.",
                    kind.status_message()
                ),
            )),
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                format!("{error}. No filing state was changed."),
            )),
        }
    }
}

impl Form1601EqView {
    /// Queue for submission: the same checks as queueing, then the official
    /// confirm prompt. Nothing changes until the filer clicks OK.
    fn request_submit(&mut self, cx: &mut Context<Self>) {
        self.submit_confirm_open = false;
        if !can_queue_for_submission(Form1601EqDraft::FORM_CODE) {
            self.status_message =
                Some("This form is not enabled for in-app submission in this build.".into());
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
        self.submit_confirm_open = true;
        cx.notify();
    }
}

impl Form1601EqView {
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1601eq",
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

    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
        self.validated = self.draft.lifecycle.is_editable();
        cx.notify();
    }

    /// Submit opens the official final-copy confirm; OK queues.
    fn header_submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.request_submit(cx);
    }

    fn header_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_draft(window, cx);
    }

    fn header_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_pdf(window, cx);
    }

    fn header_revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revert_to_draft(window, cx);
    }

    #[cfg(debug_assertions)]
    fn header_simulate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filename = self.draft.lifecycle.submission_filename.clone();
        match crate::dev_mode::simulate_confirmation(&self.db, filename.as_deref()) {
            Ok(()) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1601EqDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1601EQ marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1601EqView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: |this| this.parse_errors.len() + this.validation_errors.len(),
    on_validate: Form1601EqView::header_validate,
    on_submit: Form1601EqView::header_submit,
    on_save: Form1601EqView::header_save,
    on_print: Form1601EqView::header_print,
    on_revert: Form1601EqView::header_revert,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1601EqView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

/// The official submit prompt (rule 1601eq-final-001, `openAlertEmail`), verbatim.
const SUBMIT_CONFIRM_TEXT: &str = "Please ensure that you have INTERNET access and a VALID email address is indicated in your tax return.\n\nAre you sure you want to submit?";

impl Render for Form1601EqView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pairs = atc_pairs(self.draft.taxable_year);
        form_controls::sync_multi_select(
            &self.atc_select,
            &as_refs(&pairs),
            &selected_atc_values(&self.draft),
            window,
            cx,
        );
        let is_draft = self.draft.lifecycle.is_editable();
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };

        self.header.ensure_sent_snapshot(&self.db, &self.draft);
        let props = self.header_props();
        let header_buttons =
            render_header_buttons(self, &props, HEADER_ACTIONS, layout == Layout::Phone, cx);
        let status_box = render_status_box(self, &props, HEADER_ACTIONS, cx);
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
                .child(header_buttons);

        let schedule = self.render_schedule(layout, cx);
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
            .when(self.submit_confirm_open && is_draft, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(div().text_sm().child(SUBMIT_CONFIRM_TEXT))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    Button::new("1601eq_submit_confirm")
                                        .label("OK")
                                        .primary()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.mark_submitted(window, cx)
                                        })),
                                )
                                .child(
                                    Button::new("1601eq_submit_cancel")
                                        .label("Cancel")
                                        .outline()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.submit_confirm_open = false;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
            .child(self.render_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(schedule);
        body = body.child(self.render_totals(layout, cx));

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
                    .when_some(status_box, |d, b| d.child(div().mt_3().child(b))),
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
    #[test]
    fn submit_confirm_is_the_official_text() {
        let rules: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../rules/forms/1601eq-v2018/validations.json"
        ))
        .expect("rules json");
        let rules = rules
            .as_array()
            .or_else(|| rules.get("rules").and_then(|r| r.as_array()))
            .expect("rule list");
        let official = rules
            .iter()
            .find(|rule| rule["rule_id"] == "1601eq-final-001")
            .and_then(|rule| rule["exact_message"].as_str())
            .expect("final-copy rule")
            .replace("\\n", "\n");
        assert_eq!(super::SUBMIT_CONFIRM_TEXT, official);
    }

    use super::{Layout, atc_pairs, format_tin, parse_amount};
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
    fn atc_picker_lists_every_option_with_unique_labels() {
        let pairs = atc_pairs(2024);
        assert_eq!(pairs.len(), 111);
        assert_eq!(pairs[0].0, "WI010");
        assert!(pairs[0].1.starts_with("WI010 (5.0%)"));
        assert!(pairs[18].1.contains("RENTALS"));
        let mut labels: Vec<&String> = pairs.iter().map(|(_, l)| l).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 111);
        let mut codes: Vec<&String> = pairs.iter().map(|(c, _)| c).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), 111);
    }
}
