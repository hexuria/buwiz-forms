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
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{
    self, field_error, messages_for, select_field, with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form1702qView {}

/// Item 2 fiscal year-end month choices: (stored value, shown label). The
/// fiscal year never ends in December (that is the calendar year).
const MONTH_OPTIONS: [(&str, &str); 11] = [
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
];

/// Item 5 regular/special rate ATC choices: (stored value, shown label).
fn atc_options() -> Vec<(&'static str, &'static str)> {
    FORM_1702Q_ATC_OPTIONS
        .iter()
        .map(|option| (option.value, option.label))
        .collect()
}

/// The model's field names behind an editor key (validation uses them).
fn model_keys(key: &str) -> &'static [&'static str] {
    match key {
        "year" => &["taxable_year"],
        "name" => &["taxpayer_name"],
        "address" => &["registered_address"],
        "zip" => &["zip_code"],
        "phone" => &["contact_number"],
        "lob" => &["line_of_business"],
        "relief" => &["tax_relief_specify"],
        "s2_rate" => &["regular_rate_text"],
        "s3_rate" => &["mcit_rate_text"],
        "sheets" => &["number_of_attachments"],
        "s1a_sales" => &["exempt.sales"],
        "s1a_cost" => &["exempt.cost_of_sales"],
        "s1a_other" => &["exempt.other_income"],
        "s1a_deductions" => &["exempt.deductions"],
        "s1b_sales" => &["special.sales"],
        "s1b_cost" => &["special.cost_of_sales"],
        "s1b_other" => &["special.other_income"],
        "s1b_deductions" => &["special.deductions"],
        "s1b_rate" => &["special.rate"],
        "s1b_share" => &["share_of_other_agencies"],
        "s2_sales" => &["sales"],
        "s2_cost" => &["cost_of_sales"],
        "s2_other" => &["other_income"],
        "s2_deductions" => &["deductions"],
        "s2_previous" => &["previous_quarters_taxable_income"],
        "s4_prior" => &["prior_year_excess_credits"],
        "s4_payments" => &["previous_quarters_payments"],
        "s4_mcit_payments" => &["previous_quarters_mcit_payments"],
        "s4_cwt_previous" => &["previous_quarters_cwt"],
        "s4_cwt" => &["cwt_this_quarter"],
        "s4_previously_filed" => &["previously_filed"],
        "p2_prior_mcit" => &["prior_year_mcit_excess"],
        "p2_surcharge" => &["surcharge"],
        "p2_interest" => &["interest"],
        "p2_compromise" => &["compromise"],
        "other_desc0" => &["other_credits[0].description", "other_credits"],
        "other_desc1" => &["other_credits[1].description", "other_credits"],
        "other_amount0" => &["other_credits[0].amount"],
        "other_amount1" => &["other_credits[1].amount"],
        _ => &[],
    }
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
        label: "6 — Deductions: itemized",
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
        label: "6 — Deductions: OSD (40% of Item 5)",
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

/// Leading item number of a row label ("11A — ..." is 11).
fn label_item_no(label: &str) -> u32 {
    label
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX)
}

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

/// Display-only whole pesos with thousands separators. The printed form says
/// "DO NOT enter Centavos; 49 centavos or less drop down, 50 or more round up".
/// Stored values and the field map keep their full precision.
fn whole_pesos(value: f64) -> String {
    let rounded = value.round();
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    let digits = format!("{:.0}", rounded.abs());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if rounded < 0.0 {
        out.insert(0, '-');
    }
    out
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

/// Read every amount editor into the draft. An entry that is not an amount
/// keeps the draft's previous value and is returned as a parse error, so a
/// save stores everything that parses and never turns bad text into zero.
fn apply_amount_texts(
    draft: &mut Draft,
    text_of: impl Fn(&str) -> String,
) -> Vec<(String, String)> {
    let mut parse_errors = Vec::new();
    for money in all_money() {
        let text = text_of(money.key);
        match parse_amount(&text) {
            Some(value) => (money.set)(draft, value),
            None => parse_errors.push((
                money.key.to_string(),
                format!("{}: \"{text}\" is not a number.", money.label),
            )),
        }
    }
    let mut rows = Vec::new();
    for row in 0..FORM_1702Q_OTHER_CREDIT_ROWS {
        let description = text_of(&other_desc_key(row));
        let text = text_of(&other_amount_key(row));
        let previous = draft.other_credits.get(row).map_or(0.0, |c| c.amount);
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
                previous
            }
        };
        rows.push(Form1702qOtherCredit {
            description,
            amount,
        });
    }
    draft.other_credits = rows;
    parse_errors
}

/// The save confirmation: a warning while entries could not be read.
fn save_notice(unreadable: bool) -> (notification::NotificationType, &'static str) {
    if unreadable {
        (
            notification::NotificationType::Warning,
            "1702Q draft saved. Entries that are not amounts were not saved; fix them under Needs review.",
        )
    } else {
        (
            notification::NotificationType::Success,
            "1702Q draft saved.",
        )
    }
}

pub struct Form1702qView {
    draft: Form1702qDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    month_select: Entity<ComboboxState>,
    atc_select: Entity<ComboboxState>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    header: FormHeaderState,
    _subscriptions: Vec<Subscription>,
}

impl Form1702qView {
    fn money_text(value: f64) -> String {
        if value == 0.0 {
            String::new()
        } else {
            whole_pesos(value)
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
        // The year is locked to the library tile; keep the draft's value.
        // Line of business is not rendered; it keeps the profile value.
        for text in TEXTS.iter().filter(|t| t.key != "year" && t.key != "lob") {
            (text.set)(&mut draft, self.input_text(text.key, cx));
        }
        let texts: BTreeMap<String, String> = all_money()
            .map(|m| m.key.to_string())
            .chain(
                (0..FORM_1702Q_OTHER_CREDIT_ROWS)
                    .flat_map(|r| [other_desc_key(r), other_amount_key(r)]),
            )
            .map(|key| {
                let text = self.input_text(&key, cx);
                (key, text)
            })
            .collect();
        parse_errors.extend(apply_amount_texts(&mut draft, |key| {
            texts.get(key).cloned().unwrap_or_default()
        }));
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
        self.validated = false;
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

    /// Messages for any of `keys`, from malformed input and from validation.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter()),
            keys,
        )
    }

    /// Put the messages for `keys` directly under an already-built element.
    fn with_errors(&self, content: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(content, self.errors_for(keys))
    }

    /// A labelled input. On phones the label sits above the field; its
    /// message (if any) sits directly under it.
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let mut keys = vec![key];
        keys.extend_from_slice(model_keys(key));
        let message = self.errors_for(&keys);
        let error = message.is_some();
        let label = div()
            .min_w_0()
            .max_w_full()
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
                .child(label.w(relative(0.55)))
                .child(body.w(relative(0.45))),
        }
        .into_any_element();
        with_field_error(row, message)
    }

    fn computed(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{whole_pesos(value)}</div>
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

    /// Label then bold value; stacks on phone.
    fn read_only_row(layout: Layout, label: &str, value: &str) -> AnyElement {
        let label = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let value = div()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .child(value.to_string());
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(value)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(label)
                .child(value)
                .into_any_element()
        }
    }

    fn choice_row(&self, label: &str, buttons: Vec<Button>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .min_w_0()
                    .max_w_full()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
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
        let children = self
            .money_rows(moneys, computed, layout, cx)
            .into_iter()
            .map(|(_, el)| el)
            .collect();
        self.section(title, vec![self.grid(layout, children)], cx)
    }

    /// Inputs and computed rows keyed by the leading item number in their
    /// labels, sorted stably (input before computed on equal numbers).
    fn money_rows(
        &self,
        moneys: &[Money],
        computed: &[Computed],
        layout: Layout,
        cx: &Context<Self>,
    ) -> Vec<(u32, AnyElement)> {
        let editable = self.draft.lifecycle.is_editable();
        let mut rows: Vec<(u32, AnyElement)> = moneys
            .iter()
            .map(|m| {
                (
                    label_item_no(m.label),
                    self.field(m.key, m.label, layout, !editable || !(m.open)(&self.draft)),
                )
            })
            .collect();
        rows.extend(computed.iter().map(|c| {
            (
                label_item_no(c.label),
                self.computed(c.label, (c.get)(&self.draft), cx),
            )
        }));
        rows.sort_by_key(|(n, _)| *n);
        rows
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
            let month = d.year_end_month.to_string();
            children.push(select_field(
                "Item 2 — Month the fiscal year ends",
                &self.month_select,
                &MONTH_OPTIONS,
                (1..=11)
                    .contains(&d.year_end_month)
                    .then_some(month.as_str()),
                !editable,
                self.errors_for(&["year_end_month"]).as_deref(),
                cx,
            ));
        }
        children.push(self.with_errors(
            Self::read_only_row(layout, TEXTS[0].label, &d.taxable_year.to_string()),
            &["taxable_year"],
        ));
        children.push(self.with_errors(
            Self::read_only_row(layout, "Item 3 — Quarter", &format!("Q{}", d.quarter)),
            &["quarter"],
        ));
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
        let mut sections =
            vec![self.section("Return period", vec![self.grid(layout, children)], cx)];

        // Item 5.
        let atc_choices = atc_options();
        let atc_field = select_field(
            "Regular/special rate ATC",
            &self.atc_select,
            &atc_choices,
            (!d.atc.is_empty()).then_some(d.atc.as_str()),
            !editable,
            self.errors_for(&["atc"]).as_deref(),
            cx,
        );
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
            vec![self.choice_row("MCIT", mcit), atc_field],
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
        let identity = div()
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
            .into_any_element();
        let mut fields = vec![self.with_errors(identity, &["tin", "rdo_code"])];
        for text in &TEXTS[1..6] {
            fields.push(self.field(text.key, text.label, layout, !editable));
        }
        let deduction = self.choice_row(
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
        );
        let deduction = if d.osd_allowed() {
            deduction
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(deduction)
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("OSD needs an ATC at 30%, 25% or 20%, or IC 021."),
                )
                .into_any_element()
        };
        fields.push(self.with_errors(deduction, &["deduction"]));
        fields.push(self.choice_row(
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
        fields.push(self.field("relief", TEXTS[7].label, layout, !editable || !d.tax_relief));
        let mut children = vec![self.grid(layout, fields)];
        children.extend(self.render_profile_errors(cx));
        self.section("Part I — Background Information", children, cx)
    }

    /// Message for the hidden profile value (Line of business): it has no
    /// field here, so it is listed once with a pointer to Profile Settings.
    /// `None` when there is none.
    fn render_profile_errors(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let message = self.errors_for(&["line_of_business"])?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(field_error(format!("Line of business: {message}")))
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
        let mut s2 = self.money_rows(SCHEDULE_2, SCHEDULE_2_COMPUTED, layout, cx);
        let rate_text = match d.forced_regular_rate() {
            Some(rate) => self.computed_text("10 — Applicable income tax rate (%)", rate, cx),
            None => self.field("s2_rate", TEXTS[8].label, layout, !editable),
        };
        s2.push((10, rate_text));
        s2.sort_by_key(|(n, _)| *n);
        sections.push(self.section(
            "Schedule 2 — Regular/normal rate",
            vec![self.grid(layout, s2.into_iter().map(|(_, el)| el).collect())],
            cx,
        ));
        let mut s3 = self.money_rows(SCHEDULE_3, SCHEDULE_3_COMPUTED, layout, cx);
        let rate_open = d.mcit_open() && form_1702q_mcit_rate(d.taxable_year, d.year_end_month).1;
        s3.push((
            5,
            if rate_open {
                self.field("s3_rate", TEXTS[9].label, layout, !editable)
            } else {
                self.computed_text("5 — MCIT rate (%)", &d.mcit_rate_display(), cx)
            },
        ));
        s3.sort_by_key(|(n, _)| *n);
        sections.push(self.section(
            "Schedule 3 — Minimum corporate income tax (needs both Item 5 boxes)",
            vec![self.grid(layout, s3.into_iter().map(|(_, el)| el).collect())],
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
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{value.to_string()}</div>
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
                        this.validated = false;
                        this.sync_from_inputs(cx);
                    }
                },
            ));
            inputs.insert(key, input);
        }
        let month_select = form_controls::new_select_state(&MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &month_select,
            &MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                if let Ok(month) = value.parse::<u8>() {
                    this.edit(window, cx, |d| d.year_end_month = month);
                }
            },
        ));
        let atc_choices = atc_options();
        let atc_select = form_controls::new_select_state(&atc_choices, window, cx);
        subscriptions.push(form_controls::on_select(
            &atc_select,
            &atc_choices,
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                this.edit(window, cx, |d| d.atc = value);
            },
        ));
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            month_select,
            atc_select,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
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
        // Save what parses; unreadable amounts keep their text in the box
        // and stay listed under Needs review, which keeps Queue blocked.
        let unreadable = !self.parse_errors.is_empty();
        let result = match self.db.lock() {
            Ok(db) => db
                .save_queueable_draft(&self.draft)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(id) => {
                self.draft.id = Some(id);
                let (kind, message) = save_notice(unreadable);
                // The save never refuses; the warning stays in the status box.
                self.status_message = unreadable.then(|| message.to_string());
                self.notify(kind, message.into(), window, cx);
                cx.emit(QueueableFormEvent::Saved);
                cx.notify();
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
        // D5C: the toast and the stepper are the only confirmation.
        self.status_message = None;
        self.validated = false;
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
                    "Form 1702Q reverted to Draft.".into(),
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
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let month_text = self.draft.year_end_month.to_string();
        form_controls::sync_select(
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=11)
                .contains(&self.draft.year_end_month)
                .then_some(month_text.as_str()),
            window,
            cx,
        );
        let atc_choices = atc_options();
        form_controls::sync_select(
            &self.atc_select,
            &atc_choices,
            (!self.draft.atc.is_empty()).then_some(self.draft.atc.as_str()),
            window,
            cx,
        );
        let issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
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
                Button::new("1702q_back")
                    .label("← Back")
                    .when(phone, |b| b.small())
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                    ),
            )
            .child(header_buttons);

        let mut body = div()
            .w_full()
            .max_w(px(1180.))
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
        let attachments = self.field(
            "sheets",
            TEXTS[10].label,
            layout,
            !self.draft.lifecycle.is_editable(),
        );
        body = body.child(self.section(
            "Attachments",
            vec![self.grid(layout, vec![attachments])],
            cx,
        ));
        body = body.children(
            crate::views::event_form_kit::declaration_and_payment_with_text(
                layout == Layout::Phone,
                "We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by us, and to the best of our knowledge and belief, are true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof.",
                "Part III – Details of Payment",
                &[
                    "27 Cash/Bank Debit Memo",
                    "28 Check",
                    "29 Tax Debit Memo",
                    "30 Others (specify below)",
                ],
                crate::views::event_form_kit::PAYMENT_COLUMNS_2200,
                cx,
            ),
        );
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
                    })
                    .children(status_box.map(|b| div().mt_3().child(b))),
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

impl Form1702qView {
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

    fn blocking_errors(&self) -> usize {
        self.parse_errors.len() + self.validation_errors.len()
    }

    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1702q",
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702qDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1702Q marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1702qView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1702qView::blocking_errors,
    on_validate: Form1702qView::header_validate,
    on_submit: Form1702qView::mark_submitted,
    on_save: Form1702qView::save_draft,
    on_print: Form1702qView::preview_pdf,
    on_revert: Form1702qView::revert_to_draft,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1702qView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{
        Draft, Form1702qView, Layout, MONTH_OPTIONS, apply_amount_texts, atc_options, format_tin,
        model_keys, other_amount_key, parse_amount, save_notice, whole_pesos,
    };
    use bir_core::forms::form_1702q::FORM_1702Q_ATC_OPTIONS;
    use gpui::px;

    #[test]
    fn month_select_lists_january_to_november() {
        assert_eq!(MONTH_OPTIONS.len(), 11);
        assert_eq!(MONTH_OPTIONS[0], ("1", "January"));
        assert_eq!(MONTH_OPTIONS[10], ("11", "November"));
    }

    #[test]
    fn atc_select_lists_every_official_code_once() {
        let options = atc_options();
        assert_eq!(options.len(), FORM_1702Q_ATC_OPTIONS.len());
        let unique: std::collections::BTreeSet<_> = options.iter().map(|(v, _)| *v).collect();
        assert_eq!(unique.len(), options.len());
    }

    #[test]
    fn every_editor_key_has_model_keys_or_none_to_show() {
        // Editors whose message sits under their own key need no mapping.
        for (key, _) in Form1702qView::all_keys() {
            let mapped = model_keys(&key);
            assert!(mapped.iter().all(|m| !m.is_empty()), "{key}");
        }
        assert_eq!(model_keys("s2_sales"), ["sales"]);
        assert_eq!(model_keys("other_amount1"), ["other_credits[1].amount"]);
    }

    #[test]
    fn whole_pesos_round_half_up_with_separators() {
        assert_eq!(whole_pesos(1234567.49), "1,234,567");
        assert_eq!(whole_pesos(1234567.50), "1,234,568");
        assert_eq!(whole_pesos(0.0), "0");
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
    fn input_keys_are_unique() {
        let keys = Form1702qView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len());
    }

    #[test]
    fn save_keeps_what_parses_and_warns_about_the_rest() {
        let profile: bir_core::profile::TaxpayerProfile =
            serde_json::from_value(serde_json::json!({
                "id": null, "full_name": "Sample", "tin": {"segment1": "123",
                "segment2": "456", "segment3": "788", "branch": "00000"}, "rdo_code": "039",
                "line_of_business": "", "registered_address": "", "zip_code": "",
                "phone": "", "email": "", "default_form_type": "1702Q",
                "taxpayer_type": "Corporation"
            }))
            .unwrap();
        let mut draft = Draft::new_from_profile(&profile, 2025, 1);
        draft.exempt.sales = 7.0;
        draft.other_credits.push(super::Form1702qOtherCredit {
            description: "Prior".into(),
            amount: 5.0,
        });
        let bad_credit = other_amount_key(0);
        let errors = apply_amount_texts(&mut draft, |key| match key {
            "s2_sales" => "1,234.50".into(),
            "s1a_sales" => "12a".into(),
            k if k == bad_credit => "x".into(),
            _ => String::new(),
        });
        // Parsed entries are saved; unreadable ones keep the previous value.
        assert_eq!(draft.sales, 1234.5);
        assert_eq!(draft.exempt.sales, 7.0);
        assert_eq!(draft.other_credits[0].amount, 5.0);
        let keys: Vec<_> = errors.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["s1a_sales", bad_credit.as_str()]);
        // Whole-peso display rule (3A): 49 down, 50 up.
        assert_eq!(whole_pesos(1234.49), "1,234");
        assert_eq!(whole_pesos(1234.5), "1,235");
        assert_eq!(
            save_notice(true).1,
            "1702Q draft saved. Entries that are not amounts were not saved; fix them under Needs review."
        );
        assert_eq!(save_notice(false).1, "1702Q draft saved.");
    }
}
