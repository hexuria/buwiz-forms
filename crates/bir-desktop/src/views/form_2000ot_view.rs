//! Editor for BIR Form 2000-OT (v2018), Documentary Stamp Tax
//! Declaration/Return for One-Time Transactions. Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_2000ot`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2000ot::{
    FORM_2000OT_REAL_PROPERTY_ROWS, FORM_2000OT_SHARE_ROWS, Form2000OTAtc, Form2000OTDraft,
    Form2000OTNature, Form2000OTOtherParty, Form2000OTPropertyRow, Form2000OTShareRow,
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
use gpui_rsx::rsx;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form2000OTView {}

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

/// Single text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("date", "Item 1 — Date of transaction", "MM/DD/YYYY"),
    ("sheets", "Item 4 — No. of sheets attached", "0"),
    ("name", "Item 7 — Taxpayer's Name", ""),
    ("address", "Item 8 — Registered Address", ""),
    ("zip", "Item 8A — Zip Code", ""),
    ("phone", "Item 9 — Contact Number", "digits only"),
    ("email", "Item 10 — Email Address", ""),
    ("oname", "Item 11A — Other party's name", ""),
    ("otin", "Item 11B — Other party's TIN", "digits only"),
    ("loc", "Item 13 — Location of real property", ""),
    ("others", "Schedule 2 C — Others (specify)", ""),
];

/// Money inputs outside the schedule rows: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    (
        "gsp",
        "Schedule 2 A — Gross selling price / bid price / net gift",
    ),
    ("othamt", "Schedule 2 C — Others amount"),
    (
        "prev",
        "18 — Tax paid in return previously filed (amended only)",
    ),
    ("sur", "20A — Surcharge"),
    ("int", "20B — Interest"),
    ("comp", "20C — Compromise"),
];

/// Schedule 1.A row inputs: (prefix, label, money?).
const PROPERTY_INPUTS: &[(&str, &str, bool)] = &[
    ("ptct", "OCT/TCT/CCT No.", false),
    ("ptd", "Tax Declaration No.", false),
    ("ploc", "Location", false),
    ("plot", "Lot / Improvement", false),
    ("pcls", "Classification", false),
    ("parea", "Area (sq.m.)", false),
    ("pfmv1", "FMV per TD (Column 1)", true),
    ("pfmv2", "FMV per BIR zonal value (Column 2)", true),
];

/// Schedule 1.B row inputs: (prefix, label, money?).
const SHARE_INPUTS: &[(&str, &str, bool)] = &[
    ("scorp", "(a) Name of corporation", false),
    ("sno", "(b) No. of shares sold", false),
    ("scert", "(c) Stock certificate no.", false),
    ("spar", "(d) Par value of shares", true),
    ("sdst", "(e) DST paid on original issue (no par)", true),
];

fn row_key(prefix: &str, row: usize) -> String {
    format!("{prefix}{row}")
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

fn money_text(value: f64) -> String {
    if value == 0.0 {
        String::new()
    } else {
        official_amount(value)
    }
}

pub struct Form2000OTView {
    draft: Form2000OTDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2000OTView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2000OTDraft, key: &str) -> String {
        match key {
            "date" => draft.transaction_date.clone(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "oname" => draft.other_party_name.clone(),
            "otin" => draft.other_party_tin.clone(),
            "loc" => draft.real_property_location.clone(),
            "others" => draft.others_description.clone(),
            "gsp" => money_text(draft.gross_selling_price),
            "othamt" => money_text(draft.others_amount),
            "prev" => money_text(draft.tax_paid_previous),
            "sur" => money_text(draft.surcharge),
            "int" => money_text(draft.interest),
            "comp" => money_text(draft.compromise),
            _ => {
                let (prefix, index) = key.split_at(key.len().saturating_sub(1));
                let Ok(row) = index.parse::<usize>() else {
                    return String::new();
                };
                if let Some(p) = draft.properties.get(row) {
                    let value = match prefix {
                        "ptct" => Some(p.title_number.clone()),
                        "ptd" => Some(p.tax_declaration_number.clone()),
                        "ploc" => Some(p.location.clone()),
                        "plot" => Some(p.lot.clone()),
                        "pcls" => Some(p.classification.clone()),
                        "parea" => Some(p.area.clone()),
                        "pfmv1" => Some(money_text(p.fmv_per_td)),
                        "pfmv2" => Some(money_text(p.fmv_zonal)),
                        _ => None,
                    };
                    if let Some(value) = value {
                        return value;
                    }
                }
                if let Some(s) = draft.shares.get(row) {
                    return match prefix {
                        "scorp" => s.corporation.clone(),
                        "sno" => s.shares_sold.clone(),
                        "scert" => s.certificate_number.clone(),
                        "spar" => money_text(s.par_value),
                        "sdst" => money_text(s.dst_paid_on_issue),
                        _ => String::new(),
                    };
                }
                String::new()
            }
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
        if let Some(sheets) = number("sheets", "Item 4", cx) {
            draft.number_of_attached_sheets = if (0.0..=999.0).contains(&sheets) {
                sheets as u16
            } else {
                999
            };
        }
        for (key, label) in MONEY_INPUTS {
            if let Some(value) = number(key, label, cx) {
                match *key {
                    "gsp" => draft.gross_selling_price = value,
                    "othamt" => draft.others_amount = value,
                    "prev" => draft.tax_paid_previous = value,
                    "sur" => draft.surcharge = value,
                    "int" => draft.interest = value,
                    _ => draft.compromise = value,
                }
            }
        }
        let mut properties = Vec::new();
        for row in 0..FORM_2000OT_REAL_PROPERTY_ROWS {
            let text = |prefix: &str| {
                self.input_text(&row_key(prefix, row), cx)
                    .trim()
                    .to_string()
            };
            let fmv_per_td =
                number(&row_key("pfmv1", row), "Schedule 1.A column 1", cx).unwrap_or(0.0);
            let fmv_zonal =
                number(&row_key("pfmv2", row), "Schedule 1.A column 2", cx).unwrap_or(0.0);
            properties.push(Form2000OTPropertyRow {
                title_number: text("ptct"),
                tax_declaration_number: text("ptd"),
                location: text("ploc"),
                lot: text("plot"),
                classification: text("pcls"),
                area: text("parea"),
                fmv_per_td,
                fmv_zonal,
                fmv: 0.0,
            });
        }
        while properties
            .last()
            .is_some_and(|r| *r == Form2000OTPropertyRow::default())
        {
            properties.pop();
        }
        let mut shares = Vec::new();
        for row in 0..FORM_2000OT_SHARE_ROWS {
            let text = |prefix: &str| {
                self.input_text(&row_key(prefix, row), cx)
                    .trim()
                    .to_string()
            };
            let par_value = number(&row_key("spar", row), "Schedule 1.B (d)", cx).unwrap_or(0.0);
            let dst_paid_on_issue =
                number(&row_key("sdst", row), "Schedule 1.B (e)", cx).unwrap_or(0.0);
            shares.push(Form2000OTShareRow {
                corporation: text("scorp"),
                shares_sold: text("sno"),
                certificate_number: text("scert"),
                par_value,
                dst_paid_on_issue,
            });
        }
        while shares
            .last()
            .is_some_and(|r| *r == Form2000OTShareRow::default())
        {
            shares.pop();
        }
        draft.properties = properties;
        draft.shares = shares;
        draft.transaction_date = self.input_text("date", cx).trim().to_string();
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.other_party_name = self.input_text("oname", cx);
        draft.other_party_tin = self.input_text("otin", cx).trim().to_string();
        draft.real_property_location = self.input_text("loc", cx);
        draft.others_description = self.input_text("others", cx).trim().to_string();
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2000OTDraft)) {
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
                .release_abandoned_claimed_queueable::<Form2000OTDraft>(
                    &draft.tin,
                    draft.taxable_year(),
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
    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let body = div().child(Input::new(input).disabled(disabled));
        let error = self
            .validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| field == key || field.starts_with(&format!("{key}[")));
        let label = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let label = if error {
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

    fn label_row(label: &str) -> Div {
        div().flex().flex_wrap().items_center().gap_2().child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
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

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let amended = self.draft.is_amended;
        let atc = self.draft.atc;
        let mut atc_row = Self::label_row("Item 3 — ATC");
        for (index, (value, label)) in [
            (
                Form2000OTAtc::Do102,
                "DO102 — shares with par value (P1.50 / P200)",
            ),
            (Form2000OTAtc::Do122, "DO122 — real property (P15 / P1,000)"),
            (
                Form2000OTAtc::Do125,
                "DO125 — stock without par value (50% of DST paid)",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            atc_row = atc_row.child(
                Self::choice(("2000ot_atc", index), label, atc == value, !editable)
                    .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.atc = value))),
            );
        }
        let children = vec![
            self.field(
                "date",
                "Item 1 — Date of transaction (MM/DD/YYYY)",
                layout,
                !editable,
            ),
            Self::label_row("Item 2 — Amended return?")
                .child(
                    Self::choice("2000ot_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                )
                .child(
                    Self::choice("2000ot_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                )
                .into_any_element(),
            atc_row.into_any_element(),
            self.field(
                "sheets",
                "Item 4 — No. of sheets attached",
                layout,
                !editable,
            ),
        ];
        self.section("Transaction", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let party = self.draft.other_party;
        let nature = self.draft.nature;
        let mut fields = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Item 5 — TIN"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format_tin(&self.draft.tin)),
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
                        .child("Item 6 — RDO Code"),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(self.draft.rdo_code.clone()),
                )
                .into_any_element(),
        ];
        for key in ["name", "address", "zip", "phone", "email"] {
            let label = TEXT_INPUTS
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, label, _)| *label)
                .unwrap_or(key);
            fields.push(self.field(key, label, layout, !editable));
        }
        let mut party_row = Self::label_row("Item 11 — Other party to the transaction");
        for (index, (value, label)) in [
            (Form2000OTOtherParty::Creditor, "Creditor/Mortgagor/etc."),
            (Form2000OTOtherParty::Debtor, "Debtor/Mortgagee/etc."),
        ]
        .into_iter()
        .enumerate()
        {
            party_row = party_row.child(
                Self::choice(("2000ot_party", index), label, party == value, !editable).on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.other_party = value)),
                ),
            );
        }
        let mut nature_row = Self::label_row("Item 12 — Nature of transaction");
        for (index, (value, label)) in [
            (
                Form2000OTNature::SharesOfStock,
                "Transfer of shares of stock not traded through the local stock exchange",
            ),
            (
                Form2000OTNature::RealPropertyCapitalAsset,
                "Transfer of real property classified as capital asset",
            ),
            (
                Form2000OTNature::RealPropertyOrdinaryAsset,
                "Transfer of real property other than capital asset",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            nature_row = nature_row.child(
                Self::choice(("2000ot_nature", index), label, nature == value, !editable).on_click(
                    cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.nature = value)),
                ),
            );
        }
        let mut children = vec![
            self.grid(layout, fields),
            party_row.into_any_element(),
            self.grid(
                layout,
                vec![
                    self.field("oname", "Item 11A — Other party's name", layout, !editable),
                    self.field("otin", "Item 11B — Other party's TIN", layout, !editable),
                ],
            ),
            nature_row.into_any_element(),
        ];
        if nature.is_real_property() {
            children.push(self.field(
                "loc",
                "Item 13 — Location of real property",
                layout,
                !editable,
            ));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn row_box(
        &self,
        title: String,
        fields: Vec<AnyElement>,
        layout: Layout,
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
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(self.grid(layout, fields))
            .into_any_element()
    }

    fn render_real_property(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children = Vec::new();
        for row in 0..FORM_2000OT_REAL_PROPERTY_ROWS {
            let mut fields: Vec<AnyElement> = PROPERTY_INPUTS
                .iter()
                .map(|(prefix, label, _)| {
                    self.field(&row_key(prefix, row), label, layout, !editable)
                })
                .collect();
            let fmv = self.draft.properties.get(row).map(|r| r.fmv).unwrap_or(0.0);
            fields.push(self.computed("Fair market value (higher of 1 and 2)", fmv, cx));
            children.push(self.row_box(format!("Property {}", row + 1), fields, layout, cx));
        }
        children.push(self.computed(
            "Total fair market value",
            self.draft.total_fair_market_value,
            cx,
        ));
        let d = &self.draft;
        children.push(self.grid(
            layout,
            vec![
                self.field("gsp", MONEY_INPUTS[0].1, layout, !editable),
                self.computed(
                    "Schedule 2 B — Total FMV per Schedule 1.A",
                    d.total_fair_market_value,
                    cx,
                ),
                self.field(
                    "others",
                    "Schedule 2 C — Others (specify)",
                    layout,
                    !editable,
                ),
                self.field("othamt", MONEY_INPUTS[1].1, layout, !editable),
                self.computed(
                    "Schedule 2 item 2 — Taxable base",
                    d.real_property_taxable_base,
                    cx,
                ),
            ],
        ));
        self.section(
            "Schedules 1.A and 2 — Real Property and Taxable Base",
            children,
            cx,
        )
    }

    fn render_shares(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let children = (0..FORM_2000OT_SHARE_ROWS)
            .map(|row| {
                let fields = SHARE_INPUTS
                    .iter()
                    .map(|(prefix, label, _)| {
                        self.field(&row_key(prefix, row), label, layout, !editable)
                    })
                    .collect();
                self.row_box(format!("Shares {}", row + 1), fields, layout, cx)
            })
            .collect();
        self.section("Schedule 1.B — Shares of Stock", children, cx)
    }

    fn render_totals(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let rate = if d.atc == Form2000OTAtc::Unanswered {
            "—".to_string()
        } else {
            d.atc.rate_text().to_string()
        };
        let children = vec![
            self.computed(
                "14 — Taxable base, shares of stock",
                d.shares_taxable_base,
                cx,
            ),
            self.computed("15 — Taxable base, real property", d.real_property_base, cx),
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .p_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("16 — Tax rate"),
                )
                .child(div().font_weight(FontWeight::BOLD).child(rate))
                .into_any_element(),
            self.computed("17 — Tax due", d.tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed("19 — Tax still due/(overpayment)", d.tax_still_due, cx),
            self.field("sur", MONEY_INPUTS[3].1, layout, !editable),
            self.field("int", MONEY_INPUTS[4].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[5].1, layout, !editable),
            self.computed("20D — Total penalties", d.total_penalties, cx),
            self.computed(
                "21 — Total amount payable/(overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        self.section(
            "Part II — Computation of Tax",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2000OTView {
    type Draft = Form2000OTDraft;

    fn new(
        draft: Form2000OTDraft,
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
        for row in 0..FORM_2000OT_REAL_PROPERTY_ROWS {
            for (prefix, _, money) in PROPERTY_INPUTS {
                keys.push((
                    row_key(prefix, row),
                    if *money { "0.00" } else { "" }.into(),
                ));
            }
        }
        for row in 0..FORM_2000OT_SHARE_ROWS {
            for (prefix, _, money) in SHARE_INPUTS {
                keys.push((
                    row_key(prefix, row),
                    if *money { "0.00" } else { "" }.into(),
                ));
            }
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

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form2000OTDraft {
        Form2000OTDraft::new_from_profile(profile, year)
    }

    /// One return per transaction date (`MMDD`). Reopen this year's latest
    /// saved return, else start a new one.
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form2000OTDraft {
        let tin = profile.tin.full();
        (0..=12i64)
            .flat_map(|month| (0..=31i64).map(move |day| month * 100 + day))
            .filter_map(|key| {
                db.get_queueable_draft::<Form2000OTDraft>(&tin, year, key)
                    .ok()
                    .flatten()
            })
            .max_by(|a, b| a.lifecycle.updated_at.cmp(&b.lifecycle.updated_at))
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

impl FormViewTrait for Form2000OTView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2000-OT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Documentary Stamp Tax Declaration/Return (One-Time Transactions)"
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
                    "2000-OT draft saved.".into(),
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
        if !can_queue_for_submission(Form2000OTDraft::FORM_CODE) {
            self.status_message =
                Some("2000-OT is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 2000-OT. No submission was started: {error}"
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
            "Form 2000-OT queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2000-OT payment status needs a verified confirmation workflow.".into());
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2000OTDraft>(
                        &queued.tin,
                        queued.taxable_year(),
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
            "2000-ot-2018",
            &fields,
            "2000-OT — Print Preview",
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

impl Render for Form2000OTView {
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
                .child(Button::new("2000ot_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2000ot_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2000ot_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2000ot_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2000ot_submit")
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
                            Button::new("2000ot_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2000ot_release_cancel")
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
            .child(self.render_part_one(layout, cx));
        match self.draft.atc {
            Form2000OTAtc::Do122 => body = body.child(self.render_real_property(layout, cx)),
            Form2000OTAtc::Do102 | Form2000OTAtc::Do125 => {
                body = body.child(self.render_shares(layout, cx))
            }
            Form2000OTAtc::Unanswered => {}
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
                    .id("2000ot_scroll")
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
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }
}
