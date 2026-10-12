//! Editor for BIR Form 2000-OT (v2018), Documentary Stamp Tax
//! Declaration/Return for One-Time Transactions. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_2000ot`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2000ot::{
    FORM_2000OT_MAX_ROWS, FORM_2000OT_REAL_PROPERTY_ROWS, FORM_2000OT_SHARE_ROWS, Form2000OTAtc,
    Form2000OTDraft, Form2000OTNature, Form2000OTOtherParty, Form2000OTPropertyRow,
    Form2000OTShareRow,
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
use crate::views::form_controls::{self, messages_for, select_field, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, FormHeaderActions, FormHeaderProps, FormHeaderState,
    render_header_buttons, render_status_box, revert_for_amendment, separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

/// Item 3 options: (stored value, label).
/// Info note beside DO102/DO122 (owner decision Q6A): the tax keeps BIR's
/// page floor/round computation.
const BLOCK_ROUNDING_NOTE: &str = "Computed like BIR's eBIRForms. The law's 'per block or fraction thereof' may mean a higher amount; check with your RDO.";

const ATC_OPTIONS: &[(&str, &str)] = &[
    ("DO102", "DO102 — shares with par value (P1.50 / P200)"),
    ("DO122", "DO122 — real property (P15 / P1,000)"),
    ("DO125", "DO125 — stock without par value (50% of DST paid)"),
];

fn atc_value(atc: Form2000OTAtc) -> Option<&'static str> {
    match atc {
        Form2000OTAtc::Do102 => Some("DO102"),
        Form2000OTAtc::Do122 => Some("DO122"),
        Form2000OTAtc::Do125 => Some("DO125"),
        Form2000OTAtc::Unanswered => None,
    }
}

fn atc_from_value(value: &str) -> Option<Form2000OTAtc> {
    match value {
        "DO102" => Some(Form2000OTAtc::Do102),
        "DO122" => Some(Form2000OTAtc::Do122),
        "DO125" => Some(Form2000OTAtc::Do125),
        _ => None,
    }
}

impl EventEmitter<QueueableFormEvent> for Form2000OTView {}

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

/// Single text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("date", "Item 1 — Date of transaction", "MM/DD/YYYY"),
    ("sheets", "Item 4 — No. of sheets attached", "0"),
    ("name", "Item 7 — Taxpayer's Name", ""),
    ("address", "Item 8 — Registered Address", ""),
    ("zip", "Item 8A — Zip Code", ""),
    ("phone", "Item 9 — Contact Number", "digits only"),
    ("email", "Item 10 — Email Address", ""),
    ("oname", "Item 11A — Other party's name", ""),
    ("otin", "Item 11B — Other party's TIN", "digits only"),
    ("loc", "Item 13 — Location of real property", ""),
    ("others", "Schedule 1C — Others (specify)", ""),
];

/// Money inputs outside the schedule rows: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "gsp",
        "Schedule 1A — Gross selling price / bid price / net gift",
    ),
    ("othamt", "Schedule 1C — Others amount"),
    (
        "prev",
        "18 — Tax paid in return previously filed (amended only)",
    ),
    ("sur", "20A — Surcharge"),
    ("int", "20B — Interest"),
    ("comp", "20C — Compromise"),
];

/// Schedule 1.A row inputs: (prefix, label, money?).
const PROPERTY_INPUTS: &[(&str, &str, bool)] = &[
    ("ptct", "OCT/TCT/CCT No.", false),
    ("ptd", "Tax Declaration No.", false),
    ("ploc", "Location", false),
    ("plot", "Lot / Improvement", false),
    ("pcls", "Classification", false),
    ("parea", "Area (sq.m.)", false),
    ("pfmv1", "FMV per TD (Column 1)", true),
    ("pfmv2", "FMV per BIR zonal value (Column 2)", true),
];

/// Schedule 1.B row inputs: (prefix, label, money?).
const SHARE_INPUTS: &[(&str, &str, bool)] = &[
    ("scorp", "(a) Name of corporation", false),
    ("sno", "(b) No. of shares sold", false),
    ("scert", "(c) Stock certificate no.", false),
    ("spar", "(d) Par value of shares", true),
    ("sdst", "(e) DST paid on original issue (no par)", true),
];

fn row_key(prefix: &str, row: usize) -> String {
    format!("{prefix}{row}")
}

/// Split a row editor key into its prefix and row index. Some prefixes end
/// in a digit (`pfmv1`), so match against the known prefixes.
fn split_row_key(key: &str) -> Option<(&'static str, usize)> {
    PROPERTY_INPUTS
        .iter()
        .chain(SHARE_INPUTS)
        .find_map(|(prefix, _, _)| {
            let row = key.strip_prefix(prefix)?.parse::<usize>().ok()?;
            Some((*prefix, row))
        })
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

/// Item 4 as typed: blank is 0, otherwise digits only (no sign, no
/// decimals), up to 999. Anything else is a parse error, never cast.
fn parse_sheet_count(text: &str) -> Option<u16> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    if !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse::<u16>().ok().filter(|n| *n <= 999)
}

fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

pub struct Form2000OTView {
    draft: Form2000OTDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Rows opened per schedule: Schedule 1.A, then 1.B. Blank trailing rows
    /// are not kept in the draft, so the page remembers them.
    visible_rows: [usize; 2],
    /// Item 3 ATC dropdown.
    atc_select: Entity<ComboboxState>,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2000OTView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2000OTDraft, key: &str) -> String {
        match key {
            "date" => draft.transaction_date.clone(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "oname" => draft.other_party_name.clone(),
            "otin" => draft.other_party_tin.clone(),
            "loc" => draft.real_property_location.clone(),
            "others" => draft.others_description.clone(),
            "gsp" => money_text(draft.gross_selling_price),
            "othamt" => money_text(draft.others_amount),
            "prev" => money_text(draft.tax_paid_previous),
            "sur" => money_text(draft.surcharge),
            "int" => money_text(draft.interest),
            "comp" => money_text(draft.compromise),
            _ => {
                let Some((prefix, row)) = split_row_key(key) else {
                    return String::new();
                };
                if let Some(p) = draft.properties.get(row) {
                    let value = match prefix {
                        "ptct" => Some(p.title_number.clone()),
                        "ptd" => Some(p.tax_declaration_number.clone()),
                        "ploc" => Some(p.location.clone()),
                        "plot" => Some(p.lot.clone()),
                        "pcls" => Some(p.classification.clone()),
                        "parea" => Some(p.area.clone()),
                        "pfmv1" => Some(money_text(p.fmv_per_td)),
                        "pfmv2" => Some(money_text(p.fmv_zonal)),
                        _ => None,
                    };
                    if let Some(value) = value {
                        return value;
                    }
                }
                if let Some(s) = draft.shares.get(row) {
                    return match prefix {
                        "scorp" => s.corporation.clone(),
                        "sno" => s.shares_sold.clone(),
                        "scert" => s.certificate_number.clone(),
                        "spar" => money_text(s.par_value),
                        "sdst" => money_text(s.dst_paid_on_issue),
                        _ => String::new(),
                    };
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
        let mut draft = self.draft.clone();
        // Item 4 is a count: anything but digits (2.5, -1) is reported and
        // the stored value is left unchanged, never cast.
        let sheets_text = self.input_text("sheets", cx);
        match parse_sheet_count(&sheets_text) {
            Some(sheets) => draft.number_of_attached_sheets = sheets,
            None => parse_errors.push((
                "sheets".to_string(),
                format!("Item 4: \"{}\" is not a whole number.", sheets_text.trim()),
            )),
        }
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
        for (key, label) in MONEY_INPUTS {
            if let Some(value) = number(key, label, cx) {
                match *key {
                    "gsp" => draft.gross_selling_price = value,
                    "othamt" => draft.others_amount = value,
                    "prev" => draft.tax_paid_previous = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        let mut properties = Vec::new();
        for row in 0..self.property_row_count() {
            let text = |prefix: &str| {
                self.input_text(&row_key(prefix, row), cx)
                    .trim()
                    .to_string()
            };
            let fmv_per_td =
                number(&row_key("pfmv1", row), "Schedule 1.A column 1", cx).unwrap_or(0.0);
            let fmv_zonal =
                number(&row_key("pfmv2", row), "Schedule 1.A column 2", cx).unwrap_or(0.0);
            properties.push(Form2000OTPropertyRow {
                title_number: text("ptct"),
                tax_declaration_number: text("ptd"),
                location: text("ploc"),
                lot: text("plot"),
                classification: text("pcls"),
                area: text("parea"),
                fmv_per_td,
                fmv_zonal,
                fmv: 0.0,
            });
        }
        while properties
            .last()
            .is_some_and(|r| *r == Form2000OTPropertyRow::default())
        {
            properties.pop();
        }
        let mut shares = Vec::new();
        for row in 0..self.share_row_count() {
            let text = |prefix: &str| {
                self.input_text(&row_key(prefix, row), cx)
                    .trim()
                    .to_string()
            };
            let par_value = number(&row_key("spar", row), "Schedule 1.B (d)", cx).unwrap_or(0.0);
            let dst_paid_on_issue =
                number(&row_key("sdst", row), "Schedule 1.B (e)", cx).unwrap_or(0.0);
            shares.push(Form2000OTShareRow {
                corporation: text("scorp"),
                shares_sold: text("sno"),
                certificate_number: text("scert"),
                par_value,
                dst_paid_on_issue,
            });
        }
        while shares
            .last()
            .is_some_and(|r| *r == Form2000OTShareRow::default())
        {
            shares.pop();
        }
        draft.properties = properties;
        draft.shares = shares;
        draft.transaction_date = self.input_text("date", cx).trim().to_string();
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.other_party_name = self.input_text("oname", cx);
        draft.other_party_tin = self.input_text("otin", cx).trim().to_string();
        draft.real_property_location = self.input_text("loc", cx);
        draft.others_description = self.input_text("others", cx).trim().to_string();
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

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2000OTDraft)) {
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

    /// Schedule 1.A rows to show: the default, every row the draft holds, and
    /// any row the user has opened.
    fn property_row_count(&self) -> usize {
        FORM_2000OT_REAL_PROPERTY_ROWS
            .max(self.draft.properties.len())
            .max(self.visible_rows[0])
    }

    /// Schedule 1.B rows to show.
    fn share_row_count(&self) -> usize {
        FORM_2000OT_SHARE_ROWS
            .max(self.draft.shares.len())
            .max(self.visible_rows[1])
    }

    /// Editor keys (with placeholders) for one row index of a schedule
    /// (`0` = Schedule 1.A, otherwise 1.B).
    fn row_input_keys(schedule: usize, row: usize) -> Vec<(String, String)> {
        let inputs = if schedule == 0 {
            PROPERTY_INPUTS
        } else {
            SHARE_INPUTS
        };
        inputs
            .iter()
            .map(|(prefix, _, money)| {
                (
                    row_key(prefix, row),
                    if *money { "0.00" } else { "" }.to_string(),
                )
            })
            .collect()
    }

    /// Create the editors for one row index on demand.
    fn ensure_row_inputs(
        &mut self,
        schedule: usize,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (key, placeholder) in Self::row_input_keys(schedule, row) {
            if self.inputs.contains_key(&key) {
                continue;
            }
            let value = Self::initial(&self.draft, &key);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            self._subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
            self.inputs.insert(key, input);
        }
    }

    fn schedule_count(&self, schedule: usize) -> usize {
        if schedule == 0 {
            self.property_row_count()
        } else {
            self.share_row_count()
        }
    }

    fn schedule_default(schedule: usize) -> usize {
        if schedule == 0 {
            FORM_2000OT_REAL_PROPERTY_ROWS
        } else {
            FORM_2000OT_SHARE_ROWS
        }
    }

    fn add_row(&mut self, schedule: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let count = self.schedule_count(schedule);
        if count >= FORM_2000OT_MAX_ROWS {
            return;
        }
        self.ensure_row_inputs(schedule, count, window, cx);
        self.visible_rows[schedule] = count + 1;
        cx.notify();
    }

    /// Delete a row beyond the default count; later rows move up.
    fn remove_row(
        &mut self,
        schedule: usize,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let count = self.schedule_count(schedule);
        let default = Self::schedule_default(schedule);
        if row < default || row >= count {
            return;
        }
        if schedule == 0 {
            if row < self.draft.properties.len() {
                self.draft.properties.remove(row);
            }
        } else if row < self.draft.shares.len() {
            self.draft.shares.remove(row);
        }
        self.visible_rows[schedule] = (count - 1).max(default);
        self.draft.recompute();
        self.validated = false;
        // Reload every row editor from the draft, then read them back.
        let keys: Vec<String> = self
            .inputs
            .keys()
            .filter(|key| split_row_key(key).is_some())
            .cloned()
            .collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.sync_from_inputs(cx);
    }

    fn add_row_controls(&self, schedule: usize, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let at_max = self.schedule_count(schedule) >= FORM_2000OT_MAX_ROWS;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                Button::new(("2000ot_add_row", schedule))
                    .label("Add row")
                    .outline()
                    .small()
                    .disabled(!editable || at_max)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.add_row(schedule, window, cx)),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Up to {FORM_2000OT_MAX_ROWS} rows")),
            )
            .into_any_element()
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
                .release_abandoned_claimed_queueable::<Form2000OTDraft>(
                    &draft.tin,
                    draft.taxable_year(),
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
        let mut keys = vec![key.to_string()];
        keys.extend(Self::model_keys(key));
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let message = self.errors_for(&keys);
        let error = message.is_some();
        let label = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if error {
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

    /// Validator keys (`Form2000OTDraft::validate`) behind input `key`.
    fn model_keys(key: &str) -> Vec<String> {
        let fixed: &[&str] = match key {
            "date" => &["transaction_date"],
            "sheets" => &["number_of_attached_sheets"],
            "name" => &["taxpayer_name"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "phone" => &["contact_number"],
            "oname" => &["other_party_name"],
            "otin" => &["other_party_tin"],
            "loc" => &["real_property_location"],
            "others" => &["others_description"],
            "othamt" => &["others_amount"],
            "gsp" => &["gross_selling_price"],
            "prev" => &["tax_paid_previous"],
            "sur" => &["surcharge"],
            "int" => &["interest"],
            "comp" => &["compromise"],
            _ => &[],
        };
        if !fixed.is_empty() {
            return fixed.iter().map(|k| k.to_string()).collect();
        }
        match split_row_key(key) {
            Some(("ptct" | "ptd" | "ploc" | "plot" | "pcls" | "parea", row)) => {
                vec![format!("properties[{row}].location")]
            }
            Some(("pfmv1" | "pfmv2", row)) => vec![format!("properties[{row}].fmv_per_td")],
            Some(("scorp" | "sno" | "scert", row)) => {
                vec![format!("shares[{row}].corporation")]
            }
            Some(("spar" | "sdst", row)) => vec![format!("shares[{row}].par_value")],
            _ => Vec::new(),
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

    /// A question label plus choice chips. On phones the label sits on its own
    /// line with the chips in a full-width wrapping row below.
    fn choice_row(label: &str, layout: Layout, choices: Vec<Button>) -> AnyElement {
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
        // Let long chips shrink to the panel and wrap instead of clipping.
        button.disabled(disabled).min_w_0().max_w_full()
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let atc_error = self.errors_for(&["atc"]);
        let atc_field = select_field(
            "Item 3 — ATC",
            &self.atc_select,
            ATC_OPTIONS,
            atc_value(self.draft.atc),
            !editable,
            atc_error.as_deref(),
            cx,
        );
        let children = vec![
            self.field(
                "date",
                "Item 1 — Date of transaction (MM/DD/YYYY)",
                layout,
                !editable,
            ),
            Self::choice_row(
                "Item 2 — Amended return?",
                layout,
                vec![
                    Self::choice("2000ot_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("2000ot_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
            ),
            atc_field,
            self.field(
                "sheets",
                "Item 4 — No. of sheets attached",
                layout,
                !editable,
            ),
        ];
        self.section("Transaction", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let party = self.draft.other_party;
        let nature = self.draft.nature;
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
                            .child("Item 6 — RDO Code of Location of the Property"),
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
        for key in ["name", "address", "zip", "phone", "email"] {
            let label = TEXT_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, layout, !editable));
        }
        let mut party_choices = Vec::new();
        for (index, (value, label)) in [
            (Form2000OTOtherParty::Creditor, "Seller/Transferor/Donor"),
            (Form2000OTOtherParty::Debtor, "Buyer/Transferee/Donee"),
        ]
        .into_iter()
        .enumerate()
        {
            party_choices.push(
                Self::choice(("2000ot_party", index), label, party == value, !editable).on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.other_party = value)),
                ),
            );
        }
        let mut nature_choices = Vec::new();
        for (index, (value, label)) in [
            (
                Form2000OTNature::SharesOfStock,
                "Transfer of shares of stock not traded through the local stock exchange",
            ),
            (
                Form2000OTNature::RealPropertyCapitalAsset,
                "Transfer of real property classified as capital asset",
            ),
            (
                Form2000OTNature::RealPropertyOrdinaryAsset,
                "Transfer of real property other than capital asset",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            nature_choices.push(
                Self::choice(("2000ot_nature", index), label, nature == value, !editable).on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.nature = value)),
                ),
            );
        }
        let mut children = vec![
            self.grid(layout, fields),
            self.with_errors(
                Self::choice_row(
                    "Item 11 — Other party to the transaction",
                    layout,
                    party_choices,
                ),
                &["other_party"],
            ),
            self.grid(
                layout,
                vec![
                    self.field("oname", "Item 11A — Other party's name", layout, !editable),
                    self.field("otin", "Item 11B — Other party's TIN", layout, !editable),
                ],
            ),
            self.with_errors(
                Self::choice_row("Item 12 — Nature of transaction", layout, nature_choices),
                &["nature"],
            ),
        ];
        if nature.is_real_property() {
            children.push(self.field(
                "loc",
                "Item 13 — Location of real property",
                layout,
                !editable,
            ));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn row_box(
        &self,
        title: String,
        schedule: usize,
        row: usize,
        fields: Vec<AnyElement>,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let heading = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(div().min_w_0().font_weight(FontWeight::BOLD).child(title))
            .when(row >= Self::schedule_default(schedule), |d| {
                d.child(
                    Button::new(("2000ot_remove_row", schedule * 1000 + row))
                        .label("Remove")
                        .outline()
                        .small()
                        .disabled(!editable)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.remove_row(schedule, row, window, cx)
                        })),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading)
            .child(self.grid(layout, fields))
            .into_any_element()
    }

    fn render_real_property(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..self.property_row_count() {
            let mut fields: Vec<AnyElement> = PROPERTY_INPUTS
                .iter()
                .map(|(prefix, label, _)| {
                    self.field(&row_key(prefix, row), label, layout, !editable)
                })
                .collect();
            let fmv = self.draft.properties.get(row).map(|r| r.fmv).unwrap_or(0.0);
            fields.push(self.computed("Fair market value (higher of 1 and 2)", fmv, cx));
            children.push(self.row_box(
                format!("Property {}", row + 1),
                0,
                row,
                fields,
                layout,
                cx,
            ));
        }
        children.push(self.with_errors(self.add_row_controls(0, cx), &["properties"]));
        children.push(self.computed(
            "Total fair market value",
            self.draft.total_fair_market_value,
            cx,
        ));
        let d = &self.draft;
        children.push(self.grid(
            layout,
            vec![
                self.field("gsp", MONEY_INPUTS[0].1, layout, !editable),
                self.computed(
                    "Schedule 1B — Total FMV per Schedule 1.A",
                    d.total_fair_market_value,
                    cx,
                ),
                self.field(
                    "others",
                    "Schedule 1C — Others (specify)",
                    layout,
                    !editable,
                ),
                self.field("othamt", MONEY_INPUTS[1].1, layout, !editable),
                self.computed(
                    "Schedule 2 — Taxable base",
                    d.real_property_taxable_base,
                    cx,
                ),
            ],
        ));
        self.section(
            "Schedules 1.A and 2 — Real Property and Taxable Base",
            children,
            cx,
        )
    }

    fn render_shares(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = (0..self.share_row_count())
            .map(|row| {
                let fields = SHARE_INPUTS
                    .iter()
                    .map(|(prefix, label, _)| {
                        self.field(&row_key(prefix, row), label, layout, !editable)
                    })
                    .collect();
                self.row_box(format!("Shares {}", row + 1), 1, row, fields, layout, cx)
            })
            .collect();
        children.push(self.with_errors(self.add_row_controls(1, cx), &["shares"]));
        self.section("Schedule 1.B — Shares of Stock", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let rate = if d.atc == Form2000OTAtc::Unanswered {
            "—".to_string()
        } else {
            d.atc.rate_text().to_string()
        };
        let children = vec![
            self.computed(
                "14 — Taxable base, shares of stock",
                d.shares_taxable_base,
                cx,
            ),
            self.computed("15 — Taxable base, real property", d.real_property_base, cx),
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .p_2()
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("16 — Tax rate"),
                )
                .child(div().font_weight(FontWeight::BOLD).child(rate))
                .into_any_element(),
            self.with_errors(self.computed("17 — Tax due", d.tax_due, cx), &["tax_due"]),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed("19 — Tax still due/(overpayment)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[3].1, layout, !editable),
            self.field("int", MONEY_INPUTS[4].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[5].1, layout, !editable),
            self.computed("20D — Total penalties", d.total_penalties, cx),
            self.with_errors(
                self.computed(
                    "21 — Total amount payable/(overpayment)",
                    d.total_amount_payable,
                    cx,
                ),
                &["total_amount_payable"],
            ),
        ];
        self.section(
            "Part II — Computation of Tax",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2000OTView {
    type Draft = Form2000OTDraft;

    fn new(
        draft: Form2000OTDraft,
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
            keys.push((key.to_string(), "0.00".into()));
        }
        for row in 0..FORM_2000OT_REAL_PROPERTY_ROWS.max(draft.properties.len()) {
            keys.extend(Self::row_input_keys(0, row));
        }
        for row in 0..FORM_2000OT_SHARE_ROWS.max(draft.shares.len()) {
            keys.extend(Self::row_input_keys(1, row));
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
        let atc_select = form_controls::new_select_state(ATC_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &atc_select,
            ATC_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                if let Some(atc) = atc_from_value(&value) {
                    this.edit(cx, |d| d.atc = atc);
                }
            },
        ));
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
            visible_rows: [FORM_2000OT_REAL_PROPERTY_ROWS, FORM_2000OT_SHARE_ROWS],
            atc_select,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form2000OTDraft {
        Form2000OTDraft::new_from_profile(profile, year)
    }

    /// One return per transaction date (`MMDD`). Reopen this year's latest
    /// editable draft; once every saved return is queued or filed, start a new
    /// one, since each 2000-OT is a separate transaction.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2000OTDraft {
        let tin = profile.tin.full();
        let saved = (0..=12i64)
            .flat_map(|month| (0..=31i64).map(move |day| month * 100 + day))
            .filter_map(|key| {
                db.get_queueable_draft::<Form2000OTDraft>(&tin, year, key)
                    .ok()
                    .flatten()
            });
        latest_editable(saved).unwrap_or_else(|| Self::new_draft(profile, year, period))
    }
}

/// The most recently updated editable draft, if any. Queued and filed returns
/// are never reopened from the tile.
fn latest_editable(saved: impl Iterator<Item = Form2000OTDraft>) -> Option<Form2000OTDraft> {
    saved
        .filter(|draft| draft.lifecycle.is_editable())
        .max_by(|a, b| a.lifecycle.updated_at.cmp(&b.lifecycle.updated_at))
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

impl FormViewTrait for Form2000OTView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2000-OT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Documentary Stamp Tax Declaration/Return (One-Time Transactions)"
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
                    "2000-OT draft saved.".into(),
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
        if !can_queue_for_submission(Form2000OTDraft::FORM_CODE) {
            self.status_message =
                Some("2000-OT is not enabled for in-app submission in this build.".into());
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
                "Form 2000-OT",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 2000-OT queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2000-OT payment status needs a verified confirmation workflow.".into());
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
                    self.edit(cx, |d| d.is_amended = true);
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
                    "Form 2000-OT reverted to Draft.".into(),
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
            "2000-ot-2018",
            &fields,
            "2000-OT — Print Preview",
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

impl Render for Form2000OTView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        form_controls::sync_select(
            &self.atc_select,
            ATC_OPTIONS,
            atc_value(self.draft.atc),
            window,
            cx,
        );
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
                .child(Button::new("2000ot_back").label("← Back").on_click(
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
                            Button::new("2000ot_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2000ot_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_header_items(layout, cx))
            .child(self.render_part_one(layout, cx));
        body = body.child(self.render_totals(layout, cx));
        match self.draft.atc {
            Form2000OTAtc::Do122 => body = body.child(self.render_real_property(layout, cx)),
            Form2000OTAtc::Do102 | Form2000OTAtc::Do125 => {
                body = body.child(self.render_shares(layout, cx))
            }
            Form2000OTAtc::Unanswered => {}
        }
        if matches!(self.draft.atc, Form2000OTAtc::Do102 | Form2000OTAtc::Do122) {
            body = body.child(
                div()
                    .id("2000ot_block_rounding_note")
                    .mt_2()
                    .p_3()
                    .rounded_md()
                    .text_sm()
                    .bg(cx.theme().muted.opacity(0.6))
                    .child(BLOCK_ROUNDING_NOTE),
            );
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
                    .id("2000ot_scroll")
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
    "Validation successful. Click on 'Edit' if you wish to modify your entries.";

impl Form2000OTView {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2000OTDraft>(
                        &queued.tin,
                        queued.taxable_year(),
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
            slug: "2000ot",
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2000OTDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year(),
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "2000-OT marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form2000OTView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form2000OTView::blocking_errors,
    on_validate: Form2000OTView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form2000OTView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_2000ot_view.rs");
        assert!(src.contains(concat!("render_header_", "buttons(")));
        assert!(src.contains(concat!("render_status_", "box(")));
        assert!(src.contains(concat!("revert_for_", "amendment(")));
        assert!(!src.contains(concat!("Queued for background ", "submission")));
        assert!(!src.contains(concat!("\"Queue for ", "submission\"")));
    }
    #[test]
    fn sheet_count_rejects_fractions_and_negatives() {
        assert_eq!(super::parse_sheet_count(""), Some(0));
        assert_eq!(super::parse_sheet_count(" 12 "), Some(12));
        assert_eq!(super::parse_sheet_count("2.5"), None);
        assert_eq!(super::parse_sheet_count("-1"), None);
        assert_eq!(super::parse_sheet_count("1000"), None);
    }

    use super::{Layout, format_tin, latest_editable, parse_amount};
    use bir_core::forms::FilingStatus;
    use bir_core::forms::form_2000ot::Form2000OTDraft;
    use bir_core::profile::TaxpayerProfile;
    use gpui::px;

    fn draft(updated_at: &str, status: FilingStatus) -> Form2000OTDraft {
        let profile: TaxpayerProfile = serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "Sample", "tin": {"segment1": "123",
            "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
            "line_of_business": "", "registered_address": "", "zip_code": "",
            "phone": "", "email": "", "default_form_type": "2000OT",
            "taxpayer_type": "Corporation"
        }))
        .unwrap();
        let mut draft = Form2000OTDraft::new_from_profile(&profile, 2025);
        draft.lifecycle.updated_at = updated_at.into();
        draft.lifecycle.status = status;
        draft
    }

    #[test]
    fn tile_never_reopens_a_queued_or_filed_return() {
        let queued = draft("2025-06-02", FilingStatus::Queued);
        assert!(latest_editable([queued.clone()].into_iter()).is_none());
        let older = draft("2025-06-01", FilingStatus::Draft);
        let picked = latest_editable([older, queued].into_iter()).unwrap();
        assert_eq!(picked.lifecycle.updated_at, "2025-06-01");
        let newer = draft("2025-06-03", FilingStatus::Draft);
        let picked = latest_editable([newer].into_iter()).unwrap();
        assert_eq!(picked.lifecycle.updated_at, "2025-06-03");
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
