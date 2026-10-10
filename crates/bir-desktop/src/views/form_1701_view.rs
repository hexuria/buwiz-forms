//! Editor for BIR Form 1701, Annual Income Tax Return for Individuals,
//! Estates and Trusts (January 2018). Rust owns every calculation, validation
//! and the official submit plaintext (`bir_core::forms::form_1701_official`);
//! this view only edits source values. The layout reflows for desktop, tablet
//! and phone widths; the spouse column appears for joint filing.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1701::{
    Form1701AmountSection as Sec, Form1701Atc, Form1701CivilStatus, Form1701DeductionMethod,
    Form1701Draft, Form1701JointFilingStatus, Form1701OverpaymentDisposition, Form1701Party,
    Form1701SpouseType, Form1701TaxpayerType,
};
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingPeriod, FilingStatus, can_queue_for_submission};
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

impl EventEmitter<QueueableFormEvent> for Form1701View {}

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

const TP: Form1701Party = Form1701Party::Taxpayer;
const SP: Form1701Party = Form1701Party::Spouse;

/// Free-text inputs: (key, label).
const TEXT_INPUTS: &[(&str, &str)] = &[
    ("year", "Item 1 — Year"),
    ("month", "Item 1 — Month (short period)"),
    ("name", "Item 8 — Taxpayer's Name"),
    ("address", "Item 9 — Registered Address"),
    ("zip", "Item 9A — ZIP Code"),
    ("birth", "Item 10 — Date of Birth (MM/DD/YYYY)"),
    ("email", "Item 11 — Email Address"),
    ("citizenship", "Item 12 — Citizenship"),
    ("foreign_no", "Item 14 — Foreign Tax Number"),
    ("phone", "Item 15 — Contact Number"),
    ("attachments", "Item 33 — Number of attachments"),
    ("sp_tin", "Item 1 — Spouse TIN"),
    ("sp_rdo", "Item 2 — Spouse RDO Code"),
    ("sp_name", "Item 5 — Spouse's Name"),
    ("sp_phone", "Item 6 — Contact Number"),
    ("sp_citizenship", "Item 7 — Citizenship"),
    ("sp_foreign_no", "Item 9 — Foreign Tax Number"),
    ("d19", "Item 19 — Other taxable income (specify)"),
    ("d20", "Item 20 — Other taxable income (specify)"),
    ("d27", "Item 27 — Other non-operating income (specify)"),
    ("d17d", "Schedule 4 Item 17d — Others (specify)"),
    ("d7_9", "Part VII Item 9 — Others (specify)"),
];

/// Two-column amount inputs: (section, item, label).
const PAIR_INPUTS: &[(Sec, u8, &str)] = &[
    (
        Sec::Schedule2,
        5,
        "5 — Less: non-taxable/exempt compensation",
    ),
    (Sec::Schedule3, 8, "8 — Sales/revenues/receipts/fees"),
    (
        Sec::Schedule3,
        9,
        "9 — Less: sales returns, allowances and discounts",
    ),
    (
        Sec::Schedule3,
        11,
        "11 — Less: cost of sales/services (itemized)",
    ),
    (Sec::Schedule3, 19, "19 — Other taxable income"),
    (Sec::Schedule3, 20, "20 — Other taxable income"),
    (Sec::Schedule3, 21, "21 — Share in the net income of a GPP"),
    (Sec::Schedule3, 26, "26 — Sales/revenues/receipts/fees"),
    (Sec::Schedule3, 27, "27 — Other non-operating income"),
    (
        Sec::Schedule3,
        29,
        "29 — Allowable reduction (P250,000, pure business/profession)",
    ),
    (Sec::Schedule4, 1, "1 — Amortizations"),
    (Sec::Schedule4, 2, "2 — Bad debts"),
    (Sec::Schedule4, 3, "3 — Charitable and other contributions"),
    (Sec::Schedule4, 4, "4 — Depletion"),
    (Sec::Schedule4, 5, "5 — Depreciation"),
    (
        Sec::Schedule4,
        6,
        "6 — Entertainment, amusement and recreation",
    ),
    (Sec::Schedule4, 7, "7 — Fringe benefits"),
    (Sec::Schedule4, 8, "8 — Interest"),
    (Sec::Schedule4, 9, "9 — Losses"),
    (Sec::Schedule4, 10, "10 — Pension trust"),
    (Sec::Schedule4, 11, "11 — Rental"),
    (Sec::Schedule4, 12, "12 — Research and development"),
    (Sec::Schedule4, 13, "13 — Salaries, wages and allowances"),
    (
        Sec::Schedule4,
        14,
        "14 — SSS, GSIS, PhilHealth, HDMF and other contributions",
    ),
    (Sec::Schedule4, 15, "15 — Taxes and licenses"),
    (Sec::Schedule4, 16, "16 — Transportation and travel"),
    (Sec::Schedule6, 1, "1 — Gross income"),
    (Sec::Schedule6, 2, "2 — Less: deductions"),
    (
        Sec::PartVi,
        2,
        "2 — Add: income tax due on special/exempt income",
    ),
    (Sec::PartVi, 3, "3 — Less: allowable tax relief"),
    (Sec::PartVii, 1, "1 — Prior year's excess credits"),
    (
        Sec::PartVii,
        2,
        "2 — Tax payments for the first three quarters",
    ),
    (
        Sec::PartVii,
        3,
        "3 — Creditable tax withheld for the first three quarters",
    ),
    (
        Sec::PartVii,
        4,
        "4 — Creditable tax withheld per BIR Form 2307 (4th quarter)",
    ),
    (
        Sec::PartVii,
        6,
        "6 — Tax paid in return previously filed (amended)",
    ),
    (Sec::PartVii, 7, "7 — Foreign tax credits"),
    (Sec::PartVii, 8, "8 — Tax paid on special/exempt income"),
    (Sec::PartVii, 9, "9 — Other tax credits/payments"),
    (Sec::PartIx, 1, "1 — Net income/(loss) per books"),
    (
        Sec::PartIx,
        2,
        "2 — Add: non-deductible expenses/taxable other income",
    ),
    (
        Sec::PartIx,
        3,
        "3 — Add: non-deductible expenses/taxable other income",
    ),
    (
        Sec::PartIx,
        4,
        "4 — Add: non-deductible expenses/taxable other income",
    ),
    (
        Sec::PartIx,
        6,
        "6 — Less: non-taxable income and income subjected to final tax",
    ),
    (
        Sec::PartIx,
        7,
        "7 — Less: non-taxable income and income subjected to final tax",
    ),
    (Sec::PartIx, 8, "8 — Less: special deductions"),
    (Sec::PartIx, 9, "9 — Less: special deductions"),
    (
        Sec::PartIi,
        25,
        "25 — Less: portion of tax payable allowed for 2nd installment",
    ),
    (Sec::PartIi, 27, "27 — Interest"),
    (Sec::PartIi, 28, "28 — Surcharge"),
    (Sec::PartIi, 29, "29 — Compromise"),
];

const IX_PARTICULARS: [u8; 7] = [2, 3, 4, 6, 7, 8, 9];

fn section_code(section: Sec) -> &'static str {
    match section {
        Sec::PartIi => "p2",
        Sec::Schedule2 => "s2",
        Sec::Schedule3 => "s3",
        Sec::Schedule4 => "s4",
        Sec::Schedule6 => "s6",
        Sec::PartVi => "p6",
        Sec::PartVii => "p7",
        Sec::PartViii => "p8",
        Sec::PartIx => "p9",
    }
}

fn pair_key(section: Sec, item: u8, party: Form1701Party) -> String {
    let column = if party == TP { "a" } else { "b" };
    format!("{}_{item}_{column}", section_code(section))
}

fn party_key(prefix: &str, party: Form1701Party) -> String {
    format!("{prefix}_{}", if party == TP { "a" } else { "b" })
}

/// Every editor key besides the fixed text inputs.
fn dynamic_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for party in [TP, SP] {
        for (section, item, _) in PAIR_INPUTS {
            keys.push(pair_key(*section, *item, party));
        }
        for index in 0..4 {
            keys.push(party_key(&format!("s4_17_{index}"), party));
        }
        for row in 0..2 {
            for column in ["desc", "legal", "amt"] {
                keys.push(party_key(&format!("s5_{row}_{column}"), party));
            }
        }
        for row in 0..4 {
            for column in ["year", "a", "b", "c", "d"] {
                keys.push(party_key(&format!("s6_{row}_{column}"), party));
            }
        }
    }
    for row in 0..2 {
        for column in ["name", "tin", "ci", "tw"] {
            keys.push(format!("emp_{row}_{column}"));
        }
    }
    for item in IX_PARTICULARS {
        keys.push(format!("p9d_{item}"));
    }
    keys
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

fn money_text(value: Option<f64>) -> String {
    match value {
        Some(v) if v != 0.0 => official_amount(v),
        _ => String::new(),
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

pub struct Form1701View {
    draft: Form1701Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1701View {
    /// Editor text for one input from the draft.
    fn initial(d: &Form1701Draft, key: &str) -> String {
        let descs = &d.computations.schedule_3_descriptions;
        let desc = |item: u8| descs.get(&item).cloned().unwrap_or_default();
        match key {
            "year" => return d.taxable_year.to_string(),
            "month" => return d.period_end_month.to_string(),
            "name" => return d.taxpayer_name.clone(),
            "address" => return d.registered_address.clone(),
            "zip" => return d.zip_code.clone(),
            "birth" => return d.date_of_birth.clone(),
            "email" => return d.email.clone(),
            "citizenship" => return d.citizenship.clone(),
            "foreign_no" => return d.foreign_tax_number.clone(),
            "phone" => return d.contact_number.clone(),
            "attachments" => {
                return d
                    .number_of_attachments
                    .filter(|n| *n != 0)
                    .map(|n| n.to_string())
                    .unwrap_or_default();
            }
            "sp_tin" => return d.spouse.tin.clone(),
            "sp_rdo" => return d.spouse_rdo_code.clone(),
            "sp_name" => return d.spouse.name.clone(),
            "sp_phone" => return d.spouse.contact_number.clone(),
            "sp_citizenship" => return d.spouse.citizenship.clone(),
            "sp_foreign_no" => return d.spouse.foreign_tax_number.clone(),
            "d19" => return desc(19),
            "d20" => return desc(20),
            "d27" => return desc(27),
            "d17d" => return d.computations.schedule_4_item_17d_description.clone(),
            "d7_9" => return d.computations.part_vii_item_9_description.clone(),
            _ => {}
        }
        for party in [TP, SP] {
            for (section, item, _) in PAIR_INPUTS {
                if key == pair_key(*section, *item, party) {
                    return money_text(d.amount(*section, *item, party));
                }
            }
            for index in 0..4 {
                if key == party_key(&format!("s4_17_{index}"), party) {
                    return money_text(d.computations.schedule_4_item_17[index].value(party));
                }
            }
            let rows = if party == TP {
                &d.computations.schedule_5_taxpayer
            } else {
                &d.computations.schedule_5_spouse
            };
            for (row, entry) in rows.iter().enumerate() {
                for (column, value) in [
                    ("desc", entry.description.clone()),
                    ("legal", entry.legal_basis.clone()),
                    ("amt", money_text(entry.amount)),
                ] {
                    if key == party_key(&format!("s5_{row}_{column}"), party) {
                        return value;
                    }
                }
            }
            let nolco = if party == TP {
                &d.computations.schedule_6_taxpayer_nolco
            } else {
                &d.computations.schedule_6_spouse_nolco
            };
            for (row, entry) in nolco.iter().enumerate() {
                for (column, value) in [
                    ("year", entry.year_incurred.clone()),
                    ("a", money_text(entry.amount)),
                    ("b", money_text(entry.applied_previous_years)),
                    ("c", money_text(entry.expired)),
                    ("d", money_text(entry.applied_current_year)),
                ] {
                    if key == party_key(&format!("s6_{row}_{column}"), party) {
                        return value;
                    }
                }
            }
        }
        for (row, entry) in d.employers.iter().enumerate() {
            for (column, value) in [
                ("name", entry.employer_name.clone()),
                ("tin", entry.employer_tin.clone()),
                ("ci", money_text(entry.compensation_income)),
                ("tw", money_text(entry.tax_withheld)),
            ] {
                if key == format!("emp_{row}_{column}") {
                    return value;
                }
            }
        }
        for item in IX_PARTICULARS {
            if key == format!("p9d_{item}") {
                return d
                    .computations
                    .part_ix_descriptions
                    .get(&item)
                    .cloned()
                    .unwrap_or_default();
            }
        }
        String::new()
    }

    fn text(&self, key: &str, cx: &App) -> String {
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
        let mut errors: Vec<(String, String)> = Vec::new();
        let mut d = self.draft.clone();
        let number = |text: String, label: &str, errors: &mut Vec<(String, String)>| {
            let parsed = parse_amount(&text);
            if parsed.is_none() {
                errors.push((
                    label.to_string(),
                    format!("{label}: \"{text}\" is not a number."),
                ));
            }
            parsed
        };
        if let Some(year) = number(self.text("year", cx), "Item 1 year", &mut errors) {
            d.taxable_year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
        }
        if d.is_short_period
            && let Some(month) = number(self.text("month", cx), "Item 1 month", &mut errors)
        {
            d.period_end_month = if (1.0..=12.0).contains(&month) {
                month as u8
            } else {
                0
            };
        }
        d.taxpayer_name = self.text("name", cx);
        d.registered_address = self.text("address", cx);
        d.zip_code = self.text("zip", cx).trim().to_string();
        d.date_of_birth = self.text("birth", cx).trim().to_string();
        d.email = self.text("email", cx).trim().to_string();
        d.citizenship = self.text("citizenship", cx);
        d.foreign_tax_number = self.text("foreign_no", cx);
        d.contact_number = self.text("phone", cx).trim().to_string();
        if let Some(count) = number(self.text("attachments", cx), "Item 33", &mut errors) {
            d.number_of_attachments = Some(if (0.0..=99.0).contains(&count) {
                count as u8
            } else {
                99
            });
        }
        d.spouse.tin = self.text("sp_tin", cx).trim().to_string();
        d.spouse_rdo_code = self.text("sp_rdo", cx).trim().to_string();
        d.spouse.name = self.text("sp_name", cx);
        d.spouse.contact_number = self.text("sp_phone", cx).trim().to_string();
        d.spouse.citizenship = self.text("sp_citizenship", cx);
        d.spouse.foreign_tax_number = self.text("sp_foreign_no", cx);
        for (item, key) in [(19u8, "d19"), (20, "d20"), (27, "d27")] {
            d.computations
                .schedule_3_descriptions
                .insert(item, self.text(key, cx));
        }
        d.computations.schedule_4_item_17d_description = self.text("d17d", cx);
        d.computations.part_vii_item_9_description = self.text("d7_9", cx);
        for item in IX_PARTICULARS {
            d.computations
                .part_ix_descriptions
                .insert(item, self.text(&format!("p9d_{item}"), cx));
        }
        for party in [TP, SP] {
            for (section, item, label) in PAIR_INPUTS {
                if let Some(value) = number(
                    self.text(&pair_key(*section, *item, party), cx),
                    label,
                    &mut errors,
                ) {
                    d.set_amount(*section, *item, party, Some(value));
                }
            }
            for index in 0..4 {
                if let Some(value) = number(
                    self.text(&party_key(&format!("s4_17_{index}"), party), cx),
                    "Schedule 4 Item 17",
                    &mut errors,
                ) {
                    d.computations.schedule_4_item_17[index].set(party, Some(value));
                }
            }
            for row in 0..2 {
                let description = self.text(&party_key(&format!("s5_{row}_desc"), party), cx);
                let legal = self.text(&party_key(&format!("s5_{row}_legal"), party), cx);
                let amount = number(
                    self.text(&party_key(&format!("s5_{row}_amt"), party), cx),
                    "Schedule 5",
                    &mut errors,
                );
                let rows = if party == TP {
                    &mut d.computations.schedule_5_taxpayer
                } else {
                    &mut d.computations.schedule_5_spouse
                };
                rows[row].description = description;
                rows[row].legal_basis = legal;
                if let Some(amount) = amount {
                    rows[row].amount = Some(amount);
                }
            }
            for row in 0..4 {
                let year = self.text(&party_key(&format!("s6_{row}_year"), party), cx);
                let mut values = [None; 4];
                for (slot, column) in ["a", "b", "c", "d"].iter().enumerate() {
                    values[slot] = number(
                        self.text(&party_key(&format!("s6_{row}_{column}"), party), cx),
                        "Schedule 6 NOLCO",
                        &mut errors,
                    );
                }
                let rows = if party == TP {
                    &mut d.computations.schedule_6_taxpayer_nolco
                } else {
                    &mut d.computations.schedule_6_spouse_nolco
                };
                let entry = &mut rows[row];
                if row < 3 {
                    entry.year_incurred = year.trim().to_string();
                    if let Some(v) = values[0] {
                        entry.amount = Some(v);
                    }
                }
                if let Some(v) = values[1] {
                    entry.applied_previous_years = Some(v);
                }
                if let Some(v) = values[2] {
                    entry.expired = Some(v);
                }
                if let Some(v) = values[3] {
                    entry.applied_current_year = Some(v);
                }
            }
        }
        for row in 0..2 {
            d.employers[row].employer_name = self.text(&format!("emp_{row}_name"), cx);
            d.employers[row].employer_tin =
                self.text(&format!("emp_{row}_tin"), cx).trim().to_string();
            if let Some(v) = number(
                self.text(&format!("emp_{row}_ci"), cx),
                "Schedule 1",
                &mut errors,
            ) {
                d.employers[row].compensation_income = Some(v);
            }
            if let Some(v) = number(
                self.text(&format!("emp_{row}_tw"), cx),
                "Schedule 1",
                &mut errors,
            ) {
                d.employers[row].tax_withheld = Some(v);
            }
        }
        d.recompute();
        d.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = d.validate();
        self.parse_errors = errors;
        self.draft = d;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1701Draft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
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
                .release_abandoned_claimed_queueable::<Form1701Draft>(
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

    fn label(text: &str) -> Div {
        div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(text.to_string())
    }

    /// A labelled input. On phones the label sits above the field.
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let editable = self.draft.lifecycle.is_editable();
        let body = div().child(Input::new(input).disabled(!editable || disabled));
        match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(Self::label(label))
                .child(body.w_full()),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(Self::label(label).w(relative(0.5)))
                .child(body.w(relative(0.5))),
        }
        .into_any_element()
    }

    fn text_field(&self, key: &str, layout: Layout) -> AnyElement {
        let label = TEXT_INPUTS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, l)| *l)
            .unwrap_or("");
        self.field(key, label, layout, false)
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

    fn joint(&self) -> bool {
        self.draft.spouse_column_active()
    }

    fn amount_cell(&self, value: f64, cx: &Context<Self>) -> AnyElement {
        div()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted.opacity(0.5))
            .text_right()
            .font_weight(FontWeight::BOLD)
            .child(official_amount(value))
            .into_any_element()
    }

    /// One item row: label, then the taxpayer (A) and, for joint filing,
    /// spouse (B) column. Phones stack the columns under the label.
    fn item_row(&self, label: &str, cells: Vec<AnyElement>, layout: Layout) -> AnyElement {
        let joint = self.joint();
        let mut cells = cells.into_iter();
        let a = cells.next().unwrap_or_else(|| div().into_any_element());
        let b = cells.next().unwrap_or_else(|| div().into_any_element());
        if layout == Layout::Phone {
            let mut col = div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(Self::label(label));
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().w(px(20.)).child("A"))
                    .child(div().flex_1().child(a)),
            );
            if joint {
                col = col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_xs().w(px(20.)).child("B"))
                        .child(div().flex_1().child(b)),
                );
            }
            col.into_any_element()
        } else {
            let mut row = div()
                .flex()
                .items_center()
                .gap_3()
                .w_full()
                .child(Self::label(label).flex_1())
                .child(div().w(px(220.)).child(a));
            if joint {
                row = row.child(div().w(px(220.)).child(b));
            }
            row.into_any_element()
        }
    }

    fn input_row(
        &self,
        section: Sec,
        item: u8,
        label: &str,
        layout: Layout,
        disabled: [bool; 2],
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let cells = [TP, SP]
            .iter()
            .zip(disabled)
            .map(|(party, off)| {
                let key = pair_key(section, item, *party);
                let input = self
                    .inputs
                    .get(&key)
                    .expect("editor input registry is complete");
                Input::new(input)
                    .disabled(!editable || off)
                    .into_any_element()
            })
            .collect();
        self.item_row(label, cells, layout)
    }

    fn computed_row(
        &self,
        section: Sec,
        item: u8,
        label: &str,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let cells = [TP, SP]
            .iter()
            .map(|party| {
                self.amount_cell(self.draft.amount(section, item, *party).unwrap_or(0.0), cx)
            })
            .collect();
        self.item_row(label, cells, layout)
    }

    fn column_header(&self, layout: Layout) -> AnyElement {
        if layout == Layout::Phone {
            return div().into_any_element();
        }
        let mut row = div()
            .flex()
            .gap_3()
            .w_full()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .child(div().flex_1())
            .child(div().w(px(220.)).text_right().child("A — Taxpayer/Filer"));
        if self.joint() {
            row = row.child(div().w(px(220.)).text_right().child("B — Spouse"));
        }
        row.into_any_element()
    }

    fn label_for(section: Sec, item: u8) -> &'static str {
        PAIR_INPUTS
            .iter()
            .find(|(s, i, _)| *s == section && *i == item)
            .map(|(_, _, l)| *l)
            .unwrap_or("")
    }

    fn party_flags(&self, test: impl Fn(&Form1701Draft, Form1701Party) -> bool) -> [bool; 2] {
        [!test(&self.draft, TP), !test(&self.draft, SP)]
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

    fn choice_row(title: &str, buttons: Vec<Button>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(Self::label(title))
            .children(buttons)
            .into_any_element()
    }

    fn yes_no(
        &self,
        id: &'static str,
        title: &str,
        value: Option<bool>,
        set: fn(&mut Form1701Draft, bool),
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        Self::choice_row(
            title,
            vec![
                Self::choice((id, 1usize), "Yes", value == Some(true), !editable)
                    .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| set(d, true)))),
                Self::choice((id, 0usize), "No", value == Some(false), !editable)
                    .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| set(d, false)))),
            ],
        )
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children = vec![
            self.text_field("year", layout),
            self.yes_no(
                "1701_amended",
                "Item 2 — Amended return?",
                Some(d.is_amended),
                |d, v| d.is_amended = v,
                cx,
            ),
            self.yes_no(
                "1701_short",
                "Item 3 — Short period return?",
                Some(d.is_short_period),
                |d, v| d.is_short_period = v,
                cx,
            ),
        ];
        if d.is_short_period {
            children.push(self.text_field("month", layout));
        }
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_background(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fixed = |label: &str, value: String| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Self::label(label))
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element()
        };
        let types: Vec<Button> = Form1701TaxpayerType::ALL
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                Self::choice(
                    ("1701_type", index),
                    kind.label(),
                    d.taxpayer_type == Some(kind),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.taxpayer_type = Some(kind))
                }))
            })
            .collect();
        let mixed_possible = matches!(
            d.taxpayer_type,
            Some(Form1701TaxpayerType::SingleProprietor | Form1701TaxpayerType::Professional)
        );
        let atcs: Vec<Button> = Form1701Atc::ALL
            .into_iter()
            .enumerate()
            .map(|(index, atc)| {
                Self::choice(
                    ("1701_atc", index),
                    format!("{} — {}", atc.code(), atc.label()),
                    d.atc == Some(atc),
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.atc = Some(atc))))
            })
            .collect();
        let statuses: Vec<Button> = Form1701CivilStatus::ALL
            .into_iter()
            .enumerate()
            .map(|(index, status)| {
                Self::choice(
                    ("1701_civil", index),
                    status.label(),
                    d.civil_status == Some(status),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.civil_status = Some(status))
                }))
            })
            .collect();
        let mut rows = vec![
            self.grid(
                layout,
                vec![
                    fixed("Item 4 — TIN", format_tin(&d.tin)),
                    fixed("Item 5 — RDO Code", d.rdo_code.clone()),
                    self.text_field("name", layout),
                    self.text_field("address", layout),
                    self.text_field("zip", layout),
                    self.text_field("birth", layout),
                    self.text_field("email", layout),
                    self.text_field("citizenship", layout),
                    self.text_field("phone", layout),
                ],
            ),
            Self::choice_row("Item 6 — Taxpayer type", types),
        ];
        if mixed_possible {
            rows.push(self.yes_no(
                "1701_mixed",
                "Also a compensation earner (mixed income)?",
                Some(d.taxpayer_also_compensation_earner),
                |d, v| d.taxpayer_also_compensation_earner = v,
                cx,
            ));
        }
        rows.push(Self::choice_row("Item 7 — ATC", atcs));
        rows.push(self.yes_no(
            "1701_ftc",
            "Item 13 — Claiming foreign tax credits?",
            d.claims_foreign_tax_credits,
            |d, v| d.claims_foreign_tax_credits = Some(v),
            cx,
        ));
        if d.claims_foreign_tax_credits == Some(true) {
            rows.push(self.text_field("foreign_no", layout));
        }
        rows.push(Self::choice_row("Item 16 — Civil status", statuses));
        if d.civil_status == Some(Form1701CivilStatus::Married) {
            rows.push(self.yes_no(
                "1701_spouse_income",
                "Item 17 — Spouse has income?",
                d.spouse_has_income,
                |d, v| d.spouse_has_income = Some(v),
                cx,
            ));
            if d.spouse_has_income == Some(true) {
                let joint = d.joint_filing_status;
                rows.push(Self::choice_row(
                    "Item 18 — Filing status",
                    vec![
                        Self::choice(
                            "1701_joint",
                            "Joint filing",
                            joint == Some(Form1701JointFilingStatus::Joint),
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.joint_filing_status = Some(Form1701JointFilingStatus::Joint)
                            })
                        })),
                        Self::choice(
                            "1701_separate",
                            "Separate filing",
                            joint == Some(Form1701JointFilingStatus::Separate),
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.joint_filing_status = Some(Form1701JointFilingStatus::Separate)
                            })
                        })),
                    ],
                ));
            }
        }
        rows.push(self.yes_no(
            "1701_exempt",
            "Item 19 — Has income exempt from income tax?",
            d.has_exempt_income,
            |d, v| d.has_exempt_income = Some(v),
            cx,
        ));
        rows.push(self.yes_no(
            "1701_special",
            "Item 20 — Has income subject to special/preferential rates?",
            d.has_special_rate_income,
            |d, v| d.has_special_rate_income = Some(v),
            cx,
        ));
        if d.tax_rate == Some(bir_core::forms::form_1701::Form1701TaxRate::Graduated) {
            rows.push(Self::choice_row(
                "Item 21A — Method of deduction",
                Form1701DeductionMethod::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, method)| {
                        Self::choice(
                            ("1701_method", index),
                            method.label(),
                            d.deduction_method == Some(method),
                            !editable,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.deduction_method = Some(method))
                        }))
                    })
                    .collect(),
            ));
        }
        self.section("Part I — Background Information (Taxpayer/Filer)", rows, cx)
    }

    fn render_spouse(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let sp = &self.draft.spouse;
        let types: Vec<Button> = [
            Form1701SpouseType::SingleProprietor,
            Form1701SpouseType::Professional,
            Form1701SpouseType::CompensationEarner,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            Self::choice(
                ("1701_sp_type", index),
                kind.label(),
                sp.filer_type == Some(kind),
                !editable,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.edit(cx, |d| d.spouse.filer_type = Some(kind))
            }))
        })
        .collect();
        let atcs: Vec<Button> = Form1701Atc::ALL
            .into_iter()
            .enumerate()
            .map(|(index, atc)| {
                Self::choice(
                    ("1701_sp_atc", index),
                    format!("{} — {}", atc.code(), atc.label()),
                    sp.atc == Some(atc),
                    !editable,
                )
                .small()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.spouse.atc = Some(atc))),
                )
            })
            .collect();
        let mut rows = vec![
            self.grid(
                layout,
                vec![
                    self.text_field("sp_tin", layout),
                    self.text_field("sp_rdo", layout),
                    self.text_field("sp_name", layout),
                    self.text_field("sp_phone", layout),
                    self.text_field("sp_citizenship", layout),
                ],
            ),
            Self::choice_row("Item 3 — Spouse type", types),
        ];
        if matches!(
            sp.filer_type,
            Some(Form1701SpouseType::SingleProprietor | Form1701SpouseType::Professional)
        ) {
            rows.push(self.yes_no(
                "1701_sp_mixed",
                "Also a compensation earner (mixed income)?",
                Some(self.draft.spouse_also_compensation_earner),
                |d, v| d.spouse_also_compensation_earner = v,
                cx,
            ));
        }
        rows.push(Self::choice_row("Item 4 — ATC", atcs));
        rows.push(self.yes_no(
            "1701_sp_ftc",
            "Item 8 — Claiming foreign tax credits?",
            sp.claims_foreign_tax_credits,
            |d, v| d.spouse.claims_foreign_tax_credits = Some(v),
            cx,
        ));
        if sp.claims_foreign_tax_credits == Some(true) {
            rows.push(self.text_field("sp_foreign_no", layout));
        }
        rows.push(self.yes_no(
            "1701_sp_exempt",
            "Item 10 — Has income exempt from income tax?",
            sp.has_exempt_income,
            |d, v| d.spouse.has_exempt_income = Some(v),
            cx,
        ));
        rows.push(self.yes_no(
            "1701_sp_special",
            "Item 11 — Has income subject to special/preferential rates?",
            sp.has_special_rate_income,
            |d, v| d.spouse.has_special_rate_income = Some(v),
            cx,
        ));
        if sp.tax_rate == Some(bir_core::forms::form_1701::Form1701TaxRate::Graduated) {
            rows.push(Self::choice_row(
                "Item 12A — Method of deduction",
                Form1701DeductionMethod::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, method)| {
                        Self::choice(
                            ("1701_sp_method", index),
                            method.label(),
                            sp.deduction_method == Some(method),
                            !editable,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.spouse.deduction_method = Some(method))
                        }))
                    })
                    .collect(),
            ));
        }
        self.section("Part IV — Background Information (Spouse)", rows, cx)
    }

    fn render_employers(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..2 {
            let owner = self.draft.employers[row].owner;
            let mut owners = vec![
                Self::choice(
                    ("1701_emp_tp", row),
                    "Taxpayer",
                    owner == Some(TP),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.employers[row].owner = Some(TP))
                })),
            ];
            if self.joint() {
                owners.push(
                    Self::choice(("1701_emp_sp", row), "Spouse", owner == Some(SP), !editable)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.employers[row].owner = Some(SP))
                        })),
                );
            }
            owners.push(
                Self::choice(
                    ("1701_emp_none", row),
                    "Not used",
                    owner.is_none(),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.employers[row].owner = None)
                })),
            );
            let mut card = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(Self::choice_row(&format!("Employer {}", row + 1), owners));
            if owner.is_some() {
                card = card.child(self.grid(
                    layout,
                    vec![
                        self.field(
                            &format!("emp_{row}_name"),
                            "Name of employer",
                            layout,
                            false,
                        ),
                        self.field(&format!("emp_{row}_tin"), "Employer TIN", layout, false),
                        self.field(
                            &format!("emp_{row}_ci"),
                            "Compensation income",
                            layout,
                            false,
                        ),
                        self.field(&format!("emp_{row}_tw"), "Tax withheld", layout, false),
                    ],
                ));
            }
            children.push(card.into_any_element());
        }
        let no_comp = self.party_flags(|d, p| {
            let atc = if p == TP { d.atc } else { d.spouse.atc };
            matches!(
                atc,
                Some(Form1701Atc::Ii011 | Form1701Atc::Ii013 | Form1701Atc::Ii016)
            )
        });
        children.push(self.column_header(layout));
        children.push(self.computed_row(
            Sec::Schedule2,
            4,
            "4 — Gross compensation income (Schedule 1)",
            layout,
            cx,
        ));
        children.push(self.input_row(
            Sec::Schedule2,
            5,
            Self::label_for(Sec::Schedule2, 5),
            layout,
            no_comp,
        ));
        children.push(self.computed_row(
            Sec::Schedule2,
            6,
            "6 — Taxable compensation income",
            layout,
            cx,
        ));
        children.push(self.computed_row(
            Sec::Schedule2,
            7,
            "7 — Tax due (graduated rates)",
            layout,
            cx,
        ));
        self.section(
            "Part V — Schedules 1 and 2: Compensation Income",
            children,
            cx,
        )
    }

    fn graduated(&self, party: Form1701Party) -> bool {
        let atc = if party == TP {
            self.draft.atc
        } else {
            self.draft.spouse.atc
        };
        matches!(
            atc,
            Some(Form1701Atc::Ii012 | Form1701Atc::Ii013 | Form1701Atc::Ii014)
        )
    }

    fn eight(&self, party: Form1701Party) -> bool {
        let atc = if party == TP {
            self.draft.atc
        } else {
            self.draft.spouse.atc
        };
        matches!(
            atc,
            Some(Form1701Atc::Ii015 | Form1701Atc::Ii016 | Form1701Atc::Ii017)
        )
    }

    fn itemized(&self, party: Form1701Party) -> bool {
        let method = if party == TP {
            self.draft.deduction_method
        } else {
            self.draft.spouse.deduction_method
        };
        self.graduated(party) && method == Some(Form1701DeductionMethod::Itemized)
    }

    fn render_schedule_3(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let gr = [!self.graduated(TP), !self.graduated(SP)];
        let item = [!self.itemized(TP), !self.itemized(SP)];
        let eight = [!self.eight(TP), !self.eight(SP)];
        let pure = [
            !(self.eight(TP) && self.draft.atc != Some(Form1701Atc::Ii016)),
            !(self.eight(SP) && self.draft.spouse.atc != Some(Form1701Atc::Ii016)),
        ];
        let mut a = vec![self.column_header(layout)];
        a.push(self.input_row(
            Sec::Schedule3,
            8,
            Self::label_for(Sec::Schedule3, 8),
            layout,
            gr,
        ));
        a.push(self.input_row(
            Sec::Schedule3,
            9,
            Self::label_for(Sec::Schedule3, 9),
            layout,
            gr,
        ));
        a.push(self.computed_row(
            Sec::Schedule3,
            10,
            "10 — Net sales/revenues/receipts/fees",
            layout,
            cx,
        ));
        a.push(self.input_row(
            Sec::Schedule3,
            11,
            Self::label_for(Sec::Schedule3, 11),
            layout,
            item,
        ));
        for (n, label) in [
            (12u8, "12 — Gross income from operation"),
            (
                13,
                "13 — Ordinary allowable itemized deductions (Schedule 4)",
            ),
            (
                14,
                "14 — Special allowable itemized deductions (Schedule 5)",
            ),
            (15, "15 — NOLCO (Schedule 6)"),
            (16, "16 — Total deductions"),
            (17, "17 — Optional standard deduction (40% of Item 10)"),
            (18, "18 — Net income/(loss)"),
        ] {
            a.push(self.computed_row(Sec::Schedule3, n, label, layout, cx));
        }
        a.push(self.text_field("d19", layout));
        a.push(self.input_row(
            Sec::Schedule3,
            19,
            Self::label_for(Sec::Schedule3, 19),
            layout,
            gr,
        ));
        a.push(self.text_field("d20", layout));
        a.push(self.input_row(
            Sec::Schedule3,
            20,
            Self::label_for(Sec::Schedule3, 20),
            layout,
            gr,
        ));
        a.push(self.input_row(
            Sec::Schedule3,
            21,
            Self::label_for(Sec::Schedule3, 21),
            layout,
            gr,
        ));
        for (n, label) in [
            (22u8, "22 — Total other taxable income"),
            (23, "23 — Total taxable income"),
            (24, "24 — Total taxable income (with compensation)"),
            (25, "25 — Tax due (graduated rates)"),
        ] {
            a.push(self.computed_row(Sec::Schedule3, n, label, layout, cx));
        }
        let mut b = vec![self.column_header(layout)];
        b.push(self.input_row(
            Sec::Schedule3,
            26,
            Self::label_for(Sec::Schedule3, 26),
            layout,
            eight,
        ));
        b.push(self.text_field("d27", layout));
        b.push(self.input_row(
            Sec::Schedule3,
            27,
            Self::label_for(Sec::Schedule3, 27),
            layout,
            eight,
        ));
        b.push(self.computed_row(Sec::Schedule3, 28, "28 — Total", layout, cx));
        b.push(self.input_row(
            Sec::Schedule3,
            29,
            Self::label_for(Sec::Schedule3, 29),
            layout,
            pure,
        ));
        for (n, label) in [
            (30u8, "30 — Taxable income/(loss)"),
            (31, "31 — Tax due (8%)"),
            (32, "32 — Total tax due (with compensation)"),
        ] {
            b.push(self.computed_row(Sec::Schedule3, n, label, layout, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(self.section("Schedule 3.A — Graduated Income Tax Rates", a, cx))
            .child(self.section("Schedule 3.B — 8% Income Tax Rate", b, cx))
            .into_any_element()
    }

    fn render_deductions(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let item = [!self.itemized(TP), !self.itemized(SP)];
        let editable = self.draft.lifecycle.is_editable();
        let mut four = vec![self.column_header(layout)];
        for n in 1..=16u8 {
            four.push(self.input_row(
                Sec::Schedule4,
                n,
                Self::label_for(Sec::Schedule4, n),
                layout,
                item,
            ));
        }
        for (index, label) in [
            "17a — Others",
            "17b — Others",
            "17c — Others",
            "17d — Others",
        ]
        .iter()
        .enumerate()
        {
            if index == 3 {
                four.push(self.text_field("d17d", layout));
            }
            let cells = [TP, SP]
                .iter()
                .zip(item)
                .map(|(party, off)| {
                    let key = party_key(&format!("s4_17_{index}"), *party);
                    Input::new(&self.inputs[&key])
                        .disabled(!editable || off)
                        .into_any_element()
                })
                .collect();
            four.push(self.item_row(label, cells, layout));
        }
        four.push(self.computed_row(
            Sec::Schedule4,
            18,
            "18 — Total ordinary allowable itemized deductions",
            layout,
            cx,
        ));

        let mut five = Vec::new();
        for party in [TP, SP] {
            if party == SP && !self.joint() {
                continue;
            }
            let off = !self.itemized(party);
            for row in 0..2 {
                let item_no = if party == TP { row + 1 } else { row + 4 };
                five.push(self.grid(
                    layout,
                    vec![
                        self.field(
                            &party_key(&format!("s5_{row}_desc"), party),
                            &format!("{item_no} — Description"),
                            layout,
                            off,
                        ),
                        self.field(
                            &party_key(&format!("s5_{row}_legal"), party),
                            &format!("{item_no} — Legal basis"),
                            layout,
                            off,
                        ),
                        self.field(
                            &party_key(&format!("s5_{row}_amt"), party),
                            &format!("{item_no} — Amount"),
                            layout,
                            off,
                        ),
                    ],
                ));
            }
        }
        let c = &self.draft.computations;
        five.push(self.item_row(
            "Total special allowable itemized deductions",
            vec![
                self.amount_cell(c.schedule_5_total_taxpayer.unwrap_or(0.0), cx),
                self.amount_cell(c.schedule_5_total_spouse.unwrap_or(0.0), cx),
            ],
            layout,
        ));

        let mut six = vec![self.column_header(layout)];
        six.push(self.input_row(
            Sec::Schedule6,
            1,
            Self::label_for(Sec::Schedule6, 1),
            layout,
            item,
        ));
        six.push(self.input_row(
            Sec::Schedule6,
            2,
            Self::label_for(Sec::Schedule6, 2),
            layout,
            item,
        ));
        six.push(self.computed_row(Sec::Schedule6, 3, "3 — Net operating loss", layout, cx));
        for party in [TP, SP] {
            if party == SP && !self.joint() {
                continue;
            }
            let off = !self.itemized(party);
            let rows = if party == TP {
                &c.schedule_6_taxpayer_nolco
            } else {
                &c.schedule_6_spouse_nolco
            };
            let who = if party == TP { "Taxpayer" } else { "Spouse" };
            for (row, entry) in rows.iter().enumerate() {
                let current = row == 3;
                let mut fields = Vec::new();
                if current {
                    fields.push(
                        Self::label(&format!(
                            "{who} — current-year loss {}",
                            entry.year_incurred
                        ))
                        .into_any_element(),
                    );
                    fields.push(self.amount_cell(entry.amount.unwrap_or(0.0), cx));
                } else {
                    fields.push(self.field(
                        &party_key(&format!("s6_{row}_year"), party),
                        &format!("{who} — Year incurred"),
                        layout,
                        off,
                    ));
                    fields.push(self.field(
                        &party_key(&format!("s6_{row}_a"), party),
                        "A — Amount",
                        layout,
                        off,
                    ));
                }
                fields.push(self.field(
                    &party_key(&format!("s6_{row}_b"), party),
                    "B — Applied previous years",
                    layout,
                    off,
                ));
                fields.push(self.field(
                    &party_key(&format!("s6_{row}_c"), party),
                    "C — Expired",
                    layout,
                    off,
                ));
                fields.push(self.field(
                    &party_key(&format!("s6_{row}_d"), party),
                    "D — Applied current year",
                    layout,
                    off,
                ));
                fields.push(
                    div()
                        .flex()
                        .justify_between()
                        .child(Self::label("E — Net operating loss unapplied"))
                        .child(official_amount(entry.unapplied.unwrap_or(0.0)))
                        .into_any_element(),
                );
                six.push(
                    div()
                        .p_3()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .child(self.grid(layout, fields))
                        .into_any_element(),
                );
            }
        }
        six.push(self.item_row(
            "Total NOLCO applied",
            vec![
                self.amount_cell(c.schedule_6_total_taxpayer.unwrap_or(0.0), cx),
                self.amount_cell(c.schedule_6_total_spouse.unwrap_or(0.0), cx),
            ],
            layout,
        ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(self.section(
                "Schedule 4 — Ordinary Allowable Itemized Deductions",
                four,
                cx,
            ))
            .child(self.section(
                "Schedule 5 — Special Allowable Itemized Deductions",
                five,
                cx,
            ))
            .child(self.section(
                "Schedule 6 — Net Operating Loss Carry-Over (NOLCO)",
                six,
                cx,
            ))
            .into_any_element()
    }

    fn render_parts_vi_to_ix(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let none = [false, !self.joint()];
        let amended = [
            !self.draft.is_amended,
            !self.draft.is_amended || !self.joint(),
        ];
        let mut six = vec![self.column_header(layout)];
        six.push(self.computed_row(
            Sec::PartVi,
            1,
            "1 — Income tax due (Schedule 3)",
            layout,
            cx,
        ));
        six.push(self.input_row(
            Sec::PartVi,
            2,
            Self::label_for(Sec::PartVi, 2),
            layout,
            none,
        ));
        six.push(self.input_row(
            Sec::PartVi,
            3,
            Self::label_for(Sec::PartVi, 3),
            layout,
            none,
        ));
        six.push(self.computed_row(Sec::PartVi, 4, "4 — Net", layout, cx));
        six.push(self.computed_row(Sec::PartVi, 5, "5 — Total income tax due", layout, cx));
        let mut seven = vec![self.column_header(layout)];
        for n in 1..=9u8 {
            if n == 5 {
                seven.push(self.computed_row(
                    Sec::PartVii,
                    5,
                    "5 — Tax withheld on compensation (Schedule 1)",
                    layout,
                    cx,
                ));
                continue;
            }
            if n == 9 {
                seven.push(self.text_field("d7_9", layout));
            }
            let off = if n == 6 { amended } else { none };
            seven.push(self.input_row(
                Sec::PartVii,
                n,
                Self::label_for(Sec::PartVii, n),
                layout,
                off,
            ));
        }
        seven.push(self.computed_row(
            Sec::PartVii,
            10,
            "10 — Total tax credits/payments",
            layout,
            cx,
        ));
        let mut nine = vec![self.column_header(layout)];
        for n in [1u8, 2, 3, 4] {
            if n != 1 {
                nine.push(self.field(
                    &format!("p9d_{n}"),
                    &format!("{n} — Particulars"),
                    layout,
                    false,
                ));
            }
            nine.push(self.input_row(
                Sec::PartIx,
                n,
                Self::label_for(Sec::PartIx, n),
                layout,
                none,
            ));
        }
        nine.push(self.computed_row(Sec::PartIx, 5, "5 — Total", layout, cx));
        for n in [6u8, 7, 8, 9] {
            nine.push(self.field(
                &format!("p9d_{n}"),
                &format!("{n} — Particulars"),
                layout,
                false,
            ));
            nine.push(self.input_row(
                Sec::PartIx,
                n,
                Self::label_for(Sec::PartIx, n),
                layout,
                none,
            ));
        }
        nine.push(self.computed_row(Sec::PartIx, 10, "10 — Total", layout, cx));
        nine.push(self.computed_row(
            Sec::PartIx,
            11,
            "11 — Net taxable income/(loss)",
            layout,
            cx,
        ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(self.section("Part VI — Summary of Income Tax Due", six, cx))
            .child(self.section("Part VII — Tax Credits/Payments", seven, cx))
            .child(self.section(
                "Part IX — Reconciliation of Net Income per Books against Taxable Income",
                nine,
                cx,
            ))
            .into_any_element()
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let none = [false, !self.joint()];
        let installment = [
            self.draft.amount(Sec::PartIi, 24, TP).unwrap_or(0.0) <= 0.0,
            !self.joint() || self.draft.amount(Sec::PartIi, 24, SP).unwrap_or(0.0) <= 0.0,
        ];
        let mut rows = vec![self.column_header(layout)];
        for (n, label) in [
            (22u8, "22 — Tax due"),
            (23, "23 — Less: tax credits/payments"),
            (24, "24 — Tax payable/(overpayment)"),
        ] {
            rows.push(self.computed_row(Sec::PartIi, n, label, layout, cx));
        }
        rows.push(self.input_row(
            Sec::PartIi,
            25,
            Self::label_for(Sec::PartIi, 25),
            layout,
            installment,
        ));
        rows.push(self.computed_row(
            Sec::PartIi,
            26,
            "26 — Amount of tax payable/(overpayment)",
            layout,
            cx,
        ));
        for n in [27u8, 28, 29] {
            rows.push(self.input_row(
                Sec::PartIi,
                n,
                Self::label_for(Sec::PartIi, n),
                layout,
                none,
            ));
        }
        rows.push(self.computed_row(Sec::PartIi, 30, "30 — Total penalties", layout, cx));
        rows.push(self.computed_row(
            Sec::PartIi,
            31,
            "31 — Total amount payable/(overpayment)",
            layout,
            cx,
        ));
        rows.push(
            div()
                .flex()
                .justify_between()
                .gap_2()
                .child(Self::label("32 — Aggregate amount payable/(overpayment)"))
                .child(
                    div().font_weight(FontWeight::BOLD).child(official_amount(
                        self.draft
                            .computations
                            .part_ii_item_32_aggregate
                            .unwrap_or(0.0),
                    )),
                )
                .into_any_element(),
        );
        let overpaid = self.draft.amount(Sec::PartIi, 26, TP).unwrap_or(0.0) < 0.0
            || self.draft.amount(Sec::PartIi, 26, SP).unwrap_or(0.0) < 0.0;
        if overpaid {
            let over = self.draft.overpayment_disposition;
            rows.push(Self::choice_row(
                "If overpayment, mark one:",
                [
                    (Form1701OverpaymentDisposition::Refund, "To be refunded"),
                    (
                        Form1701OverpaymentDisposition::TaxCreditCertificate,
                        "To be issued a Tax Credit Certificate",
                    ),
                    (
                        Form1701OverpaymentDisposition::CarryOver,
                        "To be carried over as tax credit",
                    ),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (value, label))| {
                    Self::choice(("1701_over", index), label, over == value, !editable).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.edit(cx, |d| d.overpayment_disposition = value)
                        }),
                    )
                })
                .collect(),
            ));
        }
        rows.push(self.text_field("attachments", layout));
        self.section("Part II — Total Tax Payable", rows, cx)
    }
}

impl QueueableFormView for Form1701View {
    type Draft = Form1701Draft;

    fn new(
        draft: Form1701Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let keys = TEXT_INPUTS
            .iter()
            .map(|(k, _)| k.to_string())
            .chain(dynamic_keys());
        for key in keys {
            let value = Self::initial(&draft, &key);
            let input = cx.new(|cx| InputState::new(window, cx));
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1701Draft {
        let mut draft = Form1701Draft::new_from_profile(profile, year, 12);
        draft.recompute();
        draft
    }

    /// Annual: earlier builds stored the return with a NULL period column
    /// (period key `A`); the first generic save adopts that row.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form1701Draft {
        let tin = profile.tin.full();
        db.get_queueable_draft::<Form1701Draft>(&tin, year, 0)
            .ok()
            .flatten()
            .or_else(|| {
                db.get_form_draft_v2::<Form1701Draft>(&tin, "1701", year, &FilingPeriod::Annual)
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| Self::new_draft(profile, year, period))
    }
}

impl FormViewTrait for Form1701View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Individuals, Estates and Trusts"
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
                "Fix the highlighted entries before saving.".into(),
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
                    "1701 draft saved.".into(),
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
        if !can_queue_for_submission(<Form1701Draft as QueueableForm>::FORM_CODE) {
            self.status_message =
                Some("1701 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1701. No submission was started: {error}"
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
            "Form 1701 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1701 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1701Draft>(
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
                "Fix the highlighted entries first. No filing state was changed.".into(),
            ));
            return;
        }
        let fields = self.draft.to_bir_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "1701-2018",
            &fields,
            "1701 — Print Preview",
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

impl Render for Form1701View {
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
            .take(25)
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
                .child(Button::new("1701_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1701_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1701_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1701_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1701_submit")
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
                            Button::new("1701_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1701_release_cancel")
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
            .child(self.render_background(layout, cx));
        if self.joint() {
            body = body.child(self.render_spouse(layout, cx));
        }
        body = body
            .child(self.render_part_two(layout, cx))
            .child(self.render_employers(layout, cx))
            .child(self.render_schedule_3(layout, cx))
            .child(self.render_deductions(layout, cx))
            .child(self.render_parts_vi_to_ix(layout, cx));
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
                    .id("1701_scroll")
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
    use super::{Layout, dynamic_keys, format_tin, pair_key, parse_amount};
    use bir_core::forms::form_1701::{Form1701AmountSection, Form1701Party};
    use gpui::px;

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn entries_parse_and_keys_are_unique() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
        assert_eq!(
            pair_key(Form1701AmountSection::Schedule3, 8, Form1701Party::Spouse),
            "s3_8_b"
        );
        let keys = dynamic_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
    }
}
