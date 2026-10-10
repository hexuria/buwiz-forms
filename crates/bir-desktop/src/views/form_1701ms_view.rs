//! Editor for BIR Form 1701MS, Annual Income Tax Return for Individuals
//! (Micro and Small, 2024). Rust owns every calculation, validation and the
//! official submit plaintext (`bir_core::forms::form_1701ms`); this view only
//! edits source values. Taxpayer and spouse columns sit side by side on
//! desktop and stack on tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1701ms::{
    Form1701MsCivilStatus, Form1701MsColumn, Form1701MsDeduction, Form1701MsDraft,
    Form1701MsFiling, Form1701MsSource, Form1701MsTaxOption,
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

impl EventEmitter<QueueableFormEvent> for Form1701MsView {}

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

/// Which column of the return an input belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Who {
    Taxpayer,
    Spouse,
}

impl Who {
    fn prefix(self) -> &'static str {
        match self {
            Self::Taxpayer => "tp",
            Self::Spouse => "sp",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Taxpayer => "Taxpayer/Filer",
            Self::Spouse => "Spouse",
        }
    }

    fn column(self, draft: &Form1701MsDraft) -> &Form1701MsColumn {
        match self {
            Self::Taxpayer => &draft.taxpayer,
            Self::Spouse => &draft.spouse,
        }
    }

    fn column_mut(self, draft: &mut Form1701MsDraft) -> &mut Form1701MsColumn {
        match self {
            Self::Taxpayer => &mut draft.taxpayer,
            Self::Spouse => &mut draft.spouse,
        }
    }
}

type Get = fn(&Form1701MsColumn) -> f64;
type Set = fn(&mut Form1701MsColumn, f64);
type Open = fn(&Form1701MsColumn) -> bool;

/// An editable whole-peso amount of one column.
struct Money {
    id: &'static str,
    label: &'static str,
    get: Get,
    set: Set,
    open: Open,
}

/// A computed amount of one column.
struct Computed {
    label: &'static str,
    get: Get,
}

const SCHEDULE_A: &[Money] = &[
    Money {
        id: "gross_compensation",
        label: "1 — Gross compensation income",
        get: |c| c.gross_compensation,
        set: |c, v| c.gross_compensation = v,
        open: Form1701MsColumn::schedule_a_open,
    },
    Money {
        id: "non_taxable_compensation",
        label: "2 — Less: non-taxable/exempt compensation",
        get: |c| c.non_taxable_compensation,
        set: |c, v| c.non_taxable_compensation = v,
        open: Form1701MsColumn::schedule_a_open,
    },
];

const SCHEDULE_A_COMPUTED: &[Computed] = &[
    Computed {
        label: "3 — Taxable compensation income",
        get: |c| c.taxable_compensation,
    },
    Computed {
        label: "4 — Tax due on compensation (graduated rates)",
        get: |c| c.tax_on_compensation,
    },
];

const SCHEDULE_B1: &[Money] = &[
    Money {
        id: "sales",
        label: "5 — Sales/revenues/receipts/fees",
        get: |c| c.sales,
        set: |c, v| c.sales = v,
        open: Form1701MsColumn::schedule_b1_open,
    },
    Money {
        id: "sales_returns",
        label: "6 — Less: sales returns, allowances and discounts",
        get: |c| c.sales_returns,
        set: |c, v| c.sales_returns = v,
        open: Form1701MsColumn::schedule_b1_open,
    },
    Money {
        id: "cost_of_sales",
        label: "8 — Less: cost of sales/services (itemized only)",
        get: |c| c.cost_of_sales,
        set: |c, v| c.cost_of_sales = v,
        open: Form1701MsColumn::itemized_open,
    },
    Money {
        id: "itemized_deductions",
        label: "10A — Allowable itemized deductions (schedule total)",
        get: |c| c.itemized_deductions,
        set: |c, v| c.itemized_deductions = v,
        open: Form1701MsColumn::itemized_open,
    },
    Money {
        id: "special_allowable_deductions",
        label: "10B — Special allowable itemized deductions (schedule total)",
        get: |c| c.special_allowable_deductions,
        set: |c, v| c.special_allowable_deductions = v,
        open: Form1701MsColumn::itemized_open,
    },
    Money {
        id: "nolco",
        label: "10C — NOLCO",
        get: |c| c.nolco,
        set: |c, v| c.nolco = v,
        open: Form1701MsColumn::itemized_open,
    },
    Money {
        id: "non_operating_income",
        label: "13 — Other non-operating income (schedule total)",
        get: |c| c.non_operating_income,
        set: |c, v| c.non_operating_income = v,
        open: Form1701MsColumn::schedule_b1_open,
    },
];

const SCHEDULE_B1_COMPUTED: &[Computed] = &[
    Computed {
        label: "7 — Net sales/revenues/receipts/fees",
        get: |c| c.net_sales,
    },
    Computed {
        label: "9 — Gross income",
        get: |c| c.gross_income,
    },
    Computed {
        label: "10D — Total allowable itemized deductions",
        get: |c| c.total_deductions,
    },
    Computed {
        label: "11 — Optional standard deduction (40% of Item 7)",
        get: |c| c.osd,
    },
    Computed {
        label: "12 — Net income (loss)",
        get: |c| c.net_income,
    },
    Computed {
        label: "14 — Taxable income from business",
        get: |c| c.taxable_business_income,
    },
    Computed {
        label: "15 — Taxable compensation and business income",
        get: |c| c.taxable_income,
    },
    Computed {
        label: "17 — Tax due (special/preferential rate or exempt)",
        get: |c| c.special_tax_due,
    },
    Computed {
        label: "18A — Total tax due (special rate/exempt)",
        get: |c| c.tax_due_18a,
    },
    Computed {
        label: "18B — Total tax due (graduated rates)",
        get: |c| c.tax_due_18b,
    },
];

const SCHEDULE_B2: &[Money] = &[
    Money {
        id: "eight_sales",
        label: "19 — Sales/revenues/receipts/fees",
        get: |c| c.eight_sales,
        set: |c, v| c.eight_sales = v,
        open: Form1701MsColumn::schedule_b2_open,
    },
    Money {
        id: "eight_other_income",
        label: "20 — Other non-operating income (schedule total)",
        get: |c| c.eight_other_income,
        set: |c, v| c.eight_other_income = v,
        open: Form1701MsColumn::schedule_b2_open,
    },
];

const SCHEDULE_B2_COMPUTED: &[Computed] = &[
    Computed {
        label: "21 — Total income",
        get: |c| c.eight_total_income,
    },
    Computed {
        label: "22 — Less: allowable reduction (₱250,000)",
        get: |c| c.eight_exemption,
    },
    Computed {
        label: "23 — Taxable income",
        get: |c| c.eight_taxable_income,
    },
    Computed {
        label: "24 — Tax due (8%)",
        get: |c| c.eight_tax_due,
    },
    Computed {
        label: "25 — Total tax due (compensation and business)",
        get: |c| c.eight_total_tax_due,
    },
];

fn always(_: &Form1701MsColumn) -> bool {
    true
}

const PART_V: &[Money] = &[
    Money {
        id: "prior_year_excess",
        label: "1 — Prior year's excess credits",
        get: |c| c.prior_year_excess,
        set: |c, v| c.prior_year_excess = v,
        open: always,
    },
    Money {
        id: "quarterly_payments",
        label: "2 — Tax payments for the 1st to 3rd quarters",
        get: |c| c.quarterly_payments,
        set: |c, v| c.quarterly_payments = v,
        open: always,
    },
    Money {
        id: "cwt_q1_q3",
        label: "3 — Creditable tax withheld, 1st to 3rd quarters (2307)",
        get: |c| c.cwt_q1_q3,
        set: |c, v| c.cwt_q1_q3 = v,
        open: always,
    },
    Money {
        id: "cwt_q4",
        label: "4 — Creditable tax withheld, 4th quarter (2307)",
        get: |c| c.cwt_q4,
        set: |c, v| c.cwt_q4 = v,
        open: always,
    },
    Money {
        id: "cwt_2316",
        label: "5 — Tax withheld per BIR Form 2316",
        get: |c| c.cwt_2316,
        set: |c, v| c.cwt_2316 = v,
        open: Form1701MsColumn::form_2316_open,
    },
    Money {
        id: "previously_filed",
        label: "6 — Tax paid in return previously filed",
        get: |c| c.previously_filed,
        set: |c, v| c.previously_filed = v,
        open: always,
    },
    Money {
        id: "foreign_tax_credits",
        label: "7 — Foreign tax credits",
        get: |c| c.foreign_tax_credits,
        set: |c, v| c.foreign_tax_credits = v,
        open: always,
    },
    Money {
        id: "special_tax_credits",
        label: "8 — Tax credits under special law",
        get: |c| c.special_tax_credits,
        set: |c, v| c.special_tax_credits = v,
        open: always,
    },
    Money {
        id: "other_credits",
        label: "9 — Other tax credits/payments",
        get: |c| c.other_credits,
        set: |c, v| c.other_credits = v,
        open: always,
    },
];

const PART_II: &[Money] = &[
    Money {
        id: "share_of_other_agencies",
        label: "20 — Less: share of other government agencies",
        get: |c| c.share_of_other_agencies,
        set: |c, v| c.share_of_other_agencies = v,
        open: Form1701MsColumn::share_open,
    },
    Money {
        id: "second_installment",
        label: "26 — Portion of tax due allowed for 2nd installment",
        get: |c| c.second_installment,
        set: |c, v| c.second_installment = v,
        open: Form1701MsColumn::installment_open,
    },
    Money {
        id: "surcharge",
        label: "28A — Surcharge",
        get: |c| c.surcharge,
        set: |c, v| c.surcharge = v,
        open: always,
    },
    Money {
        id: "interest",
        label: "28B — Interest",
        get: |c| c.interest,
        set: |c, v| c.interest = v,
        open: always,
    },
    Money {
        id: "compromise",
        label: "28C — Compromise",
        get: |c| c.compromise,
        set: |c, v| c.compromise = v,
        open: always,
    },
];

const PART_II_COMPUTED: &[Computed] = &[
    Computed {
        label: "19 — Income tax due (special rate/exempt)",
        get: |c| c.income_tax_due,
    },
    Computed {
        label: "21 — Net special income tax due",
        get: |c| c.net_special_tax,
    },
    Computed {
        label: "22 — Income tax due (regular/8%)",
        get: |c| c.regular_income_tax,
    },
    Computed {
        label: "23 — Total income tax due",
        get: |c| c.total_income_tax_due,
    },
    Computed {
        label: "24 — Less: total tax credits/payments",
        get: |c| c.tax_credits,
    },
    Computed {
        label: "25 — Tax payable (overpayment)",
        get: |c| c.tax_payable,
    },
    Computed {
        label: "27 — Amount payable (overpayment)",
        get: |c| c.amount_payable,
    },
    Computed {
        label: "28D — Total penalties",
        get: |c| c.total_penalties,
    },
    Computed {
        label: "29 — Total amount payable (overpayment)",
        get: |c| c.total_amount_payable,
    },
    Computed {
        label: "Part VI — Total tax relief availment",
        get: |c| c.tax_relief,
    },
];

/// Text inputs of one column: (id, label, placeholder).
const COLUMN_TEXT: &[(&str, &str, &str)] = &[
    (
        "legal_basis",
        "13 — Legal basis of tax relief/exemption",
        "",
    ),
    (
        "agency",
        "14 — Investment Promotion Agency/Government Agency",
        "",
    ),
    (
        "activity",
        "15 — Registered activity/program (Reg. No.)",
        "",
    ),
    ("eff_from", "16 — Effectivity from", "MM/DD/YYYY"),
    ("eff_to", "16 — Effectivity to", "MM/DD/YYYY"),
    (
        "rate",
        "Part IV Item 16 — Special/preferential rate (%)",
        "0",
    ),
];

/// Text inputs outside the columns: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 1 — Year", "YYYY"),
    ("name", "Item 8 — Taxpayer's name", ""),
    ("email", "Item 9 — Email address", ""),
    ("phone", "Item 10 — Contact number", ""),
    (
        "sp_tin",
        "Item 6 — Spouse TIN (with branch code)",
        "000-000-000-00000",
    ),
    ("sp_rdo", "Item 7 — Spouse RDO code", "000"),
    ("sp_name", "Item 8 — Spouse's name", ""),
    ("sp_email", "Item 9 — Spouse email address", ""),
    ("sp_phone", "Item 10 — Spouse contact number", "digits only"),
    (
        "ftc_desc",
        "Part V Item 7 — Foreign tax credits (specify)",
        "",
    ),
    (
        "other_desc",
        "Part V Item 9 — Other credits/payments (specify)",
        "",
    ),
    ("refund", "Item 31 — To be refunded", "0.00"),
    (
        "tcc",
        "Item 31 — To be issued a Tax Credit Certificate",
        "0.00",
    ),
    ("carry", "Item 31 — To be carried over", "0.00"),
];

fn money_key(who: Who, id: &str) -> String {
    format!("{}:{id}", who.prefix())
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
    SCHEDULE_A
        .iter()
        .chain(SCHEDULE_B1)
        .chain(SCHEDULE_B2)
        .chain(PART_V)
        .chain(PART_II)
}

pub struct Form1701MsView {
    draft: Form1701MsDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1701MsView {
    fn money_text(value: f64) -> String {
        if value == 0.0 {
            String::new()
        } else {
            official_amount(value)
        }
    }

    /// Editor text for one input from the draft.
    fn initial(draft: &Form1701MsDraft, key: &str) -> String {
        for who in [Who::Taxpayer, Who::Spouse] {
            let Some(id) = key.strip_prefix(&format!("{}:", who.prefix())) else {
                continue;
            };
            let column = who.column(draft);
            if let Some(money) = all_money().find(|m| m.id == id) {
                return Self::money_text((money.get)(column));
            }
            return match id {
                "legal_basis" => column.legal_basis.clone(),
                "agency" => column.promotion_agency.clone(),
                "activity" => column.registered_activity.clone(),
                "eff_from" => column.effectivity_from.clone(),
                "eff_to" => column.effectivity_to.clone(),
                "rate" if column.special_rate != 0.0 => format!("{}", column.special_rate),
                _ => String::new(),
            };
        }
        match key {
            "year" => draft.taxable_year.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "email" => draft.email.clone(),
            "phone" => draft.contact_number.clone(),
            "sp_tin" => draft.spouse_info.tin.clone(),
            "sp_rdo" => draft.spouse_info.rdo_code.clone(),
            "sp_name" => draft.spouse_info.name.clone(),
            "sp_email" => draft.spouse_info.email.clone(),
            "sp_phone" => draft.spouse_info.contact_number.clone(),
            "ftc_desc" => draft.foreign_tax_credits_description.clone(),
            "other_desc" => draft.other_credits_description.clone(),
            "refund" => Self::money_text(draft.refund_amount),
            "tcc" => Self::money_text(draft.tcc_amount),
            "carry" => Self::money_text(draft.carry_over_amount),
            _ => String::new(),
        }
    }

    fn all_keys() -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for who in [Who::Taxpayer, Who::Spouse] {
            for money in all_money() {
                keys.push((money_key(who, money.id), "0".to_string()));
            }
            for (id, _, placeholder) in COLUMN_TEXT {
                keys.push((money_key(who, id), placeholder.to_string()));
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
        for who in [Who::Taxpayer, Who::Spouse] {
            for money in all_money() {
                let key = money_key(who, money.id);
                if let Some(value) = number(&key, money.label, cx) {
                    (money.set)(who.column_mut(&mut draft), value);
                }
            }
            let rate_key = money_key(who, "rate");
            if let Some(value) = number(&rate_key, "Part IV Item 16 rate", cx) {
                who.column_mut(&mut draft).special_rate = value;
            }
            let text = |id: &str| self.input_text(&money_key(who, id), cx);
            let (basis, agency, activity, from, to) = (
                text("legal_basis"),
                text("agency"),
                text("activity"),
                text("eff_from"),
                text("eff_to"),
            );
            let column = who.column_mut(&mut draft);
            column.legal_basis = basis;
            column.promotion_agency = agency;
            column.registered_activity = activity;
            column.effectivity_from = from.trim().to_string();
            column.effectivity_to = to.trim().to_string();
        }
        if let Some(value) = number("refund", "Item 31 to be refunded", cx) {
            draft.refund_amount = value;
        }
        if let Some(value) = number("tcc", "Item 31 tax credit certificate", cx) {
            draft.tcc_amount = value;
        }
        if let Some(value) = number("carry", "Item 31 to be carried over", cx) {
            draft.carry_over_amount = value;
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.spouse_info.tin = self.input_text("sp_tin", cx).trim().to_string();
        draft.spouse_info.rdo_code = self.input_text("sp_rdo", cx).trim().to_string();
        draft.spouse_info.name = self.input_text("sp_name", cx);
        draft.spouse_info.email = self.input_text("sp_email", cx).trim().to_string();
        draft.spouse_info.contact_number = self.input_text("sp_phone", cx).trim().to_string();
        draft.foreign_tax_credits_description = self.input_text("ftc_desc", cx);
        draft.other_credits_description = self.input_text("other_desc", cx);
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// Apply a choice, then reload every editor: radio rules clear amounts.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1701MsDraft),
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
                .release_abandoned_claimed_queueable::<Form1701MsDraft>(
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
            .any(|(field, _)| field == key)
    }

    /// A labelled input. On phones the label sits above the field.
    fn field(
        &self,
        key: &str,
        error_key: &str,
        label: &str,
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

    /// Taxpayer and spouse blocks: side by side on desktop, stacked otherwise.
    fn columns(
        &self,
        layout: Layout,
        blocks: Vec<(Who, Vec<AnyElement>)>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let render = |who: Who, children: Vec<AnyElement>| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .flex_1()
                .p_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(div().font_weight(FontWeight::BOLD).child(who.title()))
                .children(children)
        };
        let mut row = if layout == Layout::Desktop {
            div().flex().gap_4().w_full()
        } else {
            div().flex().flex_col().gap_4().w_full()
        };
        for (who, children) in blocks {
            row = row.child(render(who, children));
        }
        row.into_any_element()
    }

    fn joint_whos(&self) -> Vec<Who> {
        if self.draft.is_joint() {
            vec![Who::Taxpayer, Who::Spouse]
        } else {
            vec![Who::Taxpayer]
        }
    }

    fn money_block(
        &self,
        who: Who,
        moneys: &[Money],
        computed: &[Computed],
        layout: Layout,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let editable = self.draft.lifecycle.is_editable();
        let column = who.column(&self.draft);
        let mut children: Vec<AnyElement> = moneys
            .iter()
            .map(|money| {
                let key = money_key(who, money.id);
                let error_key = format!(
                    "{}.{}",
                    match who {
                        Who::Taxpayer => "taxpayer",
                        Who::Spouse => "spouse",
                    },
                    money.id
                );
                self.field(
                    &key,
                    &error_key,
                    money.label,
                    layout,
                    !editable || !(money.open)(column),
                )
            })
            .collect();
        children.extend(
            computed
                .iter()
                .map(|item| self.computed(item.label, (item.get)(column), cx)),
        );
        children
    }

    fn render_period(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> AnyElement {
        let _ = window;
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut months = Vec::new();
        if d.is_short_period {
            for month in 1..=11u8 {
                months.push(
                    Self::choice(
                        ("1701ms_month", month as usize),
                        format!("{month:02}"),
                        d.month == month,
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.month = month)
                    })),
                );
            }
        }
        let mut children =
            vec![
                self.field(
                    "year",
                    "taxable_year",
                    "Item 1 — For the year (YYYY)",
                    layout,
                    !editable,
                ),
                self.choice_row(
                    "Item 2 — Amended return?",
                    vec![
                        Self::choice("1701ms_amended_yes", "Yes", d.is_amended, !editable)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_amended = true)
                            })),
                        Self::choice("1701ms_amended_no", "No", !d.is_amended, !editable).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_amended = false)
                            }),
                        ),
                    ],
                ),
                self.choice_row(
                    "Item 3 — Short period return?",
                    vec![
                        Self::choice("1701ms_short_yes", "Yes", d.is_short_period, !editable)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| {
                                    d.is_short_period = true;
                                    if d.month == 12 {
                                        d.month = 11;
                                    }
                                })
                            })),
                        Self::choice("1701ms_short_no", "No", !d.is_short_period, !editable)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_short_period = false)
                            })),
                    ],
                ),
            ];
        if !months.is_empty() {
            children.push(self.choice_row("Item 1 — Month the short period ends", months));
        }
        let mut statuses = Vec::new();
        for (index, status) in Form1701MsCivilStatus::ALL.iter().copied().enumerate() {
            statuses.push(
                Self::choice(
                    ("1701ms_civil", index),
                    status.label(),
                    d.civil_status == status,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.civil_status = status)
                })),
            );
        }
        children.push(self.choice_row("Item 4 — Civil status", statuses));
        let married = d.civil_status == Form1701MsCivilStatus::Married;
        let filing = [
            (Form1701MsFiling::Jointly, "Jointly"),
            (Form1701MsFiling::Separately, "Separately"),
            (Form1701MsFiling::NotApplicable, "Not applicable"),
        ];
        let buttons = filing
            .iter()
            .copied()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    ("1701ms_filing", index),
                    label,
                    d.filing == value,
                    !editable || !married,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.filing = value)
                }))
            })
            .collect();
        children.push(self.choice_row("Item 5 — If married, filing", buttons));
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut taxpayer = vec![
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
        for (key, label, _) in TEXT_INPUTS.iter().skip(1).take(3) {
            taxpayer.push(self.field(key, key, label, layout, !editable));
        }
        let mut blocks = vec![(Who::Taxpayer, taxpayer)];
        if d.is_joint() {
            let spouse = TEXT_INPUTS
                .iter()
                .skip(4)
                .take(5)
                .map(|(key, label, _)| {
                    let error_key = match *key {
                        "sp_tin" => "spouse_info.tin",
                        "sp_rdo" => "spouse_info.rdo_code",
                        "sp_name" => "spouse_info.name",
                        "sp_email" => "spouse_info.email",
                        _ => "spouse_info.contact_number",
                    };
                    self.field(key, error_key, label, layout, !editable)
                })
                .collect();
            blocks.push((Who::Spouse, spouse));
        }
        let mut options = Vec::new();
        for who in self.joint_whos() {
            options.push((who, self.column_choices(who, layout, cx)));
        }
        self.section(
            "Part I — Background Information",
            vec![
                self.columns(layout, blocks, cx),
                self.columns(layout, options, cx),
            ],
            cx,
        )
    }

    fn column_choices(&self, who: Who, layout: Layout, cx: &Context<Self>) -> Vec<AnyElement> {
        let editable = self.draft.lifecycle.is_editable();
        let column = who.column(&self.draft);
        let p = who.prefix();
        let mut sources = vec![
            (Form1701MsSource::Business, "Income from business"),
            (Form1701MsSource::Mixed, "Mixed income"),
            (Form1701MsSource::Profession, "Income from profession"),
        ];
        if who == Who::Spouse {
            sources.push((Form1701MsSource::Compensation, "Compensation income"));
        }
        let source_buttons = sources
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    (SharedString::from(format!("1701ms_{p}_src")), index),
                    label,
                    column.source == value,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| {
                        let column = who.column_mut(d);
                        if column.source != value {
                            // sourceOfIncome* resets Items 12 to 17.
                            *column = Form1701MsColumn {
                                source: value,
                                ..Form1701MsColumn::default()
                            };
                        }
                    })
                }))
            })
            .collect();
        let compensation_only = column.source == Form1701MsSource::Compensation;
        let options = [
            (Form1701MsTaxOption::Graduated, "Graduated rates"),
            (Form1701MsTaxOption::EightPercent, "8% rate"),
            (Form1701MsTaxOption::Exempt, "Exempt"),
            (Form1701MsTaxOption::Special, "Special/preferential rate"),
        ];
        let option_buttons = options
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    (SharedString::from(format!("1701ms_{p}_opt")), index),
                    label,
                    column.tax_option == value,
                    !editable || compensation_only,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| who.column_mut(d).tax_option = value)
                }))
            })
            .collect();
        let deduction_open = column.deduction_open();
        let deductions = [
            (Form1701MsDeduction::Itemized, "Itemized deductions"),
            (
                Form1701MsDeduction::Osd,
                "Optional standard deduction (40%)",
            ),
        ];
        let deduction_buttons = deductions
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    (SharedString::from(format!("1701ms_{p}_ded")), index),
                    label,
                    column.deduction == value,
                    !editable || !deduction_open,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| who.column_mut(d).deduction = value)
                }))
            })
            .collect();
        let mut children = vec![
            self.choice_row("Item 11 — Source of income", source_buttons),
            self.choice_row("Item 12 — Income is subject to", option_buttons),
        ];
        if column.relief_items_open() {
            for (id, label, _) in COLUMN_TEXT.iter().take(5) {
                children.push(self.field(
                    &money_key(who, id),
                    &format!(
                        "{}.legal_basis",
                        match who {
                            Who::Taxpayer => "taxpayer",
                            Who::Spouse => "spouse",
                        }
                    ),
                    label,
                    layout,
                    !editable,
                ));
            }
        }
        children.push(self.choice_row("Item 17 — Method of deduction", deduction_buttons));
        children.push(
            div()
                .text_sm()
                .child(format!(
                    "Item 18 — ATC: {}",
                    column.atc().unwrap_or("(select Items 11 and 12)")
                ))
                .into_any_element(),
        );
        children
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut sections = Vec::new();
        let whos = self.joint_whos();
        let block = |moneys: &[Money], computed: &[Computed]| {
            whos.iter()
                .map(|who| (*who, self.money_block(*who, moneys, computed, layout, cx)))
                .collect::<Vec<_>>()
        };
        sections.push(self.section(
            "Part IV Schedule A — Compensation income",
            vec![self.columns(layout, block(SCHEDULE_A, SCHEDULE_A_COMPUTED), cx)],
            cx,
        ));
        let mut b1 = block(SCHEDULE_B1, SCHEDULE_B1_COMPUTED);
        for (who, children) in &mut b1 {
            let column = who.column(&self.draft);
            children.insert(
                7,
                self.field(
                    &money_key(*who, "rate"),
                    &format!(
                        "{}.special_rate",
                        match who {
                            Who::Taxpayer => "taxpayer",
                            Who::Spouse => "spouse",
                        }
                    ),
                    COLUMN_TEXT[5].1,
                    layout,
                    !editable || !column.relief_items_open(),
                ),
            );
        }
        sections.push(self.section(
            "Part IV Schedule B1 — Business income (graduated, exempt or special rate)",
            vec![self.columns(layout, b1, cx)],
            cx,
        ));
        sections.push(self.section(
            "Part IV Schedule B2 — Business income (8% rate)",
            vec![self.columns(layout, block(SCHEDULE_B2, SCHEDULE_B2_COMPUTED), cx)],
            cx,
        ));
        let mut credits = block(PART_V, &[]);
        for (who, children) in &mut credits {
            children.push(self.computed(
                "10 — Total tax credits/payments",
                who.column(&self.draft).total_credits,
                cx,
            ));
        }
        sections.push(self.section(
            "Part V — Tax credits/payments",
            vec![
                self.grid(
                    layout,
                    vec![
                        self.field(
                            "ftc_desc",
                            "foreign_tax_credits_description",
                            TEXT_INPUTS[9].1,
                            layout,
                            !editable,
                        ),
                        self.field(
                            "other_desc",
                            "other_credits_description",
                            TEXT_INPUTS[10].1,
                            layout,
                            !editable,
                        ),
                    ],
                ),
                self.columns(layout, credits, cx),
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

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let blocks = self
            .joint_whos()
            .into_iter()
            .map(|who| {
                (
                    who,
                    self.money_block(who, PART_II, PART_II_COMPUTED, layout, cx),
                )
            })
            .collect();
        let mut children = vec![
            self.columns(layout, blocks, cx),
            self.computed(
                "30 — Aggregate amount payable (overpayment)",
                d.aggregate_amount_payable,
                cx,
            ),
        ];
        if d.overpayment_open() {
            let toggles = vec![
                Self::choice(
                    "1701ms_refund",
                    "To be refunded",
                    d.to_be_refunded,
                    !editable,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| d.to_be_refunded = !d.to_be_refunded)
                })),
                Self::choice(
                    "1701ms_tcc",
                    "To be issued a Tax Credit Certificate",
                    d.to_be_issued_tcc,
                    !editable,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| d.to_be_issued_tcc = !d.to_be_issued_tcc)
                })),
                Self::choice(
                    "1701ms_carry",
                    "To be carried over",
                    d.to_be_carried_over,
                    !editable,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| d.to_be_carried_over = !d.to_be_carried_over)
                })),
            ];
            children.push(self.choice_row("Item 31 — If overpayment, mark at most two", toggles));
            let amounts = vec![
                self.field(
                    "refund",
                    "overpayment",
                    TEXT_INPUTS[11].1,
                    layout,
                    !editable || !d.to_be_refunded,
                ),
                self.field(
                    "tcc",
                    "overpayment",
                    TEXT_INPUTS[12].1,
                    layout,
                    !editable || !d.to_be_issued_tcc,
                ),
                self.field(
                    "carry",
                    "carry_over_amount",
                    TEXT_INPUTS[13].1,
                    layout,
                    !editable || !d.to_be_carried_over,
                ),
            ];
            children.push(self.grid(layout, amounts));
        }
        children.push(self.choice_row(
            "Declaration under the penalties of perjury",
            vec![
                    Self::choice(
                        "1701ms_perjury",
                        "I declare this return true and correct",
                        d.perjury_agreed,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.perjury_agreed = !d.perjury_agreed)
                    })),
                ],
        ));
        self.section("Part II — Total tax payable", children, cx)
    }
}

impl QueueableFormView for Form1701MsView {
    type Draft = Form1701MsDraft;

    fn new(
        draft: Form1701MsDraft,
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1701MsDraft {
        Form1701MsDraft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1701MsView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701MS"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Individuals (Micro and Small Taxpayers)"
    }
    fn form_version(&self) -> &'static str {
        "2024 (ENCS)"
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
                    "1701MS draft saved.".into(),
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
        if !can_queue_for_submission(Form1701MsDraft::FORM_CODE) {
            self.status_message =
                Some("1701MS is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1701MS. No submission was started: {error}"
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
            "Form 1701MS queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1701MS payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1701MsDraft>(
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
            "1701ms-2024",
            &fields,
            "1701MS — Print Preview",
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

impl Render for Form1701MsView {
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
                .child(Button::new("1701ms_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1701ms_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1701ms_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1701ms_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1701ms_submit")
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
            .max_w(px(1280.))
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
                            Button::new("1701ms_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1701ms_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_period(layout, window, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_schedules(layout, cx));
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
                    .id("1701ms_scroll")
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
    use super::{Form1701MsView, Layout, format_tin, parse_amount};
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
        assert_eq!(parse_amount("5%"), Some(5.0));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }

    #[test]
    fn every_input_key_has_an_initial_value_path() {
        let keys = Form1701MsView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len(), "input keys must be unique");
    }
}
