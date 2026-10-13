//! Editor for BIR Form 1707 (April 2021), Capital Gains Tax Return for Onerous
//! Transfer of Shares of Stock Not Traded Through the Local Stock Exchange.
//! Rust owns every calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1707`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1707::{
    FORM_1707_PARTY_ROWS, FORM_1707_SCHEDULE_ROWS, Form1707Atc, Form1707Draft, Form1707Expense,
    Form1707Party, Form1707Shares, Form1707TransactionType,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;
use gpui_component::Disableable;
use gpui_component::button::Button;

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice, choice_row,
    computed, field, format_tin, grid, money_text, section,
};
use crate::components::form_engine::FormViewTrait;
use crate::components::penalty_section::{
    self, PenaltyPartyView, PenaltySection, PenaltySectionEvent,
};
use crate::views::form_controls::{messages_for, with_field_error};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::penalties::PenaltyComponent;

/// Plain inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of transaction", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 4 — No. of sheets attached", "0"),
    ("rdo", "Item 5 — RDO code", "e.g. 039"),
    ("name", "Taxpayer's name (profile)", ""),
    ("address", "Registered address (profile)", ""),
    ("zip", "Zip code (profile)", ""),
    ("contact", "Contact number (profile)", ""),
    ("email", "E-mail address (profile)", ""),
    ("lob", "Line of business (profile)", ""),
    ("relief", "Item 8A — Tax relief, specify", ""),
    ("txn_description", "Item 9 — Others, specify", ""),
    ("inst_count", "Sched 1 Item 4 — No. of installments", "0"),
    (
        "inst_date",
        "Sched 1 Item 8 — Date of collection",
        "MM/DD/YYYY",
    ),
];

/// Amounts where blank is zero: (key, label).
const AMOUNTS: &[(&str, &str)] = &[
    ("inst_sell", "Sched 1 Item 1 — Selling price"),
    ("inst_cost", "Sched 1 Item 2 — Cost and expenses"),
    ("inst_mortgage", "Sched 1 Item 3 — Mortgage assumed"),
    (
        "inst_amount",
        "Sched 1 Item 5 — Installment for this period",
    ),
    (
        "inst_total",
        "Sched 1 Item 9 — Total collection in the year of sale",
    ),
    (
        "less",
        "16 — Tax paid in return previously filed (amended only)",
    ),
    ("surcharge", "18A — Surcharge"),
    ("interest", "18B — Interest"),
    ("compromise", "18C — Compromise"),
];

const PARTY_FIELDS: [(&str, &str); 3] = [("name", "Name"), ("addr", "Address"), ("tin", "TIN")];
const SHARE_FIELDS: [(&str, &str); 4] = [
    ("corp", "Name of corporate stock"),
    ("shares", "No. of shares"),
    ("cert", "Stock certificate no."),
    ("price", "Taxable base / selling price"),
];
/// Schedule 2/3 rows always shown: A–D. "Add row" appends the official
/// "More" list, which has no row limit. Labels follow the printed sheet:
/// with four rows the last is D; once rows are added, D is the OTHERS
/// subtotal and the rows from the fourth on are D.1, D.2, … (the official
/// pop-up numbering, form_1707.rs writes index 3 as entry 1).
const SCHEDULE_MIN_ROWS: usize = FORM_1707_SCHEDULE_ROWS;

fn row_label(row: usize, rows: usize) -> String {
    match row {
        0..=2 => ["A", "B", "C"][row].to_string(),
        3 if rows <= SCHEDULE_MIN_ROWS => "D".to_string(),
        _ => format!("D.{}", row - 2),
    }
}

fn party_key(kind: &str, row: usize, part: &str) -> String {
    format!("{kind}{row}_{part}")
}

fn share_key(row: usize, part: &str) -> String {
    format!("s2_{row}_{part}")
}

fn expense_key(row: usize, part: &str) -> String {
    format!("s3_{row}_{part}")
}

pub struct Form1707View {
    draft: Form1707Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
    /// The shared penalty section (computed values, overrides, note).
    penalty: Entity<PenaltySection>,
    _penalty_subscription: Subscription,
    /// Schedule 2 / 3 rows on screen: never fewer than the draft holds.
    share_rows: usize,
    expense_rows: usize,
}

impl EventEmitter<QueueableFormEvent> for Form1707View {}

impl Form1707View {
    fn text_value(d: &Form1707Draft, key: &str) -> String {
        let inst = &d.installment;
        match key {
            "month" if d.transaction_month == 0 => String::new(),
            "month" => format!("{:02}", d.transaction_month),
            "day" if d.transaction_day == 0 => String::new(),
            "day" => format!("{:02}", d.transaction_day),
            "year" => d.transaction_year.to_string(),
            "sheets" => d.number_of_attached_sheets.to_string(),
            "rdo" => d.rdo_code.clone(),
            "name" => d.taxpayer_name.clone(),
            "address" => d.registered_address.clone(),
            "zip" => d.zip_code.clone(),
            "contact" => d.contact_number.clone(),
            "email" => d.email.clone(),
            "lob" => d.line_of_business.clone(),
            "relief" => d.tax_relief.clone().unwrap_or_default(),
            "txn_description" => d.transaction_description.clone(),
            "inst_count" => inst.number_of_installments.clone(),
            "inst_date" => inst.collection_date.clone(),
            "inst_sell" => money_text(inst.selling_price),
            "inst_cost" => money_text(inst.cost_and_expenses),
            "inst_mortgage" => money_text(inst.mortgage_assumed),
            "inst_amount" => money_text(inst.installment_amount),
            "inst_total" => money_text(inst.total_collection),
            "less" => money_text(d.tax_paid_previous),
            "surcharge" => penalty_section::override_text(&d.penalty, PenaltyComponent::Surcharge),
            "interest" => penalty_section::override_text(&d.penalty, PenaltyComponent::Interest),
            "compromise" => {
                penalty_section::override_text(&d.penalty, PenaltyComponent::Compromise)
            }
            _ => Self::row_value(d, key),
        }
    }

    fn row_value(d: &Form1707Draft, key: &str) -> String {
        for (kind, rows) in [("seller", &d.sellers), ("buyer", &d.buyers)] {
            for row in 0..FORM_1707_PARTY_ROWS {
                let party = rows.get(row).cloned().unwrap_or_default();
                for (part, value) in [
                    ("name", party.name.clone()),
                    ("addr", party.address.clone()),
                    ("tin", party.tin.clone()),
                ] {
                    if key == party_key(kind, row, part) {
                        return value;
                    }
                }
            }
        }
        for row in 0..d.shares.len().max(d.expenses.len()) {
            let shares = d.shares.get(row).cloned().unwrap_or_default();
            let expense = d.expenses.get(row).cloned().unwrap_or_default();
            let values = [
                (share_key(row, "corp"), shares.corporation.clone()),
                (
                    share_key(row, "shares"),
                    shares
                        .number_of_shares
                        .map(|v| format!("{v:.3}"))
                        .unwrap_or_default(),
                ),
                (share_key(row, "cert"), shares.certificate_number.clone()),
                (share_key(row, "price"), money_text(shares.selling_price)),
                (expense_key(row, "part"), expense.particulars.clone()),
                (expense_key(row, "amount"), money_text(expense.amount)),
            ];
            for (k, value) in values {
                if k == key {
                    return value;
                }
            }
        }
        String::new()
    }

    fn specs(&self) -> Vec<InputSpec> {
        Self::input_specs(&self.draft, self.share_rows, self.expense_rows)
    }

    fn input_specs(d: &Form1707Draft, share_rows: usize, expense_rows: usize) -> Vec<InputSpec> {
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(k, _, p)| (k.to_string(), p.to_string()))
            .collect();
        keys.extend(
            AMOUNTS
                .iter()
                .map(|(k, _)| (k.to_string(), "0.00".to_string())),
        );
        for kind in ["seller", "buyer"] {
            for row in 0..FORM_1707_PARTY_ROWS {
                for (part, _) in PARTY_FIELDS {
                    keys.push((party_key(kind, row, part), String::new()));
                }
            }
        }
        for row in 0..share_rows {
            for (part, _) in SHARE_FIELDS {
                keys.push((share_key(row, part), String::new()));
            }
        }
        for row in 0..expense_rows {
            keys.push((expense_key(row, "part"), String::new()));
            keys.push((expense_key(row, "amount"), "0.00".to_string()));
        }
        keys.into_iter()
            .map(|(key, placeholder)| {
                let value = Self::text_value(d, &key);
                (key, placeholder, value)
            })
            .collect()
    }

    fn label(key: &str) -> &'static str {
        TEXT_INPUTS
            .iter()
            .map(|(k, l, _)| (*k, *l))
            .chain(AMOUNTS.iter().copied())
            .find(|(k, _)| *k == key)
            .map(|(_, l)| l)
            .unwrap_or("")
    }

    /// Validator keys (`Form1707Draft::validate`) that belong to input `key`.
    fn model_keys(key: &str) -> &'static [&'static str] {
        match key {
            "month" => &["transaction_month"],
            "day" => &["transaction_day"],
            "year" => &["transaction_year"],
            "sheets" => &["number_of_attached_sheets"],
            "rdo" => &["rdo_code"],
            "name" => &["taxpayer_name"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "contact" => &["contact_number"],
            "email" => &["email"],
            "relief" => &["tax_relief"],
            "txn_description" => &["transaction_description"],
            "inst_count" => &["installment.number_of_installments"],
            "inst_date" => &["installment.collection_date"],
            "inst_sell" => &["installment.selling_price"],
            "inst_cost" => &["installment.cost_and_expenses"],
            "inst_mortgage" => &["installment.mortgage_assumed"],
            "inst_amount" => &["installment.installment_amount"],
            "inst_total" => &["installment.total_collection"],
            "less" => &["tax_paid_previous"],
            "surcharge" => &["surcharge"],
            "interest" => &["interest"],
            "compromise" => &["compromise"],
            _ => &[],
        }
    }

    /// Messages for any of `keys`, from malformed input and from validation.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.kit
                .parse_errors
                .iter()
                .chain(self.kit.validation_errors.iter()),
            keys,
        )
    }

    /// Put the messages for `keys` directly under an already-built control.
    fn with_errors(&self, content: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(content, self.errors_for(keys))
    }

    /// A labelled input with its own messages (its input key plus `model`)
    /// shown under it.
    fn field_at(&self, key: &str, label_text: &str, layout: Layout, model: &[&str]) -> AnyElement {
        let mut keys = vec![key];
        keys.extend_from_slice(model);
        self.with_errors(
            field(
                &self.kit,
                key,
                label_text,
                layout,
                !self.draft.lifecycle.is_editable(),
            ),
            &keys,
        )
    }

    fn f(&self, key: &str, layout: Layout) -> AnyElement {
        self.field_at(key, Self::label(key), layout, Self::model_keys(key))
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = choice_row(
            layout,
            "Item 2 — Amended return?",
            vec![
                choice("1707_amended_yes", "Yes", d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(true))
                    },
                )),
                choice("1707_amended_no", "No", !d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(false))
                    },
                )),
            ],
        );
        let atcs: Vec<_> = [
            (Form1707Atc::Individual, "II 030 — Individual"),
            (
                Form1707Atc::CorporationDomestic,
                "IC 110 — Domestic corporation",
            ),
            (
                Form1707Atc::CorporationForeign,
                "IC 110 — Foreign corporation",
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (atc, caption))| {
            choice(("1707_atc", index), caption, d.atc == Some(atc), !editable).on_click(
                cx.listener(move |this: &mut Self, _, _, cx| {
                    kit::edit(this, cx, |d| d.atc = Some(atc))
                }),
            )
        })
        .collect();
        let fixed = self.with_errors(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(kit::label("TIN (header, no item number)"))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format_tin(&d.tin)),
                )
                .into_any_element(),
            &["tin"],
        );
        let atc_row = self.with_errors(choice_row(layout, "Item 3 — ATC", atcs), &["atc"]);
        let children = vec![
            fixed,
            self.f("month", layout),
            self.f("day", layout),
            self.f("year", layout),
            amended,
            atc_row,
            self.f("sheets", layout),
            self.f("rdo", layout),
            self.f("name", layout),
            self.f("address", layout),
            self.f("zip", layout),
            self.f("contact", layout),
            self.f("email", layout),
            self.f("lob", layout),
        ];
        section(
            "Date of transaction and background information",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_parties(
        &self,
        kind: &'static str,
        title: &str,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let model = if kind == "seller" {
            "sellers"
        } else {
            "buyers"
        };
        let rows: Vec<AnyElement> = (0..FORM_1707_PARTY_ROWS)
            .map(|row| {
                let number = if kind == "seller" { 6 } else { 7 };
                let who = if kind == "seller" { "Seller" } else { "Buyer" };
                let row_key = format!("{model}[{row}]");
                let tin_key = format!("{model}[{row}].tin");
                let cells: Vec<AnyElement> = PARTY_FIELDS
                    .iter()
                    .zip(['A', 'B', 'C'])
                    .map(|((part, caption), letter)| {
                        let caption = match *part {
                            "name" => "Name",
                            "addr" => "Registered address",
                            _ => *caption,
                        };
                        let tin_only = [tin_key.as_str()];
                        let model_keys: &[&str] = if *part == "tin" { &tin_only } else { &[] };
                        self.field_at(
                            &party_key(kind, row, part),
                            &format!("{number}{letter} — {who}'s {caption} (row {})", row + 1),
                            layout,
                            model_keys,
                        )
                    })
                    .collect();
                // Row-level messages ("complete Item #n", length limits); the
                // first row also carries the whole-list message.
                let mut keys = vec![row_key.as_str()];
                if row == 0 {
                    keys.push(model);
                }
                self.with_errors(grid(layout, cells), &keys)
            })
            .collect();
        section(title, rows, cx)
    }

    fn render_transaction(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let relief = choice_row(
            layout,
            "Item 8 — Availing of tax relief under Special Law or International Tax Treaty?",
            vec![
                choice("1707_relief_yes", "Yes", d.tax_relief.is_some(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| {
                            if d.tax_relief.is_none() {
                                d.tax_relief = Some(String::new());
                            }
                        })
                    }),
                ),
                choice("1707_relief_no", "No", d.tax_relief.is_none(), !editable).on_click(
                    cx.listener(|this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.tax_relief = None)
                    }),
                ),
            ],
        );
        let kinds: Vec<_> = Form1707TransactionType::ALL
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let kind = *kind;
                choice(
                    ("1707_txn", index),
                    kind.label(),
                    d.transaction_type == Some(kind),
                    !editable,
                )
                .on_click(cx.listener(move |this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| d.set_transaction_type(kind))
                }))
            })
            .collect();
        let mut children = vec![relief];
        if d.tax_relief.is_some() {
            children.push(self.f("relief", layout));
        }
        children.push(self.with_errors(
            choice_row(layout, "Item 9 — Description of transaction", kinds),
            &["transaction_type"],
        ));
        if d.transaction_type == Some(Form1707TransactionType::Others) {
            children.push(self.f("txn_description", layout));
        }
        section("Tax relief and transaction (Items 8–9)", children, cx)
    }

    /// Row caption with "Remove"; removing never drops another row.
    fn row_heading(
        &self,
        schedule: &'static str,
        row: usize,
        letter: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        let remove = Button::new((SharedString::from(format!("1707_{schedule}_remove")), row))
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                kit::edit_rows(
                    this,
                    window,
                    cx,
                    |v| v.remove_row(schedule, row),
                    Self::specs,
                )
            }));
        kit::row_heading(&format!("Row {letter}"), remove)
    }

    fn add_row(&self, schedule: &'static str, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        kit::add_row_button(SharedString::from(format!("1707_{schedule}_add")), disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                kit::edit_rows(
                    this,
                    window,
                    cx,
                    |v| {
                        if schedule == "s2" {
                            v.share_rows += 1;
                        } else {
                            v.expense_rows += 1;
                        }
                    },
                    Self::specs,
                )
            }))
            .into_any_element()
    }

    fn remove_row(&mut self, schedule: &str, row: usize) {
        if schedule == "s2" {
            if row < self.draft.shares.len() {
                self.draft.shares.remove(row);
            }
            self.share_rows = self.share_rows.saturating_sub(1).max(SCHEDULE_MIN_ROWS);
        } else {
            if row < self.draft.expenses.len() {
                self.draft.expenses.remove(row);
            }
            self.expense_rows = self.expense_rows.saturating_sub(1).max(SCHEDULE_MIN_ROWS);
        }
    }

    fn render_schedules(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let mut children = Vec::new();
        match d.transaction_type {
            Some(Form1707TransactionType::InstallmentSale) => {
                let keys = [
                    "inst_sell",
                    "inst_cost",
                    "inst_mortgage",
                    "inst_count",
                    "inst_amount",
                    "inst_date",
                    "inst_total",
                ];
                children.push(
                    kit::label("Schedule 1 — Installment sale (tax rate 15%)").into_any_element(),
                );
                children.push(grid(
                    layout,
                    keys.iter().map(|k| self.f(k, layout)).collect(),
                ));
                children.push(computed(
                    "Sched 1 Item 7 — Tax due for the period",
                    d.installment.period_tax_due,
                    cx,
                ));
            }
            Some(_) => {
                children.push(
                    kit::label("Schedule 2 — Description of shares of stock").into_any_element(),
                );
                for (row, letter) in
                    (0..self.share_rows).map(|row| (row, row_label(row, self.share_rows)))
                {
                    children.push(self.row_heading("s2", row, &letter, cx));
                    let shares_key = format!("shares[{row}].number_of_shares");
                    let price_key = format!("shares[{row}].selling_price");
                    let cells = SHARE_FIELDS
                        .iter()
                        .map(|(part, caption)| {
                            let own = [shares_key.as_str()];
                            let own_price = [price_key.as_str()];
                            let model: &[&str] = match *part {
                                "shares" => &own,
                                "price" => &own_price,
                                _ => &[],
                            };
                            self.field_at(
                                &share_key(row, part),
                                &format!("{letter} — {caption}"),
                                layout,
                                model,
                            )
                        })
                        .collect();
                    let row_key = format!("shares[{row}]");
                    children.push(self.with_errors(grid(layout, cells), &[row_key.as_str()]));
                }
                children.push(self.add_row("s2", cx));
                children.push(computed("Total taxable base", d.total_selling_price, cx));
                children.push(
                    kit::label("Schedule 3 — Cost and other allowable expenses").into_any_element(),
                );
                for (row, letter) in
                    (0..self.expense_rows).map(|row| (row, row_label(row, self.expense_rows)))
                {
                    children.push(self.row_heading("s3", row, &letter, cx));
                    let amount_key = format!("expenses[{row}].amount");
                    let cells = vec![
                        self.field_at(
                            &expense_key(row, "part"),
                            &format!("{letter} — Particulars"),
                            layout,
                            &[],
                        ),
                        self.field_at(
                            &expense_key(row, "amount"),
                            &format!("{letter} — Amount"),
                            layout,
                            &[amount_key.as_str()],
                        ),
                    ];
                    let row_key = format!("expenses[{row}]");
                    children.push(self.with_errors(grid(layout, cells), &[row_key.as_str()]));
                }
                children.push(self.add_row("s3", cx));
                children.push(computed("Total cost and expenses", d.total_expenses, cx));
            }
            None => children.push(
                kit::label("Choose the description of transaction (Item 9) first.")
                    .into_any_element(),
            ),
        }
        section("Part IV — Schedules", children, cx)
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let children = vec![
            computed("10 — Taxable base", d.taxable_base, cx),
            computed(
                "11 — Cost and other allowable expenses",
                d.allowable_expenses,
                cx,
            ),
            computed("12 — Net capital gain (loss)", d.net_capital_gain, cx),
            kit::computed_text("13 — Applicable tax rate", "15%".to_string(), cx),
            computed("14 — Tax due on the entire transaction", d.tax_due, cx),
            computed(
                "15 — Tax due for this payment period",
                d.tax_due_this_period,
                cx,
            ),
            self.with_errors(
                field(
                    &self.kit,
                    "less",
                    Self::label("less"),
                    layout,
                    !editable || !d.is_amended,
                ),
                &["less", "tax_paid_previous"],
            ),
            computed("17 — Tax payable (overpayment)", d.tax_payable, cx),
            penalty_section::render(
                &self.penalty,
                &PenaltyPartyView::all(&self.draft),
                &self
                    .kit
                    .validation_errors
                    .iter()
                    .chain(&self.kit.parse_errors)
                    .cloned()
                    .collect::<Vec<_>>(),
                cx,
            ),
            computed("18D — Total penalties", d.total_penalties, cx),
            self.with_errors(
                computed(
                    "19 — Total amount payable (overpayment)",
                    d.total_amount_payable,
                    cx,
                ),
                &["total_amount_payable"],
            ),
        ];
        section(
            "Part II — Computation of tax (Items 10–19)",
            vec![grid(layout, children)],
            cx,
        )
    }
}

impl KitView for Form1707View {
    type Draft = Form1707Draft;
    const SLUG: &'static str = "1707";
    const CODE: &'static str = "1707";

    fn kit(&self) -> &EditorState {
        &self.kit
    }
    fn kit_mut(&mut self) -> &mut EditorState {
        &mut self.kit
    }
    fn draft(&self) -> &Form1707Draft {
        &self.draft
    }
    fn draft_mut(&mut self) -> &mut Form1707Draft {
        &mut self.draft
    }
    fn db(&self) -> Arc<Mutex<Database>> {
        Arc::clone(&self.db)
    }
    fn set_draft_id(&mut self, id: i64) {
        self.draft.id = Some(id);
    }
    fn initial_text(&self, key: &str) -> String {
        Self::text_value(&self.draft, key)
    }

    fn read_inputs(&mut self, cx: &mut Context<Self>) -> Vec<(String, String)> {
        let mut r = InputReader::new(&self.kit, cx);
        let d = &mut self.draft;
        if let Some(v) = r.whole::<u8>("month", "Item 1 month", 99) {
            d.transaction_month = v;
        }
        if let Some(v) = r.whole::<u8>("day", "Item 1 day", 99) {
            d.transaction_day = v;
        }
        if let Some(v) = r.whole::<u16>("year", "Item 1 year", 9999) {
            d.transaction_year = v;
        }
        if let Some(v) = r.whole::<u16>("sheets", "Item 4", 999) {
            d.number_of_attached_sheets = v;
        }
        d.rdo_code = r.trimmed("rdo").to_uppercase();
        d.taxpayer_name = r.text("name");
        d.registered_address = r.text("address");
        d.zip_code = r.trimmed("zip");
        d.contact_number = r.trimmed("contact");
        d.email = r.trimmed("email");
        d.line_of_business = r.text("lob");
        if d.tax_relief.is_some() {
            d.tax_relief = Some(r.text("relief"));
        }
        d.transaction_description = r.text("txn_description");
        d.installment.number_of_installments = r.trimmed("inst_count");
        d.installment.collection_date = r.trimmed("inst_date");
        for (key, label) in AMOUNTS {
            if let Some(value) = r.amount(key, label) {
                let inst = &mut d.installment;
                match *key {
                    "inst_sell" => inst.selling_price = value,
                    "inst_cost" => inst.cost_and_expenses = value,
                    "inst_mortgage" => inst.mortgage_assumed = value,
                    "inst_amount" => inst.installment_amount = value,
                    "inst_total" => inst.total_collection = value,
                    "less" => d.tax_paid_previous = value,
                    "surcharge" => d.surcharge = value,
                    "interest" => d.interest = value,
                    _ => d.compromise = value,
                }
            }
        }
        for kind in ["seller", "buyer"] {
            let mut rows: Vec<Form1707Party> = (0..FORM_1707_PARTY_ROWS)
                .map(|row| Form1707Party {
                    name: r.text(&party_key(kind, row, "name")),
                    address: r.text(&party_key(kind, row, "addr")),
                    tin: r.trimmed(&party_key(kind, row, "tin")),
                })
                .collect();
            while rows.last().is_some_and(|p| {
                p.name.trim().is_empty() && p.address.trim().is_empty() && p.tin.is_empty()
            }) {
                rows.pop();
            }
            if kind == "seller" {
                d.sellers = rows;
            } else {
                d.buyers = rows;
            }
        }
        if d.transaction_type.is_some()
            && d.transaction_type != Some(Form1707TransactionType::InstallmentSale)
        {
            let mut shares = Vec::new();
            let mut expenses = Vec::new();
            for (row, letter) in
                (0..self.share_rows).map(|row| (row, row_label(row, self.share_rows)))
            {
                let number = r
                    .optional_amount(
                        &share_key(row, "shares"),
                        &format!("Sched 2 {letter} shares"),
                    )
                    .unwrap_or(None);
                let price = r
                    .amount(
                        &share_key(row, "price"),
                        &format!("Sched 2 {letter} amount"),
                    )
                    .unwrap_or(0.0);
                shares.push(Form1707Shares {
                    corporation: r.text(&share_key(row, "corp")),
                    number_of_shares: number,
                    certificate_number: r.trimmed(&share_key(row, "cert")),
                    selling_price: price,
                });
            }
            for (row, letter) in
                (0..self.expense_rows).map(|row| (row, row_label(row, self.expense_rows)))
            {
                let amount = r
                    .amount(
                        &expense_key(row, "amount"),
                        &format!("Sched 3 {letter} amount"),
                    )
                    .unwrap_or(0.0);
                expenses.push(Form1707Expense {
                    particulars: r.text(&expense_key(row, "part")),
                    amount,
                });
            }
            while shares.last().is_some_and(|s| {
                s.corporation.trim().is_empty()
                    && s.certificate_number.is_empty()
                    && s.number_of_shares.is_none_or(|v| v == 0.0)
                    && s.selling_price == 0.0
            }) {
                shares.pop();
            }
            while expenses
                .last()
                .is_some_and(|e| e.particulars.trim().is_empty() && e.amount == 0.0)
            {
                expenses.pop();
            }
            d.shares = shares;
            d.expenses = expenses;
        }
        let mut errors = r.errors;
        // An empty penalty box means "use computed"; any amount overrides it.
        errors.extend(penalty_section::apply_overrides(
            &self.penalty,
            &<Form1707Draft as bir_core::forms::queueable::QueueableForm>::penalty_items(),
            &mut self.draft,
            cx,
        ));
        errors
    }
}

impl QueueableFormView for Form1707View {
    type Draft = Form1707Draft;

    fn new(
        draft: Form1707Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let share_rows = draft.shares.len().max(SCHEDULE_MIN_ROWS);
        let expense_rows = draft.expenses.len().max(SCHEDULE_MIN_ROWS);
        let kit = EditorState::new(
            Self::input_specs(&draft, share_rows, expense_rows),
            errors,
            window,
            cx,
        );
        // The penalty boxes are overrides (empty means "use computed").
        let penalty_views = PenaltyPartyView::all(&draft);
        let penalty_inputs =
            vec![["surcharge", "interest", "compromise"].map(|key| kit.inputs[key].clone())];
        let penalty = cx.new(|cx| PenaltySection::wrap(&penalty_views, penalty_inputs, window, cx));
        let _penalty_subscription = cx.subscribe_in(
            &penalty,
            window,
            |this: &mut Self, _, _: &PenaltySectionEvent, _, cx| {
                super::event_form_kit::sync(this, cx);
            },
        );
        Self {
            draft,
            db,
            kit,
            penalty,
            _penalty_subscription,
            share_rows,
            expense_rows,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1707Draft {
        Form1707Draft::new_from_profile(profile, year, u32::from(period.max(1)))
    }
}

impl FormViewTrait for Form1707View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1707"
    }
    fn form_subtitle(&self) -> &'static str {
        "Capital Gains Tax Return (Shares of Stock Not Traded Through the Local Stock Exchange)"
    }
    fn form_version(&self) -> &'static str {
        "April 2021 (ENCS)"
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
        kit::save(self, window, cx);
    }
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::queue(self, window, cx);
    }
    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.kit.status_message =
            Some("1707 payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::cancel(self, window, cx);
    }
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        kit::preview(self, "1707-2021", cx);
    }
}

impl Render for Form1707View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let sections = vec![
            self.render_header_items(layout, cx),
            self.render_parties("seller", "Item 6 — Seller", layout, cx),
            self.render_parties("buyer", "Item 7 — Buyer", layout, cx),
            self.render_transaction(layout, cx),
            self.render_computation(layout, cx),
            self.render_schedules(layout, cx),
        ];
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{AMOUNTS, Form1707Expense, Form1707View, TEXT_INPUTS, expense_key};
    use bir_core::forms::form_1707::Form1707Draft;

    /// Printed-sheet labels: D alone with four rows, D.1 onward once the
    /// official "More" list is in use.
    #[test]
    fn row_labels_follow_printed_sheet() {
        let four: Vec<_> = (0..4).map(|r| super::row_label(r, 4)).collect();
        assert_eq!(four, ["A", "B", "C", "D"]);
        let six: Vec<_> = (0..6).map(|r| super::row_label(r, 6)).collect();
        assert_eq!(six, ["A", "B", "C", "D.1", "D.2", "D.3"]);
    }

    /// Every stored row gets an editor holding its value, so read-back keeps
    /// rows past the first ten.
    #[test]
    fn every_stored_row_has_an_editor() {
        let profile = serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Sample Taxpayer",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "789", "branch": "00000"},
            "rdo_code": "039",
            "line_of_business": "Consulting",
            "registered_address": "Quezon City",
            "zip_code": "1100",
            "phone": "09170000000",
            "email": "sample@example.com",
            "default_form_type": "1707v2021",
            "taxpayer_type": "Individual",
            "withholds_compensation": false,
            "has_employees": false
        }))
        .expect("profile");
        let mut draft = Form1707Draft::new_from_profile(&profile, 2025, 1);
        draft.expenses = (0..14)
            .map(|k| Form1707Expense {
                particulars: format!("Expense {k}"),
                amount: 0.0,
            })
            .collect();
        let specs = Form1707View::input_specs(&draft, 4, 14);
        let last = expense_key(13, "part");
        let spec = specs
            .iter()
            .find(|(key, _, _)| *key == last)
            .expect("row 14");
        assert_eq!(spec.2, "Expense 13");
    }

    #[test]
    fn every_input_has_a_label() {
        for (key, _, _) in TEXT_INPUTS {
            assert!(!Form1707View::label(key).is_empty(), "{key}");
        }
        for (key, _) in AMOUNTS {
            assert!(!Form1707View::label(key).is_empty(), "{key}");
        }
    }
}
