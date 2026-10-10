//! Editor for BIR Form 2000 (v2018), Monthly Documentary Stamp Tax
//! Declaration/Return. Rust owns every calculation, validation and the
//! official submit plaintext (`bir_core::forms::form_2000`); this view only
//! edits source values. The layout reflows for desktop, tablet and phone.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2000::{
    FORM_2000_ATC_OPTIONS, FORM_2000_ROWS, Form2000AffixtureMode, Form2000Draft,
    Form2000OtherParty, Form2000PaymentRow, Form2000RemittanceRow,
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
use gpui_rsx::rsx;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form2000View {}

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

/// Single text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("name", "Item 6 — Taxpayer's Name", ""),
    ("address", "Item 7 — Registered Address", ""),
    ("zip", "Item 7A — Zip Code", ""),
    ("phone", "Item 8 — Contact Number", "digits only"),
    ("email", "Item 9 — Email Address", ""),
    ("oname", "Item 11 — Other party's name", ""),
    ("otin", "Item 12 — Other party's TIN", "digits only"),
    ("days", "DS106 — Term in days (1–365)", "days"),
];

/// Money inputs outside the schedules: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "prev",
        "15A — Balance carried over from previous return (amended only)",
    ),
    ("sur", "17A — Surcharge"),
    ("int", "17B — Interest"),
    ("comp", "17C — Compromise"),
];

/// Per-row schedule inputs: (prefix, label, money?).
const SCHED1_INPUTS: &[(&str, &str, bool)] = &[
    ("s1base", "Tax base", true),
    ("s1due", "Tax due (DS010)", true),
];
const PAYMENT_INPUTS: &[(&str, &str, bool)] = &[
    ("date", "Payment date (MM/DD/YYYY)", false),
    ("rcpt", "Receipt / reference no.", false),
    ("amt", "Amount paid", true),
];
const REMIT_INPUTS: &[(&str, &str, bool)] = &[
    ("s4rco", "RCO code", false),
    ("s4date", "Remittance date (MM/DD/YYYY)", false),
    ("s4bank", "Authorized agent bank", false),
    ("s4amt", "Amount remitted", true),
    ("s4from", "Loose stamps no. from", false),
    ("s4to", "Loose stamps no. to", false),
];

fn row_key(prefix: &str, row: usize) -> String {
    format!("{prefix}{row}")
}

fn payment_key(schedule: u8, prefix: &str, row: usize) -> String {
    format!("s{schedule}{prefix}{row}")
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

fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

pub struct Form2000View {
    draft: Form2000Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Schedule 1 row whose ATC list is open.
    picking_row: Option<usize>,
    _subscriptions: Vec<Subscription>,
}

impl Form2000View {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2000Draft, key: &str) -> String {
        match key {
            "year" => draft.taxable_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "oname" => draft.other_party_name.clone(),
            "otin" => draft.other_party_tin.clone(),
            "days" => {
                if draft.ds106_term_days == 0 {
                    String::new()
                } else {
                    draft.ds106_term_days.to_string()
                }
            }
            "prev" => money_text(draft.balance_carried_over),
            "sur" => money_text(draft.surcharge),
            "int" => money_text(draft.interest),
            "comp" => money_text(draft.compromise),
            _ => Self::initial_row(draft, key).unwrap_or_default(),
        }
    }

    fn initial_row(draft: &Form2000Draft, key: &str) -> Option<String> {
        // Row keys end in the row digit (three rows per schedule).
        let (prefix, index) = key.split_at(key.len().checked_sub(1)?);
        let row: usize = index.parse().ok()?;
        let payment = |rows: &Vec<Form2000PaymentRow>, field: &str| {
            rows.get(row).map(|r| match field {
                "date" => r.date.clone(),
                "rcpt" => r.receipt_number.clone(),
                _ => money_text(r.amount),
            })
        };
        match prefix {
            "s1base" => draft.schedule1.get(row).map(|r| money_text(r.tax_base)),
            "s1due" => draft.schedule1.get(row).map(|r| money_text(r.tax_due)),
            "s2date" | "s2rcpt" | "s2amt" => payment(&draft.schedule2, &prefix[2..]),
            "s3date" | "s3rcpt" | "s3amt" => payment(&draft.schedule3, &prefix[2..]),
            _ => draft.schedule4.get(row).map(|r| match prefix {
                "s4rco" => r.rco_code.clone(),
                "s4date" => r.date.clone(),
                "s4bank" => r.bank.clone(),
                "s4amt" => money_text(r.amount),
                "s4from" => r.number_from.clone(),
                _ => r.number_to.clone(),
            }),
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
        if let Some(year) = number("year", "Item 1 year", cx) {
            draft.taxable_year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
        }
        if let Some(sheets) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        if let Some(days) = number("days", "DS106 term", cx) {
            draft.ds106_term_days = if (0.0..=999.0).contains(&days) {
                days as u16
            } else {
                999
            };
        }
        for (key, label) in MONEY_INPUTS {
            if let Some(value) = number(key, label, cx) {
                match *key {
                    "prev" => draft.balance_carried_over = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        for row in 0..draft.schedule1.len().min(FORM_2000_ROWS) {
            if let Some(value) = number(&row_key("s1base", row), "Schedule 1 tax base", cx) {
                draft.schedule1[row].tax_base = value;
            }
            if draft.schedule1[row].tax_due_is_manual()
                && let Some(value) = number(&row_key("s1due", row), "Schedule 1 tax due", cx)
            {
                draft.schedule1[row].tax_due = value;
            }
        }
        let (sched2, sched3, sched4) = draft.affixture_mode.schedules();
        for (schedule, enabled) in [(2u8, sched2), (3u8, sched3)] {
            let mut rows = Vec::new();
            if enabled {
                for row in 0..FORM_2000_ROWS {
                    let amount = number(
                        &payment_key(schedule, "amt", row),
                        &format!("Schedule {schedule} amount"),
                        cx,
                    )
                    .unwrap_or(0.0);
                    rows.push(Form2000PaymentRow {
                        date: self
                            .input_text(&payment_key(schedule, "date", row), cx)
                            .trim()
                            .to_string(),
                        receipt_number: self
                            .input_text(&payment_key(schedule, "rcpt", row), cx)
                            .trim()
                            .to_string(),
                        amount,
                    });
                }
                while rows
                    .last()
                    .is_some_and(|r| *r == Form2000PaymentRow::default())
                {
                    rows.pop();
                }
            }
            if schedule == 2 {
                draft.schedule2 = rows;
            } else {
                draft.schedule3 = rows;
            }
        }
        let mut remittances = Vec::new();
        if sched4 {
            for row in 0..FORM_2000_ROWS {
                let amount = number(&row_key("s4amt", row), "Schedule 4 amount", cx).unwrap_or(0.0);
                let text = |prefix: &str| {
                    self.input_text(&row_key(prefix, row), cx)
                        .trim()
                        .to_string()
                };
                remittances.push(Form2000RemittanceRow {
                    rco_code: text("s4rco"),
                    date: text("s4date"),
                    bank: text("s4bank"),
                    amount,
                    number_from: text("s4from"),
                    number_to: text("s4to"),
                });
            }
            while remittances
                .last()
                .is_some_and(|r| *r == Form2000RemittanceRow::default())
            {
                remittances.pop();
            }
        }
        draft.schedule4 = remittances;
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.other_party_name = self.input_text("oname", cx);
        draft.other_party_tin = self.input_text("otin", cx).trim().to_string();
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2000Draft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// Reload every row editor from the draft (after a row moves or clears).
    fn reload_row_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys: Vec<String> = self
            .inputs
            .keys()
            .filter(|key| {
                key.starts_with('s') && key.chars().last().is_some_and(|c| c.is_ascii_digit())
            })
            .cloned()
            .collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    fn pick_atc(&mut self, row: usize, option: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(option) = FORM_2000_ATC_OPTIONS.get(option) else {
            return;
        };
        let mut draft = self.draft.clone();
        match draft.set_atc(row, option) {
            Ok(()) => {
                self.draft = draft;
                self.picking_row = None;
                self.reload_row_inputs(window, cx);
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
        self.reload_row_inputs(window, cx);
        self.sync_from_inputs(cx);
    }

    fn set_mode(
        &mut self,
        mode: Form2000AffixtureMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(cx, |d| d.affixture_mode = mode);
        // processModeAffixture clears the schedules the new mode disables.
        self.reload_row_inputs(window, cx);
        self.sync_from_inputs(cx);
    }

    fn set_party(
        &mut self,
        party: Form2000OtherParty,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(cx, |d| d.other_party = party);
        for key in ["oname", "otin"] {
            let value = Self::initial(&self.draft, key);
            if let Some(input) = self.inputs.get(key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
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
                .release_abandoned_claimed_queueable::<Form2000Draft>(
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

    fn label_row(label: &str) -> Div {
        div().flex().flex_wrap().items_center().gap_2().child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
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
        let mut months = div().flex().flex_wrap().gap_1();
        for (index, name) in MONTHS.iter().enumerate() {
            let month = index as u8 + 1;
            months = months.child(
                Self::choice(
                    ("2000_month", index),
                    *name,
                    self.draft.month == month,
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.month = month))),
            );
        }
        let amended = self.draft.is_amended;
        let children = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 1 — For the month of"),
                )
                .child(months)
                .into_any_element(),
            self.field("year", "Item 1 — Year", layout, !editable),
            Self::label_row("Item 2 — Amended return?")
                .child(
                    Self::choice("2000_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                )
                .child(
                    Self::choice("2000_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                )
                .into_any_element(),
            self.field(
                "sheets",
                "Item 3 — No. of sheets attached",
                layout,
                !editable,
            ),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let party = self.draft.other_party;
        let mode = self.draft.affixture_mode;
        let mut fields = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 4 — TIN"),
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
                        .child("Item 5 — RDO Code"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(self.draft.rdo_code.clone()),
                )
                .into_any_element(),
        ];
        for key in ["name", "address", "zip", "phone", "email"] {
            let label = TEXT_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, layout, !editable));
        }
        let mut party_row = Self::label_row("Item 10 — Other party to the transaction");
        for (index, (value, label)) in [
            (Form2000OtherParty::Creditor, "Creditor/Mortgagor/etc."),
            (Form2000OtherParty::Debtor, "Debtor/Mortgagee/etc."),
            (Form2000OtherParty::None, "None"),
        ]
        .into_iter()
        .enumerate()
        {
            party_row = party_row.child(
                Self::choice(("2000_party", index), label, party == value, !editable).on_click(
                    cx.listener(move |this, _, window, cx| this.set_party(value, window, cx)),
                ),
            );
        }
        let mut children = vec![self.grid(layout, fields), party_row.into_any_element()];
        if party.has_other_party() {
            children.push(self.grid(
                layout,
                vec![
                    self.field("oname", "Item 11 — Other party's name", layout, !editable),
                    self.field("otin", "Item 12 — Other party's TIN", layout, !editable),
                ],
            ));
        }
        let mut mode_row = Self::label_row("Item 13 — Mode of affixture");
        for (index, (value, label)) in [
            (Form2000AffixtureMode::Edst, "eDST System"),
            (
                Form2000AffixtureMode::Constructive,
                "Constructive Affixture",
            ),
            (Form2000AffixtureMode::LooseStamps, "Loose Stamps"),
        ]
        .into_iter()
        .enumerate()
        {
            mode_row = mode_row.child(
                Self::choice(("2000_mode", index), label, mode == value, !editable).on_click(
                    cx.listener(move |this, _, window, cx| this.set_mode(value, window, cx)),
                ),
            );
        }
        children.push(mode_row.into_any_element());
        self.section("Part I — Background Information", children, cx)
    }

    fn render_atc_row(&self, row: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let entry = self.draft.schedule1.get(row).cloned().unwrap_or_default();
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
                            .child(if entry.is_empty() {
                                format!("Row {} — no ATC", row + 1)
                            } else {
                                format!("Row {} — {}", row + 1, entry.atc_code)
                            }),
                    )
                    .child(div().text_sm().child(entry.description.clone())),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new(("2000_pick_atc", row))
                            .label(if entry.is_empty() {
                                "Choose ATC"
                            } else {
                                "Change"
                            })
                            .outline()
                            .small()
                            .disabled(!editable)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.picking_row = if this.picking_row == Some(row) {
                                    None
                                } else {
                                    Some(row)
                                };
                                cx.notify();
                            })),
                    )
                    .when(!entry.is_empty(), |d| {
                        d.child(
                            Button::new(("2000_clear_atc", row))
                                .label("Remove")
                                .outline()
                                .small()
                                .disabled(!editable)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.clear_atc(row, window, cx)
                                })),
                        )
                    }),
            );
        let mut body = div().flex().flex_col().gap_2().child(heading);
        if self.picking_row == Some(row) && editable {
            let mut picks = div().flex().flex_wrap().gap_2();
            for (index, option) in FORM_2000_ATC_OPTIONS.iter().enumerate() {
                picks = picks.child(
                    Button::new(("2000_atc_option", row * 100 + index))
                        .label(format!("{} — {}", option.code, option.description))
                        .outline()
                        .small()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pick_atc(row, index, window, cx)
                        })),
                );
            }
            body = body.child(picks);
        }
        if !entry.is_empty() {
            let rate = self.draft.rate_text(&entry);
            let base = self.field(&row_key("s1base", row), "Tax base", layout, !editable);
            let due = if entry.tax_due_is_manual() {
                self.field(&row_key("s1due", row), "Tax due (DS010)", layout, !editable)
            } else {
                self.computed("Tax due", entry.tax_due, cx)
            };
            body = body
                .child(base)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Tax rate"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .child(if rate.is_empty() {
                                    "—".to_string()
                                } else {
                                    rate
                                }),
                        ),
                )
                .child(due);
            if entry.atc_code == "DS106" {
                let answer = self.draft.ds106_term_under_a_year;
                body = body.child(
                    Self::label_row("Is the term of the debt instrument less than a year?")
                        .child(
                            Self::choice("2000_ds106_yes", "Yes", answer == Some(true), !editable)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.edit(cx, |d| d.ds106_term_under_a_year = Some(true))
                                })),
                        )
                        .child(
                            Self::choice("2000_ds106_no", "No", answer == Some(false), !editable)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.edit(cx, |d| d.ds106_term_under_a_year = Some(false))
                                })),
                        ),
                );
                if answer == Some(true) {
                    body =
                        body.child(self.field("days", "Term in days (1–365)", layout, !editable));
                }
            }
        }
        body.p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .into_any_element()
    }

    fn render_schedule1(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let mut children: Vec<AnyElement> = (0..FORM_2000_ROWS)
            .map(|row| self.render_atc_row(row, layout, cx))
            .collect();
        children.push(self.computed("Total tax due (to Item 14)", self.draft.tax_due, cx));
        self.section("Schedule 1 — Computation of Tax Due", children, cx)
    }

    fn render_payment_schedule(
        &self,
        schedule: u8,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let (title, total, item) = if schedule == 2 {
            (
                "Schedule 2 — Payments thru Constructive Affixture",
                self.draft.constructive_affixture_payments,
                "15B",
            )
        } else {
            (
                "Schedule 3 — Advance Payments during the month",
                self.draft.advance_payments,
                "15C",
            )
        };
        let mut children = Vec::new();
        for row in 0..FORM_2000_ROWS {
            let fields = PAYMENT_INPUTS
                .iter()
                .map(|(prefix, label, _)| {
                    self.field(
                        &payment_key(schedule, prefix, row),
                        label,
                        layout,
                        !editable,
                    )
                })
                .collect();
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
                            .child(format!("Row {}", row + 1)),
                    )
                    .child(self.grid(layout, fields))
                    .into_any_element(),
            );
        }
        children.push(self.computed(&format!("Total payments (to Item {item})"), total, cx));
        self.section(title, children, cx)
    }

    fn render_schedule4(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..FORM_2000_ROWS {
            let fields = REMIT_INPUTS
                .iter()
                .map(|(prefix, label, _)| {
                    self.field(&row_key(prefix, row), label, layout, !editable)
                })
                .collect();
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
                            .child(format!("Row {}", row + 1)),
                    )
                    .child(self.grid(layout, fields))
                    .into_any_element(),
            );
        }
        children.push(self.computed("Total remittance (to Item 19)", self.draft.stamps_sold, cx));
        self.section(
            "Schedule 4 — Documentary Stamps Remitted / Loose Stamps",
            children,
            cx,
        )
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let children = vec![
            self.computed("14 — Tax due (from Schedule 1)", d.tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[0].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed(
                "15B — Payment thru constructive affixture",
                d.constructive_affixture_payments,
                cx,
            ),
            self.computed(
                "15C — Advance payment during the month",
                d.advance_payments,
                cx,
            ),
            self.computed("15D — Total", d.total_credits, cx),
            self.computed("16 — Net tax payable/(overpayment)", d.net_tax_payable, cx),
            self.field("sur", MONEY_INPUTS[1].1, layout, !editable),
            self.field("int", MONEY_INPUTS[2].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[3].1, layout, !editable),
            self.computed("17D — Total penalties", d.total_penalties, cx),
            self.computed(
                "18 — Total amount payable/(overpayment)",
                d.total_amount_payable,
                cx,
            ),
            self.computed(
                "19 — Documentary stamps sold for the month",
                d.stamps_sold,
                cx,
            ),
        ];
        self.section(
            "Part II — Computation of Payable",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2000View {
    type Draft = Form2000Draft;

    fn new(
        draft: Form2000Draft,
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
        for row in 0..FORM_2000_ROWS {
            for (prefix, _, money) in SCHED1_INPUTS.iter().chain(REMIT_INPUTS) {
                keys.push((
                    row_key(prefix, row),
                    if *money { "0.00" } else { "" }.into(),
                ));
            }
            for schedule in [2u8, 3u8] {
                for (prefix, _, money) in PAYMENT_INPUTS {
                    let placeholder = if *money {
                        "0.00"
                    } else if *prefix == "date" {
                        "MM/DD/YYYY"
                    } else {
                        ""
                    };
                    keys.push((payment_key(schedule, prefix, row), placeholder.into()));
                }
            }
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
            picking_row: None,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2000Draft {
        let month = if (1..=12).contains(&period) {
            period
        } else {
            chrono::Datelike::month(&chrono::Local::now().date_naive()) as u8
        };
        Form2000Draft::new_from_profile(profile, year, month)
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

impl FormViewTrait for Form2000View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2000"
    }
    fn form_subtitle(&self) -> &'static str {
        "Monthly Documentary Stamp Tax Declaration/Return"
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
                    "2000 draft saved.".into(),
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
        if !can_queue_for_submission(Form2000Draft::FORM_CODE) {
            self.status_message =
                Some("2000 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 2000. No submission was started: {error}"
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
            "Form 2000 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2000 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2000Draft>(
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
            "2000-dst-2018",
            &fields,
            "2000 — Print Preview",
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

impl Render for Form2000View {
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
                .child(Button::new("2000_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2000_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2000_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2000_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2000_submit")
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

        let (sched2, sched3, sched4) = self.draft.affixture_mode.schedules();
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
                            Button::new("2000_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2000_release_cancel")
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
            .child(self.render_schedule1(layout, cx));
        if sched2 {
            body = body.child(self.render_payment_schedule(2, layout, cx));
        }
        if sched3 {
            body = body.child(self.render_payment_schedule(3, layout, cx));
        }
        if sched4 {
            body = body.child(self.render_schedule4(layout, cx));
        }
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
                    .id("2000_scroll")
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
