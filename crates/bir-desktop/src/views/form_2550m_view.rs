//! Editor for BIR Form 2550M (February 2007), Monthly Value-Added Tax
//! Declaration. Rust owns every calculation, validation and the official
//! submit plaintext (`bir_core::forms::form_2550m`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.
//!
//! Schedules 2, 3, 6, 7 and 8 are not offered (see the core module).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2550m::{
    FORM_2550M_ATCS, Form2550MAllocation, Form2550MDraft, Form2550MPurchase, Form2550MTaxRelief,
    form_2550m_atc_index,
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

impl EventEmitter<QueueableFormEvent> for Form2550MView {}

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

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Text inputs: (key, label, placeholder, validation field).
const TEXT_INPUTS: &[(&str, &str, &str, &str)] = &[
    ("year", "Item 1 — Year", "YYYY", "taxable_year"),
    (
        "sheets",
        "Item 3 — No. of sheets attached",
        "0",
        "number_of_attached_sheets",
    ),
    ("lob", "Item 6 — Line of Business", "", "line_of_business"),
    ("name", "Item 7 — Taxpayer's Name", "", "taxpayer_name"),
    (
        "phone",
        "Item 8 — Telephone No.",
        "digits only",
        "contact_number",
    ),
    (
        "address",
        "Item 9 — Registered Address",
        "",
        "registered_address",
    ),
    ("zip", "Item 10 — Zip Code", "", "zip_code"),
    (
        "email",
        "Email address for the BIR confirmation",
        "",
        "email",
    ),
];

/// Money inputs: (key, label, validation field).
const MONEY_INPUTS: &[(&str, &str, &str)] = &[
    ("13a", "13A — Sales to Government", "sales_to_government"),
    (
        "13b",
        "13B — Output tax, sales to Government",
        "output_tax_government",
    ),
    ("14", "14 — Zero-rated sales/receipts", "zero_rated_sales"),
    ("15", "15 — Exempt sales/receipts", "exempt_sales"),
    (
        "17a",
        "17A — Input tax carried over",
        "input_tax_carried_over",
    ),
    (
        "17b",
        "17B — Input tax deferred on capital goods > P1M (previous period)",
        "input_tax_deferred_capital_goods",
    ),
    (
        "17c",
        "17C — Transitional input tax",
        "transitional_input_tax",
    ),
    (
        "17d",
        "17D — Presumptive input tax",
        "presumptive_input_tax",
    ),
    ("17e", "17E — Others", "other_input_tax"),
    ("18e", "18E — Domestic purchases of goods", "domestic_goods"),
    ("18f", "18F — Input tax", "domestic_goods_input_tax"),
    ("18g", "18G — Importation of goods", "imported_goods"),
    ("18h", "18H — Input tax", "imported_goods_input_tax"),
    (
        "18i",
        "18I — Domestic purchase of services",
        "domestic_services",
    ),
    ("18j", "18J — Input tax", "domestic_services_input_tax"),
    (
        "18k",
        "18K — Services by non-residents",
        "non_resident_services",
    ),
    ("18l", "18L — Input tax", "non_resident_services_input_tax"),
    (
        "18m",
        "18M — Purchases not qualified for input tax",
        "purchases_not_qualified",
    ),
    ("18n", "18N — Others", "other_purchases"),
    ("18o", "18O — Input tax", "other_purchases_input_tax"),
    (
        "s4_direct",
        "Sch. 4 — Input tax directly attributable",
        "schedule_4.direct_input_tax",
    ),
    (
        "s4_indirect",
        "Sch. 4 — Input tax not directly attributable",
        "schedule_4.not_direct_input_tax",
    ),
    (
        "s4_std",
        "Sch. 4 — Less: standard input tax (7% of 13A)",
        "standard_input_tax_government",
    ),
    (
        "s5_direct",
        "Sch. 5 — Input tax directly attributable",
        "schedule_5.direct_input_tax",
    ),
    (
        "s5_indirect",
        "Sch. 5 — Input tax not directly attributable",
        "schedule_5.not_direct_input_tax",
    ),
    ("20d", "20D — VAT refund/TCC claimed", "vat_refund_claimed"),
    ("20e", "20E — Others", "other_deductions"),
    (
        "23d",
        "23D — VAT paid in return previously filed (amended)",
        "vat_paid_previous",
    ),
    (
        "23e",
        "23E — Advance payments made (BIR Form 0605)",
        "advance_payments",
    ),
    ("23f", "23F — Others", "other_credits"),
    ("25a", "25A — Surcharge", "surcharge"),
    ("25b", "25B — Interest", "interest"),
    ("25c", "25C — Compromise", "compromise"),
];

/// Purchases whose input tax the page fills at 12% when the amount changes.
const PURCHASES: &[(&str, &str, Form2550MPurchase)] = &[
    ("18e", "18f", Form2550MPurchase::DomesticGoods),
    ("18g", "18h", Form2550MPurchase::ImportedGoods),
    ("18i", "18j", Form2550MPurchase::DomesticServices),
    ("18k", "18l", Form2550MPurchase::NonResidentServices),
    ("18n", "18o", Form2550MPurchase::Others),
];

fn sales_key(atc_index: usize) -> String {
    format!("atc{atc_index}")
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

/// The draft value behind a money input.
fn money_value(d: &Form2550MDraft, key: &str) -> f64 {
    let s4 = d.schedule_4.unwrap_or_default();
    let s5 = d.schedule_5.unwrap_or_default();
    match key {
        "13a" => d.sales_to_government,
        "13b" => d.output_tax_government,
        "14" => d.zero_rated_sales,
        "15" => d.exempt_sales,
        "17a" => d.input_tax_carried_over,
        "17b" => d.input_tax_deferred_capital_goods,
        "17c" => d.transitional_input_tax,
        "17d" => d.presumptive_input_tax,
        "17e" => d.other_input_tax,
        "18e" => d.domestic_goods,
        "18f" => d.domestic_goods_input_tax,
        "18g" => d.imported_goods,
        "18h" => d.imported_goods_input_tax,
        "18i" => d.domestic_services,
        "18j" => d.domestic_services_input_tax,
        "18k" => d.non_resident_services,
        "18l" => d.non_resident_services_input_tax,
        "18m" => d.purchases_not_qualified,
        "18n" => d.other_purchases,
        "18o" => d.other_purchases_input_tax,
        "s4_direct" => s4.direct_input_tax,
        "s4_indirect" => s4.not_direct_input_tax,
        "s4_std" => d.standard_input_tax_government,
        "s5_direct" => s5.direct_input_tax,
        "s5_indirect" => s5.not_direct_input_tax,
        "20d" => d.vat_refund_claimed,
        "20e" => d.other_deductions,
        "23d" => d.vat_paid_previous,
        "23e" => d.advance_payments,
        "23f" => d.other_credits,
        "25a" => d.surcharge,
        "25b" => d.interest,
        _ => d.compromise,
    }
}

fn set_money_value(d: &mut Form2550MDraft, key: &str, value: f64) {
    let slot = match key {
        "13a" => &mut d.sales_to_government,
        "13b" => &mut d.output_tax_government,
        "14" => &mut d.zero_rated_sales,
        "15" => &mut d.exempt_sales,
        "17a" => &mut d.input_tax_carried_over,
        "17b" => &mut d.input_tax_deferred_capital_goods,
        "17c" => &mut d.transitional_input_tax,
        "17d" => &mut d.presumptive_input_tax,
        "17e" => &mut d.other_input_tax,
        "18e" => &mut d.domestic_goods,
        "18f" => &mut d.domestic_goods_input_tax,
        "18g" => &mut d.imported_goods,
        "18h" => &mut d.imported_goods_input_tax,
        "18i" => &mut d.domestic_services,
        "18j" => &mut d.domestic_services_input_tax,
        "18k" => &mut d.non_resident_services,
        "18l" => &mut d.non_resident_services_input_tax,
        "18m" => &mut d.purchases_not_qualified,
        "18n" => &mut d.other_purchases,
        "18o" => &mut d.other_purchases_input_tax,
        "s4_direct" | "s4_indirect" => {
            let Some(schedule) = d.schedule_4.as_mut() else {
                return;
            };
            if key == "s4_direct" {
                &mut schedule.direct_input_tax
            } else {
                &mut schedule.not_direct_input_tax
            }
        }
        "s4_std" => &mut d.standard_input_tax_government,
        "s5_direct" | "s5_indirect" => {
            let Some(schedule) = d.schedule_5.as_mut() else {
                return;
            };
            if key == "s5_direct" {
                &mut schedule.direct_input_tax
            } else {
                &mut schedule.not_direct_input_tax
            }
        }
        "20d" => &mut d.vat_refund_claimed,
        "20e" => &mut d.other_deductions,
        "23d" => &mut d.vat_paid_previous,
        "23e" => &mut d.advance_payments,
        "23f" => &mut d.other_credits,
        "25a" => &mut d.surcharge,
        "25b" => &mut d.interest,
        _ => &mut d.compromise,
    };
    *slot = value;
}

pub struct Form2550MView {
    draft: Form2550MDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2550MView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2550MDraft, key: &str) -> String {
        match key {
            "year" => draft.taxable_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "lob" => draft.line_of_business.clone(),
            "name" => draft.taxpayer_name.clone(),
            "phone" => draft.contact_number.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "email" => draft.email.clone(),
            _ => {
                if let Some(index) = key
                    .strip_prefix("atc")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    let code = FORM_2550M_ATCS.get(index).map(|(code, _)| *code);
                    draft
                        .sales_schedule
                        .iter()
                        .find(|row| Some(row.atc_code.as_str()) == code)
                        .map(|row| money(row.amount))
                        .unwrap_or_default()
                } else {
                    money(money_value(draft, key))
                }
            }
        }
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn reload(&self, keys: &[&str], window: &mut Window, cx: &mut Context<Self>) {
        for key in keys {
            if let Some(input) = self.inputs.get(*key) {
                let value = Self::initial(&self.draft, key);
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    /// Read every editor value into the draft, then recompute and validate.
    /// Changing 13A or a purchase amount refills its 12% tax (and Schedule
    /// 4's 7% standard input tax) the way the page's blur handlers do.
    fn sync_from_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        if let Some(sheets) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        let mut refreshed: Vec<&str> = Vec::new();
        let new_13a = number("13a", "13A", cx);
        let mut purchases = Vec::new();
        for (key, label, _) in MONEY_INPUTS {
            if *key == "13a" {
                continue;
            }
            if let Some(value) = number(key, label, cx) {
                let base = PURCHASES.iter().find(|(base, _, _)| base == key);
                match base {
                    Some((_, tax_key, purchase))
                        if (value - money_value(&draft, key)).abs() > 1e-9 =>
                    {
                        purchases.push((*purchase, value, *tax_key));
                    }
                    _ => set_money_value(&mut draft, key, value),
                }
            }
        }
        for row in draft.sales_schedule.iter_mut() {
            if let Some(index) = form_2550m_atc_index(&row.atc_code)
                && let Some(value) =
                    number(&sales_key(index), &format!("Sch. 1 {}", row.atc_code), cx)
            {
                row.amount = value;
            }
        }
        draft.line_of_business = self.input_text("lob", cx);
        draft.taxpayer_name = self.input_text("name", cx);
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        if let Some(value) = new_13a
            && (value - draft.sales_to_government).abs() > 1e-9
        {
            draft.set_sales_to_government(value);
            refreshed.extend(["13b", "s4_std"]);
        }
        for (purchase, value, tax_key) in purchases {
            draft.set_purchase(purchase, value);
            refreshed.push(tax_key);
        }
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        if !refreshed.is_empty() {
            self.reload(&refreshed, window, cx);
        }
        cx.notify();
    }

    /// A choice change; editors are reloaded from the result.
    fn edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form2550MDraft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(window, cx);
        change(&mut self.draft);
        self.draft.recompute();
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.reload(&keys, window, cx);
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
                .release_abandoned_claimed_queueable::<Form2550MDraft>(
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

    fn money_field(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        let (_, label, error_key) = MONEY_INPUTS
            .iter()
            .find(|(k, _, _)| *k == key)
            .copied()
            .unwrap_or((key, key, key));
        let editable = self.draft.lifecycle.is_editable();
        self.field(key, label, error_key, layout, !editable || disabled)
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

    fn text_field(&self, key: &str, layout: Layout) -> AnyElement {
        let (_, label, _, error_key) = TEXT_INPUTS
            .iter()
            .find(|(k, _, _, _)| *k == key)
            .copied()
            .unwrap_or((key, key, "", key));
        let editable = self.draft.lifecycle.is_editable();
        self.field(key, label, error_key, layout, !editable)
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut months = div().flex().flex_wrap().gap_1();
        for (index, name) in MONTHS.iter().enumerate() {
            let month = index as u8 + 1;
            months = months.child(
                Self::choice(
                    ("2550m_month", index),
                    *name,
                    self.draft.month == month,
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(window, cx, |d| d.month = month)
                })),
            );
        }
        let amended = self.draft.is_amended;
        let children = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 1 — For the month of"),
                )
                .child(months)
                .into_any_element(),
            self.text_field("year", layout),
            Self::choice_row(
                "Item 2 — Amended return?",
                vec![
                    Self::choice("2550m_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = true)
                        }),
                    ),
                    Self::choice("2550m_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.edit(window, cx, |d| d.is_amended = false)
                        }),
                    ),
                ],
            ),
            self.text_field("sheets", layout),
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
        for key in ["lob", "name", "phone", "address", "zip", "email"] {
            fields.push(self.text_field(key, layout));
        }
        let relief = d.tax_relief;
        let pick = |id: &'static str, label: &'static str, value: Form2550MTaxRelief| {
            Self::choice(id, label, relief == value, !editable).on_click(cx.listener(
                move |this, _, window, cx| this.edit(window, cx, |d| d.tax_relief = value),
            ))
        };
        let children = vec![
            self.grid(layout, fields),
            Self::choice_row(
                "Item 11 — Availing of tax relief under a Special Law or International Tax Treaty?",
                vec![
                    pick("2550m_relief_no", "No", Form2550MTaxRelief::No),
                    pick(
                        "2550m_relief_special",
                        "Yes — Special Rate",
                        Form2550MTaxRelief::SpecialRate,
                    ),
                    pick(
                        "2550m_relief_treaty",
                        "Yes — International Tax Treaty",
                        Form2550MTaxRelief::InternationalTaxTreaty,
                    ),
                ],
            ),
        ];
        self.section("Part I — Background Information", children, cx)
    }

    fn render_schedule_1(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children: Vec<AnyElement> = Vec::new();
        for row in &d.sales_schedule {
            let Some(index) = form_2550m_atc_index(&row.atc_code) else {
                continue;
            };
            let code = row.atc_code.clone();
            let description = FORM_2550M_ATCS[index].1;
            let heading = div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::BOLD).child(code.clone()))
                        .child(div().text_sm().child(description.to_string())),
                )
                .child(
                    Button::new(("2550m_remove_atc", index))
                        .label("Remove")
                        .outline()
                        .small()
                        .disabled(!editable)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let code = code.clone();
                            this.edit(window, cx, |d| d.remove_sales_atc(&code))
                        })),
                );
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(heading)
                    .child(self.field(
                        &sales_key(index),
                        "Amount of sales/receipts for the period",
                        "sales_schedule",
                        layout,
                        !editable,
                    ))
                    .child(self.computed("Output tax (12%)", row.output_tax, cx))
                    .into_any_element(),
            );
        }
        if editable {
            let mut picks = div().flex().flex_wrap().gap_1();
            for (index, (code, description)) in FORM_2550M_ATCS.iter().enumerate() {
                if d.sales_schedule.iter().any(|row| row.atc_code == *code) {
                    continue;
                }
                let label = if layout == Layout::Phone {
                    code.to_string()
                } else {
                    let short: String = description.chars().take(40).collect();
                    format!("{code} — {short}")
                };
                picks = picks.child(
                    Button::new(("2550m_add_atc", index))
                        .label(label)
                        .outline()
                        .small()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.edit(window, cx, |d| {
                                let _ = d.add_sales_atc(FORM_2550M_ATCS[index].0);
                            })
                        })),
                );
            }
            children.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Add an ATC"),
                    )
                    .child(picks)
                    .into_any_element(),
            );
        }
        children.push(self.computed(
            "12A — Vatable sales/receipts, private",
            d.vatable_sales_private,
            cx,
        ));
        children.push(self.computed("12B — Output tax", d.output_tax_private, cx));
        self.section(
            "Schedule 1 — Vatable Sales/Receipts, Private (Item 12)",
            children,
            cx,
        )
    }

    fn render_sales(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let children = vec![
            self.money_field("13a", layout, false),
            self.money_field("13b", layout, false),
            self.money_field("14", layout, false),
            self.money_field("15", layout, false),
            self.computed("16A — Total sales/receipts", d.total_sales, cx),
            self.computed("16B — Total output tax due", d.total_output_tax, cx),
        ];
        self.section(
            "Part II — Sales and Output Tax (Items 13–16)",
            vec![self.grid(layout, children)],
            cx,
        )
    }

    fn render_input_tax(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children: Vec<AnyElement> = ["17a", "17b", "17c", "17d", "17e"]
            .iter()
            .map(|key| self.money_field(key, layout, false))
            .collect();
        children.push(self.computed("17F — Total", d.total_input_tax_17f, cx));
        for key in [
            "18e", "18f", "18g", "18h", "18i", "18j", "18k", "18l", "18m", "18n", "18o",
        ] {
            children.push(self.money_field(key, layout, false));
        }
        children.push(self.computed(
            "18P — Total current purchases",
            d.total_current_purchases,
            cx,
        ));
        children.push(self.computed(
            "19 — Total available input tax",
            d.total_available_input_tax,
            cx,
        ));
        self.section(
            "Part II — Allowable Input Tax (Items 17–19)",
            vec![
                div()
                    .text_sm()
                    .child("Capital goods (18A–18D, Schedules 2 and 3) are not supported in this editor.")
                    .into_any_element(),
                self.grid(layout, children),
            ],
            cx,
        )
    }

    fn render_deductions(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let s4_open = d.schedule_4.is_some();
        let s5_open = d.schedule_5.is_some();
        let mut children = vec![Self::choice_row(
            "Schedule 4 — Input tax on sale to Government closed to expense (20B)",
            vec![
                Self::choice(
                    "2550m_s4",
                    if s4_open { "In use" } else { "Use Schedule 4" },
                    s4_open,
                    !editable || (!s4_open && d.sales_to_government <= 0.0),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| {
                        d.schedule_4 = match d.schedule_4 {
                            Some(_) => None,
                            None => Some(Form2550MAllocation::default()),
                        }
                    })
                })),
            ],
        )];
        if s4_open {
            children.push(self.grid(
                layout,
                vec![
                    self.money_field("s4_direct", layout, false),
                    self.money_field("s4_indirect", layout, false),
                    self.computed("Allocated by sales", d.schedule_4_attributed, cx),
                    self.money_field("s4_std", layout, false),
                ],
            ));
        }
        children.push(Self::choice_row(
            "Schedule 5 — Input tax allocable to exempt sales (20C)",
            vec![
                Self::choice(
                    "2550m_s5",
                    if s5_open { "In use" } else { "Use Schedule 5" },
                    s5_open,
                    !editable || (!s5_open && d.exempt_sales <= 0.0),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit(window, cx, |d| {
                        d.schedule_5 = match d.schedule_5 {
                            Some(_) => None,
                            None => Some(Form2550MAllocation::default()),
                        }
                    })
                })),
            ],
        ));
        if s5_open {
            children.push(self.grid(
                layout,
                vec![
                    self.money_field("s5_direct", layout, false),
                    self.money_field("s5_indirect", layout, false),
                    self.computed("Allocated by sales", d.schedule_5_attributed, cx),
                ],
            ));
        }
        children.push(self.grid(
            layout,
            vec![
                self.computed(
                    "20B — Sale to Government",
                    d.input_tax_sales_to_government,
                    cx,
                ),
                self.computed("20C — Exempt sales", d.input_tax_exempt_sales, cx),
                self.money_field("20d", layout, false),
                self.money_field("20e", layout, false),
                self.computed("20F — Total deductions", d.total_deductions, cx),
                self.computed(
                    "21 — Total allowable input tax",
                    d.total_allowable_input_tax,
                    cx,
                ),
                self.computed("22 — Net VAT payable", d.net_vat_payable, cx),
            ],
        ));
        self.section(
            "Part II — Deductions from Input Tax (Items 20–22)",
            children,
            cx,
        )
    }

    fn render_payable(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let children = vec![
            self.money_field("23d", layout, !d.is_amended),
            self.money_field("23e", layout, false),
            self.money_field("23f", layout, false),
            self.computed("23G — Total tax credits/payments", d.total_credits, cx),
            self.computed(
                "24 — Tax still payable/(overpayment)",
                d.tax_still_payable,
                cx,
            ),
            self.money_field("25a", layout, false),
            self.money_field("25b", layout, false),
            self.money_field("25c", layout, false),
            self.computed("25D — Total penalties", d.total_penalties, cx),
            self.computed(
                "26 — Total amount payable/(overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        self.section(
            "Part II — Tax Credits, Penalties and Amount Payable (Items 23–26)",
            vec![
                div()
                    .text_sm()
                    .child("Schedules 6–8 (23A–23C) are not supported in this editor.")
                    .into_any_element(),
                self.grid(layout, children),
            ],
            cx,
        )
    }
}

impl QueueableFormView for Form2550MView {
    type Draft = Form2550MDraft;

    fn new(
        draft: Form2550MDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder, _)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        for index in 0..FORM_2550M_ATCS.len() {
            keys.push((sales_key(index), "0.00".into()));
        }
        for (key, placeholder) in keys {
            let value = Self::initial(&draft, &key);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(window, cx);
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2550MDraft {
        let month = if (1..=12).contains(&period) {
            period
        } else {
            1
        };
        Form2550MDraft::new_from_profile(profile, year, month)
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

impl FormViewTrait for Form2550MView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2550M"
    }
    fn form_subtitle(&self) -> &'static str {
        "Monthly Value-Added Tax Declaration"
    }
    fn form_version(&self) -> &'static str {
        "February 2007 (ENCS)"
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
        self.sync_from_inputs(window, cx);
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
                    "2550M draft saved.".into(),
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
        if !can_queue_for_submission(Form2550MDraft::FORM_CODE) {
            self.status_message =
                Some("2550M is not enabled for in-app submission in this build.".into());
            cx.notify();
            return;
        }
        if !self.draft.lifecycle.is_editable() {
            self.status_message =
                Some("This return is already queued or filed and cannot be queued again.".into());
            cx.notify();
            return;
        }
        self.sync_from_inputs(window, cx);
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
                "Could not queue Form 2550M. No submission was started: {error}"
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
            "Form 2550M queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2550M payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2550MDraft>(
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

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(window, cx);
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
            "2550m-2007",
            &fields,
            "2550M — Print Preview",
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

impl Render for Form2550MView {
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
                .child(Button::new("2550m_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2550m_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2550m_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2550m_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2550m_submit")
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
                            Button::new("2550m_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2550m_release_cancel")
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
            .child(self.render_schedule_1(layout, cx))
            .child(self.render_sales(layout, cx))
            .child(self.render_input_tax(layout, cx))
            .child(self.render_deductions(layout, cx))
            .child(self.render_payable(layout, cx));
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
                    .id("2550m_scroll")
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
    use super::{Layout, MONEY_INPUTS, format_tin, money_value, parse_amount, set_money_value};
    use bir_core::forms::form_2550m::Form2550MDraft;
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
    fn every_money_input_maps_to_its_own_draft_field() {
        let profile = serde_json::from_value(serde_json::json!({
            "id": null, "full_name": "", "tin": {"segment1": "", "segment2": "", "segment3": "", "branch": ""},
            "rdo_code": "", "line_of_business": "", "registered_address": "", "zip_code": "",
            "phone": "", "email": "", "default_form_type": "2550M", "taxpayer_type": "Corporation",
            "business_start_date": "2020-01-15", "tax_elections": []
        }))
        .unwrap();
        let mut draft = Form2550MDraft::new_from_profile(&profile, 2022, 6);
        draft.schedule_4 = Some(Default::default());
        draft.schedule_5 = Some(Default::default());
        for (index, (key, _, _)) in MONEY_INPUTS.iter().enumerate() {
            set_money_value(&mut draft, key, 1000.0 + index as f64);
        }
        for (index, (key, _, _)) in MONEY_INPUTS.iter().enumerate() {
            assert_eq!(money_value(&draft, key), 1000.0 + index as f64, "{key}");
        }
    }
}
