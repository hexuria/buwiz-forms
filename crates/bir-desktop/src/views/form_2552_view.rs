//! Editor for BIR Form 2552 (January 2018), Percentage Tax Return for
//! Transactions Involving Shares of Stock Listed and Traded through the Local
//! Stock Exchange or through Initial and/or Secondary Public Offering. Rust
//! owns every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_2552`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2552::{
    FORM_2552_LSE_RATE_TEXT, FORM_2552_ROWS, Form2552Draft, Form2552ExemptRow,
    Form2552NonTaxableRow, Form2552Overpayment, Form2552ShareRow, Form2552Transaction,
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

impl EventEmitter<QueueableFormEvent> for Form2552View {}

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

/// Single inputs: (key, label, placeholder).
const SINGLE_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    (
        "name",
        "Item 6 — Name of Stockbroker/Issuing Corporation",
        "",
    ),
    ("address", "Item 7 — Registered Address", ""),
    ("zip", "Item 7A — Zip Code", ""),
    ("phone", "Item 8 — Contact Number", "digits only"),
    ("email", "Item 9 — Email Address", ""),
    ("relief", "Item 10A — If yes, specify", ""),
    (
        "shares_sold",
        "Item 12A — No. of shares sold, bartered or exchanged",
        "",
    ),
    (
        "outstanding",
        "Item 12B — Total outstanding shares after listing",
        "",
    ),
];

/// Money inputs outside the tables: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("prev", "17 — Tax paid in return previously filed (amended)"),
    ("cwt", "18 — Creditable tax withheld per BIR Form 2307"),
    ("sur", "21 — Surcharge"),
    ("int", "22 — Interest"),
    ("comp", "23 — Compromise"),
];

/// Part V columns: (suffix, label, numeric).
const SHARE_COLUMNS: &[(&str, &str, bool)] = &[
    ("date", "(a) Date (MM/DD/YYYY)", false),
    ("seller", "(b) Seller", false),
    ("buyer", "(c) Buyer", false),
    ("corp", "(d) Issuing corporation", false),
    ("shares", "(e) Number of shares", true),
    ("base", "(f) Tax base", true),
    ("rate", "(g) Tax rate (%)", true),
];

/// Part VI columns.
const NON_TAXABLE_COLUMNS: &[(&str, &str, bool)] = &[
    ("date", "Date (MM/DD/YYYY)", false),
    ("seller", "Seller", false),
    ("buyer", "Buyer", false),
    ("corp", "Issuing corporation", false),
    ("shares", "Number of shares", true),
];

fn row_key(table: &str, row: usize, column: &str) -> String {
    format!("{table}{row}_{column}")
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

fn money(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

pub struct Form2552View {
    draft: Form2552Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2552View {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2552Draft, key: &str) -> String {
        match key {
            "month" => format!("{:02}", draft.transaction_month),
            "day" => format!("{:02}", draft.transaction_day),
            "year" => draft.transaction_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "relief" => draft.tax_relief_specification.clone(),
            "shares_sold" => draft.shares_sold.clone(),
            "outstanding" => draft.outstanding_shares.clone(),
            "prev" => money(draft.tax_paid_previous),
            "cwt" => money(draft.creditable_tax_withheld),
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            _ => Self::initial_row(draft, key).unwrap_or_default(),
        }
    }

    fn initial_row(draft: &Form2552Draft, key: &str) -> Option<String> {
        let (head, column) = key.split_once('_')?;
        let table: String = head.chars().take_while(|c| c.is_alphabetic()).collect();
        let row: usize = head[table.len()..].parse().ok()?;
        match table.as_str() {
            "exempt" => {
                let entry = draft.exempt_transactions.get(row)?;
                Some(match column {
                    "class" => entry.classification.clone(),
                    _ => money(entry.amount),
                })
            }
            "share" => {
                let entry = draft.schedule.get(row)?;
                Some(match column {
                    "date" => entry.date.clone(),
                    "seller" => entry.seller.clone(),
                    "buyer" => entry.buyer.clone(),
                    "corp" => entry.issuing_corporation.clone(),
                    "shares" => money(entry.number_of_shares),
                    "base" => money(entry.tax_base),
                    _ => money(entry.tax_rate),
                })
            }
            "free" => {
                let entry = draft.non_taxable_transactions.get(row)?;
                Some(match column {
                    "date" => entry.date.clone(),
                    "seller" => entry.seller.clone(),
                    "buyer" => entry.buyer.clone(),
                    "corp" => entry.issuing_corporation.clone(),
                    _ => money(entry.number_of_shares),
                })
            }
            _ => None,
        }
    }

    fn all_keys() -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = SINGLE_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        for row in 0..FORM_2552_ROWS {
            keys.push((row_key("exempt", row, "class"), String::new()));
            keys.push((row_key("exempt", row, "amount"), "0.00".into()));
            for (column, _, numeric) in SHARE_COLUMNS {
                let placeholder = if *numeric { "0.00" } else { "" };
                keys.push((row_key("share", row, column), placeholder.into()));
            }
            for (column, _, numeric) in NON_TAXABLE_COLUMNS {
                let placeholder = if *numeric { "0.00" } else { "" };
                keys.push((row_key("free", row, column), placeholder.into()));
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

    /// Reload every editor from the draft (after rows move).
    fn reload_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (key, input) in &self.inputs {
            let value = Self::initial(&self.draft, key);
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
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
        let whole = |value: f64, max: f64| {
            if (0.0..=max).contains(&value) {
                value as u32
            } else {
                0
            }
        };
        let mut draft = self.draft.clone();
        if let Some(value) = number("month", "Item 1 month", cx) {
            draft.transaction_month = whole(value, 99.0) as u8;
        }
        if let Some(value) = number("day", "Item 1 day", cx) {
            draft.transaction_day = whole(value, 99.0) as u8;
        }
        if let Some(value) = number("year", "Item 1 year", cx) {
            draft.transaction_year = whole(value, 9999.0) as u16;
        }
        if let Some(value) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&value) {
                value as u16
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
        for row in 0..draft.exempt_transactions.len().min(FORM_2552_ROWS) {
            draft.exempt_transactions[row].classification =
                self.input_text(&row_key("exempt", row, "class"), cx);
            if let Some(value) = number(
                &row_key("exempt", row, "amount"),
                &format!("Part IV row {} amount", row + 1),
                cx,
            ) {
                draft.exempt_transactions[row].amount = value;
            }
        }
        for row in 0..draft.schedule.len().min(FORM_2552_ROWS) {
            let text = |column: &str| self.input_text(&row_key("share", row, column), cx);
            let entry = &mut draft.schedule[row];
            entry.date = text("date").trim().to_string();
            entry.seller = text("seller");
            entry.buyer = text("buyer");
            entry.issuing_corporation = text("corp");
            let label = |column: &str| format!("Part V row {} {column}", row + 1);
            if let Some(value) = number(&row_key("share", row, "shares"), &label("(e)"), cx) {
                draft.schedule[row].number_of_shares = value;
            }
            if let Some(value) = number(&row_key("share", row, "base"), &label("(f)"), cx) {
                draft.schedule[row].tax_base = value;
            }
            if draft.transaction != Form2552Transaction::LocalStockExchange
                && let Some(value) = number(&row_key("share", row, "rate"), &label("(g)"), cx)
            {
                draft.schedule[row].tax_rate = value;
            }
        }
        for row in 0..draft.non_taxable_transactions.len().min(FORM_2552_ROWS) {
            let text = |column: &str| self.input_text(&row_key("free", row, column), cx);
            let entry = &mut draft.non_taxable_transactions[row];
            entry.date = text("date").trim().to_string();
            entry.seller = text("seller");
            entry.buyer = text("buyer");
            entry.issuing_corporation = text("corp");
            if let Some(value) = number(
                &row_key("free", row, "shares"),
                &format!("Part VI row {} shares", row + 1),
                cx,
            ) {
                draft.non_taxable_transactions[row].number_of_shares = value;
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.tax_relief_specification = self.input_text("relief", cx);
        draft.shares_sold = self.input_text("shares_sold", cx);
        draft.outstanding_shares = self.input_text("outstanding", cx);
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// A choice or row change; editors are reloaded from the result.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form2552Draft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        change(&mut self.draft);
        self.draft.recompute();
        self.reload_inputs(window, cx);
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
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
                .release_abandoned_claimed_queueable::<Form2552Draft>(
                    &draft.tin,
                    draft.transaction_year,
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
        let label = if self.has_error(key) || self.has_error(error_key) {
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

    fn choice_row(label: &str, buttons: Vec<Button>) -> AnyElement {
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
            .children(buttons)
            .into_any_element()
    }

    fn info(label: &str, value: String) -> AnyElement {
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

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let date = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Item 1 — Date of transaction or of listing (MM/DD/YYYY)"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .w(px(80.))
                            .child(Input::new(&self.inputs["month"]).disabled(!editable)),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .child(Input::new(&self.inputs["day"]).disabled(!editable)),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .child(Input::new(&self.inputs["year"]).disabled(!editable)),
                    ),
            )
            .into_any_element();
        let children = vec![
            date,
            Self::choice_row(
                "Item 2 — Amended return?",
                vec![
                    Self::choice("2552_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = true)
                        }),
                    ),
                    Self::choice("2552_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = false)
                        }),
                    ),
                ],
            ),
            self.field(
                "sheets",
                "Item 3 — No. of sheets attached",
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
            Self::info("Item 4 — TIN", format_tin(&d.tin)),
            Self::info("Item 5 — RDO Code", d.rdo_code.clone()),
        ];
        for (key, error_key) in [
            ("name", "taxpayer_name"),
            ("address", "registered_address"),
            ("zip", "zip_code"),
            ("phone", "contact_number"),
            ("email", "email"),
        ] {
            let label = SINGLE_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, error_key, layout, !editable));
        }
        let relief = d.tax_relief;
        let transaction = d.transaction;
        let offering = transaction.is_public_offering();
        let pick = |id: &'static str, label: &'static str, value: Form2552Transaction| {
            Self::choice(id, label, transaction == value, !editable).on_click(cx.listener(
                move |this, _, window, cx| this.edit(window, cx, |d| d.set_transaction(value)),
            ))
        };
        let children = vec![
            self.grid(layout, fields),
            Self::choice_row(
                "Item 10 — Availing of tax relief under a Special Law or International Tax Treaty?",
                vec![
                    Self::choice("2552_relief_yes", "Yes", relief, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.tax_relief = true)
                        }),
                    ),
                    Self::choice("2552_relief_no", "No", !relief, !editable).on_click(cx.listener(
                        |this, _, window, cx| this.edit(window, cx, |d| d.tax_relief = false),
                    )),
                ],
            ),
            self.field(
                "relief",
                "Item 10A — If yes, specify",
                "tax_relief_specification",
                layout,
                !editable || !relief,
            ),
            Self::choice_row(
                "Item 11 — Kind of transaction",
                vec![
                    pick(
                        "2552_kind_lse",
                        "11A — Listed and traded through the LSE",
                        Form2552Transaction::LocalStockExchange,
                    ),
                    pick(
                        "2552_kind_primary",
                        "11B — IPO, primary",
                        Form2552Transaction::PrimaryOffering,
                    ),
                    pick(
                        "2552_kind_secondary",
                        "11B — IPO, secondary",
                        Form2552Transaction::SecondaryOffering,
                    ),
                ],
            ),
            self.grid(
                layout,
                vec![
                    self.field(
                        "shares_sold",
                        "Item 12A — No. of shares sold, bartered or exchanged",
                        "shares_sold",
                        layout,
                        !editable || !offering,
                    ),
                    self.field(
                        "outstanding",
                        "Item 12B — Total outstanding shares after listing",
                        "outstanding_shares",
                        layout,
                        !editable || !offering,
                    ),
                ],
            ),
        ];
        self.section("Part I — Background Information", children, cx)
    }

    /// A removable table row of inputs.
    #[allow(clippy::too_many_arguments)]
    fn table_row(
        &self,
        table: &'static str,
        error_table: &str,
        row: usize,
        columns: &[(&'static str, String)],
        footer: Option<AnyElement>,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let error_key = format!("{error_table}[{row}]");
        let heading = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .when(self.has_error(&error_key), |d| d.text_color(gpui::red()))
                    .child(format!("Row {}", row + 1)),
            )
            .child(
                Button::new((SharedString::from(format!("2552_remove_{table}")), row))
                    .label("Remove")
                    .outline()
                    .small()
                    .disabled(!editable)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| match table {
                            "exempt" => {
                                d.exempt_transactions.remove(row);
                            }
                            "share" => {
                                d.schedule.remove(row);
                            }
                            _ => {
                                d.non_taxable_transactions.remove(row);
                            }
                        })
                    })),
            );
        let fields: Vec<AnyElement> = columns
            .iter()
            .map(|(column, label)| {
                self.field(
                    &row_key(table, row, column),
                    label,
                    &format!("{error_key}.{column}"),
                    layout,
                    !editable,
                )
            })
            .collect();
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
            .when_some(footer, |d, footer| d.child(footer))
            .into_any_element()
    }

    fn add_row_button(
        &self,
        table: &'static str,
        label: &str,
        count: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let editable = self.draft.lifecycle.is_editable();
        (editable && count < FORM_2552_ROWS).then(|| {
            Button::new(SharedString::from(format!("2552_add_{table}")))
                .label(label.to_string())
                .outline()
                .small()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| match table {
                        "exempt" => d.exempt_transactions.push(Form2552ExemptRow::default()),
                        "share" => d.schedule.push(Form2552ShareRow::default()),
                        _ => d
                            .non_taxable_transactions
                            .push(Form2552NonTaxableRow::default()),
                    })
                }))
                .into_any_element()
        })
    }

    fn render_part_five(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let Some(schedule) = d.transaction.schedule() else {
            return self.section(
                "Part V — Details of Taxable Transactions",
                vec![
                    div()
                        .text_sm()
                        .child("Choose the kind of transaction (Item 11) to fill its schedule.")
                        .into_any_element(),
                ],
                cx,
            );
        };
        let lse = d.transaction == Form2552Transaction::LocalStockExchange;
        let mut children: Vec<AnyElement> = Vec::new();
        for row in 0..d.schedule.len().min(FORM_2552_ROWS) {
            let columns: Vec<(&'static str, String)> = SHARE_COLUMNS
                .iter()
                .filter(|(column, _, _)| !(lse && *column == "rate"))
                .map(|(column, label, _)| (*column, label.to_string()))
                .collect();
            let entry = &d.schedule[row];
            let footer = div()
                .flex()
                .flex_col()
                .gap_1()
                .when(lse, |col| {
                    col.child(
                        div()
                            .text_sm()
                            .child(format!("(g) Tax rate: {FORM_2552_LSE_RATE_TEXT}")),
                    )
                })
                .child(self.computed("(h) Tax due", entry.tax_due, cx))
                .into_any_element();
            children.push(self.table_row(
                "share",
                "schedule",
                row,
                &columns,
                Some(footer),
                layout,
                cx,
            ));
        }
        if let Some(add) = self.add_row_button("share", "Add a transaction", d.schedule.len(), cx) {
            children.push(add);
        }
        children.push(self.computed(
            &format!("6 — Total tax due (to Part II Item {})", 12 + schedule),
            d.schedule_total,
            cx,
        ));
        let title = match schedule {
            1 => "Part V Schedule 1 — Shares listed and traded through the LSE",
            2 => "Part V Schedule 2 — Primary public offering",
            _ => "Part V Schedule 3 — Secondary public offering",
        };
        self.section(title, children, cx)
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![
            self.computed("13 — Tax due, Schedule 1 (PT 200)", d.tax_due_lse, cx),
            self.computed("14 — Tax due, Schedule 2 (PT 201)", d.tax_due_primary, cx),
            self.computed("15 — Tax due, Schedule 3 (PT 202)", d.tax_due_secondary, cx),
            self.computed("16 — Total tax due", d.total_tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[0].1,
                "tax_paid_previous",
                layout,
                !editable || !d.is_amended,
            ),
            self.field(
                "cwt",
                MONEY_INPUTS[1].1,
                "creditable_tax_withheld",
                layout,
                !editable,
            ),
            self.computed("19 — Total tax credits/payments", d.total_tax_credits, cx),
            self.computed("20 — Tax still due/(overpayment)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[2].1, "surcharge", layout, !editable),
            self.field("int", MONEY_INPUTS[3].1, "interest", layout, !editable),
            self.field("comp", MONEY_INPUTS[4].1, "compromise", layout, !editable),
            self.computed("24 — Total penalties", d.total_penalties, cx),
            self.computed(
                "25 — Total amount payable/(overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        if d.tax_still_due < 0.0 {
            let over = d.overpayment;
            children.push(Self::choice_row(
                "If overpayment, mark one:",
                vec![
                    Self::choice(
                        "2552_refund",
                        "To be refunded",
                        over == Form2552Overpayment::Refund,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.overpayment = Form2552Overpayment::Refund)
                    })),
                    Self::choice(
                        "2552_tcc",
                        "To be issued a Tax Credit Certificate",
                        over == Form2552Overpayment::TaxCreditCertificate,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.overpayment = Form2552Overpayment::TaxCreditCertificate
                        })
                    })),
                ],
            ));
        }
        self.section(
            "Part II — Computation of Tax",
            vec![self.grid(layout, children)],
            cx,
        )
    }

    fn render_part_four(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let columns = [
            ("class", "Transaction classification".to_string()),
            ("amount", "Amount involved".to_string()),
        ];
        let mut children: Vec<AnyElement> = (0..d.exempt_transactions.len().min(FORM_2552_ROWS))
            .map(|row| {
                self.table_row(
                    "exempt",
                    "exempt_transactions",
                    row,
                    &columns,
                    None,
                    layout,
                    cx,
                )
            })
            .collect();
        if let Some(add) =
            self.add_row_button("exempt", "Add a row", d.exempt_transactions.len(), cx)
        {
            children.push(add);
        }
        self.section(
            "Part IV — Summary of Transactions not Subject to Tax",
            children,
            cx,
        )
    }

    fn render_part_six(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let columns: Vec<(&'static str, String)> = NON_TAXABLE_COLUMNS
            .iter()
            .map(|(column, label, _)| (*column, label.to_string()))
            .collect();
        let mut children: Vec<AnyElement> =
            (0..d.non_taxable_transactions.len().min(FORM_2552_ROWS))
                .map(|row| {
                    self.table_row(
                        "free",
                        "non_taxable_transactions",
                        row,
                        &columns,
                        None,
                        layout,
                        cx,
                    )
                })
                .collect();
        if let Some(add) =
            self.add_row_button("free", "Add a row", d.non_taxable_transactions.len(), cx)
        {
            children.push(add);
        }
        self.section(
            "Part VI — Details of Transactions not Subject to Percentage Tax",
            children,
            cx,
        )
    }
}

impl QueueableFormView for Form2552View {
    type Draft = Form2552Draft;

    fn new(
        draft: Form2552Draft,
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

    /// Today's date in the current year; 31 December for a past year.
    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form2552Draft {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive();
        let (month, day) = if i32::from(year) == today.year() {
            (today.month() as u8, today.day() as u8)
        } else {
            (12, 31)
        };
        Form2552Draft::new_from_profile(profile, year, month, day)
    }

    /// 2552 is keyed by its Item 1 date, not the dashboard's event counter.
    /// Reopen this year's latest return still in progress, else start one.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2552Draft {
        let tin = profile.tin.full();
        (1..=12i64)
            .flat_map(|month| (1..=31i64).map(move |day| month * 100 + day))
            .filter_map(|key| {
                db.get_queueable_draft::<Form2552Draft>(&tin, year, key)
                    .ok()
                    .flatten()
            })
            .filter(|draft| {
                !matches!(
                    draft.lifecycle.status,
                    FilingStatus::Confirmed | FilingStatus::Paid
                )
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

impl FormViewTrait for Form2552View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2552"
    }
    fn form_subtitle(&self) -> &'static str {
        "Percentage Tax Return for Transactions Involving Shares of Stock"
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
                    "2552 draft saved.".into(),
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
        if !can_queue_for_submission(Form2552Draft::FORM_CODE) {
            self.status_message =
                Some("2552 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 2552. No submission was started: {error}"
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
            "Form 2552 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2552 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2552Draft>(
                        &queued.tin,
                        queued.transaction_year,
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
            "2552-2018",
            &fields,
            "2552 — Print Preview",
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

impl Render for Form2552View {
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
                .child(Button::new("2552_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2552_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2552_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2552_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2552_submit")
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
                            Button::new("2552_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2552_release_cancel")
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
            .child(self.render_part_five(layout, cx))
            .child(self.render_part_two(layout, cx))
            .child(self.render_part_four(layout, cx))
            .child(self.render_part_six(layout, cx));
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
                    .id("2552_scroll")
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
    use super::{Layout, format_tin, parse_amount, row_key};
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
        assert_eq!(row_key("share", 2, "base"), "share2_base");
    }
}
