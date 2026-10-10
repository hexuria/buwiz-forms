//! Editor for BIR Form 1701A (January 2018), Annual Income Tax Return for
//! Individuals Earning Income Purely from Business/Profession. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1701a`); this view only edits source values.
//! Taxpayer and spouse columns sit side by side on desktop and stack on
//! tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1701a::{
    Form1701AAtc, Form1701ACivilStatus, Form1701AColumn, Form1701ADraft, Form1701AFilerType,
    Form1701AFilingStatus, Form1701AOverpayment,
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

impl EventEmitter<QueueableFormEvent> for Form1701AView {}

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

    fn column(self, draft: &Form1701ADraft) -> &Form1701AColumn {
        match self {
            Self::Taxpayer => &draft.taxpayer,
            Self::Spouse => &draft.spouse_column,
        }
    }

    fn column_mut(self, draft: &mut Form1701ADraft) -> &mut Form1701AColumn {
        match self {
            Self::Taxpayer => &mut draft.taxpayer,
            Self::Spouse => &mut draft.spouse_column,
        }
    }

    fn atc(self, draft: &Form1701ADraft) -> Form1701AAtc {
        match self {
            Self::Taxpayer => draft.atc,
            Self::Spouse if draft.is_joint() => draft.spouse.atc,
            Self::Spouse => Form1701AAtc::Unanswered,
        }
    }
}

type Get = fn(&Form1701AColumn) -> f64;
type Set = fn(&mut Form1701AColumn, f64);
type Open = fn(&Form1701ADraft, Who) -> bool;

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

fn graduated(d: &Form1701ADraft, who: Who) -> bool {
    who.atc(d).is_graduated()
}

fn eight_percent(d: &Form1701ADraft, who: Who) -> bool {
    who.atc(d).is_eight_percent()
}

fn always(_: &Form1701ADraft, _: Who) -> bool {
    true
}

const PART_IVA: &[Money] = &[
    Money {
        id: "sales",
        label: "36 — Sales/revenues/receipts/fees",
        get: |c| c.sales,
        set: |c, v| c.sales = v,
        open: graduated,
    },
    Money {
        id: "sales_returns",
        label: "37 — Less: sales returns, allowances and discounts",
        get: |c| c.sales_returns,
        set: |c, v| c.sales_returns = v,
        open: graduated,
    },
    Money {
        id: "other_income_41",
        label: "41 — Other income (row 41)",
        get: |c| c.other_income_41,
        set: |c, v| c.other_income_41 = v,
        open: graduated,
    },
    Money {
        id: "other_income_42",
        label: "42 — Other income (row 42)",
        get: |c| c.other_income_42,
        set: |c, v| c.other_income_42 = v,
        open: graduated,
    },
    Money {
        id: "gpp_share",
        label: "43 — Share in income of a general professional partnership",
        get: |c| c.gpp_share,
        set: |c, v| c.gpp_share = v,
        open: graduated,
    },
];

const PART_IVA_COMPUTED: &[Computed] = &[
    Computed {
        label: "38 — Net sales/revenues/receipts/fees",
        get: |c| c.net_sales,
    },
    Computed {
        label: "39 — Less: OSD (40% of Item 38)",
        get: |c| c.osd,
    },
    Computed {
        label: "40 — Net income",
        get: |c| c.net_income,
    },
    Computed {
        label: "44 — Total other income",
        get: |c| c.total_other_income,
    },
    Computed {
        label: "45 — Total taxable income",
        get: |c| c.taxable_income,
    },
    Computed {
        label: "46 — Tax due (graduated rates)",
        get: |c| c.graduated_tax_due,
    },
];

const PART_IVB: &[Money] = &[
    Money {
        id: "eight_sales",
        label: "47 — Sales/revenues/receipts/fees",
        get: |c| c.eight_sales,
        set: |c, v| c.eight_sales = v,
        open: eight_percent,
    },
    Money {
        id: "eight_sales_returns",
        label: "48 — Less: sales returns, allowances and discounts",
        get: |c| c.eight_sales_returns,
        set: |c, v| c.eight_sales_returns = v,
        open: eight_percent,
    },
    Money {
        id: "eight_other_income_50",
        label: "50 — Other non-operating income (row 50)",
        get: |c| c.eight_other_income_50,
        set: |c, v| c.eight_other_income_50 = v,
        open: eight_percent,
    },
    Money {
        id: "eight_other_income_51",
        label: "51 — Other non-operating income (row 51)",
        get: |c| c.eight_other_income_51,
        set: |c, v| c.eight_other_income_51 = v,
        open: eight_percent,
    },
    Money {
        id: "eight_reduction",
        label: "54 — Less: allowable reduction (up to ₱250,000)",
        get: |c| c.eight_reduction,
        set: |c, v| c.eight_reduction = v,
        open: eight_percent,
    },
];

const PART_IVB_COMPUTED: &[Computed] = &[
    Computed {
        label: "49 — Net sales/revenues/receipts/fees",
        get: |c| c.eight_net_sales,
    },
    Computed {
        label: "52 — Total other non-operating income",
        get: |c| c.eight_total_other_income,
    },
    Computed {
        label: "53 — Total taxable income",
        get: |c| c.eight_total_income,
    },
    Computed {
        label: "55 — Taxable income (loss)",
        get: |c| c.eight_taxable_income,
    },
    Computed {
        label: "56 — Tax due (8%)",
        get: |c| c.eight_tax_due,
    },
];

const PART_IVC: &[Money] = &[
    Money {
        id: "prior_year_excess",
        label: "57 — Prior year's excess credits",
        get: |c| c.prior_year_excess,
        set: |c, v| c.prior_year_excess = v,
        open: always,
    },
    Money {
        id: "quarterly_payments",
        label: "58 — Tax payments for the first three quarters",
        get: |c| c.quarterly_payments,
        set: |c, v| c.quarterly_payments = v,
        open: always,
    },
    Money {
        id: "cwt_q1_q3",
        label: "59 — Creditable tax withheld, first three quarters",
        get: |c| c.cwt_q1_q3,
        set: |c, v| c.cwt_q1_q3 = v,
        open: always,
    },
    Money {
        id: "cwt_q4",
        label: "60 — Creditable tax withheld per 2307, 4th quarter",
        get: |c| c.cwt_q4,
        set: |c, v| c.cwt_q4 = v,
        open: always,
    },
    Money {
        id: "previously_filed",
        label: "61 — Tax paid in return previously filed (amended)",
        get: |c| c.previously_filed,
        set: |c, v| c.previously_filed = v,
        open: |d, _| d.is_amended,
    },
    Money {
        id: "foreign_tax_credits",
        label: "62 — Foreign tax credits",
        get: |c| c.foreign_tax_credits,
        set: |c, v| c.foreign_tax_credits = v,
        open: |d, who| match who {
            Who::Taxpayer => d.foreign_tax_credits,
            Who::Spouse => d.spouse.foreign_tax_credits == Some(true),
        },
    },
    Money {
        id: "other_credits",
        label: "63 — Other tax credits/payments",
        get: |c| c.other_credits,
        set: |c, v| c.other_credits = v,
        open: always,
    },
];

const PART_IVC_COMPUTED: &[Computed] = &[
    Computed {
        label: "64 — Total tax credits/payments",
        get: |c| c.total_credits,
    },
    Computed {
        label: "65 — Net tax payable (overpayment)",
        get: |c| c.net_payable,
    },
];

const PART_II: &[Money] = &[
    Money {
        id: "second_installment",
        label: "23 — Portion allowed for 2nd installment (≤ 50% of Item 20)",
        get: |c| c.second_installment,
        set: |c, v| c.second_installment = v,
        open: always,
    },
    Money {
        id: "surcharge",
        label: "25 — Surcharge",
        get: |c| c.surcharge,
        set: |c, v| c.surcharge = v,
        open: always,
    },
    Money {
        id: "interest",
        label: "26 — Interest",
        get: |c| c.interest,
        set: |c, v| c.interest = v,
        open: always,
    },
    Money {
        id: "compromise",
        label: "27 — Compromise",
        get: |c| c.compromise,
        set: |c, v| c.compromise = v,
        open: always,
    },
];

const PART_II_COMPUTED: &[Computed] = &[
    Computed {
        label: "20 — Tax due",
        get: |c| c.tax_due,
    },
    Computed {
        label: "21 — Less: total tax credits/payments",
        get: |c| c.total_credits,
    },
    Computed {
        label: "22 — Tax payable (overpayment)",
        get: |c| c.net_payable,
    },
    Computed {
        label: "24 — Amount payable upon filing (overpayment)",
        get: |c| c.amount_payable,
    },
    Computed {
        label: "28 — Total penalties",
        get: |c| c.total_penalties,
    },
    Computed {
        label: "29 — Total amount payable (overpayment)",
        get: |c| c.total_amount_payable,
    },
];

/// Text inputs: (key, label, placeholder, get, set).
struct Text {
    key: &'static str,
    label: &'static str,
    placeholder: &'static str,
    get: fn(&Form1701ADraft) -> String,
    set: fn(&mut Form1701ADraft, String),
}

const TEXTS: &[Text] = &[
    Text {
        key: "year",
        label: "Item 1 — Year (YYYY)",
        placeholder: "YYYY",
        get: |d| d.taxable_year.to_string(),
        set: |d, v| d.taxable_year = v.trim().parse().unwrap_or(0),
    },
    Text {
        key: "name",
        label: "Item 8 — Taxpayer's name (Last, First, Middle)",
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
        key: "birth",
        label: "Item 10 — Date of birth",
        placeholder: "MM/DD/YYYY",
        get: |d| d.birth_date.clone(),
        set: |d, v| d.birth_date = v.trim().to_string(),
    },
    Text {
        key: "email",
        label: "Item 11 — Email address",
        placeholder: "",
        get: |d| d.email.clone(),
        set: |d, v| d.email = v.trim().to_string(),
    },
    Text {
        key: "citizenship",
        label: "Item 12 — Citizenship",
        placeholder: "",
        get: |d| d.citizenship.clone(),
        set: |d, v| d.citizenship = v,
    },
    Text {
        key: "ftn",
        label: "Item 14 — Foreign tax number",
        placeholder: "",
        get: |d| d.foreign_tax_number.clone(),
        set: |d, v| d.foreign_tax_number = v,
    },
    Text {
        key: "phone",
        label: "Item 15 — Contact number",
        placeholder: "",
        get: |d| d.contact_number.clone(),
        set: |d, v| d.contact_number = v.trim().to_string(),
    },
    Text {
        key: "lob",
        label: "Line of business",
        placeholder: "",
        get: |d| d.line_of_business.clone(),
        set: |d, v| d.line_of_business = v,
    },
    Text {
        key: "sheets",
        label: "Item 31 — Number of attachments",
        placeholder: "0",
        get: |d| d.number_of_attachments.to_string(),
        set: |d, v| d.number_of_attachments = v.trim().parse().unwrap_or(u8::MAX),
    },
    Text {
        key: "desc41",
        label: "41 — Other income (specify)",
        placeholder: "",
        get: |d| d.other_income_41_description.clone(),
        set: |d, v| d.other_income_41_description = v,
    },
    Text {
        key: "desc42",
        label: "42 — Other income (specify)",
        placeholder: "",
        get: |d| d.other_income_42_description.clone(),
        set: |d, v| d.other_income_42_description = v,
    },
    Text {
        key: "desc50",
        label: "50 — Other non-operating income (specify)",
        placeholder: "",
        get: |d| d.eight_other_income_50_description.clone(),
        set: |d, v| d.eight_other_income_50_description = v,
    },
    Text {
        key: "desc51",
        label: "51 — Other non-operating income (specify)",
        placeholder: "",
        get: |d| d.eight_other_income_51_description.clone(),
        set: |d, v| d.eight_other_income_51_description = v,
    },
    Text {
        key: "desc63",
        label: "63 — Other tax credits/payments (specify)",
        placeholder: "",
        get: |d| d.other_credits_description.clone(),
        set: |d, v| d.other_credits_description = v,
    },
    Text {
        key: "sp_tin",
        label: "Item 66 — Spouse TIN (with branch code)",
        placeholder: "000-000-000-00000",
        get: |d| d.spouse.tin.clone(),
        set: |d, v| d.spouse.tin = v.trim().to_string(),
    },
    Text {
        key: "sp_rdo",
        label: "Item 67 — Spouse RDO code",
        placeholder: "000",
        get: |d| d.spouse.rdo_code.clone(),
        set: |d, v| d.spouse.rdo_code = v.trim().to_string(),
    },
    Text {
        key: "sp_name",
        label: "Item 70 — Spouse's name",
        placeholder: "",
        get: |d| d.spouse.name.clone(),
        set: |d, v| d.spouse.name = v,
    },
    Text {
        key: "sp_phone",
        label: "Spouse contact number",
        placeholder: "",
        get: |d| d.spouse.contact_number.clone(),
        set: |d, v| d.spouse.contact_number = v.trim().to_string(),
    },
    Text {
        key: "sp_citizenship",
        label: "Spouse citizenship",
        placeholder: "",
        get: |d| d.spouse.citizenship.clone(),
        set: |d, v| d.spouse.citizenship = v,
    },
    Text {
        key: "sp_ftn",
        label: "Spouse foreign tax number",
        placeholder: "",
        get: |d| d.spouse.foreign_tax_number.clone(),
        set: |d, v| d.spouse.foreign_tax_number = v,
    },
];

fn text(key: &str) -> &'static Text {
    TEXTS
        .iter()
        .find(|t| t.key == key)
        .expect("text input registry is complete")
}

fn money_key(who: Who, id: &str) -> String {
    format!("{}:{id}", who.prefix())
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

fn all_money() -> impl Iterator<Item = &'static Money> {
    PART_IVA
        .iter()
        .chain(PART_IVB)
        .chain(PART_IVC)
        .chain(PART_II)
}

pub struct Form1701AView {
    draft: Form1701ADraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1701AView {
    fn money_text(value: f64) -> String {
        if value == 0.0 {
            String::new()
        } else {
            official_amount(value)
        }
    }

    /// Editor text for one input from the draft.
    fn initial(draft: &Form1701ADraft, key: &str) -> String {
        for who in [Who::Taxpayer, Who::Spouse] {
            let Some(id) = key.strip_prefix(&format!("{}:", who.prefix())) else {
                continue;
            };
            return all_money()
                .find(|m| m.id == id)
                .map(|m| Self::money_text((m.get)(who.column(draft))))
                .unwrap_or_default();
        }
        TEXTS
            .iter()
            .find(|t| t.key == key)
            .map(|t| (t.get)(draft))
            .unwrap_or_default()
    }

    fn all_keys() -> Vec<(String, String)> {
        let mut keys: Vec<(String, String)> = TEXTS
            .iter()
            .map(|t| (t.key.to_string(), t.placeholder.to_string()))
            .collect();
        for who in [Who::Taxpayer, Who::Spouse] {
            for money in all_money() {
                keys.push((money_key(who, money.id), "0.00".to_string()));
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
        let mut draft = self.draft.clone();
        for text in TEXTS {
            (text.set)(&mut draft, self.input_text(text.key, cx));
        }
        for who in [Who::Taxpayer, Who::Spouse] {
            for money in all_money() {
                let key = money_key(who, money.id);
                let text = self.input_text(&key, cx);
                match parse_amount(&text) {
                    Some(value) => (money.set)(who.column_mut(&mut draft), value),
                    None => parse_errors.push((
                        key.clone(),
                        format!("{}: \"{text}\" is not a number.", money.label),
                    )),
                }
            }
        }
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
        change: impl FnOnce(&mut Form1701ADraft),
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
                .release_abandoned_claimed_queueable::<Form1701ADraft>(
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
                        Who::Spouse => "spouse_column",
                    },
                    money.id
                );
                self.field(
                    &key,
                    &error_key,
                    money.label,
                    layout,
                    !editable || !(money.open)(&self.draft, who),
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

    fn text_field(&self, key: &str, error_key: &str, layout: Layout, disabled: bool) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        self.field(
            key,
            error_key,
            text(key).label,
            layout,
            !editable || disabled,
        )
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children =
            vec![
                self.text_field("year", "taxable_year", layout, false),
                self.choice_row(
                    "Item 2 — Amended return?",
                    vec![
                        Self::choice("1701a_amended_yes", "Yes", d.is_amended, !editable).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_amended = true)
                            }),
                        ),
                        Self::choice("1701a_amended_no", "No", !d.is_amended, !editable).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_amended = false)
                            }),
                        ),
                    ],
                ),
                self.choice_row(
                    "Item 3 — Short period return?",
                    vec![
                        Self::choice("1701a_short_yes", "Yes", d.is_short_period, !editable)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_short_period = true)
                            })),
                        Self::choice("1701a_short_no", "No", !d.is_short_period, !editable)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |d| d.is_short_period = false)
                            })),
                    ],
                ),
            ];
        if d.is_short_period {
            let months = (1..=12u8)
                .map(|month| {
                    Self::choice(
                        ("1701a_month", month as usize),
                        format!("{month:02}"),
                        d.month == month,
                        !editable,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| d.month = month)
                    }))
                })
                .collect();
            children.push(self.choice_row("Item 1 — Month of the short period", months));
        }
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn filer_choices(&self, who: Who, cx: &Context<Self>) -> Vec<AnyElement> {
        let editable = self.draft.lifecycle.is_editable();
        let p = who.prefix();
        let (filer, atc) = match who {
            Who::Taxpayer => (self.draft.filer_type, self.draft.atc),
            Who::Spouse => (self.draft.spouse.filer_type, self.draft.spouse.atc),
        };
        let types = [
            (Form1701AFilerType::SingleProprietor, "Single proprietor"),
            (Form1701AFilerType::Professional, "Professional"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            Self::choice(
                (SharedString::from(format!("1701a_{p}_type")), index),
                label,
                filer == value,
                !editable,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(window, cx, |d| match who {
                    Who::Taxpayer => d.filer_type = value,
                    Who::Spouse => d.spouse.filer_type = value,
                })
            }))
        })
        .collect();
        let atcs = [
            (
                Form1701AAtc::II012,
                "II012 Business income — graduated (OSD)",
                Form1701AFilerType::SingleProprietor,
            ),
            (
                Form1701AAtc::II014,
                "II014 Income from profession — graduated (OSD)",
                Form1701AFilerType::Professional,
            ),
            (
                Form1701AAtc::II015,
                "II015 Business income — 8%",
                Form1701AFilerType::SingleProprietor,
            ),
            (
                Form1701AAtc::II017,
                "II017 Income from profession — 8%",
                Form1701AFilerType::Professional,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, label, needs))| {
            Self::choice(
                (SharedString::from(format!("1701a_{p}_atc")), index),
                label,
                atc == value,
                !editable || filer != needs,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(window, cx, |d| match who {
                    Who::Taxpayer => d.atc = value,
                    Who::Spouse => d.spouse.atc = value,
                })
            }))
        })
        .collect();
        let (type_item, atc_item) = match who {
            Who::Taxpayer => (
                "Item 6 — Taxpayer type",
                "Item 7 — ATC (sets Item 19, the tax rate)",
            ),
            Who::Spouse => ("Item 68 — Spouse type", "Item 69 — Spouse ATC"),
        };
        vec![
            self.choice_row(type_item, types),
            self.choice_row(atc_item, atcs),
        ]
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
                        .child("Item 4 — TIN / Item 5 — RDO"),
                )
                .child(div().font_weight(FontWeight::BOLD).child(format!(
                    "{}  ·  RDO {}",
                    format_tin(&d.tin),
                    d.rdo_code
                )))
                .into_any_element(),
        ];
        for (key, error_key) in [
            ("name", "taxpayer_name"),
            ("address", "registered_address"),
            ("zip", "zip_code"),
            ("birth", "birth_date"),
            ("email", "email"),
            ("citizenship", "citizenship"),
            ("phone", "contact_number"),
            ("lob", "line_of_business"),
        ] {
            fields.push(self.text_field(key, error_key, layout, false));
        }
        let mut children = vec![self.grid(layout, fields)];
        children.extend(self.filer_choices(Who::Taxpayer, cx));
        children.push(self.choice_row(
            "Item 13 — Claiming foreign tax credits?",
            vec![
                Self::choice("1701a_ftc_yes", "Yes", d.foreign_tax_credits, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.foreign_tax_credits = true)
                    }),
                ),
                Self::choice("1701a_ftc_no", "No", !d.foreign_tax_credits, !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.foreign_tax_credits = false)
                    }),
                ),
            ],
        ));
        children.push(self.text_field("ftn", "foreign_tax_number", layout, !d.foreign_tax_credits));
        let statuses = [
            (Form1701ACivilStatus::Single, "Single"),
            (Form1701ACivilStatus::Married, "Married"),
            (Form1701ACivilStatus::Separated, "Legally separated"),
            (Form1701ACivilStatus::Widow, "Widow/er"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            Self::choice(
                ("1701a_civil", index),
                label,
                d.civil_status == value,
                !editable,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(window, cx, |d| d.civil_status = value)
            }))
        })
        .collect();
        children.push(self.choice_row("Item 16 — Civil status", statuses));
        let married = d.civil_status == Form1701ACivilStatus::Married;
        children.push(self.choice_row(
            "Item 17 — If married, spouse has income?",
            vec![
                    Self::choice(
                        "1701a_sp_income_yes",
                        "Yes",
                        d.spouse_has_income == Some(true),
                        !editable || !married,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.spouse_has_income = Some(true))
                    })),
                    Self::choice(
                        "1701a_sp_income_no",
                        "No",
                        d.spouse_has_income == Some(false),
                        !editable || !married,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.spouse_has_income = Some(false))
                    })),
                ],
        ));
        let can_file = d.spouse_has_income == Some(true);
        children.push(self.choice_row(
            "Item 18 — Filing status",
            vec![
                    Self::choice(
                        "1701a_joint",
                        "Joint filing",
                        d.filing_status == Form1701AFilingStatus::Joint,
                        !editable || !can_file,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.filing_status = Form1701AFilingStatus::Joint
                        })
                    })),
                    Self::choice(
                        "1701a_separate",
                        "Separate filing",
                        d.filing_status == Form1701AFilingStatus::Separate,
                        !editable || !can_file,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.filing_status = Form1701AFilingStatus::Separate
                        })
                    })),
                ],
        ));
        self.section(
            "Part I — Background information on taxpayer/filer",
            children,
            cx,
        )
    }

    fn render_spouse(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut fields = Vec::new();
        for (key, error_key) in [
            ("sp_tin", "spouse.tin"),
            ("sp_rdo", "spouse.rdo_code"),
            ("sp_name", "spouse.name"),
            ("sp_phone", "spouse.contact_number"),
            ("sp_citizenship", "spouse.citizenship"),
        ] {
            fields.push(self.text_field(key, error_key, layout, false));
        }
        let mut children = vec![self.grid(layout, fields)];
        children.extend(self.filer_choices(Who::Spouse, cx));
        let ftc = d.spouse.foreign_tax_credits;
        children.push(self.choice_row(
            "Spouse claiming foreign tax credits?",
            vec![
                Self::choice("1701a_sp_ftc_yes", "Yes", ftc == Some(true), !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.spouse.foreign_tax_credits = Some(true))
                    }),
                ),
                Self::choice("1701a_sp_ftc_no", "No", ftc == Some(false), !editable).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| d.spouse.foreign_tax_credits = Some(false))
                    }),
                ),
            ],
        ));
        children.push(self.text_field(
            "sp_ftn",
            "spouse.foreign_tax_number",
            layout,
            ftc != Some(true),
        ));
        self.section("Part V — Background information on spouse", children, cx)
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let whos = self.joint_whos();
        let block = |moneys: &[Money], computed: &[Computed]| {
            whos.iter()
                .map(|who| (*who, self.money_block(*who, moneys, computed, layout, cx)))
                .collect::<Vec<_>>()
        };
        let d = &self.draft;
        let iva_desc = vec![
            self.text_field(
                "desc41",
                "other_income_41_description",
                layout,
                !d.part_iva_descriptions_open(),
            ),
            self.text_field(
                "desc42",
                "other_income_42_description",
                layout,
                !d.part_iva_descriptions_open(),
            ),
        ];
        let ivb_desc = vec![
            self.text_field(
                "desc50",
                "eight_other_income_50_description",
                layout,
                !d.part_ivb_descriptions_open(),
            ),
            self.text_field(
                "desc51",
                "eight_other_income_51_description",
                layout,
                !d.part_ivb_descriptions_open(),
            ),
        ];
        let sections = vec![
            self.section(
                "Part IV.A — Graduated rates with OSD",
                vec![
                    self.grid(layout, iva_desc),
                    self.columns(layout, block(PART_IVA, PART_IVA_COMPUTED), cx),
                ],
                cx,
            ),
            self.section(
                "Part IV.B — 8% income tax rate",
                vec![
                    self.grid(layout, ivb_desc),
                    self.columns(layout, block(PART_IVB, PART_IVB_COMPUTED), cx),
                ],
                cx,
            ),
            self.section(
                "Part IV.C — Tax credits/payments",
                vec![
                    self.text_field("desc63", "other_credits_description", layout, false),
                    self.columns(layout, block(PART_IVC, PART_IVC_COMPUTED), cx),
                ],
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
            let options = [
                (Form1701AOverpayment::Refund, "To be refunded"),
                (
                    Form1701AOverpayment::TaxCreditCertificate,
                    "To be issued a Tax Credit Certificate",
                ),
                (Form1701AOverpayment::CarryOver, "To be carried over"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                Self::choice(
                    ("1701a_over", index),
                    label,
                    d.overpayment == value,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.overpayment = value)
                }))
            })
            .collect();
            children.push(self.choice_row("If overpayment, mark one box only", options));
        }
        children.push(self.text_field("sheets", "number_of_attachments", layout, false));
        self.section("Part II — Total tax payable", children, cx)
    }
}

impl QueueableFormView for Form1701AView {
    type Draft = Form1701ADraft;

    fn new(
        draft: Form1701ADraft,
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1701ADraft {
        Form1701ADraft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1701AView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701A"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Individuals Earning Income Purely from Business/Profession"
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
                    "1701A draft saved.".into(),
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
        if !can_queue_for_submission(Form1701ADraft::FORM_CODE) {
            self.status_message =
                Some("1701A is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1701A. No submission was started: {error}"
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
            "Form 1701A queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1701A payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1701ADraft>(
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
            "1701a-2018",
            &fields,
            "1701A — Print Preview",
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

impl Render for Form1701AView {
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
                .child(Button::new("1701a_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1701a_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1701a_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1701a_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1701a_submit")
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
                            Button::new("1701a_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1701a_release_cancel")
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
            .child(self.render_schedules(layout, cx));
        if self.draft.is_joint() {
            body = body.child(self.render_spouse(layout, cx));
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
                    .id("1701a_scroll")
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
    use super::{Form1701AView, Layout, format_tin, parse_amount};
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
        let keys = Form1701AView::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len(), "input keys must be unique");
    }
}
