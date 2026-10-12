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
    FORM_1702EX_MAX_ROWS, FORM_1702EX_ORDINARY_ITEMS, FORM_1702EX_OTHER_DEDUCTION_ROWS,
    FORM_1702EX_SPECIAL_DEDUCTION_ROWS, Form1702ExAtc, Form1702ExDeduction, Form1702ExDraft,
    Form1702ExOverpayment, Form1702ExRow, Form1702ExSpecialRow,
};
use bir_core::forms::form_2551q::TaxPeriodBasis;
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
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
    AMENDED_REVERT_MESSAGE, FormHeaderActions, FormHeaderProps, FormHeaderState,
    render_header_buttons, render_status_box, revert_for_amendment, separate_validate_button,
};
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

/// Item 2 year-end month choices: (stored value, shown label).
const MONTH_OPTIONS: [(&str, &str); 12] = [
    ("1", "01"),
    ("2", "02"),
    ("3", "03"),
    ("4", "04"),
    ("5", "05"),
    ("6", "06"),
    ("7", "07"),
    ("8", "08"),
    ("9", "09"),
    ("10", "10"),
    ("11", "11"),
    ("12", "12"),
];

/// Item 5 ATC choices: (stored value, shown label).
const ATC_OPTIONS: [(&str, &str); 2] = [
    ("IC011", "IC 011 Exempt corporation on exempt activities"),
    ("IC021", "IC 021 General professional partnership"),
];

/// The stored ATC option value of a draft's ATC (`None` while unanswered).
fn atc_value(atc: Form1702ExAtc) -> Option<&'static str> {
    match atc {
        Form1702ExAtc::Unanswered => None,
        Form1702ExAtc::IC011 => Some("IC011"),
        Form1702ExAtc::IC021 => Some("IC021"),
    }
}

/// Items the page keeps out of sight: the draft keeps the profile values and
/// they are still sent (owner decision 8A).
const HIDDEN_TEXTS: [&str; 2] = ["phone", "lob"];

/// The model's field names behind an editor key (validation uses them).
fn model_keys(key: &str) -> &'static [&'static str] {
    match key {
        "year" => &["taxable_year"],
        "name" => &["registered_name"],
        "address" => &["registered_address"],
        "zip" => &["zip_code"],
        "incorporated" => &["date_of_incorporation"],
        "email" => &["email"],
        "legal_basis" => &["legal_basis"],
        "agency" => &["promotion_agency"],
        "activity" => &["registered_activity"],
        "eff_from" => &["effectivity_from"],
        "eff_to" => &["effectivity_to"],
        "sheets" => &["number_of_attachments"],
        "sales" => &["sales"],
        "returns" => &["sales_returns"],
        "cost" => &["cost_of_sales"],
        "other_income" => &["other_income"],
        "rate" => &["tax_rate"],
        "prior_excess" => &["prior_year_excess"],
        "quarterly" => &["quarterly_payments"],
        "cwt_previous" => &["cwt_previous_quarters"],
        "cwt_q4" => &["cwt_q4"],
        "foreign" => &["foreign_tax_credits"],
        "previously_filed" => &["previously_filed"],
        "regular_tax" => &["regular_income_tax"],
        "special_relief" => &["special_allowable_relief"],
        "penalties" => &["penalties"],
        _ => &[],
    }
}

/// Validation-key prefixes that belong to a row list (the model keys rows as
/// `other_deductions[3]` or by an item label).
fn list_prefixes(list: &str) -> &'static [&'static str] {
    match list {
        "credit" => &["Page 2 Part 4 Item"],
        "s1other" => &["Page 3 Part 6 Item 17", "other_deductions"],
        "s3nondeduct" => &["non_deductible"],
        "s3nontax" => &["non_taxable_income"],
        "s3special" => &["special_deductions_s3"],
        SPECIAL_LIST => &["special_rows"],
        _ => &[],
    }
}

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
        label: "Item 2 — Year ended: year (YYYY)",
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
    /// Whether the list can grow past `rows` (the model keeps only 2 other
    /// credits, so that list cannot).
    grows: bool,
    get: fn(&Draft) -> &Vec<Form1702ExRow>,
    get_mut: fn(&mut Draft) -> &mut Vec<Form1702ExRow>,
    open: fn(&Draft) -> bool,
}

const ROW_LISTS: &[RowList] = &[
    RowList {
        key: "credit",
        label: "48–49 — Other tax credits/payments",
        rows: 2,
        grows: false,
        get: |d| &d.other_credits,
        get_mut: |d| &mut d.other_credits,
        open: always,
    },
    RowList {
        key: "s1other",
        label: "Schedule 1 Items 17D–17I — Others",
        rows: FORM_1702EX_OTHER_DEDUCTION_ROWS,
        grows: true,
        get: |d| &d.other_deductions,
        get_mut: |d| &mut d.other_deductions,
        open: itemized,
    },
    RowList {
        key: "s3nondeduct",
        label: "Schedule 3 Items 2–3 — Non-deductible expenses/taxable other income",
        rows: 2,
        grows: true,
        get: |d| &d.non_deductible,
        get_mut: |d| &mut d.non_deductible,
        open: always,
    },
    RowList {
        key: "s3nontax",
        label: "Schedule 3 Items 5–6 — Non-taxable income and income subject to final tax",
        rows: 2,
        grows: true,
        get: |d| &d.non_taxable_income,
        get_mut: |d| &mut d.non_taxable_income,
        open: always,
    },
    RowList {
        key: "s3special",
        label: "Schedule 3 Items 7–8 — Special deductions",
        rows: 2,
        grows: true,
        get: |d| &d.special_deductions_s3,
        get_mut: |d| &mut d.special_deductions_s3,
        open: always,
    },
];

fn row_key(list: &str, row: usize, field: &str) -> String {
    format!("{list}{row}:{field}")
}

/// Key of the special-deductions rows (Schedule 2) in `visible_rows`.
const SPECIAL_LIST: &str = "s2";

/// Split a row editor key (`s1other7:desc`) into list, row and field.
fn split_list_key(key: &str) -> Option<(&'static str, usize, &str)> {
    let (head, field) = key.split_once(':')?;
    ROW_LISTS
        .iter()
        .map(|list| list.key)
        .chain(std::iter::once(SPECIAL_LIST))
        .find_map(|list| {
            let row = head.strip_prefix(list)?.parse::<usize>().ok()?;
            Some((list, row, field))
        })
}

/// Editor keys (with placeholders) for one row of a list.
fn list_row_keys(list: &str, row: usize) -> Vec<(String, String)> {
    let mut keys = vec![(row_key(list, row, "desc"), "Description".to_string())];
    if list == SPECIAL_LIST {
        keys.push((row_key(list, row, "basis"), "Legal basis".to_string()));
    }
    keys.push((row_key(list, row, "amount"), "0.00".to_string()));
    keys
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
    /// Item 2 year-end month (fiscal or short period).
    month_select: Entity<ComboboxState>,
    /// Item 5 ATC.
    atc_select: Entity<ComboboxState>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Rows the user has opened per list (see `ROW_LISTS` keys and
    /// `SPECIAL_LIST`). Blank trailing rows are not kept in the draft.
    visible_rows: BTreeMap<&'static str, usize>,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
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

    /// Rows a list holds in the draft.
    fn held_rows(draft: &Draft, list_key: &str) -> usize {
        if list_key == SPECIAL_LIST {
            draft.special_rows.len()
        } else {
            ROW_LISTS
                .iter()
                .find(|list| list.key == list_key)
                .map_or(0, |list| (list.get)(draft).len())
        }
    }

    /// Rows to show for a list: the default, every row the draft holds, and
    /// any row the user has opened.
    fn row_count(&self, list_key: &'static str, default: usize) -> usize {
        default
            .max(Self::held_rows(&self.draft, list_key))
            .max(self.visible_rows.get(list_key).copied().unwrap_or(0))
    }

    /// Keys for the rows a draft holds beyond the default count.
    fn grown_keys(draft: &Draft) -> Vec<(String, String)> {
        let mut keys = Vec::new();
        for list in ROW_LISTS.iter().filter(|list| list.grows) {
            for row in list.rows..Self::held_rows(draft, list.key) {
                keys.extend(list_row_keys(list.key, row));
            }
        }
        for row in FORM_1702EX_SPECIAL_DEDUCTION_ROWS..Self::held_rows(draft, SPECIAL_LIST) {
            keys.extend(list_row_keys(SPECIAL_LIST, row));
        }
        keys
    }

    /// Create the editors for one row of a list on demand.
    fn ensure_row_inputs(
        &mut self,
        list: &str,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (key, placeholder) in list_row_keys(list, row) {
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

    fn add_row(
        &mut self,
        list: &'static str,
        default: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let count = self.row_count(list, default);
        if count >= FORM_1702EX_MAX_ROWS {
            return;
        }
        self.ensure_row_inputs(list, count, window, cx);
        self.visible_rows.insert(list, count + 1);
        cx.notify();
    }

    /// Delete a row beyond the default count; later rows move up.
    fn remove_row(
        &mut self,
        list: &'static str,
        default: usize,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let count = self.row_count(list, default);
        if row < default || row >= count {
            return;
        }
        // `edit` reads every editor first, so no typed row is lost.
        self.edit(window, cx, |draft| {
            if list == SPECIAL_LIST {
                if row < draft.special_rows.len() {
                    draft.special_rows.remove(row);
                }
            } else if let Some(entry) = ROW_LISTS.iter().find(|entry| entry.key == list) {
                let rows = (entry.get_mut)(draft);
                if row < rows.len() {
                    rows.remove(row);
                }
            }
        });
        self.visible_rows.insert(list, (count - 1).max(default));
        cx.notify();
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
        if let Some((list_key, row, field)) = split_list_key(key) {
            if list_key == SPECIAL_LIST {
                let entry = draft.special_rows.get(row).cloned().unwrap_or_default();
                return match field {
                    "desc" => entry.description,
                    "basis" => entry.legal_basis,
                    "amount" => Self::money_text(entry.amount),
                    _ => String::new(),
                };
            }
            if let Some(list) = ROW_LISTS.iter().find(|list| list.key == list_key) {
                let entry = (list.get)(draft).get(row).cloned().unwrap_or_default();
                return match field {
                    "desc" => entry.description,
                    "amount" => Self::money_text(entry.amount),
                    _ => String::new(),
                };
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
            for row in 0..self.row_count(list.key, list.rows) {
                let amount = number(&row_key(list.key, row, "amount"), list.label, cx);
                rows.push(Form1702ExRow {
                    description: String::new(),
                    amount,
                });
            }
            *(list.get_mut)(&mut draft) = rows;
        }
        let mut special = Vec::new();
        for row in 0..self.row_count(SPECIAL_LIST, FORM_1702EX_SPECIAL_DEDUCTION_ROWS) {
            let amount = number(&row_key("s2", row, "amount"), "Schedule 2", cx);
            special.push(Form1702ExSpecialRow {
                description: String::new(),
                legal_basis: String::new(),
                amount,
            });
        }
        // The year is locked to the tile; the hidden items keep the profile
        // values already in the draft.
        for text in TEXTS
            .iter()
            .filter(|t| t.key != "year" && !HIDDEN_TEXTS.contains(&t.key))
        {
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
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            if let Some(input) = self.inputs.get(&key) {
                let value = Self::initial(&self.draft, &key);
                if input.read(cx).value() != value.as_str() {
                    input.update(cx, |state, cx| state.set_value(value, window, cx));
                }
            }
        }
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        self.validated = false;
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

    /// Messages for any of `keys`, from malformed input and from validation.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter()),
            keys,
        )
    }

    /// Messages whose key starts with any of `prefixes` (row lists).
    fn errors_with_prefix(&self, prefixes: &[&str]) -> Option<String> {
        let all = || {
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter())
        };
        let keys: Vec<&str> = all()
            .map(|(field, _)| field.as_str())
            .filter(|field| prefixes.iter().any(|p| field.starts_with(p)))
            .collect();
        messages_for(all(), &keys)
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

    /// Read-only item: the label, then the value in bold.
    fn read_only_row(layout: Layout, label: &str, value: String) -> AnyElement {
        let label = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let value = div().font_weight(FontWeight::BOLD).child(value);
        match layout {
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
                .child(label.w(relative(0.55)))
                .child(value.w(relative(0.45))),
        }
        .into_any_element()
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
        button.small().disabled(disabled)
    }

    fn choice_row(&self, label: &str, buttons: Vec<Button>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
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

    /// Inputs and computed rows interleaved by the leading item number in
    /// their labels (stable, so equal numbers keep input-before-computed).
    fn money_by_item(
        &self,
        moneys: &[Money],
        computed: &[Computed],
        layout: Layout,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        fn item_no(label: &str) -> u32 {
            label
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(u32::MAX)
        }
        let editable = self.draft.lifecycle.is_editable();
        let mut rows: Vec<(u32, AnyElement)> = moneys
            .iter()
            .map(|m| {
                (
                    item_no(m.label),
                    self.field(m.key, m.label, layout, !editable || !(m.open)(&self.draft)),
                )
            })
            .collect();
        rows.extend(computed.iter().map(|c| {
            (
                item_no(c.label),
                self.computed(c.label, (c.get)(&self.draft), cx),
            )
        }));
        rows.sort_by_key(|(n, _)| *n);
        rows.into_iter().map(|(_, el)| el).collect()
    }

    fn money_section(
        &self,
        title: &str,
        moneys: &[Money],
        computed: &[Computed],
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let children = self.money_by_item(moneys, computed, layout, cx);
        self.section(title, vec![self.grid(layout, children)], cx)
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fiscal = d.tax_period_basis == TaxPeriodBasis::Fiscal;
        let mut children = vec![self.choice_row(
            "Item 1 — For (Calendar or Fiscal year)",
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
            let month = d.year_end_month.to_string();
            children.push(select_field(
                "Item 2 — Year ended: month (MM)",
                &self.month_select,
                &MONTH_OPTIONS,
                (1..=12)
                    .contains(&d.year_end_month)
                    .then_some(month.as_str()),
                !editable,
                self.errors_for(&["year_end_month"]).as_deref(),
                cx,
            ));
        }
        children.push(self.with_errors(
            Self::read_only_row(layout, TEXTS[0].label, d.taxable_year.to_string()),
            &["taxable_year"],
        ));
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
        children.push(select_field(
            "Item 5 — ATC",
            &self.atc_select,
            &ATC_OPTIONS,
            atc_value(d.atc),
            !editable,
            self.errors_for(&["atc"]).as_deref(),
            cx,
        ));
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn deduction_row(&self, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let row = self.choice_row(
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
        );
        self.with_errors(row, &["deduction"])
    }

    fn render_attachments(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let field = self.field("sheets", TEXTS[13].label, layout, !editable);
        self.section("Attachments", vec![self.grid(layout, vec![field])], cx)
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
        // Items 8-12, then Item 13, then Items 14-17. Telephone and line of
        // business are not rendered; the draft keeps the profile values.
        for text in TEXTS[1..8]
            .iter()
            .filter(|text| !HIDDEN_TEXTS.contains(&text.key))
        {
            fields.push(self.field(text.key, text.label, layout, !editable));
        }
        fields.push(self.deduction_row(cx));
        for text in &TEXTS[8..13] {
            fields.push(self.field(text.key, text.label, layout, !editable));
        }
        let mut children = vec![self.grid(layout, fields)];
        children.extend(self.render_profile_errors(cx));
        self.section("Part I — Background Information", children, cx)
    }

    /// Messages for the hidden profile values (Telephone, Line of business):
    /// they have no field here, so they are listed once with a pointer to
    /// Profile Settings. `None` when there are none.
    fn render_profile_errors(&self, cx: &Context<Self>) -> Option<AnyElement> {
        const LINES: [(&str, &[&str]); 2] = [
            ("Telephone", &["contact_number"]),
            ("Line of business", &["line_of_business"]),
        ];
        let lines: Vec<AnyElement> = LINES
            .iter()
            .filter_map(|(label, keys)| {
                self.errors_for(keys)
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
                .children(lines)
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

    /// "Add row" button and "Up to N rows" hint under a growable list.
    fn add_row_controls(
        &self,
        list: &'static str,
        default: usize,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let at_max = self.row_count(list, default) >= FORM_1702EX_MAX_ROWS;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                Button::new(format!("1702ex_add_row_{list}"))
                    .label("Add row")
                    .outline()
                    .small()
                    .disabled(disabled || at_max)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.add_row(list, default, window, cx)
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Up to {FORM_1702EX_MAX_ROWS} rows")),
            )
            .into_any_element()
    }

    /// A row beyond the default count: heading with Remove, then its fields.
    fn extra_row(
        &self,
        list: &'static str,
        default: usize,
        row: usize,
        content: AnyElement,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
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
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Row {}", row + 1)),
                    )
                    .child(
                        Button::new(format!("1702ex_remove_row_{list}_{row}"))
                            .label("Remove")
                            .outline()
                            .small()
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_row(list, default, row, window, cx)
                            })),
                    ),
            )
            .child(content)
            .into_any_element()
    }

    fn row_list(&self, list: &RowList, layout: Layout, cx: &Context<Self>) -> AnyElement {
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
        let grid = self.grid(layout, fields);
        let message = self.errors_with_prefix(list_prefixes(list.key));
        if !list.grows {
            return with_field_error(grid, message);
        }
        let disabled = !editable || !open;
        let mut body = div().flex().flex_col().gap_3().w_full().child(grid);
        for row in list.rows..self.row_count(list.key, list.rows) {
            let pair = vec![
                self.field(
                    &row_key(list.key, row, "desc"),
                    &format!("{} — row {} description", list.label, row + 1),
                    layout,
                    disabled,
                ),
                self.field(
                    &row_key(list.key, row, "amount"),
                    &format!("{} — row {} amount", list.label, row + 1),
                    layout,
                    disabled,
                ),
            ];
            body = body.child(self.extra_row(
                list.key,
                list.rows,
                row,
                self.grid(layout, pair),
                disabled,
                cx,
            ));
        }
        let body = body
            .child(self.add_row_controls(list.key, list.rows, disabled, cx))
            .into_any_element();
        with_field_error(body, message)
    }

    /// Schedule 2 rows: the default rows as before, extra rows boxed with a
    /// Remove button, then "Add row".
    fn special_rows_block(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable() || !self.draft.itemized_open();
        let default = FORM_1702EX_SPECIAL_DEDUCTION_ROWS;
        let row_fields = |row: usize| -> Vec<AnyElement> {
            [
                ("desc", "description"),
                ("basis", "legal basis"),
                ("amount", "amount"),
            ]
            .into_iter()
            .map(|(field, label)| {
                self.field(
                    &row_key(SPECIAL_LIST, row, field),
                    &format!("Item {} — {label}", row + 1),
                    layout,
                    disabled,
                )
            })
            .collect()
        };
        let mut first = Vec::new();
        for row in 0..default {
            first.extend(row_fields(row));
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .child(self.grid(layout, first));
        for row in default..self.row_count(SPECIAL_LIST, default) {
            body = body.child(self.extra_row(
                SPECIAL_LIST,
                default,
                row,
                self.grid(layout, row_fields(row)),
                disabled,
                cx,
            ));
        }
        let body = body
            .child(self.add_row_controls(SPECIAL_LIST, default, disabled, cx))
            .into_any_element();
        with_field_error(body, self.errors_with_prefix(list_prefixes(SPECIAL_LIST)))
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let itemized = d.itemized_open();
        let part_iv = self.money_by_item(PART_IV, PART_IV_COMPUTED, layout, cx);
        let mut credits: Vec<AnyElement> = CREDITS
            .iter()
            .map(|m| self.field(m.key, m.label, layout, !editable || !(m.open)(d)))
            .collect();
        credits.push(self.row_list(&ROW_LISTS[0], layout, cx));
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
        s1.extend(
            self.errors_for(&["ordinary_items"])
                .map(|message| field_error(message).into_any_element()),
        );
        s1.push(self.row_list(&ROW_LISTS[1], layout, cx));
        s1.push(self.computed(
            "18 — Total ordinary allowable itemized deductions",
            d.ordinary_deductions,
            cx,
        ));
        let mut s2 = vec![self.special_rows_block(layout, cx)];
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
        s3.push(self.row_list(&ROW_LISTS[2], layout, cx));
        s3.push(self.computed("4 — Total", d.reconciliation_total, cx));
        s3.push(self.row_list(&ROW_LISTS[3], layout, cx));
        s3.push(self.row_list(&ROW_LISTS[4], layout, cx));
        s3.push(self.computed("9 — Total", d.reconciliation_less, cx));
        s3.push(self.with_errors(
            self.computed(
                "10 — Net taxable income (must equal Item 39)",
                d.reconciled_taxable_income,
                cx,
            ),
            &["reconciled_taxable_income"],
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
            self.section("Schedule 2 — Special allowable itemized deductions", s2, cx),
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
        let children = self.money_by_item(PART_II, PART_II_COMPUTED, layout, cx);
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
            sections.push(self.with_errors(
                self.choice_row("If overpayment, mark one box only", options),
                &["overpayment"],
            ));
        }
        self.section("Part II — Total tax payable", sections, cx)
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
        let mut keys = Self::all_keys();
        keys.extend(Self::grown_keys(&draft));
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
        let atc_select = form_controls::new_select_state(&ATC_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &atc_select,
            &ATC_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                let atc = match value.as_str() {
                    "IC011" => Form1702ExAtc::IC011,
                    "IC021" => Form1702ExAtc::IC021,
                    _ => return,
                };
                this.edit(window, cx, |d| d.atc = atc);
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
            header: FormHeaderState::new(),
            validated: false,
            visible_rows: BTreeMap::new(),
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
                "Form 1702EX",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
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
                    self.edit(window, cx, |d| d.is_amended = true);
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
                    "Form 1702EX reverted to Draft.".into(),
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
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let month_text = self.draft.year_end_month.to_string();
        form_controls::sync_select(
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=12)
                .contains(&self.draft.year_end_month)
                .then_some(month_text.as_str()),
            window,
            cx,
        );
        form_controls::sync_select(
            &self.atc_select,
            &ATC_OPTIONS,
            atc_value(self.draft.atc),
            window,
            cx,
        );
        let error_count = self.parse_errors.len() + self.validation_errors.len();
        let _issues: Vec<AnyElement> = self
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
                .child(Button::new("1702ex_back").label("← Back").on_click(
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
            .child(self.render_totals(layout, cx))
            .child(self.render_schedules(layout, cx))
            .child(self.render_attachments(layout, cx));
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

/// The official success alert after a passing validation.
const VALIDATION_SUCCESS: &str =
    "Validation successful. Click on Edit if you wish to modify your entries.";

impl Form1702ExView {
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

    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1702ex",
            status: self.draft.lifecycle.status.clone(),
            editor_safe: true,
            validated: self.validated,
            blocking_errors: self.blocking_errors(),
            errors: self
                .parse_errors
                .iter()
                .chain(self.validation_errors.iter())
                .map(|(_, message)| message.replace('\n', " "))
                .collect(),
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702ExDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1702EX marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form1702ExView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1702ExView::blocking_errors,
    on_validate: Form1702ExView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1702ExView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Form1702ExView, Layout, format_tin, parse_amount};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_1702ex_view.rs");
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
    fn input_keys_are_unique() {
        let keys = Form1702ExView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len());
    }
}
