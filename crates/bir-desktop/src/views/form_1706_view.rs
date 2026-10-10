//! Editor for BIR Form 1706, Capital Gains Tax Return for Onerous Transfer of
//! Real Property Classified as Capital Asset (v2018 package). Rust owns every
//! calculation, validation and the official submit plaintext
//! (`bir_core::forms::form_1706`); this view only edits source values. The
//! layout reflows for desktop, tablet and phone widths.

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1706::{
    FORM_1706_SCHEDULE_1_CELLS, Form1706Draft, Form1706Overpayment, Form1706PropertyClass,
    Form1706SellerType, Form1706TaxRelief, Form1706TaxableBase, Form1706TransactionType,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice, choice_row,
    computed, field, format_tin, grid, money_text, optional_money_text, section, yes_no,
};
use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

/// Plain text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of transaction", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("buyer_tin", "Item 7 — Buyer TIN", "000-000-000"),
    ("buyer_branch", "Item 7 — Buyer branch code", "00000"),
    ("buyer_rdo", "Item 8 — Buyer RDO code", "e.g. 040"),
    ("seller_name", "Item 9 — Seller's name", ""),
    ("buyer_name", "Item 10 — Buyer's name", ""),
    (
        "seller_address",
        "Item 11 — Seller's registered address",
        "",
    ),
    ("buyer_address", "Item 12 — Buyer's registered address", ""),
    (
        "seller_residence",
        "Item 13 — Seller's residence address (individuals)",
        "",
    ),
    ("location", "Item 14 — Location of property", ""),
    (
        "property_rdo",
        "Item 14A — RDO code of the property",
        "e.g. 039",
    ),
    ("class_other", "Item 15 — Other classification", ""),
    ("tct", "Item 16 — TCT/OCT/CCT No.", "T-000000"),
    ("area", "Item 16 — Area sold (sq. m)", ""),
    ("tax_dec", "Item 16 — Tax Dec. No.", ""),
    ("property_other", "Item 16 — Others", ""),
    (
        "txn_description",
        "Item 21 — If exempt or others, specify",
        "",
    ),
    ("inst_date_month", "28 — Date of installment (MM)", "MM"),
    ("inst_date_day", "28 — Day (DD)", "DD"),
    ("inst_date_year", "28 — Year (YYYY)", "YYYY"),
    ("others_description", "30F — Others (specify)", ""),
    ("email", "Email address for the BIR confirmation", ""),
];

/// Optional amounts (blank stays blank on the official page).
const OPTIONAL_AMOUNTS: &[(&str, &str)] = &[
    ("inst_selling", "22 — Selling price"),
    ("inst_cost", "23 — Cost and other expenses"),
    ("inst_mortgage", "24 — Mortgage assumed"),
    (
        "inst_initial",
        "25 — Total payments during the initial year",
    ),
    ("inst_month_amount", "26 — Amount of installment this month"),
    ("inst_count", "27 — No. of installments in the contract"),
    (
        "fmv_land_td",
        "29A — FMV of land per latest tax declaration",
    ),
    (
        "fmv_improvements_td",
        "29B — FMV of improvements per latest tax declaration",
    ),
    ("fmv_land_zonal", "29C — FMV of land per BIR zonal value"),
    (
        "fmv_improvements_bir",
        "29D — FMV of improvements per BIR rules",
    ),
    ("gross", "30A — Gross selling price"),
    ("bid", "30B — Bid price (foreclosure sale)"),
    ("installment", "30D — Taxable installment collected"),
    ("unutilized", "30E — Unutilized portion of sales proceeds"),
    ("others_amount", "30F — Others amount"),
];

/// Amounts where blank is zero.
const AMOUNTS: &[(&str, &str)] = &[
    (
        "less",
        "33 — Tax paid in return previously filed (amended only)",
    ),
    ("surcharge", "35A — Surcharge"),
    ("interest", "35B — Interest"),
    ("compromise", "35C — Compromise"),
];

fn schedule_key(index: usize) -> String {
    format!("sched{index}")
}

pub struct Form1706View {
    draft: Form1706Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
}

impl EventEmitter<QueueableFormEvent> for Form1706View {}

impl Form1706View {
    fn optional_amount(draft: &Form1706Draft, key: &str) -> Option<f64> {
        let inst = &draft.installment;
        match key {
            "inst_selling" => inst.selling_price,
            "inst_cost" => inst.cost_and_expenses,
            "inst_mortgage" => inst.mortgage_assumed,
            "inst_initial" => inst.initial_year_payments,
            "inst_month_amount" => inst.installment_this_month,
            "inst_count" => inst.number_of_installments,
            "fmv_land_td" => draft.fmv_land_tax_declaration,
            "fmv_improvements_td" => draft.fmv_improvements_tax_declaration,
            "fmv_land_zonal" => draft.fmv_land_zonal,
            "fmv_improvements_bir" => draft.fmv_improvements_bir,
            "gross" => draft.gross_selling_price,
            "bid" => draft.bid_price,
            "installment" => draft.taxable_installment,
            "unutilized" => draft.unutilized_proceeds,
            "others_amount" => draft.others_amount,
            _ => None,
        }
    }

    fn set_optional_amount(draft: &mut Form1706Draft, key: &str, value: Option<f64>) {
        let inst = &mut draft.installment;
        match key {
            "inst_selling" => inst.selling_price = value,
            "inst_cost" => inst.cost_and_expenses = value,
            "inst_mortgage" => inst.mortgage_assumed = value,
            "inst_initial" => inst.initial_year_payments = value,
            "inst_month_amount" => inst.installment_this_month = value,
            "inst_count" => inst.number_of_installments = value,
            "fmv_land_td" => draft.fmv_land_tax_declaration = value,
            "fmv_improvements_td" => draft.fmv_improvements_tax_declaration = value,
            "fmv_land_zonal" => draft.fmv_land_zonal = value,
            "fmv_improvements_bir" => draft.fmv_improvements_bir = value,
            "gross" => draft.gross_selling_price = value,
            "bid" => draft.bid_price = value,
            "installment" => draft.taxable_installment = value,
            "unutilized" => draft.unutilized_proceeds = value,
            "others_amount" => draft.others_amount = value,
            _ => {}
        }
    }

    fn text_value(draft: &Form1706Draft, key: &str) -> String {
        match key {
            "month" => format!("{:02}", draft.transaction_month),
            "day" => format!("{:02}", draft.transaction_day),
            "year" => draft.transaction_year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "buyer_tin" => draft.buyer_tin.clone(),
            "buyer_branch" => draft.buyer_branch_code.clone(),
            "buyer_rdo" => draft.buyer_rdo_code.clone(),
            "seller_name" => draft.seller_name.clone(),
            "buyer_name" => draft.buyer_name.clone(),
            "seller_address" => draft.seller_address.clone(),
            "buyer_address" => draft.buyer_address.clone(),
            "seller_residence" => draft.seller_residence_address.clone(),
            "location" => draft.property_location.clone(),
            "property_rdo" => draft.property_rdo_code.clone(),
            "class_other" => draft.property_class_other.clone(),
            "tct" => draft.tct_number.clone(),
            "area" => draft.area_sold.clone(),
            "tax_dec" => draft.tax_declaration_number.clone(),
            "property_other" => draft.property_other_description.clone(),
            "txn_description" => draft.transaction_description.clone(),
            "inst_date_month" => draft.installment.date_month.clone(),
            "inst_date_day" => draft.installment.date_day.clone(),
            "inst_date_year" => draft.installment.date_year.clone(),
            "others_description" => draft.others_description.clone(),
            "email" => draft.email.clone(),
            "less" => money_text(draft.tax_paid_previous),
            "surcharge" => money_text(draft.surcharge),
            "interest" => money_text(draft.interest),
            "compromise" => money_text(draft.compromise),
            _ => {
                if let Some(index) = key
                    .strip_prefix("sched")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    draft.schedule_1.get(index).cloned().unwrap_or_default()
                } else {
                    optional_money_text(Self::optional_amount(draft, key))
                }
            }
        }
    }

    fn input_specs(draft: &Form1706Draft) -> Vec<InputSpec> {
        let mut specs: Vec<InputSpec> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| {
                (
                    key.to_string(),
                    placeholder.to_string(),
                    Self::text_value(draft, key),
                )
            })
            .collect();
        for (key, _) in OPTIONAL_AMOUNTS.iter().chain(AMOUNTS) {
            specs.push((key.to_string(), "0.00".into(), Self::text_value(draft, key)));
        }
        for index in 0..FORM_1706_SCHEDULE_1_CELLS {
            let key = schedule_key(index);
            let value = Self::text_value(draft, &key);
            specs.push((key, String::new(), value));
        }
        specs
    }

    fn label(key: &str) -> &'static str {
        TEXT_INPUTS
            .iter()
            .map(|(k, l, _)| (*k, *l))
            .chain(OPTIONAL_AMOUNTS.iter().copied())
            .chain(AMOUNTS.iter().copied())
            .find(|(k, _)| *k == key)
            .map(|(_, l)| l)
            .unwrap_or("")
    }

    fn f(&self, key: &str, layout: Layout) -> AnyElement {
        field(
            &self.kit,
            key,
            Self::label(key),
            layout,
            !self.draft.lifecycle.is_editable(),
        )
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = choice_row(
            "Item 2 — Amended return?",
            vec![
                choice("1706_amended_yes", "Yes", d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(true))
                    },
                )),
                choice("1706_amended_no", "No", !d.is_amended, !editable).on_click(cx.listener(
                    |this: &mut Self, _, window, cx| {
                        kit::edit_and_reload(this, window, cx, |d| d.set_amended(false))
                    },
                )),
            ],
        );
        let seller_type = choice_row(
            "Item 4 — ATC",
            vec![
                choice(
                    "1706_atc_ii420",
                    "II420 — Individual",
                    d.seller_type == Some(Form1706SellerType::Individual),
                    !editable,
                )
                .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| {
                        d.set_seller_type(Form1706SellerType::Individual)
                    })
                })),
                choice(
                    "1706_atc_ic420",
                    "IC420 — Corporation",
                    d.seller_type == Some(Form1706SellerType::Corporation),
                    !editable,
                )
                .on_click(cx.listener(|this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| {
                        d.set_seller_type(Form1706SellerType::Corporation)
                    })
                })),
            ],
        );
        let children = vec![
            self.f("month", layout),
            self.f("day", layout),
            self.f("year", layout),
            self.f("sheets", layout),
            amended,
            seller_type,
        ];
        section(
            "Date of transaction and return type",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_parties(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let d = &self.draft;
        let fixed = |caption: &str, value: String| -> AnyElement {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(kit::label(caption))
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element()
        };
        let children = vec![
            fixed("Item 5 — Seller TIN", format_tin(&d.tin)),
            fixed("Item 6 — Seller RDO code", d.rdo_code.clone()),
            self.f("buyer_tin", layout),
            self.f("buyer_branch", layout),
            self.f("buyer_rdo", layout),
            self.f("seller_name", layout),
            self.f("buyer_name", layout),
            self.f("seller_address", layout),
            self.f("buyer_address", layout),
            self.f("seller_residence", layout),
        ];
        section(
            "Part I — Seller and buyer",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_property(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let classes: Vec<_> = Form1706PropertyClass::ALL
            .iter()
            .enumerate()
            .map(|(index, class)| {
                let class = *class;
                choice(
                    ("1706_class", index),
                    class.label(),
                    d.property_class == Some(class),
                    !editable,
                )
                .on_click(cx.listener(move |this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| {
                        d.property_class = Some(class);
                        if class != Form1706PropertyClass::Others {
                            d.property_class_other.clear();
                        }
                    })
                }))
            })
            .collect();
        let mut fields = vec![self.f("location", layout), self.f("property_rdo", layout)];
        if d.property_class == Some(Form1706PropertyClass::Others) {
            fields.push(self.f("class_other", layout));
        }
        fields.extend([
            self.f("tct", layout),
            self.f("area", layout),
            self.f("tax_dec", layout),
            self.f("property_other", layout),
        ]);
        let individual = d.seller_type == Some(Form1706SellerType::Individual);
        let mut questions = Vec::new();
        if individual {
            questions.push(yes_no::<Self>(
                "1706_item17",
                "Item 17 — Is the property being sold your principal residence?",
                d.principal_residence,
                !editable,
                cx,
                |d, yes| d.set_individual_answers(Some(yes), d.new_residence_within_18_months),
            ));
            questions.push(yes_no::<Self>(
                "1706_item18",
                "Item 18 — Do you intend to construct or acquire a new principal residence within 18 months?",
                d.new_residence_within_18_months,
                !editable,
                cx,
                |d, yes| d.set_individual_answers(d.principal_residence, Some(yes)),
            ));
        }
        questions.push(yes_no::<Self>(
            "1706_item19",
            "Item 19 — Does the selling price cover more than one property?",
            d.covers_multiple_properties,
            !editable,
            cx,
            |d, yes| d.covers_multiple_properties = Some(yes),
        ));
        let relief = d.tax_relief;
        let reliefs: Vec<_> = [
            (Form1706TaxRelief::No, "No"),
            (
                Form1706TaxRelief::InternationalTaxTreaty,
                "Yes — International Tax Treaty",
            ),
            (Form1706TaxRelief::SpecialLaw, "Yes — Special Law"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, caption))| {
            choice(("1706_relief", index), caption, relief == value, !editable).on_click(
                cx.listener(move |this: &mut Self, _, _, cx| {
                    kit::edit(this, cx, |d| d.set_tax_relief(value))
                }),
            )
        })
        .collect();
        questions.push(choice_row(
            "Item 20 — Availing of tax relief under an International Tax Treaty or Special Law?",
            reliefs,
        ));
        section(
            "Part I — Property (Items 14–20)",
            vec![
                choice_row("Item 15 — Classification of property", classes),
                grid(layout, fields),
                grid(layout, questions),
            ],
            cx,
        )
    }

    fn render_transaction(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let kinds: Vec<_> = Form1706TransactionType::ALL
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                let kind = *kind;
                choice(
                    ("1706_txn", index),
                    kind.label(),
                    d.transaction_type == Some(kind),
                    !editable,
                )
                .on_click(cx.listener(move |this: &mut Self, _, window, cx| {
                    kit::edit_and_reload(this, window, cx, |d| d.set_transaction_type(kind))
                }))
            })
            .collect();
        let mut children = vec![choice_row("Item 21 — Description of transaction", kinds)];
        if d.transaction_type.is_some_and(|t| t.needs_description()) {
            children.push(self.f("txn_description", layout));
        }
        if d.transaction_type == Some(Form1706TransactionType::InstallmentSale) {
            let keys = [
                "inst_selling",
                "inst_cost",
                "inst_mortgage",
                "inst_initial",
                "inst_month_amount",
                "inst_count",
                "inst_date_month",
                "inst_date_day",
                "inst_date_year",
            ];
            children.push(grid(
                layout,
                keys.iter().map(|key| self.f(key, layout)).collect(),
            ));
        }
        section("Transaction (Items 21–28)", children, cx)
    }

    fn render_taxable_base(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fmv = grid(
            layout,
            [
                "fmv_land_td",
                "fmv_improvements_td",
                "fmv_land_zonal",
                "fmv_improvements_bir",
            ]
            .iter()
            .map(|key| self.f(key, layout))
            .collect(),
        );
        let options: Vec<_> = Form1706TaxableBase::ALL
            .iter()
            .enumerate()
            .map(|(index, option)| {
                let option = *option;
                let unavailable = option == Form1706TaxableBase::UnutilizedProceeds
                    && !d.unutilized_proceeds_apply();
                choice(
                    ("1706_base", index),
                    option.label(),
                    d.taxable_base_option == Some(option),
                    !editable || unavailable,
                )
                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                    kit::edit(this, cx, |d| d.taxable_base_option = Some(option))
                }))
            })
            .collect();
        let mut amounts = Vec::new();
        match d.transaction_type {
            Some(Form1706TransactionType::CashSale) => {
                amounts.push(self.f("gross", layout));
            }
            Some(Form1706TransactionType::ForeclosureSale) => {
                amounts.push(self.f("gross", layout));
                amounts.push(self.f("bid", layout));
            }
            Some(Form1706TransactionType::InstallmentSale) => {
                amounts.push(self.f("installment", layout));
            }
            Some(Form1706TransactionType::Exempt | Form1706TransactionType::Others) => {
                amounts.push(self.f("others_description", layout));
                amounts.push(self.f("others_amount", layout));
            }
            None => {}
        }
        if d.unutilized_proceeds_apply() {
            amounts.push(self.f("unutilized", layout));
        }
        amounts.push(kit::computed_text(
            "30C — FMV of land and improvement (higher pair of Item 29)",
            optional_money_text(d.fair_market_value),
            cx,
        ));
        section(
            "Valuation and taxable base (Items 29–30)",
            vec![
                kit::label("Item 29 — Fair market value at the time of the contract")
                    .into_any_element(),
                fmv,
                choice_row("Item 30 — Determination of taxable base", options),
                grid(layout, amounts),
            ],
            cx,
        )
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![
            computed("31 — Taxable base", d.taxable_base, cx),
            computed("32 — 6% tax due", d.tax_due, cx),
            field(
                &self.kit,
                "less",
                Self::label("less"),
                layout,
                !editable || !d.is_amended,
            ),
            computed("34 — Tax payable (overpayment)", d.tax_payable, cx),
            self.f("surcharge", layout),
            self.f("interest", layout),
            self.f("compromise", layout),
            computed("35D — Total penalties", d.total_penalties, cx),
            computed(
                "36 — Total amount payable (overpayment)",
                d.total_amount_payable,
                cx,
            ),
        ];
        if d.total_amount_payable < 0.0 {
            let over = d.overpayment;
            children.push(choice_row(
                "If overpayment, mark one:",
                vec![
                    choice(
                        "1706_refund",
                        "To be refunded",
                        over == Form1706Overpayment::Refund,
                        !editable,
                    )
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| d.overpayment = Form1706Overpayment::Refund)
                    })),
                    choice(
                        "1706_tcc",
                        "To be issued a Tax Credit Certificate",
                        over == Form1706Overpayment::TaxCreditCertificate,
                        !editable,
                    )
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                        kit::edit(this, cx, |d| {
                            d.overpayment = Form1706Overpayment::TaxCreditCertificate
                        })
                    })),
                ],
            ));
        }
        children.push(self.f("email", layout));
        section(
            "Part II — Computation of tax (Items 31–36)",
            vec![grid(layout, children)],
            cx,
        )
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.lifecycle.is_editable();
        let cells: Vec<AnyElement> = (0..FORM_1706_SCHEDULE_1_CELLS)
            .map(|index| {
                let row = index / 2 + 1;
                let column = if index % 2 == 0 { "A" } else { "B" };
                field(
                    &self.kit,
                    &schedule_key(index),
                    &format!("Row {row} {column}"),
                    layout,
                    disabled,
                )
            })
            .collect();
        section(
            "Schedule 1 — Computation of tax base on the unutilized portion of sales proceeds",
            vec![grid(layout, cells)],
            cx,
        )
    }
}

impl KitView for Form1706View {
    type Draft = Form1706Draft;
    const SLUG: &'static str = "1706";
    const CODE: &'static str = "1706";

    fn kit(&self) -> &EditorState {
        &self.kit
    }
    fn kit_mut(&mut self) -> &mut EditorState {
        &mut self.kit
    }
    fn draft(&self) -> &Form1706Draft {
        &self.draft
    }
    fn draft_mut(&mut self) -> &mut Form1706Draft {
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
        let mut reader = InputReader::new(&self.kit, cx);
        let d = &mut self.draft;
        if let Some(month) = reader.whole::<u8>("month", "Item 1 month", 99) {
            d.transaction_month = month;
        }
        if let Some(day) = reader.whole::<u8>("day", "Item 1 day", 99) {
            d.transaction_day = day;
        }
        if let Some(year) = reader.whole::<u16>("year", "Item 1 year", 9999) {
            d.transaction_year = year;
        }
        if let Some(sheets) = reader.whole::<u16>("sheets", "Item 3", 999) {
            d.number_of_attached_sheets = sheets;
        }
        d.buyer_tin = reader.trimmed("buyer_tin");
        d.buyer_branch_code = reader.trimmed("buyer_branch");
        d.buyer_rdo_code = reader.trimmed("buyer_rdo").to_uppercase();
        d.seller_name = reader.text("seller_name");
        d.buyer_name = reader.text("buyer_name");
        d.seller_address = reader.text("seller_address");
        d.buyer_address = reader.text("buyer_address");
        d.seller_residence_address = reader.text("seller_residence");
        d.property_location = reader.text("location");
        d.property_rdo_code = reader.trimmed("property_rdo").to_uppercase();
        d.property_class_other = reader.text("class_other");
        d.tct_number = reader.trimmed("tct");
        d.area_sold = reader.trimmed("area");
        d.tax_declaration_number = reader.trimmed("tax_dec");
        d.property_other_description = reader.text("property_other");
        d.transaction_description = reader.text("txn_description");
        d.installment.date_month = reader.trimmed("inst_date_month");
        d.installment.date_day = reader.trimmed("inst_date_day");
        d.installment.date_year = reader.trimmed("inst_date_year");
        d.others_description = reader.text("others_description");
        d.email = reader.trimmed("email");
        for (key, label) in OPTIONAL_AMOUNTS {
            if let Some(value) = reader.optional_amount(key, label) {
                Self::set_optional_amount(d, key, value);
            }
        }
        if let Some(value) = reader.amount("less", "Item 33") {
            d.tax_paid_previous = value;
        }
        if let Some(value) = reader.amount("surcharge", "Item 35A") {
            d.surcharge = value;
        }
        if let Some(value) = reader.amount("interest", "Item 35B") {
            d.interest = value;
        }
        if let Some(value) = reader.amount("compromise", "Item 35C") {
            d.compromise = value;
        }
        let cells: Vec<String> = (0..FORM_1706_SCHEDULE_1_CELLS)
            .map(|index| reader.text(&schedule_key(index)))
            .collect();
        let used = cells
            .iter()
            .rposition(|cell| !cell.trim().is_empty())
            .map_or(0, |last| last + 1);
        d.schedule_1 = cells.into_iter().take(used).collect();
        reader.errors
    }
}

impl QueueableFormView for Form1706View {
    type Draft = Form1706Draft;

    fn new(
        draft: Form1706Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let kit = EditorState::new(Self::input_specs(&draft), errors, window, cx);
        Self { draft, db, kit }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form1706Draft {
        Form1706Draft::new_from_profile(profile, year, u32::from(period.max(1)))
    }
}

impl FormViewTrait for Form1706View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1706"
    }
    fn form_subtitle(&self) -> &'static str {
        "Capital Gains Tax Return (Onerous Transfer of Real Property Classified as Capital Asset)"
    }
    fn form_version(&self) -> &'static str {
        "eBIRForms package 2018"
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
            Some("1706 payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        kit::cancel(self, window, cx);
    }
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        kit::preview(self, "1706-2018", cx);
    }
}

impl Render for Form1706View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let sections = vec![
            self.render_header_items(layout, cx),
            self.render_parties(layout, cx),
            self.render_property(layout, cx),
            self.render_transaction(layout, cx),
            self.render_taxable_base(layout, cx),
            self.render_computation(layout, cx),
            self.render_schedule(layout, cx),
        ];
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{Form1706View, OPTIONAL_AMOUNTS, TEXT_INPUTS};

    #[test]
    fn every_input_has_a_label() {
        for (key, _, _) in TEXT_INPUTS {
            assert!(!Form1706View::label(key).is_empty(), "{key}");
        }
        for (key, _) in OPTIONAL_AMOUNTS {
            assert!(!Form1706View::label(key).is_empty(), "{key}");
        }
    }
}
