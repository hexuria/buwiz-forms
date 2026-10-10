//! Editor for BIR Form 0605, Payment Form (July 1999). Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_0605_official`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_0605::{
    FORM_VERSION_LABEL, Form0605ApprovalSelection, Form0605Date, Form0605Draft,
    Form0605FilingBasis, Form0605MannerOfPayment, Form0605TaxpayerClassification,
    Form0605TypeOfPayment,
};
use bir_core::forms::form_0605_official::{
    FORM_0605_ATCS, FORM_0605_TAX_TYPES, form_0605_atc, form_0605_atc_offered_for,
    form_0605_tax_type,
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

impl EventEmitter<QueueableFormEvent> for Form0605View {}

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

/// Matches the pickers show at once.
const PICKER_LIMIT: usize = 24;

/// Every editor input: (key, label, placeholder).
const INPUTS: &[(&str, &str, &str)] = &[
    ("year", "Item 2 — Year ended", "YYYY"),
    ("due", "Item 4 — Due date (MM/DD/YYYY)", "optional"),
    ("sheets", "Item 5 — No. of sheets attached", "0"),
    (
        "period",
        "Item 7 — Return period (MM/DD/YYYY)",
        "MM/DD/YYYY",
    ),
    ("tax_filter", "Item 8 — Find a tax type", "code or name"),
    ("atc_filter", "Item 6 — Find an ATC", "code or name"),
    ("lob", "Item 12 — Line of Business/Occupation", ""),
    ("name", "Item 13 — Taxpayer's Name", ""),
    ("phone", "Item 14 — Telephone Number", "digits only"),
    ("address", "Item 15 — Registered Address", ""),
    ("zip", "Item 16 — Zip Code", ""),
    ("email", "Email address for the BIR confirmation", ""),
    ("others", "Item 17 — Others (specify)", ""),
    ("installments", "Item 18 — No. of installments (1–20)", ""),
    ("i19", "19 — Basic tax / deposit / advance payment", "0.00"),
    ("i20a", "20A — Surcharge", "0.00"),
    ("i20b", "20B — Interest", "0.00"),
    ("i20c", "20C — Compromise", "0.00"),
    (
        "sig_taxpayer",
        "Item 22 — Taxpayer / authorized representative",
        "",
    ),
    ("sig_title", "Item 22 — Title / position", ""),
    ("sig_head", "Item 22 — Head of office", ""),
    ("cash", "23 — Cash / bank debit memo amount", "0.00"),
    ("check_bank", "24 — Check: drawee bank / agency", ""),
    ("check_number", "24 — Check: number", ""),
    ("check_date", "24 — Check: date (MM/DD/YYYY)", ""),
    ("check_amount", "24 — Check: amount", "0.00"),
    ("tdm_number", "25 — Tax debit memo: number", ""),
    ("tdm_date", "25 — Tax debit memo: date (MM/DD/YYYY)", ""),
    ("tdm_amount", "25 — Tax debit memo: amount", "0.00"),
    ("other_bank", "26 — Others: drawee bank / agency", ""),
    ("other_number", "26 — Others: number", ""),
    ("other_date", "26 — Others: date (MM/DD/YYYY)", ""),
    ("other_amount", "26 — Others: amount", "0.00"),
    ("machine", "Machine validation / receipt details", ""),
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

/// Blank is `None`; otherwise MM/DD/YYYY.
fn parse_date(value: &str) -> Result<Option<Form0605Date>, ()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Form0605Date::parse_mm_dd_yyyy(trimmed)
        .map(Some)
        .map_err(|_| ())
}

fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

fn optional_money_text(value: Option<f64>) -> String {
    value.map(official_amount).unwrap_or_default()
}

/// Rows of an official code table matching `filter` (code or description).
fn matching(
    table: &[(&'static str, &'static str)],
    filter: &str,
) -> Vec<(usize, &'static str, &'static str)> {
    let needle = filter.trim().to_uppercase();
    table
        .iter()
        .enumerate()
        .filter(|(_, (code, description))| {
            needle.is_empty()
                || code.to_uppercase().contains(&needle)
                || description.to_uppercase().contains(&needle)
        })
        .map(|(index, (code, description))| (index, *code, *description))
        .collect()
}

pub struct Form0605View {
    draft: Form0605Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form0605View {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form0605Draft, key: &str) -> String {
        let p = &draft.payment_details;
        match key {
            "year" => draft.taxable_year.to_string(),
            "due" => draft.due_date.map(|d| d.to_string()).unwrap_or_default(),
            "sheets" => draft.number_of_sheets.to_string(),
            "period" => draft
                .return_period
                .map(|d| d.to_string())
                .unwrap_or_default(),
            "lob" => draft.line_of_business.clone(),
            "name" => draft.taxpayer_name.clone(),
            "phone" => draft.contact_number.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "email" => draft.email.clone(),
            "others" => draft.other_manner_description.clone(),
            "installments" => draft
                .number_of_installments
                .map(|n| n.to_string())
                .unwrap_or_default(),
            "i19" => money_text(draft.item_19_basic_tax_or_payment),
            "i20a" => money_text(draft.item_20a_surcharge),
            "i20b" => money_text(draft.item_20b_interest),
            "i20c" => money_text(draft.item_20c_compromise),
            "sig_taxpayer" => draft
                .signatures
                .taxpayer_or_authorized_representative
                .clone(),
            "sig_title" => draft.signatures.title_or_position.clone(),
            "sig_head" => draft.signatures.head_of_office.clone(),
            "cash" => optional_money_text(p.cash_or_bank_debit_memo_amount),
            "check_bank" => p.check.drawee_bank_or_agency.clone(),
            "check_number" => p.check.number.clone(),
            "check_date" => p.check.date.clone(),
            "check_amount" => optional_money_text(p.check.amount),
            "tdm_number" => p.tax_debit_memo.number.clone(),
            "tdm_date" => p.tax_debit_memo.date.clone(),
            "tdm_amount" => optional_money_text(p.tax_debit_memo.amount),
            "other_bank" => p.others.drawee_bank_or_agency.clone(),
            "other_number" => p.others.number.clone(),
            "other_date" => p.others.date.clone(),
            "other_amount" => optional_money_text(p.others.amount),
            "machine" => p.machine_validation_or_receipt_details.clone(),
            _ => String::new(),
        }
    }

    fn label_of(key: &str) -> &'static str {
        INPUTS
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, label, _)| *label)
            .unwrap_or("")
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Read every editor value into the draft, then recompute and validate.
    /// A malformed number or date is reported instead of becoming zero.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let mut errors = Vec::new();
        let mut draft = self.draft.clone();
        let number = |key: &str, text: String, errors: &mut Vec<(String, String)>| {
            let parsed = parse_amount(&text);
            if parsed.is_none() {
                errors.push((
                    key.to_string(),
                    format!("{}: \"{text}\" is not a number.", Self::label_of(key)),
                ));
            }
            parsed
        };
        if let Some(year) = number("year", self.input_text("year", cx), &mut errors) {
            draft.taxable_year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
        }
        if let Some(sheets) = number("sheets", self.input_text("sheets", cx), &mut errors) {
            draft.number_of_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        for (key, slot) in [
            ("i19", &mut draft.item_19_basic_tax_or_payment),
            ("i20a", &mut draft.item_20a_surcharge),
            ("i20b", &mut draft.item_20b_interest),
            ("i20c", &mut draft.item_20c_compromise),
        ] {
            if let Some(value) = number(key, self.input_text(key, cx), &mut errors) {
                *slot = value;
            }
        }
        let installments = self.input_text("installments", cx);
        draft.number_of_installments = if installments.trim().is_empty() {
            None
        } else {
            match installments.trim().parse::<u16>() {
                Ok(count) => Some(count),
                Err(_) => {
                    errors.push((
                        "installments".to_string(),
                        format!("Item 18: \"{installments}\" is not a whole number."),
                    ));
                    draft.number_of_installments
                }
            }
        };
        let payments = &mut draft.payment_details;
        for (key, slot) in [
            ("cash", &mut payments.cash_or_bank_debit_memo_amount),
            ("check_amount", &mut payments.check.amount),
            ("tdm_amount", &mut payments.tax_debit_memo.amount),
            ("other_amount", &mut payments.others.amount),
        ] {
            let text = self.input_text(key, cx);
            if text.trim().is_empty() {
                *slot = None;
            } else if let Some(value) = number(key, text, &mut errors) {
                *slot = Some(value);
            }
        }
        for (key, slot) in [
            ("due", &mut draft.due_date),
            ("period", &mut draft.return_period),
        ] {
            let text = self.input_text(key, cx);
            match parse_date(&text) {
                Ok(value) => *slot = value,
                Err(()) => errors.push((
                    key.to_string(),
                    format!(
                        "{}: \"{text}\" is not a MM/DD/YYYY date.",
                        Self::label_of(key)
                    ),
                )),
            }
        }
        draft.line_of_business = self.input_text("lob", cx);
        draft.taxpayer_name = self.input_text("name", cx);
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.other_manner_description = self.input_text("others", cx);
        draft.signatures.taxpayer_or_authorized_representative =
            self.input_text("sig_taxpayer", cx);
        draft.signatures.title_or_position = self.input_text("sig_title", cx);
        draft.signatures.head_of_office = self.input_text("sig_head", cx);
        let payments = &mut draft.payment_details;
        payments.check.drawee_bank_or_agency = self.input_text("check_bank", cx);
        payments.check.number = self.input_text("check_number", cx);
        payments.check.date = self.input_text("check_date", cx).trim().to_string();
        payments.tax_debit_memo.number = self.input_text("tdm_number", cx);
        payments.tax_debit_memo.date = self.input_text("tdm_date", cx).trim().to_string();
        payments.others.drawee_bank_or_agency = self.input_text("other_bank", cx);
        payments.others.number = self.input_text("other_number", cx);
        payments.others.date = self.input_text("other_date", cx).trim().to_string();
        payments.machine_validation_or_receipt_details = self.input_text("machine", cx);

        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form0605Draft)) {
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
                .release_abandoned_claimed_queueable::<Form0605Draft>(
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
    fn field(&self, key: &str, layout: Layout, disabled: bool) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled || !editable));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(Self::label_of(key).to_string());
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
                .child(label.w(relative(0.5)))
                .child(body.w(relative(0.5))),
        }
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

    fn computed(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div text_right font_weight={FontWeight::BOLD}>{official_amount(value)}</div>
            </div>
        }
        .into_any_element()
    }

    fn fixed(label: &str, value: String) -> AnyElement {
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

    fn choice_row(title: &str, buttons: Vec<Button>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title.to_string()),
            )
            .children(buttons)
            .into_any_element()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fiscal = d.filing_basis == Form0605FilingBasis::Fiscal;
        let mut months = div().flex().flex_wrap().gap_1();
        for (index, name) in MONTHS.iter().enumerate() {
            let month = index as u8 + 1;
            months = months.child(
                Self::choice(
                    ("0605_month", index),
                    *name,
                    d.year_end_month == month,
                    !editable || !fiscal || month == 12,
                )
                .small()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.year_end_month = month)),
                ),
            );
        }
        let quarters: Vec<Button> = (0..=4u8)
            .map(|quarter| {
                let label = if quarter == 0 {
                    "None".to_string()
                } else {
                    format!("Q{quarter}")
                };
                Self::choice(
                    ("0605_qtr", quarter as usize),
                    label,
                    d.quarter == quarter,
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.quarter = quarter)))
            })
            .collect();
        let children = vec![
            Self::choice_row(
                "Item 1 —",
                vec![
                    Self::choice("0605_calendar", "Calendar", !fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.filing_basis = Form0605FilingBasis::Calendar)
                        }),
                    ),
                    Self::choice("0605_fiscal", "Fiscal", fiscal, !editable).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.filing_basis = Form0605FilingBasis::Fiscal;
                                if d.year_end_month == 12 {
                                    d.year_end_month = 6;
                                }
                            })
                        },
                    )),
                ],
            ),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 2 — Year ended (month)"),
                )
                .child(months)
                .into_any_element(),
            self.field("year", layout, false),
            Self::choice_row("Item 3 — Quarter", quarters),
            self.field("due", layout, false),
            self.field("sheets", layout, false),
            self.field("period", layout, false),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_codes(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let tax_code = d.tax_type.as_ref().map(|t| t.code().to_string());
        let atc_code = d.atc.as_ref().map(|a| a.code().to_string());
        let describe = |table: &[(&str, &str)], code: &Option<String>| {
            code.as_ref()
                .and_then(|code| table.iter().find(|(c, _)| c == code))
                .map(|(c, description)| format!("{c} — {description}"))
                .unwrap_or_else(|| "Not selected".to_string())
        };
        let mut tax_picks = div().flex().flex_wrap().gap_2();
        let tax_matches = matching(FORM_0605_TAX_TYPES, &self.input_text("tax_filter", cx));
        let tax_count = tax_matches.len();
        for (index, code, description) in tax_matches.into_iter().take(PICKER_LIMIT) {
            tax_picks = tax_picks.child(
                Self::choice(
                    ("0605_tax", index),
                    format!("{code} — {description}"),
                    tax_code.as_deref() == Some(code),
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| {
                        // getTaxTypeCode() clears the ATC.
                        d.tax_type = form_0605_tax_type(code);
                        d.atc = None;
                    })
                })),
            );
        }
        let mut atc_picks = div().flex().flex_wrap().gap_2();
        let mut atc_count = 0;
        if let Some(tax) = tax_code.clone() {
            let offered: Vec<_> = matching(FORM_0605_ATCS, &self.input_text("atc_filter", cx))
                .into_iter()
                .filter(|(_, code, _)| form_0605_atc_offered_for(code, &tax))
                .collect();
            atc_count = offered.len();
            for (index, code, description) in offered.into_iter().take(PICKER_LIMIT) {
                atc_picks = atc_picks.child(
                    Self::choice(
                        ("0605_atc", index),
                        format!("{code} — {description}"),
                        atc_code.as_deref() == Some(code),
                        !editable,
                    )
                    .small()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(cx, |d| d.atc = form_0605_atc(code))
                    })),
                );
            }
        }
        let more = |shown: usize| {
            div()
                .text_xs()
                .child(if shown > PICKER_LIMIT {
                    format!("{shown} match; showing {PICKER_LIMIT}. Type to narrow.")
                } else {
                    format!("{shown} match.")
                })
                .into_any_element()
        };
        let mut children = vec![
            Self::fixed(
                "Item 8 — Tax type",
                describe(FORM_0605_TAX_TYPES, &tax_code),
            ),
            self.field("tax_filter", layout, false),
            tax_picks.into_any_element(),
            more(tax_count),
            Self::fixed("Item 6 — ATC", describe(FORM_0605_ATCS, &atc_code)),
        ];
        if tax_code.is_some() {
            children.push(self.field("atc_filter", layout, false));
            children.push(atc_picks.into_any_element());
            children.push(more(atc_count));
        } else {
            children.push(
                div()
                    .text_sm()
                    .child("Please select a valid Tax Type on Item 8.")
                    .into_any_element(),
            );
        }
        self.section("Tax type and ATC (Items 6 and 8)", children, cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let individual = d.classification == Form0605TaxpayerClassification::Individual;
        let fields = vec![
            Self::fixed("Item 9 — TIN", format_tin(&d.tin)),
            Self::fixed("Item 10 — RDO Code", d.rdo_code.clone()),
            self.field("lob", layout, false),
            self.field("name", layout, false),
            self.field("phone", layout, false),
            self.field("address", layout, false),
            self.field("zip", layout, false),
            self.field("email", layout, false),
        ];
        let rows = vec![
            Self::choice_row(
                "Item 11 — Taxpayer classification",
                vec![
                    Self::choice("0605_individual", "Individual", individual, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.classification = Form0605TaxpayerClassification::Individual
                            })
                        }),
                    ),
                    Self::choice(
                        "0605_non_individual",
                        "Non-Individual",
                        !individual,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.classification = Form0605TaxpayerClassification::NonIndividual
                        })
                    })),
                ],
            ),
            self.grid(layout, fields),
        ];
        self.section("Part I — Background Information", rows, cx)
    }

    fn render_payment(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let manners: Vec<Button> = Form0605MannerOfPayment::ALL
            .into_iter()
            .enumerate()
            .map(|(index, manner)| {
                Self::choice(
                    ("0605_manner", index),
                    manner.label(),
                    d.manner_of_payment == Some(manner),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.manner_of_payment = Some(manner))
                }))
            })
            .collect();
        let types: Vec<Button> = Form0605TypeOfPayment::ALL
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                Self::choice(
                    ("0605_type", index),
                    kind.label(),
                    d.type_of_payment == Some(kind),
                    !editable,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit(cx, |d| d.type_of_payment = Some(kind))
                }))
            })
            .collect();
        let assessment = matches!(
            d.manner_of_payment,
            Some(
                Form0605MannerOfPayment::PreliminaryOrFinalAssessmentOrDeficiencyTax
                    | Form0605MannerOfPayment::AccountsReceivableOrDelinquentAccount
            )
        );
        let mut children = vec![Self::choice_row("Item 17 — Manner of payment", manners)];
        if d.manner_of_payment == Some(Form0605MannerOfPayment::Others) {
            children.push(self.field("others", layout, false));
        }
        if assessment {
            let approval = d.approval_selection;
            children.push(Self::choice_row(
                "Approval —",
                vec![
                    Self::choice(
                        "0605_preapproved",
                        "Pre-approved by Investigating Office",
                        approval == Form0605ApprovalSelection::XmlOption1,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.approval_selection = Form0605ApprovalSelection::XmlOption1
                        })
                    })),
                    Self::choice(
                        "0605_not_approved",
                        "Not approved by Investigating Office",
                        approval == Form0605ApprovalSelection::XmlOption2,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.approval_selection = Form0605ApprovalSelection::XmlOption2
                        })
                    })),
                ],
            ));
        }
        children.push(Self::choice_row("Item 18 — Type of payment", types));
        if d.type_of_payment == Some(Form0605TypeOfPayment::Installment) {
            children.push(self.field("installments", layout, false));
        }
        let amounts = vec![
            self.field("i19", layout, false),
            self.field("i20a", layout, false),
            self.field("i20b", layout, false),
            self.field("i20c", layout, false),
            self.computed("20D — Total penalties", d.item_20d_total_penalties, cx),
            self.computed(
                "21 — Total amount payable",
                d.item_21_total_amount_payable,
                cx,
            ),
        ];
        children.push(self.grid(layout, amounts));
        self.section("Part II — Payment", children, cx)
    }

    fn render_printed_only(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let keys = [
            "sig_taxpayer",
            "sig_title",
            "sig_head",
            "cash",
            "check_bank",
            "check_number",
            "check_date",
            "check_amount",
            "tdm_number",
            "tdm_date",
            "tdm_amount",
            "other_bank",
            "other_number",
            "other_date",
            "other_amount",
            "machine",
        ];
        let fields = keys
            .iter()
            .map(|key| self.field(key, layout, false))
            .collect();
        self.section(
            "Signatures and Part III payment details (printed, not submitted)",
            vec![self.grid(layout, fields)],
            cx,
        )
    }
}

impl QueueableFormView for Form0605View {
    type Draft = Form0605Draft;

    fn new(
        draft: Form0605Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (key, _, placeholder) in INPUTS {
            let value = Self::initial(&draft, key);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(*placeholder));
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
            inputs.insert(key.to_string(), input);
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

    /// The dashboard's open-ended key is the next payment number; the draft
    /// stores it as its `month` slot (1–12), as the pre-generic editor did.
    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form0605Draft {
        Form0605Draft::new_from_profile(profile, year, period)
    }

    /// Drafts saved by the pre-generic editor keep a NULL period column and
    /// are found by their `period_key`; the first save adopts the row.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form0605Draft {
        let tin = profile.tin.full();
        let slot = period.clamp(1, 12);
        db.get_queueable_draft::<Form0605Draft>(&tin, year, i64::from(slot))
            .ok()
            .flatten()
            .or_else(|| {
                db.get_form_draft_v2::<Form0605Draft>(
                    &tin,
                    "0605",
                    year,
                    &FilingPeriod::OpenEnded(u32::from(slot)),
                )
                .ok()
                .flatten()
            })
            .unwrap_or_else(|| Self::new_draft(profile, year, slot))
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

impl FormViewTrait for Form0605View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 0605"
    }
    fn form_subtitle(&self) -> &'static str {
        "Payment Form"
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
                    "0605 draft saved.".into(),
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
        if !can_queue_for_submission(<Form0605Draft as QueueableForm>::FORM_CODE) {
            self.status_message =
                Some("0605 is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 0605. No submission was started: {error}"
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
            "Form 0605 queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("0605 payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form0605Draft>(
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
            "0605-1999",
            &fields,
            "0605 — Print Preview",
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

impl Render for Form0605View {
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
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.replace('\n', " ")}</div> }
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
                .child(Button::new("0605_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("0605_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("0605_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("0605_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("0605_submit")
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
                            Button::new("0605_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("0605_release_cancel")
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
            .child(self.render_codes(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_payment(layout, cx))
            .child(self.render_printed_only(layout, cx));
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
                    .id("0605_scroll")
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
    use super::{FORM_0605_TAX_TYPES, Layout, format_tin, matching, parse_amount, parse_date};
    use gpui::px;

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn entries_parse_like_the_official_page() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(parse_date(""), Ok(None));
        assert!(parse_date("12/31/2025").unwrap().is_some());
        assert!(parse_date("31/12/2025").is_err());
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
        assert!(
            matching(FORM_0605_TAX_TYPES, "income")
                .iter()
                .any(|(_, code, _)| *code == "IT")
        );
    }
}
