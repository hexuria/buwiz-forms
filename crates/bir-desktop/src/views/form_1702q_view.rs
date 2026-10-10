//! Editor for BIR Form 1702Q (January 2018), Quarterly Income Tax Return for
//! Corporations, Partnerships and Other Non-Individual Taxpayers. Rust owns
//! every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1702q`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1702q::{
    FORM_1702Q_ATC_OPTIONS, FORM_1702Q_OTHER_CREDIT_ROWS, Form1702qDeduction, Form1702qDraft,
    Form1702qOtherCredit, form_1702q_mcit_rate,
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

impl EventEmitter<QueueableFormEvent> for Form1702qView {}

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

type Draft = Form1702qDraft;

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

fn previous_quarters(d: &Draft) -> bool {
    d.previous_quarters_open()
}

fn itemized(d: &Draft) -> bool {
    d.deduction != Form1702qDeduction::Osd
}

const SCHEDULE_1A: &[Money] = &[
    Money {
        key: "s1a_sales",
        label: "1A — Sales/receipts/revenues/fees",
        get: |d| d.exempt.sales,
        set: |d, v| d.exempt.sales = v,
        open: always,
    },
    Money {
        key: "s1a_cost",
        label: "2A — Less: cost of sales/services",
        get: |d| d.exempt.cost_of_sales,
        set: |d, v| d.exempt.cost_of_sales = v,
        open: always,
    },
    Money {
        key: "s1a_other",
        label: "4A — Add: non-operating and other taxable income",
        get: |d| d.exempt.other_income,
        set: |d, v| d.exempt.other_income = v,
        open: always,
    },
    Money {
        key: "s1a_deductions",
        label: "6A — Less: deductions",
        get: |d| d.exempt.deductions,
        set: |d, v| d.exempt.deductions = v,
        open: always,
    },
    Money {
        key: "s1a_previous",
        label: "8A — Add: taxable income previous quarter/s",
        get: |d| d.exempt.previous_quarters,
        set: |d, v| d.exempt.previous_quarters = v,
        open: previous_quarters,
    },
];

const SCHEDULE_1A_COMPUTED: &[Computed] = &[
    Computed {
        label: "3A — Gross income from operation",
        get: |d| d.exempt.gross_income,
    },
    Computed {
        label: "5A — Total gross income",
        get: |d| d.exempt.total_gross_income,
    },
    Computed {
        label: "7A — Taxable income this quarter",
        get: |d| d.exempt.taxable_income,
    },
    Computed {
        label: "9A — Total taxable income to date",
        get: |d| d.exempt.total_taxable_income,
    },
    Computed {
        label: "11A — Income tax due (rate 0%)",
        get: |d| d.exempt.tax_due,
    },
];

const SCHEDULE_1B: &[Money] = &[
    Money {
        key: "s1b_sales",
        label: "1B — Sales/receipts/revenues/fees",
        get: |d| d.special.sales,
        set: |d, v| d.special.sales = v,
        open: always,
    },
    Money {
        key: "s1b_cost",
        label: "2B — Less: cost of sales/services",
        get: |d| d.special.cost_of_sales,
        set: |d, v| d.special.cost_of_sales = v,
        open: always,
    },
    Money {
        key: "s1b_other",
        label: "4B — Add: non-operating and other taxable income",
        get: |d| d.special.other_income,
        set: |d, v| d.special.other_income = v,
        open: always,
    },
    Money {
        key: "s1b_deductions",
        label: "6B — Less: deductions",
        get: |d| d.special.deductions,
        set: |d, v| d.special.deductions = v,
        open: always,
    },
    Money {
        key: "s1b_previous",
        label: "8B — Add: taxable income previous quarter/s",
        get: |d| d.special.previous_quarters,
        set: |d, v| d.special.previous_quarters = v,
        open: previous_quarters,
    },
    Money {
        key: "s1b_rate",
        label: "10B — Applicable income tax rate (%)",
        get: |d| d.special.rate,
        set: |d, v| d.special.rate = v,
        open: always,
    },
    Money {
        key: "s1b_share",
        label: "12B — Less: share of other agencies",
        get: |d| d.share_of_other_agencies,
        set: |d, v| d.share_of_other_agencies = v,
        open: always,
    },
];

const SCHEDULE_1B_COMPUTED: &[Computed] = &[
    Computed {
        label: "3B — Gross income from operation",
        get: |d| d.special.gross_income,
    },
    Computed {
        label: "5B — Total gross income",
        get: |d| d.special.total_gross_income,
    },
    Computed {
        label: "7B — Taxable income this quarter",
        get: |d| d.special.taxable_income,
    },
    Computed {
        label: "9B — Total taxable income to date",
        get: |d| d.special.total_taxable_income,
    },
    Computed {
        label: "11B — Income tax due other than MCIT",
        get: |d| d.special.tax_due,
    },
    Computed {
        label: "13B — Net income tax due (to Item 17)",
        get: |d| d.special_net_tax_due,
    },
];

const SCHEDULE_2: &[Money] = &[
    Money {
        key: "s2_sales",
        label: "1 — Sales/receipts/revenues/fees",
        get: |d| d.sales,
        set: |d, v| d.sales = v,
        open: always,
    },
    Money {
        key: "s2_cost",
        label: "2 — Less: cost of sales/services",
        get: |d| d.cost_of_sales,
        set: |d, v| d.cost_of_sales = v,
        open: always,
    },
    Money {
        key: "s2_other",
        label: "4 — Add: non-operating and other taxable income",
        get: |d| d.other_income,
        set: |d, v| d.other_income = v,
        open: always,
    },
    Money {
        key: "s2_deductions",
        label: "6 — Less: deductions (itemized)",
        get: |d| d.deductions,
        set: |d, v| d.deductions = v,
        open: itemized,
    },
    Money {
        key: "s2_previous",
        label: "8 — Add: taxable income previous quarter/s",
        get: |d| d.previous_quarters_taxable_income,
        set: |d, v| d.previous_quarters_taxable_income = v,
        open: previous_quarters,
    },
];

const SCHEDULE_2_COMPUTED: &[Computed] = &[
    Computed {
        label: "3 — Gross income from operation",
        get: |d| d.gross_income,
    },
    Computed {
        label: "5 — Total gross income",
        get: |d| d.total_gross_income,
    },
    Computed {
        label: "6 — Deductions (OSD: 40% of Item 5)",
        get: |d| d.deductions,
    },
    Computed {
        label: "7 — Taxable income this quarter",
        get: |d| d.taxable_income,
    },
    Computed {
        label: "9 — Total taxable income to date",
        get: |d| d.total_taxable_income,
    },
    Computed {
        label: "11 — Income tax due other than MCIT",
        get: |d| d.regular_tax_due,
    },
    Computed {
        label: "12 — MCIT (from Schedule 3)",
        get: |d| d.mcit,
    },
    Computed {
        label: "13 — Income tax due (higher of 11 and 12)",
        get: |d| d.income_tax_due,
    },
];

fn mcit_open(d: &Draft) -> bool {
    d.mcit_open()
}

const SCHEDULE_3: &[Money] = &[
    Money {
        key: "s3_q1",
        label: "1 — Gross income, 1st quarter",
        get: |d| d.mcit_gross_income_q1,
        set: |d, v| d.mcit_gross_income_q1 = v,
        open: mcit_open,
    },
    Money {
        key: "s3_q2",
        label: "2 — Gross income, 2nd quarter",
        get: |d| d.mcit_gross_income_q2,
        set: |d, v| d.mcit_gross_income_q2 = v,
        open: |d| d.mcit_open() && d.quarter >= 2,
    },
    Money {
        key: "s3_q3",
        label: "3 — Gross income, 3rd quarter",
        get: |d| d.mcit_gross_income_q3,
        set: |d, v| d.mcit_gross_income_q3 = v,
        open: |d| d.mcit_open() && d.quarter >= 3,
    },
];

const SCHEDULE_3_COMPUTED: &[Computed] = &[
    Computed {
        label: "4 — Total gross income",
        get: |d| d.mcit_total_gross_income,
    },
    Computed {
        label: "6 — Minimum corporate income tax",
        get: |d| d.mcit,
    },
];

const SCHEDULE_4: &[Money] = &[
    Money {
        key: "s4_prior",
        label: "1 — Prior year's excess credits",
        get: |d| d.prior_year_excess_credits,
        set: |d, v| d.prior_year_excess_credits = v,
        open: always,
    },
    Money {
        key: "s4_payments",
        label: "2 — Tax payments, previous quarters (other than MCIT)",
        get: |d| d.previous_quarters_payments,
        set: |d, v| d.previous_quarters_payments = v,
        open: previous_quarters,
    },
    Money {
        key: "s4_mcit_payments",
        label: "3 — MCIT payments, previous quarters",
        get: |d| d.previous_quarters_mcit_payments,
        set: |d, v| d.previous_quarters_mcit_payments = v,
        open: previous_quarters,
    },
    Money {
        key: "s4_cwt_previous",
        label: "4 — Creditable tax withheld, previous quarters",
        get: |d| d.previous_quarters_cwt,
        set: |d, v| d.previous_quarters_cwt = v,
        open: previous_quarters,
    },
    Money {
        key: "s4_cwt",
        label: "5 — Creditable tax withheld this quarter (2307)",
        get: |d| d.cwt_this_quarter,
        set: |d, v| d.cwt_this_quarter = v,
        open: always,
    },
    Money {
        key: "s4_previously_filed",
        label: "6 — Tax paid in return previously filed (amended)",
        get: |d| d.previously_filed,
        set: |d, v| d.previously_filed = v,
        open: |d| d.previously_filed_open(),
    },
];

const PART_II: &[Money] = &[
    Money {
        key: "p2_prior_mcit",
        label: "15 — Less: unexpired excess of prior year's MCIT",
        get: |d| d.prior_year_mcit_excess,
        set: |d, v| d.prior_year_mcit_excess = v,
        open: |d| d.prior_mcit_open(),
    },
    Money {
        key: "p2_surcharge",
        label: "21 — Surcharge",
        get: |d| d.surcharge,
        set: |d, v| d.surcharge = v,
        open: always,
    },
    Money {
        key: "p2_interest",
        label: "22 — Interest",
        get: |d| d.interest,
        set: |d, v| d.interest = v,
        open: always,
    },
    Money {
        key: "p2_compromise",
        label: "23 — Compromise",
        get: |d| d.compromise,
        set: |d, v| d.compromise = v,
        open: always,
    },
];

const PART_II_COMPUTED: &[Computed] = &[
    Computed {
        label: "14 — Income tax due, regular/normal rate",
        get: |d| d.income_tax_due,
    },
    Computed {
        label: "16 — Balance/income tax still due, regular rate",
        get: |d| d.regular_tax_still_due,
    },
    Computed {
        label: "17 — Add: income tax due, special rate",
        get: |d| d.special_net_tax_due,
    },
    Computed {
        label: "18 — Aggregate income tax due",
        get: |d| d.aggregate_tax_due,
    },
    Computed {
        label: "19 — Less: total tax credits/payments",
        get: |d| d.total_credits,
    },
    Computed {
        label: "20 — Net tax payable (overpayment)",
        get: |d| d.net_tax_payable,
    },
    Computed {
        label: "24 — Total penalties",
        get: |d| d.total_penalties,
    },
    Computed {
        label: "25 — Total amount payable (overpayment)",
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
        get: |d| d.taxpayer_name.clone(),
        set: |d, v| d.taxpayer_name = v,
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
        key: "phone",
        label: "Item 10 — Contact number",
        placeholder: "digits only",
        get: |d| d.contact_number.clone(),
        set: |d, v| d.contact_number = v.trim().to_string(),
    },
    Text {
        key: "email",
        label: "Item 11 — Email address",
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
        key: "relief",
        label: "Item 13A — Special law/treaty (specify)",
        placeholder: "",
        get: |d| d.tax_relief_specify.clone(),
        set: |d, v| d.tax_relief_specify = v,
    },
    Text {
        key: "s2_rate",
        label: "Schedule 2 Item 10 — Rate (%), when not set by the ATC",
        placeholder: "0.00",
        get: |d| d.regular_rate_text.clone(),
        set: |d, v| d.regular_rate_text = v.trim().to_string(),
    },
    Text {
        key: "s3_rate",
        label: "Schedule 3 Item 5 — MCIT rate (%), when open",
        placeholder: "0.00",
        get: |d| d.mcit_rate_text.clone(),
        set: |d, v| d.mcit_rate_text = v.trim().to_string(),
    },
    Text {
        key: "sheets",
        label: "Item 26 — Number of attachments",
        placeholder: "0",
        get: |d| d.number_of_attachments.to_string(),
        set: |d, v| d.number_of_attachments = v.trim().parse().unwrap_or(u8::MAX),
    },
];

fn other_desc_key(row: usize) -> String {
    format!("other_desc{row}")
}

fn other_amount_key(row: usize) -> String {
    format!("other_amount{row}")
}

/// Accepts `1,234.56`, `1234.5`, blank (zero).
fn parse_amount(value: &str) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ',' && *c != '%' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Some(0.0);
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn all_money() -> impl Iterator<Item = &'static Money> {
    SCHEDULE_1A
        .iter()
        .chain(SCHEDULE_1B)
        .chain(SCHEDULE_2)
        .chain(SCHEDULE_3)
        .chain(SCHEDULE_4)
        .chain(PART_II)
}

pub struct Form1702qView {
    draft: Form1702qDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1702qView {
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
        keys.extend(all_money().map(|m| (m.key.to_string(), "0".to_string())));
        for row in 0..FORM_1702Q_OTHER_CREDIT_ROWS {
            keys.push((other_desc_key(row), "Description".to_string()));
            keys.push((other_amount_key(row), "0".to_string()));
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
        for row in 0..FORM_1702Q_OTHER_CREDIT_ROWS {
            let entry = draft.other_credits.get(row).cloned().unwrap_or_default();
            if key == other_desc_key(row) {
                return entry.description;
            }
            if key == other_amount_key(row) {
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
        for text in TEXTS {
            (text.set)(&mut draft, self.input_text(text.key, cx));
        }
        for money in all_money() {
            let text = self.input_text(money.key, cx);
            match parse_amount(&text) {
                Some(value) => (money.set)(&mut draft, value),
                None => parse_errors.push((
                    money.key.to_string(),
                    format!("{}: \"{text}\" is not a number.", money.label),
                )),
            }
        }
        let mut rows = Vec::new();
        for row in 0..FORM_1702Q_OTHER_CREDIT_ROWS {
            let description = self.input_text(&other_desc_key(row), cx);
            let text = self.input_text(&other_amount_key(row), cx);
            let amount = match parse_amount(&text) {
                Some(value) => value,
                None => {
                    parse_errors.push((
                        other_amount_key(row),
                        format!(
                            "Schedule 4 other credit {}: \"{text}\" is not a number.",
                            row + 1
                        ),
                    ));
                    0.0
                }
            };
            rows.push(Form1702qOtherCredit {
                description,
                amount,
            });
        }
        draft.other_credits = rows;
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
                .release_abandoned_claimed_queueable::<Form1702qDraft>(
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
                Self::choice("1702q_calendar", "Calendar", !fiscal, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.set_tax_period_basis(TaxPeriodBasis::Calendar)
                        })
                    }),
                ),
                Self::choice("1702q_fiscal", "Fiscal", fiscal, !editable).on_click(cx.listener(
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
        if fiscal {
            let months = (1..=11u8)
                .map(|month| {
                    Self::choice(
                        ("1702q_month", month as usize),
                        format!("{month:02}"),
                        d.year_end_month == month,
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.year_end_month = month)
                    }))
                })
                .collect();
            children.push(self.choice_row("Item 2 — Month the fiscal year ends", months));
        }
        children.push(self.field("year", TEXTS[0].label, layout, !editable));
        let quarters = (1..=3u8)
            .map(|quarter| {
                Self::choice(
                    ("1702q_qtr", quarter as usize),
                    format!("Q{quarter}"),
                    d.quarter == quarter,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.quarter = quarter)
                }))
            })
            .collect();
        children.push(self.choice_row("Item 3 — Quarter", quarters));
        children.push(self.choice_row(
            "Item 4 — Amended return?",
            vec![
                Self::choice("1702q_amended_yes", "Yes", d.is_amended, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_amended = true)
                    }),
                ),
                Self::choice("1702q_amended_no", "No", !d.is_amended, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.is_amended = false)
                    }),
                ),
            ],
        ));
        children.push(self.choice_row(
            "Item 12 — Method of deductions",
            vec![
                Self::choice(
                    "1702q_itemized",
                    "Itemized deductions",
                    d.deduction == Form1702qDeduction::Itemized,
                    !editable,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| d.deduction = Form1702qDeduction::Itemized)
                })),
                Self::choice(
                    "1702q_osd",
                    "Optional standard deduction (40%)",
                    d.deduction == Form1702qDeduction::Osd,
                    !editable || !d.osd_allowed(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| d.deduction = Form1702qDeduction::Osd)
                })),
            ],
        ));
        children.push(self.choice_row(
            "Item 13 — Availing of tax relief under a special law/treaty?",
            vec![
                Self::choice("1702q_relief_yes", "Yes", d.tax_relief, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.tax_relief = true)
                    }),
                ),
                Self::choice("1702q_relief_no", "No", !d.tax_relief, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.tax_relief = false)
                    }),
                ),
            ],
        ));
        children.push(self.field("relief", TEXTS[7].label, layout, !editable || !d.tax_relief));
        children.push(self.field("sheets", TEXTS[10].label, layout, !editable));
        let mut sections =
            vec![self.section("Return period", vec![self.grid(layout, children)], cx)];

        // Item 5.
        let atc_buttons = FORM_1702Q_ATC_OPTIONS
            .iter()
            .enumerate()
            .map(|(index, option)| {
                let value = option.value;
                Self::choice(
                    ("1702q_atc", index),
                    option.label,
                    d.atc == value,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.atc = value.to_string())
                }))
            })
            .collect();
        let mcit = vec![
            Self::choice(
                "1702q_mcit",
                "IC055 — Minimum Corporate Income Tax (MCIT)",
                d.atc_mcit,
                !editable,
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.edit(window, cx, |d| d.atc_mcit = !d.atc_mcit)
            })),
        ];
        sections.push(self.section(
            "Item 5 — Alphanumeric Tax Code (ATC)",
            vec![
                self.choice_row("MCIT", mcit),
                self.choice_row("Regular/special rate ATC", atc_buttons),
            ],
            cx,
        ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .children(sections)
            .into_any_element()
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
        for text in &TEXTS[1..7] {
            fields.push(self.field(text.key, text.label, layout, !editable));
        }
        self.section(
            "Part I — Background Information",
            vec![self.grid(layout, fields)],
            cx,
        )
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut sections = vec![
            self.money_section(
                "Schedule 1A — Exempt",
                SCHEDULE_1A,
                SCHEDULE_1A_COMPUTED,
                layout,
                cx,
            ),
            self.money_section(
                "Schedule 1B — Special rate",
                SCHEDULE_1B,
                SCHEDULE_1B_COMPUTED,
                layout,
                cx,
            ),
        ];
        let mut s2: Vec<AnyElement> = SCHEDULE_2
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        let rate_text = match d.forced_regular_rate() {
            Some(rate) => self.computed_text("10 — Applicable income tax rate (%)", rate, cx),
            None => self.field("s2_rate", TEXTS[8].label, layout, !editable),
        };
        s2.push(rate_text);
        s2.extend(
            SCHEDULE_2_COMPUTED
                .iter()
                .map(|c| self.computed(c.label, (c.get)(d), cx)),
        );
        sections.push(self.section(
            "Schedule 2 — Regular/normal rate",
            vec![self.grid(layout, s2)],
            cx,
        ));
        let mut s3: Vec<AnyElement> = SCHEDULE_3
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        let rate_open = d.mcit_open() && form_1702q_mcit_rate(d.taxable_year, d.year_end_month).1;
        s3.push(if rate_open {
            self.field("s3_rate", TEXTS[9].label, layout, !editable)
        } else {
            self.computed_text("5 — MCIT rate (%)", &d.mcit_rate_display(), cx)
        });
        s3.extend(
            SCHEDULE_3_COMPUTED
                .iter()
                .map(|c| self.computed(c.label, (c.get)(d), cx)),
        );
        sections.push(self.section(
            "Schedule 3 — Minimum corporate income tax (needs both Item 5 boxes)",
            vec![self.grid(layout, s3)],
            cx,
        ));
        let mut s4: Vec<AnyElement> = SCHEDULE_4
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        for row in 0..FORM_1702Q_OTHER_CREDIT_ROWS {
            let letter = if row == 0 { "6a" } else { "6b" };
            s4.push(self.field(
                &other_desc_key(row),
                &format!("{letter} — Other tax credit/payment (specify)"),
                layout,
                !editable,
            ));
            s4.push(self.field(
                &other_amount_key(row),
                &format!("{letter} — Amount"),
                layout,
                !editable,
            ));
        }
        s4.push(self.computed("7 — Total tax credits/payments", d.total_credits, cx));
        sections.push(self.section(
            "Schedule 4 — Tax credits/payments",
            vec![self.grid(layout, s4)],
            cx,
        ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .children(sections)
            .into_any_element()
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

impl QueueableFormView for Form1702qView {
    type Draft = Form1702qDraft;

    fn new(
        draft: Form1702qDraft,
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1702qDraft {
        let quarter = if (1..=3).contains(&period) { period } else { 1 };
        Form1702qDraft::new_from_profile(profile, year, quarter)
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

impl FormViewTrait for Form1702qView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702Q"
    }
    fn form_subtitle(&self) -> &'static str {
        "Quarterly Income Tax Return for Corporations, Partnerships and Other Non-Individual Taxpayers"
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
                    "1702Q draft saved.".into(),
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
        if !can_queue_for_submission(Form1702qDraft::FORM_CODE) {
            self.status_message =
                Some("1702Q is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1702Q. No submission was started: {error}"
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
            "Form 1702Q queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1702Q payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702qDraft>(
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
            "1702q-2018",
            &fields,
            "1702Q — Print Preview",
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

impl Render for Form1702qView {
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
                .child(Button::new("1702q_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1702q_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1702q_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1702q_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1702q_submit")
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
                            Button::new("1702q_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1702q_release_cancel")
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
            .child(self.money_section(
                "Part II — Total tax payable",
                PART_II,
                PART_II_COMPUTED,
                layout,
                cx,
            ));
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
                    .id("1702q_scroll")
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
    use super::{Form1702qView, Layout, format_tin, parse_amount};
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
        let keys = Form1702qView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len());
    }
}
