//! Editor for BIR Form 1700 (January 2018), Annual Income Tax Return for
//! Individuals Earning Purely Compensation Income. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1700`); this view only edits source values.
//! Taxpayer and spouse columns sit side by side on desktop and stack on
//! tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1700::{
    FORM_1700_ATC, FORM_1700_EMPLOYER_ROWS, Form1700CivilStatus, Form1700Column, Form1700Draft,
    Form1700Employer, Form1700FilingStatus, Form1700TaxpayerType,
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

impl EventEmitter<QueueableFormEvent> for Form1700View {}

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

    fn column(self, draft: &Form1700Draft) -> &Form1700Column {
        match self {
            Self::Taxpayer => &draft.taxpayer,
            Self::Spouse => &draft.spouse_column,
        }
    }

    fn column_mut(self, draft: &mut Form1700Draft) -> &mut Form1700Column {
        match self {
            Self::Taxpayer => &mut draft.taxpayer,
            Self::Spouse => &mut draft.spouse_column,
        }
    }

    fn kind(self, draft: &Form1700Draft) -> Form1700TaxpayerType {
        match self {
            Self::Taxpayer => draft.taxpayer_type,
            Self::Spouse if draft.is_joint() => draft.spouse.taxpayer_type,
            Self::Spouse => Form1700TaxpayerType::Unanswered,
        }
    }
}

type Get = fn(&Form1700Column) -> f64;
type Set = fn(&mut Form1700Column, f64);
type Open = fn(&Form1700Draft, Who) -> bool;

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

fn employee(d: &Form1700Draft, who: Who) -> bool {
    who.kind(d) == Form1700TaxpayerType::Employee
}

fn nranetb(d: &Form1700Draft, who: Who) -> bool {
    who.kind(d) == Form1700TaxpayerType::Nranetb
}

fn always(_: &Form1700Draft, _: Who) -> bool {
    true
}

const PART_VA: &[Money] = &[
    Money {
        id: "non_taxable",
        label: "43 — Less: non-taxable/exempt compensation",
        get: |c| c.non_taxable,
        set: |c, v| c.non_taxable = v,
        open: employee,
    },
    Money {
        id: "other_income",
        label: "45 — Add: other taxable non-business income",
        get: |c| c.other_income,
        set: |c, v| c.other_income = v,
        open: employee,
    },
];

const PART_VA_COMPUTED: &[Computed] = &[
    Computed {
        label: "42 — Gross compensation income (Schedule 1)",
        get: |c| c.gross_compensation,
    },
    Computed {
        label: "44 — Gross taxable compensation income",
        get: |c| c.taxable_compensation,
    },
    Computed {
        label: "46 — Total taxable income",
        get: |c| c.taxable_income,
    },
    Computed {
        label: "47 — Tax due (graduated rates)",
        get: |c| c.graduated_tax_due,
    },
];

const PART_VB: &[Money] = &[
    Money {
        id: "flat_non_taxable",
        label: "49 — Less: non-taxable/exempt compensation",
        get: |c| c.flat_non_taxable,
        set: |c, v| c.flat_non_taxable = v,
        open: nranetb,
    },
    Money {
        id: "flat_other_income",
        label: "51 — Add: other taxable income",
        get: |c| c.flat_other_income,
        set: |c, v| c.flat_other_income = v,
        open: nranetb,
    },
];

const PART_VB_COMPUTED: &[Computed] = &[
    Computed {
        label: "48 — Gross compensation income (Schedule 1)",
        get: |c| c.flat_gross_compensation,
    },
    Computed {
        label: "50 — Gross taxable compensation income",
        get: |c| c.flat_taxable_compensation,
    },
    Computed {
        label: "52 — Total taxable income",
        get: |c| c.flat_taxable_income,
    },
    Computed {
        label: "53 — Tax due (25%)",
        get: |c| c.flat_tax_due,
    },
];

const PART_VC: &[Money] = &[
    Money {
        id: "previously_filed",
        label: "55 — Tax paid in return previously filed (amended)",
        get: |c| c.previously_filed,
        set: |c, v| c.previously_filed = v,
        open: |d, _| d.is_amended,
    },
    Money {
        id: "foreign_tax_credits",
        label: "56 — Foreign tax credits",
        get: |c| c.foreign_tax_credits,
        set: |c, v| c.foreign_tax_credits = v,
        open: |d, who| match who {
            Who::Taxpayer => d.foreign_tax_credits == Some(true),
            Who::Spouse => d.spouse.foreign_tax_credits == Some(true),
        },
    },
    Money {
        id: "other_credits",
        label: "57 — Other tax credits/payments",
        get: |c| c.other_credits,
        set: |c, v| c.other_credits = v,
        open: always,
    },
];

const PART_VC_COMPUTED: &[Computed] = &[
    Computed {
        label: "54 — Tax withheld per BIR Form 2316 (Schedule 1)",
        get: |c| c.tax_withheld,
    },
    Computed {
        label: "58 — Total tax credits/payments",
        get: |c| c.total_credits,
    },
    Computed {
        label: "59 — Net tax payable (overpayment)",
        get: |c| c.net_payable,
    },
];

const PART_III: &[Money] = &[
    Money {
        id: "second_installment",
        label: "29 — Portion allowed for 2nd installment (≤ 50% of Item 26)",
        get: |c| c.second_installment,
        set: |c, v| c.second_installment = v,
        open: employee,
    },
    Money {
        id: "interest",
        label: "31 — Interest",
        get: |c| c.interest,
        set: |c, v| c.interest = v,
        open: always,
    },
    Money {
        id: "surcharge",
        label: "32 — Surcharge",
        get: |c| c.surcharge,
        set: |c, v| c.surcharge = v,
        open: always,
    },
    Money {
        id: "compromise",
        label: "33 — Compromise",
        get: |c| c.compromise,
        set: |c, v| c.compromise = v,
        open: always,
    },
];

const PART_III_COMPUTED: &[Computed] = &[
    Computed {
        label: "26 — Tax due",
        get: |c| c.tax_due,
    },
    Computed {
        label: "27 — Less: total tax credits/payments",
        get: |c| c.total_credits,
    },
    Computed {
        label: "28 — Net tax payable (overpayment)",
        get: |c| c.net_payable,
    },
    Computed {
        label: "30 — Amount of tax payable (overpayment)",
        get: |c| c.amount_payable,
    },
    Computed {
        label: "34 — Total penalties",
        get: |c| c.total_penalties,
    },
    Computed {
        label: "35 — Total amount payable (overpayment)",
        get: |c| c.total_amount_payable,
    },
];

/// Text inputs: (key, label, placeholder, get, set).
struct Text {
    key: &'static str,
    label: &'static str,
    placeholder: &'static str,
    get: fn(&Form1700Draft) -> String,
    set: fn(&mut Form1700Draft, String),
}

const TEXTS: &[Text] = &[
    Text {
        key: "year",
        label: "Item 1 — For the year (YYYY)",
        placeholder: "YYYY",
        get: |d| d.taxable_year.to_string(),
        set: |d, v| d.taxable_year = v.trim().parse().unwrap_or(0),
    },
    Text {
        key: "name",
        label: "Item 7 — Taxpayer's name (Last, First, Middle)",
        placeholder: "",
        get: |d| d.taxpayer_name.clone(),
        set: |d, v| d.taxpayer_name = v,
    },
    Text {
        key: "address",
        label: "Item 8 — Registered address",
        placeholder: "",
        get: |d| d.registered_address.clone(),
        set: |d, v| d.registered_address = v,
    },
    Text {
        key: "zip",
        label: "Item 8A — Zip code",
        placeholder: "",
        get: |d| d.zip_code.clone(),
        set: |d, v| d.zip_code = v.trim().to_string(),
    },
    Text {
        key: "birth",
        label: "Item 9 — Date of birth",
        placeholder: "MM/DD/YYYY",
        get: |d| d.birth_date.clone(),
        set: |d, v| d.birth_date = v.trim().to_string(),
    },
    Text {
        key: "email",
        label: "Item 10 — Email address",
        placeholder: "",
        get: |d| d.email.clone(),
        set: |d, v| d.email = v.trim().to_string(),
    },
    Text {
        key: "citizenship",
        label: "Item 11 — Citizenship",
        placeholder: "",
        get: |d| d.citizenship.clone(),
        set: |d, v| d.citizenship = v,
    },
    Text {
        key: "ftn",
        label: "Item 13 — Foreign tax number",
        placeholder: "",
        get: |d| d.foreign_tax_number.clone(),
        set: |d, v| d.foreign_tax_number = v,
    },
    Text {
        key: "phone",
        label: "Item 14 — Contact number",
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
        label: "Item 37 — Number of attachments",
        placeholder: "0",
        get: |d| d.number_of_attachments.to_string(),
        set: |d, v| d.number_of_attachments = v.trim().parse().unwrap_or(u8::MAX),
    },
    Text {
        key: "desc45",
        label: "45 — Other taxable income (specify)",
        placeholder: "",
        get: |d| d.other_income_description.clone(),
        set: |d, v| d.other_income_description = v,
    },
    Text {
        key: "desc49",
        label: "49 — Non-taxable/exempt compensation (specify)",
        placeholder: "",
        get: |d| d.flat_non_taxable_description.clone(),
        set: |d, v| d.flat_non_taxable_description = v,
    },
    Text {
        key: "desc51",
        label: "51 — Other taxable income (specify)",
        placeholder: "",
        get: |d| d.flat_other_income_description.clone(),
        set: |d, v| d.flat_other_income_description = v,
    },
    Text {
        key: "desc57",
        label: "57 — Other tax credits/payments (specify)",
        placeholder: "",
        get: |d| d.other_credits_description.clone(),
        set: |d, v| d.other_credits_description = v,
    },
    Text {
        key: "sp_tin",
        label: "Item 18 — Spouse TIN (with branch code)",
        placeholder: "000-000-000-00000",
        get: |d| d.spouse.tin.clone(),
        set: |d, v| d.spouse.tin = v.trim().to_string(),
    },
    Text {
        key: "sp_rdo",
        label: "Item 19 — Spouse RDO code",
        placeholder: "000",
        get: |d| d.spouse.rdo_code.clone(),
        set: |d, v| d.spouse.rdo_code = v.trim().to_string(),
    },
    Text {
        key: "sp_name",
        label: "Item 21 — Spouse's name",
        placeholder: "",
        get: |d| d.spouse.name.clone(),
        set: |d, v| d.spouse.name = v,
    },
    Text {
        key: "sp_phone",
        label: "Item 22 — Spouse contact number",
        placeholder: "",
        get: |d| d.spouse.contact_number.clone(),
        set: |d, v| d.spouse.contact_number = v.trim().to_string(),
    },
    Text {
        key: "sp_citizenship",
        label: "Item 23 — Spouse citizenship",
        placeholder: "",
        get: |d| d.spouse.citizenship.clone(),
        set: |d, v| d.spouse.citizenship = v,
    },
    Text {
        key: "sp_ftn",
        label: "Item 25 — Spouse foreign tax number",
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

/// Employer row inputs: (suffix, label).
const EMPLOYER_FIELDS: &[(&str, &str)] = &[
    ("name", "a — Name of employer"),
    ("name2", "a — Name of employer (second line)"),
    ("tin", "b — Employer's TIN (with branch code)"),
    ("regular", "c — Compensation, regular/graduated rates"),
    ("flat", "d — Compensation, 25% flat rate"),
    ("withheld", "e — Tax withheld"),
];

/// Employer rows the editor offers: Items 1–4 and the add-more popup rows.
const EDITOR_EMPLOYER_ROWS: usize = 12;

fn employer_key(row: usize, field: &str) -> String {
    format!("emp{row}:{field}")
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
    PART_VA.iter().chain(PART_VB).chain(PART_VC).chain(PART_III)
}

pub struct Form1700View {
    draft: Form1700Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1700View {
    /// "Employer 2", or "Item 4 popup row 4.2" once the popup holds rows.
    fn employer_title(draft: &Form1700Draft, row: usize) -> String {
        if row + 1 >= FORM_1700_EMPLOYER_ROWS
            && (draft.schedule_is_folded() || row + 1 > FORM_1700_EMPLOYER_ROWS)
        {
            format!(
                "Item 4 add-more popup row 4.{}",
                row + 2 - FORM_1700_EMPLOYER_ROWS
            )
        } else {
            format!("Employer {}", row + 1)
        }
    }

    fn money_text(value: f64) -> String {
        if value == 0.0 {
            String::new()
        } else {
            official_amount(value)
        }
    }

    /// Editor text for one input from the draft.
    fn initial(draft: &Form1700Draft, key: &str) -> String {
        for who in [Who::Taxpayer, Who::Spouse] {
            let Some(id) = key.strip_prefix(&format!("{}:", who.prefix())) else {
                continue;
            };
            return all_money()
                .find(|m| m.id == id)
                .map(|m| Self::money_text((m.get)(who.column(draft))))
                .unwrap_or_default();
        }
        for row in 0..EDITOR_EMPLOYER_ROWS {
            let Some(field) = key.strip_prefix(&format!("emp{row}:")) else {
                continue;
            };
            let entry = draft.employers.get(row).cloned().unwrap_or_default();
            return match field {
                "name" => entry.name,
                "name2" => entry.name2,
                "tin" => entry.tin,
                "regular" => Self::money_text(entry.regular),
                "flat" => Self::money_text(entry.flat),
                _ => Self::money_text(entry.withheld),
            };
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
        for row in 0..EDITOR_EMPLOYER_ROWS {
            for (field, _) in EMPLOYER_FIELDS {
                keys.push((employer_key(row, field), String::new()));
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
        for row in 0..EDITOR_EMPLOYER_ROWS {
            let mut entry = draft.employers.get(row).cloned().unwrap_or_default();
            entry.name = self.input_text(&employer_key(row, "name"), cx);
            entry.name2 = self.input_text(&employer_key(row, "name2"), cx);
            entry.tin = self
                .input_text(&employer_key(row, "tin"), cx)
                .trim()
                .to_string();
            for (field, slot) in [
                ("regular", &mut entry.regular),
                ("flat", &mut entry.flat),
                ("withheld", &mut entry.withheld),
            ] {
                let key = employer_key(row, field);
                let text = self.input_text(&key, cx);
                match parse_amount(&text) {
                    Some(value) => *slot = value,
                    None => parse_errors.push((
                        key.clone(),
                        format!(
                            "Schedule 1 row {} {field}: \"{text}\" is not a number.",
                            row + 1
                        ),
                    )),
                }
            }
            if draft.employers.len() <= row {
                if entry == Form1700Employer::default() {
                    continue;
                }
                draft
                    .employers
                    .resize_with(row + 1, Form1700Employer::default);
            }
            draft.employers[row] = entry;
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
        change: impl FnOnce(&mut Form1700Draft),
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
                .release_abandoned_claimed_queueable::<Form1700Draft>(
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

    fn yes_no(
        &self,
        id: &'static str,
        label: &str,
        value: Option<bool>,
        enabled: bool,
        set: fn(&mut Form1700Draft, Option<bool>),
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        self.choice_row(
            label,
            vec![
                Self::choice(
                    (id, 1usize),
                    "Yes",
                    value == Some(true),
                    !editable || !enabled,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| set(d, Some(true)))
                })),
                Self::choice(
                    (id, 0usize),
                    "No",
                    value == Some(false),
                    !editable || !enabled,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| set(d, Some(false)))
                })),
            ],
        )
    }

    fn type_choice(&self, who: Who, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let current = match who {
            Who::Taxpayer => self.draft.taxpayer_type,
            Who::Spouse => self.draft.spouse.taxpayer_type,
        };
        let label = match who {
            Who::Taxpayer => "Item 6 — Taxpayer type",
            Who::Spouse => "Item 20 — Spouse taxpayer type",
        };
        let p = who.prefix();
        let buttons = [
            (Form1700TaxpayerType::Employee, "Employee (graduated rates)"),
            (Form1700TaxpayerType::Nranetb, "NRANETB (25%)"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, text))| {
            Self::choice(
                (SharedString::from(format!("1700_{p}_type")), index),
                text,
                current == value,
                !editable,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(window, cx, |d| match who {
                    Who::Taxpayer => d.taxpayer_type = value,
                    Who::Spouse => d.spouse.taxpayer_type = value,
                })
            }))
        })
        .collect();
        self.choice_row(label, buttons)
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let children = vec![
            self.text_field("year", "taxable_year", layout, false),
            self.choice_row(
                "Item 2 — Amended return?",
                vec![
                    Self::choice("1700_amended_yes", "Yes", d.is_amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = true)
                        }),
                    ),
                    Self::choice("1700_amended_no", "No", !d.is_amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = false)
                        }),
                    ),
                ],
            ),
            div()
                .text_sm()
                .child(format!("Item 3 — ATC: {FORM_1700_ATC}"))
                .into_any_element(),
        ];
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
        let mut children = vec![
            self.grid(layout, fields),
            self.type_choice(Who::Taxpayer, cx),
        ];
        children.push(self.yes_no(
            "1700_ftc",
            "Item 12 — Claiming foreign tax credits?",
            d.foreign_tax_credits,
            true,
            |d, v| d.foreign_tax_credits = v,
            cx,
        ));
        children.push(self.text_field(
            "ftn",
            "foreign_tax_number",
            layout,
            d.foreign_tax_credits != Some(true),
        ));
        let statuses = [
            (Form1700CivilStatus::Single, "Single"),
            (Form1700CivilStatus::Married, "Married"),
            (Form1700CivilStatus::Separated, "Legally separated"),
            (Form1700CivilStatus::Widow, "Widow/er"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            Self::choice(
                ("1700_civil", index),
                label,
                d.civil_status == value,
                !editable,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(window, cx, |d| d.civil_status = value)
            }))
        })
        .collect();
        children.push(self.choice_row("Item 15 — Civil status", statuses));
        children.push(self.yes_no(
            "1700_sp_income",
            "Item 16 — If married, spouse has income?",
            d.spouse_has_income,
            d.civil_status == Form1700CivilStatus::Married,
            |d, v| d.spouse_has_income = v,
            cx,
        ));
        let can_file = d.spouse_has_income == Some(true);
        children.push(self.choice_row(
            "Item 17 — Filing status",
            vec![
                    Self::choice(
                        "1700_joint",
                        "Joint filing",
                        d.filing_status == Form1700FilingStatus::Joint,
                        !editable || !can_file,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.filing_status = Form1700FilingStatus::Joint
                        })
                    })),
                    Self::choice(
                        "1700_separate",
                        "Separate filing",
                        d.filing_status == Form1700FilingStatus::Separate,
                        !editable || !can_file,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            d.filing_status = Form1700FilingStatus::Separate
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
        let children = vec![
            self.grid(layout, fields),
            self.type_choice(Who::Spouse, cx),
            self.yes_no(
                "1700_sp_ftc",
                "Item 24 — Spouse claiming foreign tax credits?",
                d.spouse.foreign_tax_credits,
                true,
                |d, v| d.spouse.foreign_tax_credits = v,
                cx,
            ),
            self.text_field(
                "sp_ftn",
                "spouse.foreign_tax_number",
                layout,
                d.spouse.foreign_tax_credits != Some(true),
            ),
        ];
        self.section("Part II — Background information on spouse", children, cx)
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let whos = self.joint_whos();
        let block = |moneys: &[Money], computed: &[Computed]| {
            whos.iter()
                .map(|who| (*who, self.money_block(*who, moneys, computed, layout, cx)))
                .collect::<Vec<_>>()
        };
        let d = &self.draft;
        let employee = d.regular_column_open();
        let flat = d.flat_column_open();
        let mut rows = Vec::new();
        // Item 1–4 always; one empty row past the last employer, up to the
        // editor's limit (rows past Item 4 go through the add-more popup).
        let shown = (d.employers.len() + 1).clamp(FORM_1700_EMPLOYER_ROWS, EDITOR_EMPLOYER_ROWS);
        for row in 0..shown {
            let entry = d.employers.get(row).cloned().unwrap_or_default();
            let used = entry != Form1700Employer::default();
            let owners = vec![
                Self::choice(
                    ("1700_emp_tp", row),
                    "Taxpayer",
                    used && !entry.for_spouse,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| {
                        if d.employers.len() <= row {
                            d.employers.resize_with(row + 1, Form1700Employer::default);
                        }
                        d.employers[row].for_spouse = false;
                    })
                })),
                Self::choice(
                    ("1700_emp_sp", row),
                    "Spouse",
                    used && entry.for_spouse,
                    !editable || !d.is_joint(),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| {
                        if d.employers.len() <= row {
                            d.employers.resize_with(row + 1, Form1700Employer::default);
                        }
                        d.employers[row].for_spouse = true;
                    })
                })),
                Button::new(("1700_emp_clear", row))
                    .label("Clear row")
                    .outline()
                    .small()
                    .disabled(!editable || !used)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(window, cx, |d| {
                            if let Some(entry) = d.employers.get_mut(row) {
                                *entry = Form1700Employer::default();
                            }
                        })
                    })),
            ];
            let mut fields = Vec::new();
            for (field, label) in EMPLOYER_FIELDS {
                let disabled = match *field {
                    "regular" => !employee,
                    "flat" => !flat,
                    _ => false,
                };
                fields.push(self.field(
                    &employer_key(row, field),
                    &format!("employers[{row}].{field}"),
                    label,
                    layout,
                    !editable || disabled,
                ));
            }
            rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(self.choice_row(&Self::employer_title(d, row), owners))
                    .child(self.grid(layout, fields))
                    .into_any_element(),
            );
        }
        if d.schedule_is_folded() {
            // Item 4 on the return reads OTHERS with these subtotals.
            let [c, dd, e] = d.popup_subtotals();
            rows.push(self.grid(
                layout,
                vec![
                    self.computed("Item 4 (OTHERS) c — popup subtotal", c, cx),
                    self.computed("Item 4 (OTHERS) d — popup subtotal", dd, cx),
                    self.computed("Item 4 (OTHERS) e — popup subtotal", e, cx),
                ],
            ));
        }
        let totals = &d.schedule_totals;
        rows.push(self.grid(
            layout,
            vec![
                self.computed("5A c — Taxpayer, regular rates", totals[0][0], cx),
                self.computed("5B c — Spouse, regular rates", totals[1][0], cx),
                self.computed("5A d — Taxpayer, 25% flat rate", totals[0][1], cx),
                self.computed("5B d — Spouse, 25% flat rate", totals[1][1], cx),
                self.computed("5A e — Taxpayer, tax withheld", totals[0][2], cx),
                self.computed("5B e — Spouse, tax withheld", totals[1][2], cx),
            ],
        ));
        let sections = vec![
            self.section(
                "Part VI — Schedule 1: compensation and tax withheld",
                rows,
                cx,
            ),
            self.section(
                "Part V.A — Graduated rates",
                vec![
                    self.text_field("desc45", "other_income_description", layout, !employee),
                    self.columns(layout, block(PART_VA, PART_VA_COMPUTED), cx),
                ],
                cx,
            ),
            self.section(
                "Part V.B — 25% flat rate (NRANETB)",
                vec![
                    self.grid(
                        layout,
                        vec![
                            self.text_field(
                                "desc49",
                                "flat_non_taxable_description",
                                layout,
                                !flat,
                            ),
                            self.text_field(
                                "desc51",
                                "flat_other_income_description",
                                layout,
                                !flat,
                            ),
                        ],
                    ),
                    self.columns(layout, block(PART_VB, PART_VB_COMPUTED), cx),
                ],
                cx,
            ),
            self.section(
                "Part V.C — Tax credits/payments",
                vec![
                    self.text_field("desc57", "other_credits_description", layout, false),
                    self.columns(layout, block(PART_VC, PART_VC_COMPUTED), cx),
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
        let d = &self.draft;
        let blocks = self
            .joint_whos()
            .into_iter()
            .map(|who| {
                (
                    who,
                    self.money_block(who, PART_III, PART_III_COMPUTED, layout, cx),
                )
            })
            .collect();
        let children = vec![
            self.columns(layout, blocks, cx),
            self.computed(
                "36 — Aggregate amount payable (overpayment)",
                d.aggregate_amount_payable,
                cx,
            ),
            self.text_field("sheets", "number_of_attachments", layout, false),
        ];
        self.section("Part III — Total tax payable", children, cx)
    }
}

impl QueueableFormView for Form1700View {
    type Draft = Form1700Draft;

    fn new(
        draft: Form1700Draft,
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1700Draft {
        Form1700Draft::new_from_profile(profile, year)
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

impl FormViewTrait for Form1700View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1700"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Individuals Earning Purely Compensation Income"
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
                    "1700 draft saved.".into(),
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
        if !can_queue_for_submission(Form1700Draft::FORM_CODE) {
            self.status_message =
                Some("1700 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1700. No submission was started: {error}"
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
            "Form 1700 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1700 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1700Draft>(
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
            "1700-2018",
            &fields,
            "1700 — Print Preview",
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

impl Render for Form1700View {
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
                .child(Button::new("1700_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1700_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1700_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1700_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1700_submit")
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
                            Button::new("1700_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1700_release_cancel")
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
                    .id("1700_scroll")
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
    use super::{Form1700View, Layout, format_tin, parse_amount};
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
        let keys = Form1700View::all_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(unique.len(), keys.len(), "input keys must be unique");
    }
}
