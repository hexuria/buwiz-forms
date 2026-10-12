//! Editor for BIR Form 1702-RT, Annual Income Tax Return for Corporations,
//! Partnerships and Other Non-Individual Taxpayers Subject Only to the
//! Regular Income Tax Rate (January 2018). Rust owns every calculation,
//! validation and the official submit plaintext
//! (`bir_core::forms::form_1702rt_official`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1702rt::{
    FORM_1702RT_MODALS, FORM_VERSION_LABEL, Form1702RTDate, Form1702RTDeductionMethod,
    Form1702RTDraft, Form1702RTFilingBasis, Form1702RTModal, Form1702RTOverpaymentDisposition,
    Form1702RTSpecialDeductionRow, WholePeso,
};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingPeriod, FilingStatus, can_queue_for_submission};
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use super::event_form_kit as kit;
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

impl EventEmitter<QueueableFormEvent> for Form1702RTView {}

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

/// Month the year ends: (stored month number, shown name).
const MONTH_OPTIONS: [(&str, &str); 12] = [
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

/// Value of the "no other ATC" choice in the Item 5 select.
const NO_ATC: &str = "none";

/// The other ATCs of Item 5 (`drpPg1I5AtcOther`), after the "None" choice.
const OTHER_ATCS: &[(&str, &str)] = &[
    (NO_ATC, "None"),
    ("IC010", "IC010 Corporation in general"),
    ("IC020", "IC020 Taxable partnership"),
    ("IC040", "IC040 Government entity"),
    ("IC041", "IC041 National gov't & LGUs"),
    ("IC070", "IC070 Resident foreign corporation"),
    ("IC101_25%", "IC101 Regional operating HQ 25%"),
    ("IC190_25%", "IC190 Offshore banking units 25%"),
    ("IC191_25%", "IC191 FCDUs 25%"),
];

const PROFILE_NOTE: &str = "These come from your taxpayer profile. Edit them in Profile Settings.";

/// Validation keys (the model's) that belong to one text input. Messages
/// that name a whole row or group are shown by that row instead.
fn model_error_keys(key: &str) -> Vec<String> {
    let indexed = |prefix: &str| {
        key.strip_prefix(prefix)
            .and_then(|n| n.parse::<usize>().ok())
    };
    let keys: &[&str] = match key {
        "name0" => &["registered_name_lines"],
        "addr0" => &["registered_address_lines"],
        "incorp" => &["incorporation_date"],
        "phone" => &["contact_number"],
        "email" => &["email"],
        "rate" => &["part_iv.item_40_income_tax_rate_percent"],
        "d53" => &["part_iv.tax_credits.item_53_other"],
        "d54" => &["part_iv.tax_credits.item_54_other"],
        "i57" => &["part_v.item_57_special_allowable_deductions_tax_effect"],
        _ => &[],
    };
    let mut out: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    for (prefix, field) in [
        ("s1od_", "schedule_1.other"),
        ("s5ad_", "schedule_5.additions"),
        ("s5nd_", "schedule_5.non_taxable_income"),
        ("s5sd_", "schedule_5.special_deductions"),
    ] {
        if let Some(i) = indexed(prefix) {
            out.push(format!("{field}[{i}]"));
        }
    }
    out
}

/// The select's stored value for the draft's other ATC.
fn other_atc_value(draft: &Form1702RTDraft) -> &str {
    if draft.atc.other_selected && OTHER_ATCS.iter().any(|(v, _)| *v == draft.atc.other_code) {
        draft.atc.other_code.as_str()
    } else {
        NO_ATC
    }
}

/// Schedule I lines 1–17c, in the order of `Form1702RTSchedule1::source_amounts`.
const SCHEDULE_1_LABELS: [&str; 19] = [
    "1 Amortizations",
    "2 Bad debts",
    "3 Charitable and other contributions",
    "4 Depletion",
    "5 Depreciation",
    "6 Entertainment, amusement and recreation",
    "7 Fringe benefits",
    "8 Interest",
    "9 Losses",
    "10 Pension trusts",
    "11 Rental",
    "12 Research and development",
    "13 Salaries, wages and allowances",
    "14 SSS, GSIS, Medicare, HDMF and other contributions",
    "15 Taxes and licenses",
    "16 Transportation and travel",
    "17a Janitorial and messengerial services",
    "17b Professional fees",
    "17c Security services",
];

/// Single inputs: (key, label, is_amount).
const FIELDS: &[(&str, &str, bool)] = &[
    ("year", "Item 2 — Year ended (YYYY)", false),
    ("name0", "Item 8 — Registered name", false),
    ("name1", "Item 8 — Registered name (line 2)", false),
    ("name2", "Item 8 — Registered name (line 3)", false),
    ("addr0", "Item 9 — Registered address", false),
    ("addr1", "Item 9 — Registered address (line 2)", false),
    ("addr2", "Item 9 — Registered address (line 3)", false),
    ("zip", "Item 9A — ZIP code", false),
    (
        "incorp",
        "Item 10 — Date of incorporation (MM/DD/YYYY)",
        false,
    ),
    ("phone", "Item 11 — Contact number", false),
    ("email", "Item 12 — Email address", false),
    ("i17", "17 — Surcharge", true),
    ("i18", "18 — Interest", true),
    ("i19", "19 — Compromise", true),
    ("pages", "22 — Number of attachments", false),
    ("sig1", "Title of signatory (President)", false),
    ("sigtin1", "TIN of signatory (President)", false),
    ("sig2", "Title of signatory (Treasurer)", false),
    ("sigtin2", "TIN of signatory (Treasurer)", false),
    ("i27", "27 — Sales/receipts/revenues/fees", true),
    ("i28", "28 — Sales returns, allowances and discounts", true),
    ("i30", "30 — Cost of sales/services", true),
    (
        "i32",
        "32 — Other taxable income not subjected to final tax",
        true,
    ),
    ("rate", "40 — Applicable income tax rate (%)", false),
    ("i42", "42 — Minimum corporate income tax (MCIT)", true),
    ("i44", "44 — Prior year's excess credits", true),
    (
        "i45",
        "45 — Income tax payment under MCIT from previous quarters",
        true,
    ),
    (
        "i46",
        "46 — Income tax payment under regular rate from previous quarters",
        true,
    ),
    (
        "i48",
        "48 — Creditable tax withheld from previous quarters",
        true,
    ),
    (
        "i49",
        "49 — Creditable tax withheld for the 4th quarter",
        true,
    ),
    ("i50", "50 — Foreign tax credits", true),
    (
        "i51",
        "51 — Tax paid in return previously filed (amended only)",
        true,
    ),
    ("i52", "52 — Special tax credits", true),
    ("d53", "53 — Other tax credit (specify)", false),
    ("i53", "53 — Amount", true),
    ("d54", "54 — Other tax credit (specify)", false),
    ("i54", "54 — Amount", true),
    (
        "i57",
        "57 — Special allowable itemized deductions (tax effect)",
        true,
    ),
    ("s5_1", "Schedule V 1 — Net income/(loss) per books", true),
];

/// Payment rows (Items 23–26): (row, label, has specification, has bank).
const PAYMENT_ROWS: [(usize, &str, bool, bool); 4] = [
    (0, "23 — Cash/Bank debit memo", false, true),
    (1, "24 — Check", false, true),
    (2, "25 — Tax debit memo", false, false),
    (3, "26 — Others", true, true),
];

/// Whole pesos: digits with optional commas, `-` or `(x)` for a negative.
/// The official page only lets digits be typed (`wholenumber` on
/// keypress); typed centavos are rounded half-up to whole pesos, the same
/// rule as the 1702-MX page.
fn parse_peso(value: &str) -> Option<WholePeso> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(WholePeso(0));
    }
    let (negative, body) = match trimmed.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        Some(inner) => (true, inner),
        None => match trimmed.strip_prefix('-') {
            Some(inner) => (true, inner),
            None => (false, trimmed),
        },
    };
    let digits: String = body.chars().filter(|c| *c != ',').collect();
    let (whole, cents) = match digits.split_once('.') {
        Some((whole, cents)) => (whole, cents),
        None => (digits.as_str(), ""),
    };
    let all_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() && cents.is_empty() || !all_digits(whole) || !all_digits(cents) {
        return None;
    }
    let mut value: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    if cents.bytes().next().is_some_and(|b| b >= b'5') {
        value = value.checked_add(1)?;
    }
    Some(WholePeso(if negative { -value } else { value }))
}

fn peso_text(value: WholePeso) -> String {
    if value.0 == 0 {
        String::new()
    } else {
        value.format_bir()
    }
}

fn date_text(date: Option<Form1702RTDate>) -> String {
    date.map(|d| d.to_string()).unwrap_or_default()
}

/// An "(Add more...)" modal row key: `m<modal>_<column>_<row>` with column
/// `d` (description), `l` (legal basis) or `a` (amount).
fn more_key(modal: usize, column: char, row: usize) -> String {
    format!("m{modal}_{column}_{row}")
}

fn parse_more_key(key: &str) -> Option<(Form1702RTModal, char, usize)> {
    let rest = key.strip_prefix('m')?;
    let mut parts = rest.split('_');
    let modal = FORM_1702RT_MODALS.get(parts.next()?.parse::<usize>().ok()?)?;
    let column = parts.next()?.chars().next()?;
    let row = parts.next()?.parse().ok()?;
    Some((*modal, column, row))
}

fn more_row_mut<'a>(
    d: &'a mut Form1702RTDraft,
    key: &str,
) -> Option<(char, &'a mut Form1702RTSpecialDeductionRow)> {
    let (modal, column, row) = parse_more_key(key)?;
    let row = d.more_rows.rows_mut(modal).get_mut(row)?;
    Some((column, row))
}

/// Editor keys of every modal row the draft holds.
fn more_keys(d: &Form1702RTDraft) -> Vec<String> {
    let mut keys = Vec::new();
    for (index, modal) in FORM_1702RT_MODALS.iter().enumerate() {
        for row in 0..d.more_rows.rows(*modal).len() {
            keys.push(more_key(index, 'd', row));
            if modal.has_legal_basis() {
                keys.push(more_key(index, 'l', row));
            }
            keys.push(more_key(index, 'a', row));
        }
    }
    keys
}

/// The item inputs a folded modal fills (OTHERS and the subtotal).
fn folded_key(d: &Form1702RTDraft, key: &str) -> bool {
    let modal = match key {
        "d54" | "i54" => Form1702RTModal::Item54,
        "s1od_5" | "s1oa_5" => Form1702RTModal::Schedule1Item17i,
        "s2d_3" | "s2l_3" | "s2a_3" => Form1702RTModal::Schedule2Item4,
        "s5ad_1" | "s5a_1" => Form1702RTModal::Schedule5Item3,
        "s5nd_1" | "s5n_1" => Form1702RTModal::Schedule5Item6,
        "s5sd_1" | "s5s_1" => Form1702RTModal::Schedule5Item8,
        _ => return false,
    };
    d.more_rows.is_folded(modal)
}

fn money_slot<'a>(d: &'a mut Form1702RTDraft, key: &str) -> Option<&'a mut WholePeso> {
    if key.starts_with('m') && parse_more_key(key).is_some() {
        return match more_row_mut(d, key)? {
            ('a', row) => Some(&mut row.amount),
            _ => None,
        };
    }
    let indexed = |prefix: &str| {
        key.strip_prefix(prefix)
            .and_then(|n| n.parse::<usize>().ok())
    };
    Some(match key {
        "i17" => &mut d.part_ii.item_17_surcharge,
        "i18" => &mut d.part_ii.item_18_interest,
        "i19" => &mut d.part_ii.item_19_compromise,
        "i27" => &mut d.part_iv.item_27_sales,
        "i28" => &mut d.part_iv.item_28_sales_returns,
        "i30" => &mut d.part_iv.item_30_cost_of_sales_or_services,
        "i32" => &mut d.part_iv.item_32_other_taxable_income,
        "i42" => &mut d.part_iv.item_42_mcit_due,
        "i44" => &mut d.part_iv.tax_credits.item_44_prior_year_excess_credits,
        "i45" => &mut d.part_iv.tax_credits.item_45_previous_quarter_mcit_payments,
        "i46" => {
            &mut d
                .part_iv
                .tax_credits
                .item_46_previous_quarter_regular_payments
        }
        "i48" => &mut d.part_iv.tax_credits.item_48_previous_quarter_withholding,
        "i49" => &mut d.part_iv.tax_credits.item_49_fourth_quarter_withholding,
        "i50" => &mut d.part_iv.tax_credits.item_50_foreign_tax_credits,
        "i51" => &mut d.part_iv.tax_credits.item_51_tax_paid_on_previous_return,
        "i52" => &mut d.part_iv.tax_credits.item_52_special_tax_credits,
        "i53" => &mut d.part_iv.tax_credits.item_53_other.amount,
        "i54" => &mut d.part_iv.tax_credits.item_54_other.amount,
        "i57" => &mut d.part_v.item_57_special_allowable_deductions_tax_effect,
        "s5_1" => &mut d.schedule_5.item_1_net_income_or_loss_per_books,
        _ => {
            if let Some(i) = indexed("s1_") {
                let s1 = &mut d.schedule_1;
                return Some(match i {
                    0 => &mut s1.amortizations,
                    1 => &mut s1.bad_debts,
                    2 => &mut s1.charitable_contributions,
                    3 => &mut s1.depletion,
                    4 => &mut s1.depreciation,
                    5 => &mut s1.entertainment,
                    6 => &mut s1.fringe_benefits,
                    7 => &mut s1.interest,
                    8 => &mut s1.losses,
                    9 => &mut s1.pension_trusts,
                    10 => &mut s1.rental,
                    11 => &mut s1.research_and_development,
                    12 => &mut s1.salaries_wages_allowances,
                    13 => &mut s1.statutory_contributions,
                    14 => &mut s1.taxes_and_licenses,
                    15 => &mut s1.transportation_and_travel,
                    16 => &mut s1.janitorial_and_messengerial,
                    17 => &mut s1.professional_fees,
                    18 => &mut s1.security_services,
                    _ => return None,
                });
            }
            if let Some(i) = indexed("s1oa_") {
                return d.schedule_1.other.get_mut(i).map(|row| &mut row.amount);
            }
            if let Some(i) = indexed("s2a_") {
                return d.schedule_2.rows.get_mut(i).map(|row| &mut row.amount);
            }
            if let Some(i) = indexed("pay_") {
                return d.payment_details.get_mut(i).map(|row| &mut row.amount);
            }
            if let Some(i) = indexed("s5a_") {
                return d.schedule_5.additions.get_mut(i).map(|row| &mut row.amount);
            }
            if let Some(i) = indexed("s5n_") {
                return d
                    .schedule_5
                    .non_taxable_income
                    .get_mut(i)
                    .map(|row| &mut row.amount);
            }
            if let Some(i) = indexed("s5s_") {
                return d
                    .schedule_5
                    .special_deductions
                    .get_mut(i)
                    .map(|row| &mut row.amount);
            }
            for (prefix, column) in [("s3a_", 0), ("s3b_", 1), ("s3c_", 2), ("s3d_", 3)] {
                if let Some(i) = indexed(prefix) {
                    let row = d.schedule_3.rows.get_mut(i)?;
                    return Some(match column {
                        0 => &mut row.amount,
                        1 => &mut row.applied_previous_years,
                        2 => &mut row.expired,
                        _ => &mut row.applied_current_year,
                    });
                }
            }
            for (prefix, column) in [
                ("s4n_", 0),
                ("s4m_", 1),
                ("s4p_", 2),
                ("s4e_", 3),
                ("s4c_", 4),
            ] {
                if let Some(i) = indexed(prefix) {
                    let row = d.schedule_4.rows.get_mut(i)?;
                    return Some(match column {
                        0 => &mut row.normal_income_tax,
                        1 => &mut row.mcit,
                        2 => &mut row.applied_previous_years,
                        3 => &mut row.expired,
                        _ => &mut row.applied_current_year,
                    });
                }
            }
            return None;
        }
    })
}

fn text_slot<'a>(d: &'a mut Form1702RTDraft, key: &str) -> Option<&'a mut String> {
    if key.starts_with('m') && parse_more_key(key).is_some() {
        return match more_row_mut(d, key)? {
            ('d', row) => Some(&mut row.description),
            ('l', row) => Some(&mut row.legal_basis),
            _ => None,
        };
    }
    let indexed = |prefix: &str| {
        key.strip_prefix(prefix)
            .and_then(|n| n.parse::<usize>().ok())
    };
    Some(match key {
        "zip" => &mut d.zip_code,
        "phone" => &mut d.contact_number,
        "email" => &mut d.email,
        "pages" => &mut d.number_of_attachments,
        "sig1" => &mut d.president_signatory_title,
        "sigtin1" => &mut d.president_signatory_tin,
        "sig2" => &mut d.treasurer_signatory_title,
        "sigtin2" => &mut d.treasurer_signatory_tin,
        "d53" => &mut d.part_iv.tax_credits.item_53_other.description,
        "d54" => &mut d.part_iv.tax_credits.item_54_other.description,
        _ => {
            if let Some(i) = indexed("name") {
                return d.registered_name_lines.get_mut(i);
            }
            if let Some(i) = indexed("addr") {
                return d.registered_address_lines.get_mut(i);
            }
            if let Some(i) = indexed("s1od_") {
                return d
                    .schedule_1
                    .other
                    .get_mut(i)
                    .map(|row| &mut row.description);
            }
            if let Some(i) = indexed("s2d_") {
                return d.schedule_2.rows.get_mut(i).map(|row| &mut row.description);
            }
            if let Some(i) = indexed("s2l_") {
                return d.schedule_2.rows.get_mut(i).map(|row| &mut row.legal_basis);
            }
            if let Some(i) = indexed("s3y_") {
                return d
                    .schedule_3
                    .rows
                    .get_mut(i)
                    .map(|row| &mut row.year_incurred);
            }
            if let Some(i) = indexed("s4y_") {
                return d.schedule_4.rows.get_mut(i).map(|row| &mut row.year);
            }
            if let Some(i) = indexed("s5ad_") {
                return d
                    .schedule_5
                    .additions
                    .get_mut(i)
                    .map(|row| &mut row.description);
            }
            if let Some(i) = indexed("s5nd_") {
                return d
                    .schedule_5
                    .non_taxable_income
                    .get_mut(i)
                    .map(|row| &mut row.description);
            }
            if let Some(i) = indexed("s5sd_") {
                return d
                    .schedule_5
                    .special_deductions
                    .get_mut(i)
                    .map(|row| &mut row.description);
            }
            if let Some(i) = indexed("payb_") {
                return d
                    .payment_details
                    .get_mut(i)
                    .map(|row| &mut row.drawee_bank_or_agency);
            }
            if let Some(i) = indexed("payn_") {
                return d.payment_details.get_mut(i).map(|row| &mut row.number);
            }
            if let Some(i) = indexed("pays_") {
                return d
                    .payment_details
                    .get_mut(i)
                    .map(|row| &mut row.specification);
            }
            return None;
        }
    })
}

/// Every editor key the page shows.
fn all_keys() -> Vec<String> {
    let mut keys: Vec<String> = FIELDS.iter().map(|(k, _, _)| k.to_string()).collect();
    keys.extend((0..19).map(|i| format!("s1_{i}")));
    for i in 0..6 {
        keys.push(format!("s1od_{i}"));
        keys.push(format!("s1oa_{i}"));
    }
    for i in 0..4 {
        keys.extend([format!("s2d_{i}"), format!("s2l_{i}"), format!("s2a_{i}")]);
        keys.extend([
            format!("payb_{i}"),
            format!("payn_{i}"),
            format!("payd_{i}"),
            format!("pay_{i}"),
            format!("pays_{i}"),
        ]);
    }
    for i in 1..4 {
        for prefix in ["s3y_", "s3a_", "s3b_", "s3c_", "s3d_"] {
            keys.push(format!("{prefix}{i}"));
        }
    }
    for i in 0..3 {
        for prefix in ["s4y_", "s4n_", "s4m_", "s4p_", "s4e_", "s4c_"] {
            keys.push(format!("{prefix}{i}"));
        }
    }
    for i in 0..2 {
        for prefix in ["s5ad_", "s5a_", "s5nd_", "s5n_", "s5sd_", "s5s_"] {
            keys.push(format!("{prefix}{i}"));
        }
    }
    keys
}

pub struct Form1702RTView {
    draft: Form1702RTDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    month_select: Entity<ComboboxState>,
    atc_select: Entity<ComboboxState>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    /// On-screen label of each input, recorded as it renders, so a parse
    /// message can name its field.
    field_labels: RefCell<BTreeMap<String, String>>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    header: FormHeaderState,
    _subscriptions: Vec<Subscription>,
}

impl Form1702RTView {
    /// Validation messages recorded for any of the model's `keys`.
    fn message_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(&self.validation_errors, keys)
    }

    /// Put the messages for `keys` directly under an already-built row.
    fn with_errors(&self, row: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(row, self.message_for(keys))
    }

    /// The message under one input: its parse error and the model messages
    /// that belong to it alone.
    fn input_message(&self, key: &str) -> Option<String> {
        let parse = messages_for(&self.parse_errors, &[key]);
        let model_keys = model_error_keys(key);
        let model_refs: Vec<&str> = model_keys.iter().map(String::as_str).collect();
        let model = self.message_for(&model_refs);
        match (parse, model) {
            (Some(parse), Some(model)) => Some(format!("{parse} {model}")),
            (parse, model) => parse.or(model),
        }
    }

    /// Messages for values that come from the taxpayer profile and have no
    /// input here (TIN and RDO code), with the note on where to fix them.
    fn render_profile_errors(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let lines: Vec<AnyElement> = [("TIN", "tin"), ("RDO Code", "rdo_code")]
            .into_iter()
            .filter_map(|(label, key)| {
                self.message_for(&[key])
                    .map(|message| field_error(format!("{label}: {message}")).into_any_element())
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .children(lines)
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(PROFILE_NOTE),
                )
                .into_any_element(),
        )
    }

    fn initial(draft: &Form1702RTDraft, key: &str) -> String {
        let mut scratch = draft.clone();
        if let Some(slot) = money_slot(&mut scratch, key) {
            return peso_text(*slot);
        }
        if let Some(slot) = text_slot(&mut scratch, key) {
            return slot.clone();
        }
        match key {
            "year" => draft.taxable_year.to_string(),
            "incorp" => date_text(draft.incorporation_date),
            "rate" => match draft.part_iv.item_40_income_tax_rate_percent {
                0 => String::new(),
                rate => rate.to_string(),
            },
            _ => key
                .strip_prefix("payd_")
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|i| draft.payment_details.get(i))
                .map(|row| date_text(row.date))
                .unwrap_or_default(),
        }
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Read every editor value into the draft, then recompute and validate.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let mut parse_errors = Vec::new();
        let mut draft = self.draft.clone();
        let keys: Vec<String> = all_keys().into_iter().chain(more_keys(&draft)).collect();
        for key in keys {
            let text = self.input_text(&key, cx);
            if let Some(slot) = money_slot(&mut draft, &key) {
                match parse_peso(&text) {
                    Some(value) => *slot = value,
                    None => {
                        let label = self.field_label(&key);
                        parse_errors.push((
                            key.clone(),
                            format!("{label}: \"{text}\" is not a whole-peso amount."),
                        ))
                    }
                }
                continue;
            }
            if let Some(slot) = text_slot(&mut draft, &key) {
                *slot = text;
                continue;
            }
            let date = |text: &str| -> Result<Option<Form1702RTDate>, ()> {
                if text.trim().is_empty() {
                    Ok(None)
                } else {
                    Form1702RTDate::parse(text).map(Some).map_err(|_| ())
                }
            };
            match key.as_str() {
                // Year is locked to the library tile; keep the draft's value.
                "year" => {}
                "rate" => match text.trim().parse::<u8>() {
                    Ok(rate) => draft.part_iv.item_40_income_tax_rate_percent = rate,
                    Err(_) if text.trim().is_empty() => {
                        draft.part_iv.item_40_income_tax_rate_percent = 0
                    }
                    Err(_) => parse_errors
                        .push((key.clone(), "Item 40: enter a whole percentage.".into())),
                },
                "incorp" => match date(&text) {
                    Ok(value) => draft.incorporation_date = value,
                    Err(()) => parse_errors.push((key.clone(), "Item 10: use MM/DD/YYYY.".into())),
                },
                _ => {
                    if let Some(i) = key
                        .strip_prefix("payd_")
                        .and_then(|n| n.parse::<usize>().ok())
                    {
                        match date(&text) {
                            Ok(value) => draft.payment_details[i].date = value,
                            Err(()) => parse_errors.push((
                                key.clone(),
                                format!("Part III row {}: use MM/DD/YYYY.", i + 1),
                            )),
                        }
                    }
                }
            }
        }
        draft.taxpayer_name = draft.registered_name_lines[0].clone();
        draft.registered_address = draft
            .registered_address_lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn new_input(
        draft: &Form1702RTDraft,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let value = Self::initial(draft, key);
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |state, cx| state.set_value(value, window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.validated = false;
                    this.sync_from_inputs(cx);
                }
            },
        );
        (input, subscription)
    }

    /// "(Add more...)" Add Row / Remove: read the page, change the rows,
    /// create editors for new rows, then show every row again from the
    /// draft so nothing shifts into the wrong row.
    fn edit_rows(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1702RTDraft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        self.sync_from_inputs(cx);
        change(&mut self.draft);
        self.draft.recompute();
        for key in more_keys(&self.draft) {
            if self.inputs.contains_key(&key) {
                continue;
            }
            let (input, subscription) = Self::new_input(&self.draft, &key, window, cx);
            self.inputs.insert(key, input);
            self._subscriptions.push(subscription);
        }
        let updates: Vec<(Entity<InputState>, String)> = self
            .inputs
            .iter()
            .map(|(key, input)| (input.clone(), Self::initial(&self.draft, key)))
            .collect();
        for (input, value) in updates {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.sync_from_inputs(cx);
        self.status_message = None;
    }

    /// Add Row: the first click opens the modal with the item as row 1
    /// (`loadModalTable`) and adds row 2; later clicks add one blank row.
    /// The official modal has no row limit.
    fn add_more_row(d: &mut Form1702RTDraft, modal: Form1702RTModal) {
        if d.more_rows.rows(modal).is_empty() {
            let (description, legal_basis, amount) = d.modal_item_mut(modal);
            let first = Form1702RTSpecialDeductionRow {
                description: description.clone(),
                legal_basis: legal_basis.map(|l| l.clone()).unwrap_or_default(),
                amount: *amount,
            };
            d.more_rows.rows_mut(modal).push(first);
        }
        d.more_rows.rows_mut(modal).push(Default::default());
    }

    /// Remove (`deleteRow`, rows renumber). A single row left goes back to
    /// the item, as `saveModalTable` does with one row.
    fn remove_more_row(d: &mut Form1702RTDraft, modal: Form1702RTModal, index: usize) {
        let rows = d.more_rows.rows_mut(modal);
        if index >= rows.len() {
            return;
        }
        rows.remove(index);
        if rows.len() == 1 {
            let last = rows.remove(0);
            let (description, legal_basis, amount) = d.modal_item_mut(modal);
            *description = last.description;
            if let Some(legal_basis) = legal_basis {
                *legal_basis = last.legal_basis;
            }
            *amount = last.amount;
        }
    }

    /// One modal: every row with its Remove, Add Row, and the subtotal.
    fn render_more(
        &self,
        modal: Form1702RTModal,
        enabled: bool,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let index = FORM_1702RT_MODALS
            .iter()
            .position(|m| *m == modal)
            .unwrap_or_default();
        let editable = self.draft.lifecycle.is_editable() && enabled;
        let mut children = vec![
            div()
                .font_weight(FontWeight::BOLD)
                .child(format!("{} — (Add more...)", modal.title()))
                .into_any_element(),
        ];
        for row in 0..self.draft.more_rows.rows(modal).len() {
            let remove = Button::new((
                SharedString::from(format!("1702rt_more_{index}_remove")),
                row,
            ))
            .disabled(!editable)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit_rows(window, cx, |d| Self::remove_more_row(d, modal, row))
            }));
            let mut fields =
                vec![self.field(&more_key(index, 'd', row), "Description", layout, !enabled)];
            if modal.has_legal_basis() {
                fields.push(self.field(
                    &more_key(index, 'l', row),
                    "Legal basis",
                    layout,
                    !enabled,
                ));
            }
            fields.push(self.field(&more_key(index, 'a', row), "Amount", layout, !enabled));
            children.push(kit::row_heading(
                &format!("{}.{}", modal.item(), row + 1),
                remove,
            ));
            children.push(self.grid(layout, fields));
        }
        let (description, legal_basis, amount) = {
            let mut scratch = self.draft.clone();
            let (d, l, a) = scratch.modal_item_mut(modal);
            (d.clone(), l.map(|l| l.clone()), *a)
        };
        let complete = !description.trim().is_empty()
            && legal_basis.is_none_or(|l| !l.trim().is_empty())
            && amount.0 != 0;
        let can_add = self.draft.more_rows.is_folded(modal)
            || !self.draft.more_rows.rows(modal).is_empty()
            || complete;
        children.push(
            kit::add_row_button(
                SharedString::from(format!("1702rt_more_{index}_add")),
                !editable || !can_add,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit_rows(window, cx, |d| Self::add_more_row(d, modal))
            }))
            .into_any_element(),
        );
        let rows_key = format!("more_rows.{}", modal.row_name());
        children.extend(
            self.message_for(&[rows_key.as_str()])
                .map(|message| field_error(message).into_any_element()),
        );
        if self.draft.more_rows.is_folded(modal) {
            children.push(self.computed("Subtotal", self.draft.more_rows.subtotal(modal), cx));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(children)
            .into_any_element()
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1702RTDraft)) {
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
                .release_abandoned_claimed_queueable::<Form1702RTDraft>(
                    &draft.tin,
                    draft.taxable_year,
                    period_column(&FilingPeriod::Annual),
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
            Err(error) => self.notify(
                notification::NotificationType::Error,
                format!("Could not release the claim: {error}"),
                window,
                cx,
            ),
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
    /// The label recorded for `key` (or its `FIELDS` label before the first
    /// render).
    fn field_label(&self, key: &str) -> String {
        if let Some(label) = self.field_labels.borrow().get(key) {
            return label.clone();
        }
        FIELDS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, label, _)| (*label).to_string())
            .unwrap_or_else(|| "Amount".to_string())
    }

    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        self.field_labels
            .borrow_mut()
            .insert(key.to_string(), label.to_string());
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let disabled =
            disabled || !self.draft.lifecycle.is_editable() || folded_key(&self.draft, key);
        let body = div().child(Input::new(input).disabled(disabled));
        let label = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
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
                .child(label.w(relative(0.5)))
                .child(body.w(relative(0.5))),
        };
        with_field_error(row.into_any_element(), self.input_message(key))
    }

    fn input(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        let label = FIELDS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, label, _)| *label)
            .unwrap_or(key);
        self.field(key, label, layout, disabled)
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

    fn computed(&self, label: &str, value: WholePeso, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{value.format_bir()}</div>
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
        // Let long chips shrink to the panel instead of running past its edge.
        button.disabled(disabled).min_w_0().max_w_full()
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

    fn choice_row(title: &str, buttons: Vec<Button>, layout: Layout) -> AnyElement {
        let text = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(title.to_string());
        if layout == Layout::Phone {
            return div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(text)
                .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
                .into_any_element();
        }
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(text.flex_1())
            .children(buttons)
            .into_any_element()
    }

    fn row_card(&self, title: String, body: AnyElement, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(body)
            .into_any_element()
    }

    fn render_page_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fiscal = d.filing_basis == Form1702RTFilingBasis::Fiscal;
        let month_free = fiscal || d.is_short_period;
        // The fiscal or short-period month stays editable; otherwise the
        // select shows December read-only.
        let month_text = d.month.to_string();
        let month_select = select_field(
            "Item 2 — Month ended",
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=12).contains(&d.month).then_some(month_text.as_str()),
            !editable || !month_free,
            self.message_for(&["month", "taxable_year"]).as_deref(),
            cx,
        );
        let amended = d.is_amended;
        let short = d.is_short_period;
        let other_atc = select_field(
            "Item 5 — Other ATC",
            &self.atc_select,
            OTHER_ATCS,
            Some(other_atc_value(d)),
            !editable,
            None,
            cx,
        );
        let period = vec![
            Self::choice_row(
                "Item 1 — For (Calendar or Fiscal year)",
                vec![
                    Self::choice("1702rt_calendar", "Calendar", !fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.filing_basis = Form1702RTFilingBasis::Calendar;
                                d.month = 12;
                            })
                        }),
                    ),
                    Self::choice("1702rt_fiscal", "Fiscal", fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.filing_basis = Form1702RTFilingBasis::Fiscal;
                                if d.month == 12 {
                                    d.month = 6;
                                }
                            })
                        }),
                    ),
                ],
                layout,
            ),
            month_select,
            Self::read_only_row(
                layout,
                "Item 2 — Year ended (YYYY)",
                &d.taxable_year.to_string(),
            ),
            Self::choice_row(
                "Item 3 — Amended return?",
                vec![
                    Self::choice("1702rt_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("1702rt_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
                layout,
            ),
            Self::choice_row(
                "Item 4 — Short period return?",
                vec![
                    Self::choice("1702rt_short_yes", "Yes", short, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_short_period = true)),
                    ),
                    Self::choice("1702rt_short_no", "No", !short, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_short_period = false)),
                    ),
                ],
                layout,
            ),
        ];
        let mcit_note = if d.atc.printed_mcit_selected {
            "IC055 (MCIT) — ticked: four or more years since incorporation"
        } else {
            "IC055 (MCIT) — not ticked (less than four years since incorporation)"
        };
        let background = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 6 — TIN"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format_tin(&d.tin)),
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
                        .child("Item 7 — RDO Code"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(d.rdo_code.clone()),
                )
                .into_any_element(),
            self.input("name0", layout, false),
            self.input("name1", layout, false),
            self.input("name2", layout, false),
            self.input("addr0", layout, false),
            self.input("addr1", layout, false),
            self.input("addr2", layout, false),
            self.input("zip", layout, false),
            self.input("incorp", layout, false),
            self.input("phone", layout, false),
            self.input("email", layout, false),
        ];
        let method = d.deduction_method;
        let mut children = vec![
            self.grid(layout, period),
            div().text_sm().child(mcit_note).into_any_element(),
            other_atc,
            self.grid(layout, background),
        ];
        children.extend(self.render_profile_errors(cx));
        children.push(self.with_errors(
            Self::choice_row(
                "Item 13 — Method of deductions",
                vec![
                    Self::choice(
                        "1702rt_itemized",
                        "Itemized",
                        method == Form1702RTDeductionMethod::Itemized,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.deduction_method = Form1702RTDeductionMethod::Itemized
                        })
                    })),
                    Self::choice(
                        "1702rt_osd",
                        "Optional standard deduction (40%)",
                        method == Form1702RTDeductionMethod::OptionalStandard,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.deduction_method = Form1702RTDeductionMethod::OptionalStandard
                        })
                    })),
                ],
                layout,
            ),
            &["deduction_method"],
        ));
        self.section("Page 1 — Return period and background", children, cx)
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let p2 = &self.draft.part_ii;
        let mut children = vec![self.grid(
            layout,
            vec![
                self.computed("14 — Tax due (Item 43)", p2.item_14_tax_due, cx),
                self.computed(
                    "15 — Tax credits/payments (Item 55)",
                    p2.item_15_total_tax_credits,
                    cx,
                ),
                self.computed(
                    "16 — Net tax payable/(overpayment)",
                    p2.item_16_net_tax_payable_or_overpayment,
                    cx,
                ),
                self.input("i17", layout, false),
                self.input("i18", layout, false),
                self.input("i19", layout, false),
                self.computed("20 — Total penalties", p2.item_20_total_penalties, cx),
                self.computed(
                    "21 — Total amount payable/(overpayment)",
                    p2.item_21_total_amount_payable_or_overpayment,
                    cx,
                ),
            ],
        )];
        if p2.item_16_net_tax_payable_or_overpayment.0 < 0 {
            let current = p2.overpayment_disposition;
            let pick =
                |id: &'static str, label: &'static str, value: Form1702RTOverpaymentDisposition| {
                    Self::choice(id, label, current == Some(value), !editable).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.part_ii.overpayment_disposition = Some(value))
                        }),
                    )
                };
            children.push(self.with_errors(
                Self::choice_row(
                    "If overpayment, mark one box only (irrevocable):",
                    vec![
                        pick(
                            "1702rt_refund",
                            "To be refunded",
                            Form1702RTOverpaymentDisposition::Refund,
                        ),
                        pick(
                            "1702rt_tcc",
                            "To be issued a Tax Credit Certificate",
                            Form1702RTOverpaymentDisposition::TaxCreditCertificate,
                        ),
                        pick(
                            "1702rt_carry",
                            "To be carried over",
                            Form1702RTOverpaymentDisposition::CarryOver,
                        ),
                    ],
                    layout,
                ),
                &["part_ii.overpayment_disposition"],
            ));
        }
        children.extend(
            self.message_for(&["part_ii.item_21_total_amount_payable_or_overpayment"])
                .map(|message| field_error(message).into_any_element()),
        );
        children.push(self.grid(
            layout,
            vec![
                self.input("pages", layout, false),
                self.input("sig1", layout, false),
                self.input("sigtin1", layout, false),
                self.input("sig2", layout, false),
                self.input("sigtin2", layout, false),
            ],
        ));
        let mut payments = Vec::new();
        for (row, label, has_spec, has_bank) in PAYMENT_ROWS {
            let mut fields = Vec::new();
            if has_spec {
                fields.push(self.field(&format!("pays_{row}"), "Particulars", layout, false));
            }
            if has_bank {
                fields.push(self.field(
                    &format!("payb_{row}"),
                    "Drawee bank/agency",
                    layout,
                    false,
                ));
            }
            fields.push(self.field(&format!("payn_{row}"), "Number", layout, false));
            fields.push(self.field(&format!("payd_{row}"), "Date (MM/DD/YYYY)", layout, false));
            fields.push(self.field(&format!("pay_{row}"), "Amount", layout, false));
            // The official alert numbers its rows three ahead of the printed
            // ones, so the row label goes in front of the message.
            let row_key = format!("payment_details[{row}]");
            let message = self
                .message_for(&[row_key.as_str()])
                .map(|message| format!("{label}: {message}"));
            payments.push(with_field_error(
                self.row_card(label.to_string(), self.grid(layout, fields), cx),
                message,
            ));
        }
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Part III — Details of payment")
                .into_any_element(),
        );
        children.extend(
            self.message_for(&["payment_details"])
                .map(|message| field_error(message).into_any_element()),
        );
        children.extend(payments);
        self.section("Page 1 — Part II Total tax payable", children, cx)
    }

    fn render_part_four(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let p4 = &d.part_iv;
        let mcit = d.atc.printed_mcit_selected;
        let itemized = d.deduction_method == Form1702RTDeductionMethod::Itemized;
        let children = vec![
            self.input("i27", layout, false),
            self.input("i28", layout, false),
            self.computed("29 — Net sales/revenues", p4.item_29_net_sales, cx),
            self.input("i30", layout, false),
            self.computed(
                "31 — Gross income from operation",
                p4.item_31_gross_income_from_operations,
                cx,
            ),
            self.input("i32", layout, false),
            self.computed(
                "33 — Total taxable income",
                p4.item_33_total_taxable_income,
                cx,
            ),
            self.computed(
                "34 — Ordinary allowable itemized deductions (Schedule I)",
                p4.item_34_ordinary_itemized_deductions,
                cx,
            ),
            self.computed(
                "35 — Special allowable itemized deductions (Schedule II)",
                p4.item_35_special_itemized_deductions,
                cx,
            ),
            self.computed("36 — NOLCO (Schedule IIIA)", p4.item_36_nolco, cx),
            self.computed(
                "37 — Total itemized deductions",
                p4.item_37_total_itemized_deductions,
                cx,
            ),
            self.computed(
                "38 — Optional standard deduction (40% of Item 33)",
                p4.item_38_optional_standard_deduction,
                cx,
            ),
            self.computed(
                "39 — Net taxable income/(loss)",
                p4.item_39_net_taxable_income_or_loss,
                cx,
            ),
            self.input("rate", layout, false),
            self.computed(
                "41 — Income tax due other than MCIT",
                p4.item_41_normal_income_tax_due,
                cx,
            ),
            self.input("i42", layout, !mcit),
            self.computed("43 — Total income tax due", p4.item_43_tax_due, cx),
            self.input("i44", layout, false),
            self.input("i45", layout, !mcit),
            self.input("i46", layout, false),
            self.computed(
                "47 — Excess MCIT applied (Schedule IV)",
                p4.tax_credits.item_47_excess_mcit_applied,
                cx,
            ),
            self.input("i48", layout, false),
            self.input("i49", layout, false),
            self.input("i50", layout, false),
            self.input("i51", layout, !d.is_amended),
            self.input("i52", layout, false),
            self.input("d53", layout, false),
            self.input("i53", layout, false),
            self.input("d54", layout, false),
            self.input("i54", layout, false),
            self.computed(
                "55 — Total tax credits/payments",
                p4.tax_credits.item_55_total,
                cx,
            ),
            self.computed(
                "56 — Net tax payable/(overpayment)",
                p4.item_56_net_tax_payable_or_overpayment,
                cx,
            ),
            self.input("i57", layout, !itemized),
            self.computed(
                "58 — Special tax credits (Item 52)",
                d.part_v.item_58_special_tax_credits,
                cx,
            ),
            self.computed(
                "59 — Total tax relief availment",
                d.part_v.item_59_total_tax_relief,
                cx,
            ),
        ];
        self.section(
            "Page 2 — Parts IV and V",
            vec![
                self.grid(layout, children),
                self.render_more(Form1702RTModal::Item54, true, layout, cx),
            ],
            cx,
        )
    }

    fn render_schedules_1_2(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let itemized = d.deduction_method == Form1702RTDeductionMethod::Itemized;
        let mut s1: Vec<AnyElement> = SCHEDULE_1_LABELS
            .iter()
            .enumerate()
            .map(|(i, label)| self.field(&format!("s1_{i}"), label, layout, !itemized))
            .collect();
        for (i, letter) in ["d", "e", "f", "g", "h", "i"].iter().enumerate() {
            s1.push(self.field(
                &format!("s1od_{i}"),
                &format!("17{letter} Others (specify)"),
                layout,
                !itemized,
            ));
            s1.push(self.field(
                &format!("s1oa_{i}"),
                &format!("17{letter} Amount"),
                layout,
                !itemized,
            ));
        }
        s1.push(self.computed(
            "18 — Total ordinary allowable itemized deductions",
            d.schedule_1.item_18_total,
            cx,
        ));
        let mut s2 = Vec::new();
        for i in 0..4 {
            let body = self.grid(
                layout,
                vec![
                    self.field(&format!("s2d_{i}"), "Description", layout, !itemized),
                    self.field(&format!("s2l_{i}"), "Legal basis", layout, !itemized),
                    self.field(&format!("s2a_{i}"), "Amount", layout, !itemized),
                ],
            );
            let row_key = format!("schedule_2.rows[{i}]");
            s2.push(self.with_errors(
                self.row_card(format!("Item {}", i + 1), body, cx),
                &[row_key.as_str()],
            ));
        }
        s2.push(self.computed(
            "5 — Total special allowable itemized deductions",
            d.schedule_2.item_5_total,
            cx,
        ));
        let mut children = vec![
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule I — Ordinary allowable itemized deductions")
                .into_any_element(),
            self.grid(layout, s1),
            self.render_more(Form1702RTModal::Schedule1Item17i, itemized, layout, cx),
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule II — Special allowable itemized deductions")
                .into_any_element(),
        ];
        let s2_total = s2.pop();
        children.extend(s2);
        children.push(self.render_more(Form1702RTModal::Schedule2Item4, itemized, layout, cx));
        children.extend(s2_total);
        self.section("Page 3 — Schedules I and II", children, cx)
    }

    fn render_schedules_3_to_5(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let itemized = d.deduction_method == Form1702RTDeductionMethod::Itemized;
        let mcit = d.atc.printed_mcit_selected;
        let s3 = &d.schedule_3;
        let mut children = vec![
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule III — Computation of NOLCO")
                .into_any_element(),
            self.with_errors(
                self.grid(
                    layout,
                    vec![
                        self.computed("1 — Gross income", s3.item_1_gross_income, cx),
                        self.computed(
                            "2 — Ordinary allowable deductions",
                            s3.item_2_ordinary_deductions,
                            cx,
                        ),
                        self.computed("3 — Net operating loss", s3.item_3_net_operating_loss, cx),
                    ],
                ),
                &["schedule_3"],
            ),
        ];
        for i in 1..4 {
            let body = self.grid(
                layout,
                vec![
                    self.field(&format!("s3y_{i}"), "Year incurred", layout, !itemized),
                    self.field(&format!("s3a_{i}"), "A — Amount", layout, !itemized),
                    self.field(
                        &format!("s3b_{i}"),
                        "B — Applied previous years",
                        layout,
                        !itemized,
                    ),
                    self.field(&format!("s3c_{i}"), "C — Expired", layout, !itemized),
                    self.field(
                        &format!("s3d_{i}"),
                        "D — Applied current year",
                        layout,
                        !itemized,
                    ),
                    self.computed("E — Unapplied balance", s3.rows[i].unapplied_balance, cx),
                ],
            );
            let row_keys = [
                format!("schedule_3.rows[{i}]"),
                format!("schedule_3.rows[{i}].applied_current_year"),
                format!("schedule_3.rows[{i}].year_incurred"),
            ];
            children.push(self.with_errors(
                self.row_card(format!("Schedule IIIA Item {}", i + 3), body, cx),
                &row_keys.each_ref().map(String::as_str),
            ));
        }
        children.push(self.computed("7 — Current year's NOL (Column A)", s3.rows[0].amount, cx));
        children.push(self.computed(
            "8 — Total NOLCO applied (Item 36)",
            s3.item_8_total_applied_current_year,
            cx,
        ));
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule IV — Excess MCIT over normal income tax")
                .into_any_element(),
        );
        for i in 0..3 {
            let row = &d.schedule_4.rows[i];
            let body = self.grid(
                layout,
                vec![
                    self.field(&format!("s4y_{i}"), "Year", layout, !mcit),
                    self.field(&format!("s4n_{i}"), "A — Normal income tax", layout, !mcit),
                    self.field(&format!("s4m_{i}"), "B — MCIT", layout, !mcit),
                    self.computed("C — Excess MCIT", row.excess_mcit, cx),
                    self.field(
                        &format!("s4p_{i}"),
                        "D — Applied previous years",
                        layout,
                        !mcit,
                    ),
                    self.field(&format!("s4e_{i}"), "E — Expired", layout, !mcit),
                    self.field(
                        &format!("s4c_{i}"),
                        "F — Applied current year",
                        layout,
                        !mcit,
                    ),
                    self.computed("G — Balance", row.allowable_balance, cx),
                ],
            );
            let row_keys = [
                format!("schedule_4.rows[{i}]"),
                format!("schedule_4.rows[{i}].year"),
            ];
            children.push(self.with_errors(
                self.row_card(format!("Schedule IV Item {}", i + 1), body, cx),
                &row_keys.each_ref().map(String::as_str),
            ));
        }
        children.push(self.computed(
            "4 — Total excess MCIT applied (Item 47)",
            d.schedule_4.item_4_total_applied_current_year,
            cx,
        ));
        let s5 = &d.schedule_5;
        let mut recon = vec![self.input("s5_1", layout, false)];
        for (i, item) in [2, 3].iter().enumerate() {
            recon.push(self.field(
                &format!("s5ad_{i}"),
                &format!("{item} Add (specify)"),
                layout,
                false,
            ));
            recon.push(self.field(
                &format!("s5a_{i}"),
                &format!("{item} Amount"),
                layout,
                false,
            ));
        }
        recon.push(self.computed("4 — Total", s5.item_4_total, cx));
        for (i, item) in [5, 6].iter().enumerate() {
            recon.push(self.field(
                &format!("s5nd_{i}"),
                &format!("{item} Non-taxable income (specify)"),
                layout,
                false,
            ));
            recon.push(self.field(
                &format!("s5n_{i}"),
                &format!("{item} Amount"),
                layout,
                false,
            ));
        }
        for (i, item) in [7, 8].iter().enumerate() {
            recon.push(self.field(
                &format!("s5sd_{i}"),
                &format!("{item} Special deductions (specify)"),
                layout,
                false,
            ));
            recon.push(self.field(
                &format!("s5s_{i}"),
                &format!("{item} Amount"),
                layout,
                false,
            ));
        }
        recon.push(self.computed("9 — Total", s5.item_9_total, cx));
        recon.push(self.computed(
            "10 — Net taxable income/(loss) (must equal Item 39)",
            s5.item_10_net_taxable_income_or_loss,
            cx,
        ));
        children.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Schedule V — Reconciliation of net income per books")
                .into_any_element(),
        );
        children.push(self.grid(layout, recon));
        children.extend(
            self.message_for(&["schedule_5.item_10_net_taxable_income_or_loss"])
                .map(|message| field_error(message).into_any_element()),
        );
        for modal in [
            Form1702RTModal::Schedule5Item3,
            Form1702RTModal::Schedule5Item6,
            Form1702RTModal::Schedule5Item8,
        ] {
            children.push(self.render_more(modal, true, layout, cx));
        }
        self.section("Page 4 — Schedules III to V", children, cx)
    }
}

impl QueueableFormView for Form1702RTView {
    type Draft = Form1702RTDraft;

    const PRINT_SLUG: &'static str = "1702rt-2018c";

    fn print_fields(draft: &Form1702RTDraft) -> std::collections::BTreeMap<String, String> {
        draft.to_bir_field_map()
    }

    fn agent_draft(&self) -> &Form1702RTDraft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        HEADER_ACTIONS
    }

    fn new(
        draft: Form1702RTDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for key in all_keys().into_iter().chain(more_keys(&draft)) {
            let (input, subscription) = Self::new_input(&draft, &key, window, cx);
            subscriptions.push(subscription);
            inputs.insert(key, input);
        }
        let month_select = form_controls::new_select_state(&MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &month_select,
            &MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                if let Ok(month) = value.parse::<u8>() {
                    this.edit(cx, |d| d.month = month);
                }
            },
        ));
        let atc_select = form_controls::new_select_state(OTHER_ATCS, window, cx);
        subscriptions.push(form_controls::on_select(
            &atc_select,
            OTHER_ATCS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                this.edit(cx, |d| {
                    if value == NO_ATC {
                        d.atc.other_selected = false;
                    } else {
                        d.atc.other_selected = true;
                        d.atc.other_code = value;
                    }
                })
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
            field_labels: RefCell::default(),
            status_message: None,
            release_claim_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
            _subscriptions: subscriptions,
        }
    }

    /// The dashboard passes the taxable year; calendar returns end in December.
    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1702RTDraft {
        let mut draft = Form1702RTDraft::new_from_profile(profile, year, 12);
        draft.part_iv.item_40_income_tax_rate_percent = 30;
        draft.deduction_method = Form1702RTDeductionMethod::Itemized;
        draft.recompute();
        draft
    }

    /// Annual. A row saved before the generic queue has a NULL period column;
    /// read it by its period key, and the next save adopts it.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form1702RTDraft {
        let tin = profile.tin.full();
        db.get_queueable_draft::<Form1702RTDraft>(&tin, year, 0)
            .ok()
            .flatten()
            .or_else(|| {
                db.get_form_draft_v2::<Form1702RTDraft>(&tin, "1702RT", year, &FilingPeriod::Annual)
                    .ok()
                    .flatten()
            })
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

impl FormViewTrait for Form1702RTView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702-RT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return (Regular Income Tax Rate)"
    }
    fn form_version(&self) -> &'static str {
        FORM_VERSION_LABEL
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
                .map(|id| {
                    // save-009: saved anyway; the alert goes in the status box.
                    let amended_required = db.amended_return_required(&self.draft).unwrap_or(false);
                    (id, amended_required)
                })
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok((id, amended_required)) => {
                self.draft.id = Some(id);
                let (kind, message) = if unreadable {
                    (
                        notification::NotificationType::Warning,
                        "1702-RT draft saved. Entries that are not amounts were not saved; fix them under Needs review.",
                    )
                } else {
                    (
                        notification::NotificationType::Success,
                        "1702-RT draft saved.",
                    )
                };
                // The save never refuses; the warning stays in the status box.
                let mut status: Vec<&str> = Vec::new();
                if unreadable {
                    status.push(message);
                }
                if amended_required {
                    status.push(bir_core::forms::queueable::AMENDED_RETURN_ALERT);
                }
                self.status_message = (!status.is_empty()).then(|| status.join(" "));
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
        if !can_queue_for_submission(<Form1702RTDraft as QueueableForm>::FORM_CODE) {
            self.status_message =
                Some("1702-RT is not enabled for in-app submission in this build.".into());
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
                "Form 1702-RT",
                &error,
            ));
            cx.notify();
            return;
        }
        // D5C: the toast and the stepper are the only confirmation.
        self.status_message = None;
        self.validated = false;
        self.notify(
            notification::NotificationType::Success,
            "Form 1702-RT queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1702-RT payment status needs a verified confirmation workflow.".into());
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
                    "Form 1702-RT reverted to Draft.".into(),
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
                "Fix the highlighted entries first. No filing state was changed.".into(),
            ));
            return;
        }
        let fields = Self::print_fields(&self.draft);
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            Self::PRINT_SLUG,
            &fields,
            "1702-RT — Print Preview",
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

impl Render for Form1702RTView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let month_text = self.draft.month.to_string();
        form_controls::sync_select(
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=12)
                .contains(&self.draft.month)
                .then_some(month_text.as_str()),
            window,
            cx,
        );
        form_controls::sync_select(
            &self.atc_select,
            OTHER_ATCS,
            Some(other_atc_value(&self.draft)),
            window,
            cx,
        );
        let error_count = self.parse_errors.len() + self.validation_errors.len();
        let issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
            .take(25)
            .map(|(_, message)| {
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.trim().to_string()}</div> }
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
                Button::new("1702rt_back")
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
                            Button::new("1702rt_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1702rt_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_page_one(layout, cx))
            .child(self.render_part_two(layout, cx))
            .child(self.render_part_four(layout, cx))
            .child(self.render_schedules_1_2(layout, cx))
            .child(self.render_schedules_3_to_5(layout, cx));
        if !issues.is_empty() {
            let mut issues = issues;
            if error_count > issues.len() {
                issues.push(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "Showing the first {} of {error_count} items.",
                            issues.len()
                        ))
                        .into_any_element(),
                );
            }
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
                    .id("1702rt_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form1702RTView {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702RTDraft>(
                        &queued.tin,
                        queued.taxable_year,
                        period_column(&FilingPeriod::Annual),
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
            slug: "1702rt",
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702RTDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1702-RT marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1702RTView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1702RTView::blocking_errors,
    on_validate: Form1702RTView::header_validate,
    on_submit: Form1702RTView::mark_submitted,
    on_save: Form1702RTView::save_draft,
    on_print: Form1702RTView::preview_pdf,
    on_revert: Form1702RTView::revert_to_draft,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1702RTView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{
        Layout, MONTH_OPTIONS, NO_ATC, OTHER_ATCS, all_keys, format_tin, model_error_keys,
        parse_peso,
    };
    use bir_core::forms::form_1702rt::WholePeso;
    use gpui::px;

    #[test]
    fn selects_list_every_month_and_a_none_choice() {
        assert_eq!(MONTH_OPTIONS.len(), 12);
        assert_eq!(OTHER_ATCS[0].0, NO_ATC);
        assert_eq!(OTHER_ATCS.len(), 9);
        assert_eq!(
            model_error_keys("s5nd_1"),
            ["schedule_5.non_taxable_income[1]"]
        );
        assert_eq!(model_error_keys("email"), ["email"]);
        assert!(model_error_keys("i17").is_empty());
    }

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn entries_parse_as_whole_pesos() {
        assert_eq!(parse_peso("1,234"), Some(WholePeso(1234)));
        assert_eq!(parse_peso("(5,000)"), Some(WholePeso(-5000)));
        assert_eq!(parse_peso(""), Some(WholePeso(0)));
        assert_eq!(parse_peso("12.50"), Some(WholePeso(13)));
        assert_eq!(parse_peso("1,234.49"), Some(WholePeso(1234)));
        assert_eq!(parse_peso("1,234.50"), Some(WholePeso(1235)));
        assert_eq!(parse_peso("(0.5)"), Some(WholePeso(-1)));
        assert_eq!(parse_peso("1.234.567"), None);
        assert_eq!(parse_peso("."), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
        let keys = all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
    }
}
