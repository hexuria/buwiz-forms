//! Editor for BIR Form 1702-EX (January 2018), Annual Income Tax Return for
//! Corporations, Partnerships and Other Non-Individual Taxpayers Exempt from
//! Income Tax. Rust owns every calculation, validation and the official
//! submit plaintext (`bir_core::forms::form_1702ex`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1702ex::{
    FORM_1702EX_ORDINARY_ITEMS, FORM_1702EX_OTHER_DEDUCTION_ROWS,
    FORM_1702EX_SPECIAL_DEDUCTION_ROWS, Form1702ExAtc, Form1702ExDeduction, Form1702ExDraft,
    Form1702ExOverpayment, Form1702ExRow, Form1702ExSpecialRow,
};
use bir_core::forms::form_2551q::TaxPeriodBasis;
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

impl EventEmitter<QueueableFormEvent> for Form1702ExView {}

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

type Draft = Form1702ExDraft;

/// An editable whole-peso amount: (key, label, get, set, open).
struct Money {
    key: &'static str,
    label: &'static str,
    get: fn(&Draft) -> f64,
    set: fn(&mut Draft, f64),
    open: fn(&Draft) -> bool,
}

/// A computed amount: (label, get).
struct Computed {
    label: &'static str,
    get: fn(&Draft) -> f64,
}

/// A text input: (key, label, placeholder, get, set).
struct Text {
    key: &'static str,
    label: &'static str,
    placeholder: &'static str,
    get: fn(&Draft) -> String,
    set: fn(&mut Draft, String),
}

fn always(_: &Draft) -> bool {
    true
}

fn itemized(d: &Draft) -> bool {
    d.itemized_open()
}

const PART_IV: &[Money] = &[
    Money {
        key: "sales",
        label: "28 — Sales/receipts/revenues/fees",
        get: |d| d.sales,
        set: |d, v| d.sales = v,
        open: always,
    },
    Money {
        key: "returns",
        label: "29 — Less: sales returns, allowances and discounts",
        get: |d| d.sales_returns,
        set: |d, v| d.sales_returns = v,
        open: always,
    },
    Money {
        key: "cost",
        label: "31 — Less: cost of sales/services",
        get: |d| d.cost_of_sales,
        set: |d, v| d.cost_of_sales = v,
        open: always,
    },
    Money {
        key: "other_income",
        label: "33 — Add: other taxable income",
        get: |d| d.other_income,
        set: |d, v| d.other_income = v,
        open: always,
    },
    Money {
        key: "rate",
        label: "40 — Applicable income tax rate (%)",
        get: |d| d.tax_rate,
        set: |d, v| d.tax_rate = v,
        open: always,
    },
];

const PART_IV_COMPUTED: &[Computed] = &[
    Computed {
        label: "30 — Net sales/receipts/revenues/fees",
        get: |d| d.net_sales,
    },
    Computed {
        label: "32 — Gross income from operation",
        get: |d| d.gross_income,
    },
    Computed {
        label: "34 — Total gross income",
        get: |d| d.total_gross_income,
    },
    Computed {
        label: "35 — Ordinary allowable itemized deductions (Schedule 1)",
        get: |d| d.ordinary_deductions,
    },
    Computed {
        label: "36 — Special allowable itemized deductions (Schedule 2)",
        get: |d| d.special_deductions,
    },
    Computed {
        label: "37 — Total itemized deductions",
        get: |d| d.total_itemized,
    },
    Computed {
        label: "38 — Optional standard deduction (40% of Item 34)",
        get: |d| d.osd,
    },
    Computed {
        label: "39 — Net taxable income",
        get: |d| d.net_taxable_income,
    },
    Computed {
        label: "41 — Tax due",
        get: |d| d.tax_due,
    },
];

const CREDITS: &[Money] = &[
    Money {
        key: "prior_excess",
        label: "42 — Prior year's excess credits",
        get: |d| d.prior_year_excess,
        set: |d, v| d.prior_year_excess = v,
        open: always,
    },
    Money {
        key: "quarterly",
        label: "43 — Income tax payments, previous quarters",
        get: |d| d.quarterly_payments,
        set: |d, v| d.quarterly_payments = v,
        open: always,
    },
    Money {
        key: "cwt_previous",
        label: "44 — Creditable tax withheld, previous quarters",
        get: |d| d.cwt_previous_quarters,
        set: |d, v| d.cwt_previous_quarters = v,
        open: always,
    },
    Money {
        key: "cwt_q4",
        label: "45 — Creditable tax withheld, 4th quarter",
        get: |d| d.cwt_q4,
        set: |d, v| d.cwt_q4 = v,
        open: always,
    },
    Money {
        key: "foreign",
        label: "46 — Foreign tax credits",
        get: |d| d.foreign_tax_credits,
        set: |d, v| d.foreign_tax_credits = v,
        open: always,
    },
    Money {
        key: "previously_filed",
        label: "47 — Tax paid in return previously filed (amended)",
        get: |d| d.previously_filed,
        set: |d, v| d.previously_filed = v,
        open: |d| d.previously_filed_open(),
    },
];

const PART_V: &[Money] = &[
    Money {
        key: "regular_tax",
        label: "52 — Regular income tax otherwise due",
        get: |d| d.regular_income_tax,
        set: |d, v| d.regular_income_tax = v,
        open: always,
    },
    Money {
        key: "special_relief",
        label: "53 — Special allowable itemized deductions (× regular rate)",
        get: |d| d.special_allowable_relief,
        set: |d, v| d.special_allowable_relief = v,
        open: always,
    },
];

const PART_II: &[Money] = &[Money {
    key: "penalties",
    label: "21 — Add: penalties (compromise)",
    get: |d| d.penalties,
    set: |d, v| d.penalties = v,
    open: always,
}];

const PART_II_COMPUTED: &[Computed] = &[
    Computed {
        label: "18 — Tax due",
        get: |d| d.tax_due,
    },
    Computed {
        label: "19 — Less: total tax credits/payments",
        get: |d| d.total_credits,
    },
    Computed {
        label: "20 — Net tax payable (overpayment)",
        get: |d| d.net_payable,
    },
    Computed {
        label: "22 — Total amount payable (overpayment)",
        get: |d| d.total_amount_payable,
    },
];

const TEXTS: &[Text] = &[
    Text {
        key: "year",
        label: "Item 2 — Year ended (YYYY)",
        placeholder: "YYYY",
        get: |d| d.taxable_year.to_string(),
        set: |d, v| d.taxable_year = v.trim().parse().unwrap_or(0),
    },
    Text {
        key: "name",
        label: "Item 8 — Registered name",
        placeholder: "",
        get: |d| d.registered_name.clone(),
        set: |d, v| d.registered_name = v,
    },
    Text {
        key: "address",
        label: "Item 9 — Registered address",
        placeholder: "",
        get: |d| d.registered_address.clone(),
        set: |d, v| d.registered_address = v,
    },
    Text {
        key: "zip",
        label: "Item 9A — Zip code",
        placeholder: "",
        get: |d| d.zip_code.clone(),
        set: |d, v| d.zip_code = v.trim().to_string(),
    },
    Text {
        key: "incorporated",
        label: "Item 10 — Date of incorporation/organization",
        placeholder: "MM/DD/YYYY",
        get: |d| d.date_of_incorporation.clone(),
        set: |d, v| d.date_of_incorporation = v.trim().to_string(),
    },
    Text {
        key: "phone",
        label: "Item 11 — Contact number",
        placeholder: "digits only",
        get: |d| d.contact_number.clone(),
        set: |d, v| d.contact_number = v.trim().to_string(),
    },
    Text {
        key: "email",
        label: "Item 12 — Email address",
        placeholder: "",
        get: |d| d.email.clone(),
        set: |d, v| d.email = v.trim().to_string(),
    },
    Text {
        key: "lob",
        label: "Line of business",
        placeholder: "",
        get: |d| d.line_of_business.clone(),
        set: |d, v| d.line_of_business = v,
    },
    Text {
        key: "legal_basis",
        label: "Item 14 — Legal basis of tax relief/exemption",
        placeholder: "",
        get: |d| d.legal_basis.clone(),
        set: |d, v| d.legal_basis = v,
    },
    Text {
        key: "agency",
        label: "Item 15 — Investment Promotion Agency/Government Agency",
        placeholder: "",
        get: |d| d.promotion_agency.clone(),
        set: |d, v| d.promotion_agency = v,
    },
    Text {
        key: "activity",
        label: "Item 16 — Registered activity/program (Reg. No.)",
        placeholder: "",
        get: |d| d.registered_activity.clone(),
        set: |d, v| d.registered_activity = v,
    },
    Text {
        key: "eff_from",
        label: "Item 17 — Effectivity from",
        placeholder: "MM/DD/YYYY",
        get: |d| d.effectivity_from.clone(),
        set: |d, v| d.effectivity_from = v.trim().to_string(),
    },
    Text {
        key: "eff_to",
        label: "Item 17 — Effectivity to",
        placeholder: "MM/DD/YYYY",
        get: |d| d.effectivity_to.clone(),
        set: |d, v| d.effectivity_to = v.trim().to_string(),
    },
    Text {
        key: "sheets",
        label: "Item 23 — Number of attachments",
        placeholder: "0",
        get: |d| d.number_of_attachments.to_string(),
        set: |d, v| d.number_of_attachments = v.trim().parse().unwrap_or(u8::MAX),
    },
];

/// A list of description/amount rows: (key, label, rows, get, get_mut, open).
struct RowList {
    key: &'static str,
    label: &'static str,
    rows: usize,
    get: fn(&Draft) -> &Vec<Form1702ExRow>,
    get_mut: fn(&mut Draft) -> &mut Vec<Form1702ExRow>,
    open: fn(&Draft) -> bool,
}

const ROW_LISTS: &[RowList] = &[
    RowList {
        key: "credit",
        label: "48–49 — Other tax credits/payments",
        rows: 2,
        get: |d| &d.other_credits,
        get_mut: |d| &mut d.other_credits,
        open: always,
    },
    RowList {
        key: "s1other",
        label: "Schedule 1 Items 17D–17I — Others",
        rows: FORM_1702EX_OTHER_DEDUCTION_ROWS,
        get: |d| &d.other_deductions,
        get_mut: |d| &mut d.other_deductions,
        open: itemized,
    },
    RowList {
        key: "s3nondeduct",
        label: "Schedule 3 Items 2–3 — Non-deductible expenses/taxable other income",
        rows: 2,
        get: |d| &d.non_deductible,
        get_mut: |d| &mut d.non_deductible,
        open: always,
    },
    RowList {
        key: "s3nontax",
        label: "Schedule 3 Items 5–6 — Non-taxable income and income subject to final tax",
        rows: 2,
        get: |d| &d.non_taxable_income,
        get_mut: |d| &mut d.non_taxable_income,
        open: always,
    },
    RowList {
        key: "s3special",
        label: "Schedule 3 Items 7–8 — Special deductions",
        rows: 2,
        get: |d| &d.special_deductions_s3,
        get_mut: |d| &mut d.special_deductions_s3,
        open: always,
    },
];

fn row_key(list: &str, row: usize, field: &str) -> String {
    format!("{list}{row}:{field}")
}

fn ordinary_key(index: usize) -> String {
    format!("s1item{index}")
}

/// Accepts `1,234.56`, `(1,234.56)`, `1234.5`, blank (zero).
fn parse_amount(value: &str) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ',' && *c != '%' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Some(0.0);
    }
    if let Some(inner) = cleaned.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        return inner
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| -v);
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn all_money() -> impl Iterator<Item = &'static Money> {
    PART_IV.iter().chain(CREDITS).chain(PART_V).chain(PART_II)
}

pub struct Form1702ExView {
    draft: Form1702ExDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1702ExView {
    fn money_text(value: f64) -> String {
        if value == 0.0 {
            String::new()
        } else {
            official_amount(value)
        }
    }

    fn all_keys() -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = TEXTS
            .iter()
            .map(|t| (t.key.to_string(), t.placeholder.to_string()))
            .collect();
        keys.extend(all_money().map(|m| (m.key.to_string(), "0.00".to_string())));
        keys.push(("net_books".to_string(), "0.00".to_string()));
        for index in 0..FORM_1702EX_ORDINARY_ITEMS.len() {
            keys.push((ordinary_key(index), "0.00".to_string()));
        }
        for list in ROW_LISTS {
            for row in 0..list.rows {
                keys.push((row_key(list.key, row, "desc"), "Description".to_string()));
                keys.push((row_key(list.key, row, "amount"), "0.00".to_string()));
            }
        }
        for row in 0..FORM_1702EX_SPECIAL_DEDUCTION_ROWS {
            keys.push((row_key("s2", row, "desc"), "Description".to_string()));
            keys.push((row_key("s2", row, "basis"), "Legal basis".to_string()));
            keys.push((row_key("s2", row, "amount"), "0.00".to_string()));
        }
        keys
    }

    /// Editor text for one input from the draft.
    fn initial(draft: &Draft, key: &str) -> String {
        if let Some(text) = TEXTS.iter().find(|t| t.key == key) {
            return (text.get)(draft);
        }
        if let Some(money) = all_money().find(|m| m.key == key) {
            return Self::money_text((money.get)(draft));
        }
        if key == "net_books" {
            return Self::money_text(draft.net_income_per_books);
        }
        for index in 0..FORM_1702EX_ORDINARY_ITEMS.len() {
            if key == ordinary_key(index) {
                return Self::money_text(draft.ordinary_items[index]);
            }
        }
        for list in ROW_LISTS {
            for row in 0..list.rows {
                let entry = (list.get)(draft).get(row).cloned().unwrap_or_default();
                if key == row_key(list.key, row, "desc") {
                    return entry.description;
                }
                if key == row_key(list.key, row, "amount") {
                    return Self::money_text(entry.amount);
                }
            }
        }
        for row in 0..FORM_1702EX_SPECIAL_DEDUCTION_ROWS {
            let entry = draft.special_rows.get(row).cloned().unwrap_or_default();
            if key == row_key("s2", row, "desc") {
                return entry.description;
            }
            if key == row_key("s2", row, "basis") {
                return entry.legal_basis;
            }
            if key == row_key("s2", row, "amount") {
                return Self::money_text(entry.amount);
            }
        }
        String::new()
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
        let mut number = |key: &str, label: &str, cx: &App| -> f64 {
            let text = self.input_text(key, cx);
            parse_amount(&text).unwrap_or_else(|| {
                parse_errors.push((
                    key.to_string(),
                    format!("{label}: \"{text}\" is not a number."),
                ));
                0.0
            })
        };
        for money in all_money() {
            let value = number(money.key, money.label, cx);
            (money.set)(&mut draft, value);
        }
        draft.net_income_per_books = number("net_books", "Schedule 3 Item 1", cx);
        for (index, (_, label)) in FORM_1702EX_ORDINARY_ITEMS.iter().enumerate() {
            draft.ordinary_items[index] = number(&ordinary_key(index), label, cx);
        }
        for list in ROW_LISTS {
            let mut rows = Vec::new();
            for row in 0..list.rows {
                let amount = number(&row_key(list.key, row, "amount"), list.label, cx);
                rows.push(Form1702ExRow {
                    description: String::new(),
                    amount,
                });
            }
            *(list.get_mut)(&mut draft) = rows;
        }
        let mut special = Vec::new();
        for row in 0..FORM_1702EX_SPECIAL_DEDUCTION_ROWS {
            let amount = number(&row_key("s2", row, "amount"), "Schedule 2", cx);
            special.push(Form1702ExSpecialRow {
                description: String::new(),
                legal_basis: String::new(),
                amount,
            });
        }
        for text in TEXTS {
            (text.set)(&mut draft, self.input_text(text.key, cx));
        }
        for list in ROW_LISTS {
            for (row, entry) in (list.get_mut)(&mut draft).iter_mut().enumerate() {
                entry.description = self.input_text(&row_key(list.key, row, "desc"), cx);
            }
        }
        for (row, entry) in special.iter_mut().enumerate() {
            entry.description = self.input_text(&row_key("s2", row, "desc"), cx);
            entry.legal_basis = self.input_text(&row_key("s2", row, "basis"), cx);
        }
        draft.special_rows = special;
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// Apply a choice, then reload every editor: the ATC and radio rules
    /// clear and force values.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Draft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        change(&mut self.draft);
        self.draft.recompute();
        for (key, _) in Self::all_keys() {
            if let Some(input) = self.inputs.get(&key) {
                let value = Self::initial(&self.draft, &key);
                if input.read(cx).value() != value.as_str() {
                    input.update(cx, |state, cx| state.set_value(value, window, cx));
                }
            }
        }
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
                .release_abandoned_claimed_queueable::<Form1702ExDraft>(
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
            .any(|(field, _)| field == key);
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
                .child(label.w(relative(0.55)))
                .child(body.w(relative(0.45))),
        }
        .into_any_element()
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
        button.small().disabled(disabled)
    }

    fn choice_row(&self, label: &str, buttons: Vec<Button>) -> AnyElement {
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
            .child(div().flex().flex_wrap().gap_2().children(buttons))
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

    fn money_section(
        &self,
        title: &str,
        moneys: &[Money],
        computed: &[Computed],
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = moneys
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(&self.draft)))
            .collect();
        children.extend(
            computed
                .iter()
                .map(|c| self.computed(c.label, (c.get)(&self.draft), cx)),
        );
        self.section(title, vec![self.grid(layout, children)], cx)
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fiscal = d.tax_period_basis == TaxPeriodBasis::Fiscal;
        let mut children = vec![self.choice_row(
            "Item 1 — For the",
            vec![
                Self::choice("1702ex_calendar", "Calendar", !fiscal, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.set_tax_period_basis(TaxPeriodBasis::Calendar)
                        })
                    }),
                ),
                Self::choice("1702ex_fiscal", "Fiscal", fiscal, !editable).on_click(cx.listener(
                    |this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.set_tax_period_basis(TaxPeriodBasis::Fiscal);
                            if d.year_end_month == 12 {
                                d.year_end_month = 6;
                            }
                        })
                    },
                )),
            ],
        )];
        if fiscal || d.is_short_period {
            let months = (1..=12u8)
                .map(|month| {
                    Self::choice(
                        ("1702ex_month", month as usize),
                        format!("{month:02}"),
                        d.year_end_month == month,
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.year_end_month = month)
                    }))
                })
                .collect();
            children.push(self.choice_row("Item 2 — Month the year ends", months));
        }
        children.push(self.field("year", TEXTS[0].label, layout, !editable));
        children.push(self.choice_row(
            "Item 3 — Amended return?",
            vec![
                Self::choice("1702ex_amended_yes", "Yes", d.is_amended, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_amended = true)
                    }),
                ),
                Self::choice("1702ex_amended_no", "No", !d.is_amended, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_amended = false)
                    }),
                ),
            ],
        ));
        children.push(self.choice_row(
            "Item 4 — Short period return?",
            vec![
                Self::choice("1702ex_short_yes", "Yes", d.is_short_period, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_short_period = true)
                    }),
                ),
                Self::choice("1702ex_short_no", "No", !d.is_short_period, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_short_period = false)
                    }),
                ),
            ],
        ));
        children.push(self.choice_row(
            "Item 5 — ATC",
            vec![
                    Self::choice(
                        "1702ex_ic011",
                        "IC 011 Exempt corporation on exempt activities",
                        d.atc == Form1702ExAtc::IC011,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.atc = Form1702ExAtc::IC011)
                    })),
                    Self::choice(
                        "1702ex_ic021",
                        "IC 021 General professional partnership",
                        d.atc == Form1702ExAtc::IC021,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.atc = Form1702ExAtc::IC021)
                    })),
                ],
        ));
        children.push(self.choice_row(
            "Item 13 — Method of deduction",
            vec![
                    Self::choice(
                        "1702ex_itemized",
                        "Itemized deductions",
                        d.deduction == Form1702ExDeduction::Itemized,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.deduction = Form1702ExDeduction::Itemized)
                    })),
                    Self::choice(
                        "1702ex_osd",
                        "Optional standard deduction (40%)",
                        d.deduction == Form1702ExDeduction::Osd,
                        !editable || d.atc == Form1702ExAtc::IC011,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.deduction = Form1702ExDeduction::Osd)
                    })),
                ],
        ));
        children.push(self.field("sheets", TEXTS[13].label, layout, !editable));
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut fields = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 6 — TIN / Item 7 — RDO"),
                )
                .child(div().font_weight(FontWeight::BOLD).child(format!(
                    "{}  ·  RDO {}",
                    format_tin(&d.tin),
                    d.rdo_code
                )))
                .into_any_element(),
        ];
        for text in &TEXTS[1..13] {
            fields.push(self.field(text.key, text.label, layout, !editable));
        }
        self.section(
            "Part I — Background Information",
            vec![self.grid(layout, fields)],
            cx,
        )
    }

    fn row_list(&self, list: &RowList, layout: Layout) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let open = (list.open)(&self.draft);
        let mut fields = Vec::new();
        for row in 0..list.rows {
            fields.push(self.field(
                &row_key(list.key, row, "desc"),
                &format!("{} — row {} description", list.label, row + 1),
                layout,
                !editable || !open,
            ));
            fields.push(self.field(
                &row_key(list.key, row, "amount"),
                &format!("{} — row {} amount", list.label, row + 1),
                layout,
                !editable || !open,
            ));
        }
        self.grid(layout, fields)
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let itemized = d.itemized_open();
        let mut part_iv: Vec<AnyElement> = PART_IV
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        part_iv.extend(
            PART_IV_COMPUTED
                .iter()
                .map(|c| self.computed(c.label, (c.get)(d), cx)),
        );
        let mut credits: Vec<AnyElement> = CREDITS
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        credits.push(self.row_list(&ROW_LISTS[0], layout));
        credits.push(self.computed("50 — Total tax credits/payments", d.total_credits, cx));
        credits.push(self.computed("51 — Net tax payable (overpayment)", d.net_payable, cx));
        let mut part_v: Vec<AnyElement> = PART_V
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable))
            .collect();
        part_v.push(self.computed("54 — Total tax relief availment", d.tax_relief, cx));
        let mut s1: Vec<AnyElement> = FORM_1702EX_ORDINARY_ITEMS
            .iter()
            .enumerate()
            .map(|(index, (_, label))| {
                self.field(&ordinary_key(index), label, layout, !editable || !itemized)
            })
            .collect();
        s1.push(self.row_list(&ROW_LISTS[1], layout));
        s1.push(self.computed(
            "18 — Total ordinary allowable itemized deductions",
            d.ordinary_deductions,
            cx,
        ));
        let mut s2 = Vec::new();
        for row in 0..FORM_1702EX_SPECIAL_DEDUCTION_ROWS {
            for (field, label) in [
                ("desc", "description"),
                ("basis", "legal basis"),
                ("amount", "amount"),
            ] {
                s2.push(self.field(
                    &row_key("s2", row, field),
                    &format!("Item {} — {label}", row + 1),
                    layout,
                    !editable || !itemized,
                ));
            }
        }
        s2.push(self.computed(
            "5 — Total special allowable itemized deductions",
            d.special_deductions,
            cx,
        ));
        let mut s3 = vec![self.field(
            "net_books",
            "1 — Net income (loss) per books",
            layout,
            !editable,
        )];
        s3.push(self.row_list(&ROW_LISTS[2], layout));
        s3.push(self.computed("4 — Total", d.reconciliation_total, cx));
        s3.push(self.row_list(&ROW_LISTS[3], layout));
        s3.push(self.row_list(&ROW_LISTS[4], layout));
        s3.push(self.computed("9 — Total", d.reconciliation_less, cx));
        s3.push(self.computed(
            "10 — Net taxable income (must equal Item 39)",
            d.reconciled_taxable_income,
            cx,
        ));
        let sections = vec![
            self.section(
                "Part IV — Computation of tax",
                vec![self.grid(layout, part_iv)],
                cx,
            ),
            self.section("Part IV — Tax credits/payments", credits, cx),
            self.section(
                "Part V — Tax relief availment",
                vec![self.grid(layout, part_v)],
                cx,
            ),
            self.section(
                "Schedule 1 — Ordinary allowable itemized deductions",
                vec![self.grid(layout, s1)],
                cx,
            ),
            self.section(
                "Schedule 2 — Special allowable itemized deductions",
                vec![self.grid(layout, s2)],
                cx,
            ),
            self.section(
                "Schedule 3 — Reconciliation of net income per books",
                s3,
                cx,
            ),
        ];
        div()
            .flex()
            .flex_col()
            .gap_5()
            .children(sections)
            .into_any_element()
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children: Vec<AnyElement> = PART_II
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable))
            .collect();
        children.extend(
            PART_II_COMPUTED
                .iter()
                .map(|c| self.computed(c.label, (c.get)(d), cx)),
        );
        let mut sections = vec![self.grid(layout, children)];
        if d.overpayment_open() {
            let options = [
                (Form1702ExOverpayment::Refund, "To be refunded"),
                (
                    Form1702ExOverpayment::TaxCreditCertificate,
                    "To be issued a Tax Credit Certificate",
                ),
                (Form1702ExOverpayment::CarryOver, "To be carried over"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    ("1702ex_over", index),
                    label,
                    d.overpayment == value,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.overpayment = value)
                }))
            })
            .collect();
            sections.push(self.choice_row("If overpayment, mark one box only", options));
        }
        self.section("Part II — Total tax payable", sections, cx)
    }

    fn computed_text(&self, label: &str, value: &str, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div text_right font_weight={FontWeight::BOLD}>{value.to_string()}</div>
            </div>
        }
        .into_any_element()
    }
}

impl QueueableFormView for Form1702ExView {
    type Draft = Form1702ExDraft;

    fn new(
        draft: Form1702ExDraft,
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1702ExDraft {
        Form1702ExDraft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1702ExView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702EX"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Corporations, Partnerships and Other Non-Individual Taxpayers Exempt from Income Tax"
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
                    "1702EX draft saved.".into(),
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
        if !can_queue_for_submission(Form1702ExDraft::FORM_CODE) {
            self.status_message =
                Some("1702EX is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1702EX. No submission was started: {error}"
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
            "Form 1702EX queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1702EX payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702ExDraft>(
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
        let fields = self.draft.to_print_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "1702ex-2018",
            &fields,
            "1702EX — Print Preview",
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

impl Render for Form1702ExView {
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
                .child(Button::new("1702ex_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1702ex_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1702ex_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1702ex_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1702ex_submit")
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
                            Button::new("1702ex_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1702ex_release_cancel")
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
            .child(self.render_part_one(layout, cx))
            .child(self.render_schedules(layout, cx))
            .child(self.render_totals(layout, cx));
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
                    .id("1702ex_scroll")
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
    use super::{Form1702ExView, Layout, format_tin, parse_amount};
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
    fn input_keys_are_unique() {
        let keys = Form1702ExView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len());
    }
}
