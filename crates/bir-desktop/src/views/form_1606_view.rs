//! Editor for BIR Form 1606, Withholding Tax Remittance Return for Onerous
//! Transfer of Real Property Other Than Capital Asset (v2018). Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1606`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1606::{
    Form1606AgentCategory, Form1606Draft, Form1606Overremittance, Form1606PropertyClass,
    Form1606SellerType, Form1606TaxRate, Form1606TaxRelief, Form1606TaxableBaseBox,
    Form1606Transaction,
};
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form1606View {}

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

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of transaction", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("seller_tin", "Item 7 — Seller's TIN", "000-000-000-00000"),
    ("seller_rdo", "Item 8 — Seller's RDO Code", "e.g. 039"),
    ("buyer_name", "Item 9 — Buyer's Name", ""),
    ("seller_name", "Item 10 — Seller's Name", ""),
    ("buyer_address", "Item 11 — Buyer's Registered Address", ""),
    (
        "seller_address",
        "Item 12 — Seller's Registered Address",
        "",
    ),
    ("class_other", "Item 15 — Others (specify)", ""),
    ("location", "Item 16 — Location of the Property", ""),
    (
        "property_rdo",
        "Item 16A — RDO Code of the property",
        "e.g. 039",
    ),
    ("tct", "Item 17 — TCT/OCT/CCT No.", ""),
    ("area", "Item 17 — Area sold (sq. m)", "digits only"),
    ("tax_dec", "Item 17 — Tax Dec. No.", "digits only"),
    ("other_desc", "Item 17 — Others", ""),
    ("txn_other", "Item 20 — If Exempt or Others, specify", ""),
    ("base_other_desc", "Item 28E — Others (specify)", ""),
    ("email", "Email address for the BIR confirmation", ""),
];

/// Money inputs where blank means zero: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "gross",
        "28A — Gross Selling Price (Item 21 on an installment sale)",
    ),
    ("cost", "22 — Cost and Other Expenses"),
    ("mortgage", "23 — Mortgage Assumed"),
    ("initial", "24 — Total Payments during the Initial Year"),
    ("inst_month", "25 — Amount of Installment this Month"),
    ("n_inst", "26 — Total No. of Installments in the Contract"),
    ("inst_collected", "28D — Installment Collected"),
    ("others_base", "28E — Others"),
    ("bid", "28C — Bid Price (foreclosure sale)"),
    (
        "less",
        "33 — Tax remitted in return previously filed (amended only)",
    ),
    ("sur", "35A — Surcharge"),
    ("int", "35B — Interest"),
    ("comp", "35C — Compromise"),
];

/// Item 27 inputs where blank stays blank (the box stays unticked).
const FMV_INPUTS: &[(&str, &str)] = &[
    ("fmv_a", "27A — FMV of Land per latest Tax Declaration"),
    ("fmv_c", "27C — FMV of Land per BIR (zonal value)"),
    (
        "fmv_b",
        "27B — FMV of Improvements per latest Tax Declaration",
    ),
    ("fmv_d", "27D — FMV of Improvements per BIR"),
];

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

pub struct Form1606View {
    draft: Form1606Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form1606View {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form1606Draft, key: &str) -> String {
        let money = |value: f64| {
            if value == 0.0 {
                String::new()
            } else {
                official_amount(value)
            }
        };
        let fmv = |value: Option<f64>| value.map(official_amount).unwrap_or_default();
        match key {
            "month" => format!("{:02}", draft.transaction_month),
            "day" => format!("{:02}", draft.transaction_day),
            "year" => draft.transaction_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "seller_tin" => {
                if draft.seller_tin.is_empty() {
                    String::new()
                } else {
                    format_tin(&draft.seller_tin)
                }
            }
            "seller_rdo" => draft.seller_rdo_code.clone(),
            "buyer_name" => draft.buyer_name.clone(),
            "seller_name" => draft.seller_name.clone(),
            "buyer_address" => draft.buyer_address.clone(),
            "seller_address" => draft.seller_address.clone(),
            "class_other" => draft.property_class_other.clone(),
            "location" => draft.property_location.clone(),
            "property_rdo" => draft.property_rdo_code.clone(),
            "tct" => draft.tct_number.clone(),
            "area" => draft.area_sold.clone(),
            "tax_dec" => draft.tax_declaration_number.clone(),
            "other_desc" => draft.other_description.clone(),
            "txn_other" => draft.transaction_other.clone(),
            "base_other_desc" => draft.others_base_description.clone(),
            "email" => draft.email.clone(),
            "gross" => money(draft.gross_selling_price),
            "cost" => money(draft.cost_and_expenses),
            "mortgage" => money(draft.mortgage_assumed),
            "initial" => money(draft.initial_year_payments),
            "inst_month" => money(draft.installment_this_month),
            "n_inst" => money(draft.number_of_installments),
            "inst_collected" => money(draft.installment_collected),
            "others_base" => money(draft.others_base),
            "bid" => money(draft.bid_price),
            "less" => money(draft.tax_remitted_previous),
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            "fmv_a" => fmv(draft.fmv_land_tax_declaration),
            "fmv_b" => fmv(draft.fmv_improvements_tax_declaration),
            "fmv_c" => fmv(draft.fmv_land_zonal),
            "fmv_d" => fmv(draft.fmv_improvements_bir),
            _ => String::new(),
        }
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
        let small = |value: f64, max: f64| {
            if (0.0..=max).contains(&value) {
                value
            } else {
                max + 1.0
            }
        };
        if let Some(value) = number("month", "Item 1 month", cx) {
            draft.transaction_month = small(value, 99.0) as u8;
        }
        if let Some(value) = number("day", "Item 1 day", cx) {
            draft.transaction_day = small(value, 99.0) as u8;
        }
        if let Some(value) = number("year", "Item 1 year", cx) {
            draft.transaction_year = if (0.0..=9999.0).contains(&value) {
                value as u16
            } else {
                0
            };
        }
        if let Some(value) = number("sheets", "Item 3", cx) {
            draft.number_of_attached_sheets = small(value, 999.0) as u16;
        }
        for (key, label) in MONEY_INPUTS {
            if let Some(value) = number(key, label, cx) {
                let slot = match *key {
                    "gross" => &mut draft.gross_selling_price,
                    "cost" => &mut draft.cost_and_expenses,
                    "mortgage" => &mut draft.mortgage_assumed,
                    "initial" => &mut draft.initial_year_payments,
                    "inst_month" => &mut draft.installment_this_month,
                    "n_inst" => &mut draft.number_of_installments,
                    "inst_collected" => &mut draft.installment_collected,
                    "others_base" => &mut draft.others_base,
                    "bid" => &mut draft.bid_price,
                    "less" => &mut draft.tax_remitted_previous,
                    "sur" => &mut draft.surcharge,
                    "int" => &mut draft.interest,
                    _ => &mut draft.compromise,
                };
                *slot = value;
            }
        }
        for (key, label) in FMV_INPUTS {
            let blank = self.input_text(key, cx).trim().is_empty();
            if let Some(value) = number(key, label, cx) {
                let value = (!blank).then_some(value);
                match *key {
                    "fmv_a" => draft.fmv_land_tax_declaration = value,
                    "fmv_b" => draft.fmv_improvements_tax_declaration = value,
                    "fmv_c" => draft.fmv_land_zonal = value,
                    _ => draft.fmv_improvements_bir = value,
                }
            }
        }
        let text = |key: &str| self.input_text(key, cx);
        let seller_tin: String = text("seller_tin")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        draft.seller_tin = seller_tin;
        draft.seller_rdo_code = text("seller_rdo").trim().to_string();
        draft.buyer_name = text("buyer_name");
        draft.seller_name = text("seller_name");
        draft.buyer_address = text("buyer_address");
        draft.seller_address = text("seller_address");
        draft.property_class_other = text("class_other");
        draft.property_location = text("location");
        draft.property_rdo_code = text("property_rdo").trim().to_string();
        draft.tct_number = text("tct").trim().to_string();
        draft.area_sold = text("area").trim().to_string();
        draft.tax_declaration_number = text("tax_dec").trim().to_string();
        draft.other_description = text("other_desc");
        draft.transaction_other = text("txn_other");
        draft.others_base_description = text("base_other_desc");
        draft.email = text("email").trim().to_string();
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// Reload every editor from the draft (after a handler cleared fields).
    fn reload_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        self.sync_from_inputs(cx);
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1606Draft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    fn pick_transaction(
        &mut self,
        transaction: Form1606Transaction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        self.draft.set_transaction(transaction);
        self.reload_inputs(window, cx);
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
                .release_abandoned_claimed_queueable::<Form1606Draft>(
                    &draft.tin,
                    draft.transaction_year,
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
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child(title.to_string()),
            )
            .children(children)
            .into_any_element()
    }

    fn has_error(&self, keys: &[&str]) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| keys.iter().any(|key| field == key))
    }

    /// The draft fields an editor key feeds (for highlighting).
    fn draft_fields(key: &str) -> &'static [&'static str] {
        match key {
            "month" => &["transaction_month"],
            "day" => &["transaction_day"],
            "year" => &["transaction_year"],
            "sheets" => &["number_of_attached_sheets"],
            "seller_tin" => &["seller_tin"],
            "seller_rdo" => &["seller_rdo_code"],
            "buyer_name" => &["buyer_name"],
            "seller_name" => &["seller_name"],
            "buyer_address" => &["buyer_address"],
            "seller_address" => &["seller_address"],
            "class_other" => &["property_class_other"],
            "location" => &["property_location"],
            "property_rdo" => &["property_rdo_code"],
            "tct" => &["tct_number"],
            "area" => &["area_sold"],
            "tax_dec" => &["tax_declaration_number"],
            "other_desc" => &["other_description"],
            "txn_other" => &["transaction_other"],
            "base_other_desc" => &["others_base_description"],
            "email" => &["email"],
            "gross" => &["gross_selling_price"],
            "cost" => &["cost_and_expenses"],
            "mortgage" => &["mortgage_assumed"],
            "initial" => &["initial_year_payments"],
            "inst_month" => &["installment_this_month"],
            "n_inst" => &["number_of_installments"],
            "inst_collected" => &["installment_collected"],
            "others_base" => &["others_base"],
            "bid" => &["bid_price"],
            "less" => &["tax_remitted_previous"],
            "sur" => &["surcharge"],
            "int" => &["interest"],
            "comp" => &["compromise"],
            "fmv_a" => &["fmv_land_tax_declaration"],
            "fmv_b" => &["fmv_improvements_tax_declaration"],
            "fmv_c" => &["fmv_land_zonal"],
            "fmv_d" => &["fmv_improvements_bir"],
            _ => &[],
        }
    }

    /// A labelled input. On phones the label sits above the field.
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let mut keys = vec![key];
        keys.extend_from_slice(Self::draft_fields(key));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if self.has_error(&keys) {
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

    fn text_field(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        let label = TEXT_INPUTS
            .iter()
            .map(|(k, label, _)| (*k, *label))
            .chain(MONEY_INPUTS.iter().copied())
            .chain(FMV_INPUTS.iter().copied())
            .find(|(k, _)| *k == key)
            .map(|(_, label)| label)
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

    fn computed(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_right()
                    .font_weight(FontWeight::BOLD)
                    .child(official_amount(value)),
            )
            .into_any_element()
    }

    fn readonly(&self, label: &str, value: String) -> AnyElement {
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

    /// A labelled row of choice buttons.
    fn choice_row(&self, label: &str, error_keys: &[&str], buttons: Vec<Button>) -> AnyElement {
        let title = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let title = if self.has_error(error_keys) {
            title.text_color(gpui::red())
        } else {
            title
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(title)
            .child(div().flex().flex_wrap().gap_2().children(buttons))
            .into_any_element()
    }

    fn yes_no(
        &self,
        id: &'static str,
        label: &str,
        error_key: &str,
        value: Option<bool>,
        set: fn(&mut Form1606Draft, bool),
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let buttons = [(true, "Yes"), (false, "No")]
            .into_iter()
            .map(|(answer, text)| {
                Self::choice(
                    (id, usize::from(answer)),
                    text,
                    value == Some(answer),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| set(d, answer))))
            })
            .collect();
        self.choice_row(label, &[error_key], buttons)
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let children = vec![
            self.text_field("month", layout, !editable),
            self.text_field("day", layout, !editable),
            self.text_field("year", layout, !editable),
            self.yes_no(
                "1606_amended",
                "Item 2 — Amended return?",
                "is_amended",
                Some(self.draft.is_amended),
                |d, v| d.is_amended = v,
                cx,
            ),
            self.text_field("sheets", layout, !editable),
            self.yes_no(
                "1606_withheld",
                "Item 4 — Any taxes withheld?",
                "taxes_withheld",
                self.draft.taxes_withheld,
                |d, v| d.taxes_withheld = Some(v),
                cx,
            ),
        ];
        self.section("Date of transaction", vec![self.grid(layout, children)], cx)
    }

    fn render_parties(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let seller_type = d.seller_type;
        let category = d.agent_category;
        let children = vec![
            self.readonly("Item 5 — Buyer's TIN", format_tin(&d.tin)),
            self.readonly("Item 6 — Buyer's RDO Code", d.rdo_code.clone()),
            self.text_field("seller_tin", layout, !editable),
            self.text_field("seller_rdo", layout, !editable),
            self.text_field("buyer_name", layout, !editable),
            self.text_field("seller_name", layout, !editable),
            self.text_field("buyer_address", layout, !editable),
            self.text_field("seller_address", layout, !editable),
            self.choice_row(
                "Item 13 — ATC (seller)",
                &["seller_type"],
                vec![
                    Self::choice(
                        "1606_atc_wi",
                        "WI155 — Individual",
                        seller_type == Some(Form1606SellerType::Individual),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| d.seller_type = Some(Form1606SellerType::Individual))
                    })),
                    Self::choice(
                        "1606_atc_wc",
                        "WC155 — Corporation",
                        seller_type == Some(Form1606SellerType::Corporation),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.seller_type = Some(Form1606SellerType::Corporation)
                        })
                    })),
                ],
            ),
            self.choice_row(
                "Item 14 — Category of Withholding Agent",
                &["agent_category"],
                vec![
                    Self::choice(
                        "1606_private",
                        "Private",
                        category == Some(Form1606AgentCategory::Private),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.agent_category = Some(Form1606AgentCategory::Private)
                        })
                    })),
                    Self::choice(
                        "1606_government",
                        "Government",
                        category == Some(Form1606AgentCategory::Government),
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.agent_category = Some(Form1606AgentCategory::Government)
                        })
                    })),
                ],
            ),
        ];
        self.section(
            "Part I — Buyer and Seller",
            vec![self.grid(layout, children)],
            cx,
        )
    }

    fn render_property(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let classes = Form1606PropertyClass::ALL
            .iter()
            .enumerate()
            .map(|(index, class)| {
                let class = *class;
                Self::choice(
                    ("1606_class", index),
                    class.label(),
                    d.property_class == Some(class),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.property_class = Some(class))
                }))
            })
            .collect();
        let relief = d.tax_relief;
        let relief_buttons = [
            ("1606_relief_no", "No", Form1606TaxRelief::No),
            (
                "1606_relief_itt",
                "Yes — International Tax Treaty",
                Form1606TaxRelief::InternationalTaxTreaty,
            ),
            (
                "1606_relief_sl",
                "Yes — Special Law",
                Form1606TaxRelief::SpecialLaw,
            ),
        ]
        .into_iter()
        .map(|(id, text, value)| {
            Self::choice(id, text, relief == value, !editable).on_click(
                cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.tax_relief = value)),
            )
        })
        .collect();
        let fields = vec![
            self.text_field(
                "class_other",
                layout,
                !editable || d.property_class != Some(Form1606PropertyClass::Others),
            ),
            self.text_field("location", layout, !editable),
            self.text_field("property_rdo", layout, !editable),
            self.text_field("tct", layout, !editable),
            self.text_field("area", layout, !editable),
            self.text_field("tax_dec", layout, !editable),
            self.text_field("other_desc", layout, !editable),
            self.yes_no(
                "1606_more_than_one",
                "Item 18 — Does the selling price cover more than one property?",
                "covers_more_than_one_property",
                d.covers_more_than_one_property,
                |d, v| d.covers_more_than_one_property = Some(v),
                cx,
            ),
        ];
        self.section(
            "Part I — Property",
            vec![
                self.choice_row(
                    "Item 15 — Classification of Property",
                    &["property_class"],
                    classes,
                ),
                self.grid(layout, fields),
                self.choice_row(
                    "Item 19 — Tax relief under an International Tax Treaty or Special Law?",
                    &["tax_relief"],
                    relief_buttons,
                ),
            ],
            cx,
        )
    }

    fn render_transaction(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let transaction = d.transaction;
        let installment = transaction == Some(Form1606Transaction::InstallmentSale);
        let cash = transaction == Some(Form1606Transaction::CashSale);
        let exempt_or_others = matches!(
            transaction,
            Some(Form1606Transaction::Exempt | Form1606Transaction::Others)
        );
        let kinds = Form1606Transaction::ALL
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let kind = *kind;
                Self::choice(
                    ("1606_txn", index),
                    kind.label(),
                    transaction == Some(kind),
                    !editable,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.pick_transaction(kind, window, cx)),
                )
            })
            .collect();
        let mut installment_fields = vec![self.computed(
            "21 — Selling Price (the 28A amount)",
            if installment {
                d.gross_selling_price
            } else {
                0.0
            },
            cx,
        )];
        for key in ["cost", "mortgage", "initial", "inst_month", "n_inst"] {
            installment_fields.push(self.text_field(key, layout, !editable || !installment));
        }
        let fmv: Vec<AnyElement> = FMV_INPUTS
            .iter()
            .map(|(key, _)| self.text_field(key, layout, !editable))
            .collect();
        let boxes = [
            ("A", Form1606TaxableBaseBox::GrossSellingPrice),
            ("B", Form1606TaxableBaseBox::FairMarketValue),
            ("C", Form1606TaxableBaseBox::BidPrice),
            ("D", Form1606TaxableBaseBox::InstallmentCollected),
            ("E", Form1606TaxableBaseBox::Others),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (text, value))| {
            Self::choice(
                ("1606_box28", index),
                format!("28{text}"),
                d.taxable_base_box == Some(value),
                !editable,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.edit(cx, |d| d.taxable_base_box = Some(value))
            }))
        })
        .collect();
        let base_fields = vec![
            self.text_field("gross", layout, !editable),
            self.computed(
                "28B — FMV of Land and Improvement (higher pair of Item 27)",
                d.fmv_land_and_improvement,
                cx,
            ),
            self.text_field("bid", layout, !editable || cash),
            self.text_field("inst_collected", layout, !editable || cash),
            self.text_field("base_other_desc", layout, !editable || cash),
            self.text_field("others_base", layout, !editable || cash),
        ];
        self.section(
            "Part I — Transaction and Taxable Base",
            vec![
                self.choice_row(
                    "Item 20 — Description of Transaction",
                    &["transaction"],
                    kinds,
                ),
                self.text_field("txn_other", layout, !editable || !exempt_or_others),
                self.grid(layout, installment_fields),
                self.grid(layout, fmv),
                self.choice_row(
                    "Item 28 — Determination of Taxable Base (mark one)",
                    &["taxable_base_box"],
                    boxes,
                ),
                self.grid(layout, base_fields),
                self.yes_no(
                    "1606_habitual",
                    "Item 29 — Is the seller habitually engaged in real estate business?",
                    "seller_habitual",
                    d.seller_habitual,
                    |d, v| d.seller_habitual = Some(v),
                    cx,
                ),
            ],
            cx,
        )
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let withheld = d.taxes_withheld == Some(true);
        let rate = d.tax_rate;
        let mut rates: Vec<Button> = Form1606TaxRate::ALL
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let value = *value;
                Self::choice(
                    ("1606_rate", index),
                    format!("{}%", value.value()),
                    rate == Some(value),
                    !editable || !withheld,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.tax_rate = Some(value))),
                )
            })
            .collect();
        rates.push(
            Self::choice("1606_rate_none", "Not selected", rate.is_none(), !editable)
                .on_click(cx.listener(|this, _, _, cx| this.edit(cx, |d| d.tax_rate = None))),
        );
        let mut children = vec![
            self.computed("30 — Taxable Base", d.taxable_base, cx),
            self.computed("32 — Tax Required to be Withheld", d.tax_required, cx),
            self.text_field("less", layout, !editable || !d.is_amended),
            self.computed("34 — Tax Still Due/(Overremittance)", d.tax_still_due, cx),
            self.text_field("sur", layout, !editable || !withheld),
            self.text_field("int", layout, !editable || !withheld),
            self.text_field("comp", layout, !editable || !withheld),
            self.computed("35D — Total Penalties", d.total_penalties, cx),
            self.computed(
                "36 — Total Amount Still Due/(Overremittance)",
                d.total_amount_due,
                cx,
            ),
            self.text_field("email", layout, !editable),
        ];
        if d.total_amount_due < 0.0 {
            let over = d.overremittance;
            children.push(self.choice_row(
                "Item 37 — If overremittance, mark one",
                &["overremittance"],
                vec![
                        Self::choice(
                            "1606_refund",
                            "To be refunded",
                            over == Form1606Overremittance::Refund,
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.overremittance = Form1606Overremittance::Refund)
                        })),
                        Self::choice(
                            "1606_tcc",
                            "To be issued a Tax Credit Certificate",
                            over == Form1606Overremittance::TaxCreditCertificate,
                            !editable,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.overremittance = Form1606Overremittance::TaxCreditCertificate
                            })
                        })),
                    ],
            ));
        }
        self.section(
            "Part II — Computation of Tax",
            vec![
                self.choice_row("Item 31 — Tax Rate", &["tax_rate"], rates),
                self.grid(layout, children),
            ],
            cx,
        )
    }
}

impl QueueableFormView for Form1606View {
    type Draft = Form1606Draft;

    fn new(
        draft: Form1606Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        for (key, _) in FMV_INPUTS {
            keys.push((key.to_string(), "blank".into()));
        }
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1606Draft {
        let month = if (1..=12).contains(&period) {
            period
        } else {
            chrono::Datelike::month(&chrono::Local::now().date_naive()) as u8
        };
        Form1606Draft::new_from_profile(profile, year, month)
    }

    /// 1606 is filed per transaction; drafts are keyed by the transaction
    /// month. Open that month's draft, or this year's latest one.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form1606Draft {
        let tin = profile.tin.full();
        let months: Vec<i64> = if (1..=12).contains(&period) {
            vec![i64::from(period)]
        } else {
            (1..=12).collect()
        };
        months
            .into_iter()
            .filter_map(|m| {
                db.get_queueable_draft::<Form1606Draft>(&tin, year, m)
                    .ok()
                    .flatten()
            })
            .max_by(|a, b| a.lifecycle.updated_at.cmp(&b.lifecycle.updated_at))
            .unwrap_or_else(|| Self::new_draft(profile, year, period))
    }
}

impl FormViewTrait for Form1606View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1606"
    }
    fn form_subtitle(&self) -> &'static str {
        "Withholding Tax Remittance Return for Onerous Transfer of Real Property Other Than Capital Asset"
    }
    fn form_version(&self) -> &'static str {
        "2018 (ENCS)"
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
                    "1606 draft saved.".into(),
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
        if !can_queue_for_submission(Form1606Draft::FORM_CODE) {
            self.status_message =
                Some("1606 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 1606. No submission was started: {error}"
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
            "Form 1606 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1606 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1606Draft>(
                        &queued.tin,
                        queued.transaction_year,
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
            "1606-2018",
            &fields,
            "1606 — Print Preview",
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

impl Render for Form1606View {
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
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(message.clone())
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
                .child(Button::new("1606_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("1606_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("1606_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("1606_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("1606_submit")
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
                            Button::new("1606_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1606_release_cancel")
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
            .child(self.render_parties(layout, cx))
            .child(self.render_property(layout, cx))
            .child(self.render_transaction(layout, cx))
            .child(self.render_computation(layout, cx));
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
                    .id("1606_scroll")
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
    use super::{Layout, format_tin, parse_amount};
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
        assert_eq!(format_tin("98765432100000"), "987-654-321-00000");
    }
}
