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
use gpui_component::button::Button;

use super::event_form_kit::{
    self as kit, EditorState, InputReader, InputSpec, KitView, Layout, choice_row, computed, field,
    format_tin, grid, money_text, optional_money_text, section, yes_no,
};
use super::form_controls::{
    messages_for, new_select_state, on_select, select_field, sync_select, with_field_error,
};
use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

/// Kit choice chip that may shrink and wrap inside its card on narrow widths.
fn choice(
    id: impl Into<ElementId>,
    label_text: impl Into<SharedString>,
    selected: bool,
    disabled: bool,
) -> Button {
    kit::choice(id, label_text, selected, disabled)
        .min_w_0()
        .max_w_full()
}

/// Plain text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("month", "Item 1 — Month of transaction", "MM"),
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 4 — No. of sheets attached", "0"),
    ("buyer_tin", "Item 7A — Buyer TIN", "000-000-000"),
    ("buyer_branch", "Item 7A — Buyer branch code", "00000"),
    ("buyer_rdo", "Buyer RDO code", "e.g. 040"),
    ("seller_name", "Item 6A — Seller's name", ""),
    ("buyer_name", "Item 7A — Buyer's name", ""),
    (
        "seller_address",
        "Item 6A — Seller's registered address",
        "",
    ),
    ("buyer_address", "Item 7A — Buyer's registered address", ""),
    (
        "seller_residence",
        "Seller's residence address (individuals)",
        "",
    ),
    ("location", "Schedule 1 — Location of property", ""),
    (
        "property_rdo",
        "Item 5 — RDO code of the location of the property",
        "e.g. 039",
    ),
    ("class_other", "Schedule 1 — Other classification", ""),
    ("tct", "Schedule 1 — OCT/TCT/CCT No.", "T-000000"),
    ("area", "Schedule 1 — Area (sq. m)", ""),
    ("tax_dec", "Schedule 1 — Tax Declaration No.", ""),
    ("property_other", "Schedule 1 — Others", ""),
    (
        "txn_description",
        "Schedule 2 — If exempt or others, specify",
        "",
    ),
    (
        "inst_date_month",
        "Schedule 2 Item 7 — Date of installment (MM)",
        "MM",
    ),
    ("inst_date_day", "Schedule 2 Item 7 — Day (DD)", "DD"),
    ("inst_date_year", "Schedule 2 Item 7 — Year (YYYY)", "YYYY"),
    (
        "others_description",
        "Schedule 4 Item E — Others (specify)",
        "",
    ),
    ("email", "Email address for the BIR confirmation", ""),
];

/// Optional amounts (blank stays blank on the official page).
const OPTIONAL_AMOUNTS: &[(&str, &str)] = &[
    ("inst_selling", "Schedule 2 Item 1 — Selling price"),
    ("inst_cost", "Schedule 2 Item 2 — Cost and expenses"),
    ("inst_mortgage", "Schedule 2 Item 3 — Mortgage assumed"),
    (
        "inst_initial",
        "Schedule 2 Item 4 — Total payments during the initial year",
    ),
    (
        "inst_month_amount",
        "Schedule 2 Item 5 — Amount of periodic payment",
    ),
    (
        "inst_count",
        "Schedule 2 Item 6 — No. of installments in the contract",
    ),
    (
        "fmv_land_td",
        "Schedule 1 — FMV of land per latest tax declaration",
    ),
    (
        "fmv_improvements_td",
        "Schedule 1 — FMV of improvements per latest tax declaration",
    ),
    (
        "fmv_land_zonal",
        "Schedule 1 — FMV of land per BIR zonal value",
    ),
    (
        "fmv_improvements_bir",
        "Schedule 1 — FMV of improvements per BIR rules",
    ),
    ("gross", "Schedule 4 Item A — Gross selling price"),
    ("bid", "Schedule 4 Item A — Bid price (foreclosure sale)"),
    (
        "installment",
        "Schedule 4 Item D — Installment collected (Schedule 2 Item 5)",
    ),
    (
        "unutilized",
        "Schedule 4 Item C — Unutilized portion of sales proceeds",
    ),
    ("others_amount", "Schedule 4 Item E — Others amount"),
];

/// Amounts where blank is zero.
const AMOUNTS: &[(&str, &str)] = &[
    (
        "less",
        "Item 15 — Tax paid in return previously filed (amended only)",
    ),
    ("surcharge", "Item 17A — Surcharge"),
    ("interest", "Item 17B — Interest"),
    ("compromise", "Item 17C — Compromise"),
];

fn schedule_key(index: usize) -> String {
    format!("sched{index}")
}

/// Official radio-id order of the classes (`j_id391:_1` .. `_6`, then `_8`).
const CLASS_RADIO_ORDER: [Form1706PropertyClass; 7] = [
    Form1706PropertyClass::Residential,
    Form1706PropertyClass::Agricultural,
    Form1706PropertyClass::Commercial,
    Form1706PropertyClass::Industrial,
    Form1706PropertyClass::CondominiumResidential,
    Form1706PropertyClass::CondominiumCommercial,
    Form1706PropertyClass::Others,
];

/// Classification classes as `(value, label)` in official radio-id order;
/// the printed label is the value.
fn class_options() -> Vec<(&'static str, &'static str)> {
    CLASS_RADIO_ORDER
        .iter()
        .map(|c| (c.label(), c.label()))
        .collect()
}

/// Item 21 transaction kinds as `(value, label)`.
fn transaction_options() -> Vec<(&'static str, &'static str)> {
    Form1706TransactionType::ALL
        .iter()
        .map(|t| (t.label(), t.label()))
        .collect()
}

/// Item 30 taxable-base options; the unutilized-proceeds option is listed
/// only while it applies.
fn base_options(unutilized_applies: bool) -> Vec<(&'static str, &'static str)> {
    Form1706TaxableBase::ALL
        .iter()
        .filter(|o| unutilized_applies || **o != Form1706TaxableBase::UnutilizedProceeds)
        .map(|o| (o.label(), o.label()))
        .collect()
}

/// Validation keys (draft field names) behind a page input key, so each
/// message shows under the input it belongs to.
fn error_keys(key: &str) -> Vec<String> {
    let mapped: &[&str] = match key {
        "month" => &["transaction_date"],
        "day" | "year" => &[],
        "sheets" => &["number_of_attached_sheets"],
        "buyer_rdo" => &["buyer_rdo_code"],
        "seller_residence" => &["seller_residence_address"],
        "location" => &["property_location"],
        "property_rdo" => &["property_rdo_code"],
        "class_other" => &["property_class_other"],
        "tct" => &["tct_number"],
        "area" => &["area_sold"],
        "tax_dec" => &["tax_declaration_number"],
        "property_other" => &["property_other_description"],
        "txn_description" => &["transaction_description"],
        "inst_date_month" => &["installment.date_month"],
        "inst_date_day" => &["installment.date_day"],
        "inst_date_year" => &["installment.date_year"],
        "inst_selling" => &["installment.selling_price"],
        "inst_cost" => &["installment.cost_and_expenses"],
        "inst_mortgage" => &["installment.mortgage_assumed"],
        "inst_initial" => &["installment.initial_year_payments"],
        "inst_month_amount" => &["installment.installment_this_month"],
        "inst_count" => &["installment.number_of_installments"],
        "fmv_land_td" => &["fmv_land_tax_declaration"],
        "fmv_improvements_td" => &["fmv_improvements_tax_declaration"],
        "gross" => &["gross_selling_price"],
        "bid" => &["bid_price"],
        "installment" => &["taxable_installment"],
        "unutilized" => &["unutilized_proceeds"],
        "less" => &["tax_paid_previous"],
        _ => &[],
    };
    let mut keys: Vec<String> = mapped.iter().map(|k| k.to_string()).collect();
    keys.push(key.to_string());
    if let Some(index) = key.strip_prefix("sched") {
        keys.push(format!("schedule_1[{index}]"));
    }
    keys
}

pub struct Form1706View {
    draft: Form1706Draft,
    db: Arc<Mutex<Database>>,
    kit: EditorState,
    class_select: Entity<ComboboxState>,
    transaction_select: Entity<ComboboxState>,
    base_select: Entity<ComboboxState>,
    _subscriptions: Vec<Subscription>,
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

    /// The messages recorded for any of `keys`, if any.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.kit
                .validation_errors
                .iter()
                .chain(self.kit.parse_errors.iter()),
            keys,
        )
    }

    /// `element` with the messages for `keys` directly under it.
    fn with_err(&self, element: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(element, self.error_for(keys))
    }

    fn f(&self, key: &str, layout: Layout) -> AnyElement {
        let element = field(
            &self.kit,
            key,
            Self::label(key),
            layout,
            !self.draft.lifecycle.is_editable(),
        );
        let keys = error_keys(key);
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.with_err(element, &keys)
    }

    fn render_header_items(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let amended = choice_row(
            layout,
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
            layout,
            "Item 3 — ATC",
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
            amended,
            self.f("sheets", layout),
            self.with_err(seller_type, &["seller_type"]),
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
            self.with_err(fixed("Item 6A — Seller TIN", format_tin(&d.tin)), &["tin"]),
            self.with_err(fixed("Seller RDO code", d.rdo_code.clone()), &["rdo_code"]),
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
        let classes = select_field(
            "Schedule 1 — Classification of property",
            &self.class_select,
            &class_options(),
            d.property_class.map(Form1706PropertyClass::label),
            !editable,
            self.error_for(&["property_class"]).as_deref(),
            cx,
        );
        let head = vec![self.f("location", layout), self.f("property_rdo", layout)];
        let mut fields = Vec::new();
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
            questions.push(self.with_err(
                yes_no::<Self>(
                    layout,
                    "1706_item17",
                    "Item 8 — Is the property being sold your principal residence?",
                    d.principal_residence,
                    !editable,
                    cx,
                    |d, yes| d.set_individual_answers(Some(yes), d.new_residence_within_18_months),
                ),
                &["principal_residence"],
            ));
            questions.push(self.with_err(
                yes_no::<Self>(
                    layout,
                    "1706_item18",
                    "Item 9 — Do you intend to construct or acquire a new principal residence within 18 months?",
                    d.new_residence_within_18_months,
                    !editable,
                    cx,
                    |d, yes| d.set_individual_answers(d.principal_residence, Some(yes)),
                ),
                &["new_residence_within_18_months"],
            ));
        }
        questions.push(self.with_err(
            yes_no::<Self>(
                layout,
                "1706_item19",
                "Item 10 — Does the selling price cover more than one property?",
                d.covers_multiple_properties,
                !editable,
                cx,
                |d, yes| d.covers_multiple_properties = Some(yes),
            ),
            &["covers_multiple_properties"],
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
        questions.push(self.with_err(
            choice_row(
                layout,
                "Item 11 — Availing of tax relief under an International Tax Treaty or Special Law?",
                reliefs,
            ),
            &["tax_relief"],
        ));
        section(
            "Part I — Property (Item 5, Schedule 1, Items 8–11)",
            vec![
                grid(layout, head),
                classes,
                grid(layout, fields),
                grid(layout, questions),
            ],
            cx,
        )
    }

    fn render_transaction(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![select_field(
            "Schedule 2 — Description of transaction",
            &self.transaction_select,
            &transaction_options(),
            d.transaction_type.map(Form1706TransactionType::label),
            !editable,
            self.error_for(&["transaction_type"]).as_deref(),
            cx,
        )];
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
        section("Schedule 2 — Transaction", children, cx)
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
        let options = select_field(
            "Schedule 4 — Determination of taxable base",
            &self.base_select,
            &base_options(d.unutilized_proceeds_apply()),
            d.taxable_base_option.map(Form1706TaxableBase::label),
            !editable,
            self.error_for(&["taxable_base_option"]).as_deref(),
            cx,
        );
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
            "Schedule 4 Item B — Total FMV of land and improvement (higher pair in Schedule 1)",
            optional_money_text(d.fair_market_value),
            cx,
        ));
        section(
            "Valuation and taxable base (Schedules 1 and 4)",
            vec![
                kit::label("Schedule 1 — Fair market value at the time of the contract")
                    .into_any_element(),
                fmv,
                options,
                grid(layout, amounts),
            ],
            cx,
        )
    }

    fn render_computation(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let mut children = vec![
            self.with_err(
                computed("12 — Taxable base", d.taxable_base, cx),
                &["taxable_base"],
            ),
            computed("14 — Tax due (6%)", d.tax_due, cx),
            self.with_err(
                field(
                    &self.kit,
                    "less",
                    Self::label("less"),
                    layout,
                    !editable || !d.is_amended,
                ),
                &["tax_paid_previous", "less"],
            ),
            computed("16 — Tax payable (overpayment)", d.tax_payable, cx),
            self.f("surcharge", layout),
            self.f("interest", layout),
            self.f("compromise", layout),
            computed("17D — Total penalties", d.total_penalties, cx),
            self.with_err(
                computed(
                    "18 — Total amount payable (overpayment)",
                    d.total_amount_payable,
                    cx,
                ),
                &["total_amount_payable"],
            ),
        ];
        if d.total_amount_payable < 0.0 {
            let over = d.overpayment;
            children.push(self.with_err(
                choice_row(
                    layout,
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
                ),
                &["overpayment"],
            ));
        }
        children.push(self.f("email", layout));
        section(
            "Part II — Computation of tax (Items 12–18)",
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
        if let Some(sheets) = reader.whole::<u16>("sheets", "Item 4", 999) {
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
        if let Some(value) = reader.amount("less", "Item 15") {
            d.tax_paid_previous = value;
        }
        if let Some(value) = reader.amount("surcharge", "Item 17A") {
            d.surcharge = value;
        }
        if let Some(value) = reader.amount("interest", "Item 17B") {
            d.interest = value;
        }
        if let Some(value) = reader.amount("compromise", "Item 17C") {
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

    const PRINT_SLUG: &'static str = "1706-2018";

    fn print_fields(draft: &Form1706Draft) -> std::collections::BTreeMap<String, String> {
        bir_core::forms::queueable::QueueableForm::field_map(draft)
    }

    fn agent_draft(&self) -> &Form1706Draft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        super::event_form_kit::header_actions::<Self>()
    }

    fn new(
        draft: Form1706Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let errors = bir_core::forms::queueable::QueueableForm::validate(&draft);
        let kit = EditorState::new(Self::input_specs(&draft), errors, window, cx);
        let class_options = class_options();
        let class_select = new_select_state(&class_options, window, cx);
        let transaction_options = transaction_options();
        let transaction_select = new_select_state(&transaction_options, window, cx);
        let base_options = base_options(true);
        let base_select = new_select_state(&base_options, window, cx);
        let subscriptions = vec![
            on_select(
                &class_select,
                &class_options,
                window,
                cx,
                |this: &mut Self, value, window, cx| {
                    if let Some(class) = Form1706PropertyClass::ALL
                        .into_iter()
                        .find(|c| c.label() == value)
                    {
                        kit::edit_and_reload(this, window, cx, |d| {
                            d.property_class = Some(class);
                            if class != Form1706PropertyClass::Others {
                                d.property_class_other.clear();
                            }
                        })
                    }
                },
            ),
            on_select(
                &transaction_select,
                &transaction_options,
                window,
                cx,
                |this: &mut Self, value, window, cx| {
                    if let Some(kind) = Form1706TransactionType::ALL
                        .into_iter()
                        .find(|t| t.label() == value)
                    {
                        kit::edit_and_reload(this, window, cx, |d| d.set_transaction_type(kind))
                    }
                },
            ),
            on_select(
                &base_select,
                &base_options,
                window,
                cx,
                |this: &mut Self, value, _, cx| {
                    if let Some(option) = Form1706TaxableBase::ALL
                        .into_iter()
                        .find(|o| o.label() == value)
                    {
                        kit::edit(this, cx, |d| d.taxable_base_option = Some(option))
                    }
                },
            ),
        ];
        Self {
            draft,
            db,
            kit,
            class_select,
            transaction_select,
            base_select,
            _subscriptions: subscriptions,
        }
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
        kit::preview(self, Self::PRINT_SLUG, cx);
    }
}

impl Render for Form1706View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        sync_select(
            &self.class_select,
            &class_options(),
            self.draft.property_class.map(Form1706PropertyClass::label),
            window,
            cx,
        );
        sync_select(
            &self.transaction_select,
            &transaction_options(),
            self.draft
                .transaction_type
                .map(Form1706TransactionType::label),
            window,
            cx,
        );
        sync_select(
            &self.base_select,
            &base_options(self.draft.unutilized_proceeds_apply()),
            self.draft
                .taxable_base_option
                .map(Form1706TaxableBase::label),
            window,
            cx,
        );
        let mut sections = vec![
            self.render_header_items(layout, cx),
            self.render_parties(layout, cx),
            self.render_property(layout, cx),
            self.render_transaction(layout, cx),
            self.render_taxable_base(layout, cx),
            self.render_computation(layout, cx),
            self.render_schedule(layout, cx),
        ];
        sections.extend(super::event_form_kit::declaration_and_payment_with_text(
            layout == Layout::Phone,
            "We declare under the penalties of perjury that this return, and all its attachments, have been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give my/our consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes.",
            "Part III – Details of Payment",
            &[
                "19 Cash/Bank Debit Memo",
                "20 Check",
                "21 Tax Debit Memo",
                "22 Others (Specify below)",
            ],
            &[
                "Drawee Bank/Bank Code/Agency",
                "Number",
                "Date (MM/DD/YYYY)",
                "Amount",
            ],
            cx,
        ));
        kit::render_page(self, sections, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{Form1706View, OPTIONAL_AMOUNTS, TEXT_INPUTS, class_options};

    #[test]
    fn classes_follow_official_radio_order() {
        let labels: Vec<&str> = class_options().iter().map(|(_, l)| *l).collect();
        assert_eq!(
            labels,
            [
                "Residential",
                "Agricultural",
                "Commercial",
                "Industrial",
                "Condominium Residential",
                "Condominium Commercial",
                "Others (specify)",
            ]
        );
    }

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
