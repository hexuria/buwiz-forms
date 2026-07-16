//! Evidence-safe editor for exact Form 1701, January 2018 (ENCS).
//!
//! The editor owns the semantic four-page return. It deliberately keeps
//! electronic submission disabled: the reviewed source pack proves editable
//! save round-trip, but not queue/final-flag behavior. Part X and attachment
//! worksheet fields survive imported XML snapshots but are not guessed here.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_1701::{
    Form1701AmountSection, Form1701Atc, Form1701CivilStatus, Form1701DeductionMethod,
    Form1701Draft, Form1701EmployerRow, Form1701JointFilingStatus, Form1701OverpaymentDisposition,
    Form1701Party, Form1701PaymentRow, Form1701SpouseType, Form1701TaxRate, Form1701TaxpayerType,
};
use bir_core::forms::{FilingStatus, FormValidator};
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;

pub enum Form1701Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1701Event> for Form1701View {}

const AMOUNT_INPUT_SPECS: &[(Form1701AmountSection, u8, &str)] = &[
    (Form1701AmountSection::PartIi, 25, "Second installment"),
    (Form1701AmountSection::PartIi, 27, "Interest"),
    (Form1701AmountSection::PartIi, 28, "Surcharge"),
    (Form1701AmountSection::PartIi, 29, "Compromise"),
    (
        Form1701AmountSection::Schedule2,
        5,
        "Less: non-taxable/exempt compensation",
    ),
    (
        Form1701AmountSection::Schedule3,
        8,
        "Sales/revenues/receipts/fees",
    ),
    (
        Form1701AmountSection::Schedule3,
        9,
        "Less: sales returns, allowances and discounts",
    ),
    (
        Form1701AmountSection::Schedule3,
        11,
        "Less: cost of sales/services",
    ),
    (Form1701AmountSection::Schedule3, 13, "Itemized deductions"),
    (
        Form1701AmountSection::Schedule3,
        14,
        "Special allowable itemized deductions",
    ),
    (Form1701AmountSection::Schedule3, 15, "NOLCO"),
    (
        Form1701AmountSection::Schedule3,
        19,
        "Taxable income from prior business/profession",
    ),
    (Form1701AmountSection::Schedule3, 20, "Other taxable income"),
    (Form1701AmountSection::Schedule3, 21, "Share from GPP"),
    (
        Form1701AmountSection::Schedule3,
        26,
        "8% sales/revenues/receipts/fees",
    ),
    (
        Form1701AmountSection::Schedule3,
        27,
        "8% non-operating income",
    ),
    (Form1701AmountSection::Schedule4, 1, "Amortization"),
    (Form1701AmountSection::Schedule4, 2, "Bad debts"),
    (
        Form1701AmountSection::Schedule4,
        3,
        "Charitable contributions",
    ),
    (Form1701AmountSection::Schedule4, 4, "Depletion"),
    (Form1701AmountSection::Schedule4, 5, "Depreciation"),
    (
        Form1701AmountSection::Schedule4,
        6,
        "Entertainment/recreation",
    ),
    (Form1701AmountSection::Schedule4, 7, "Fringe benefits"),
    (Form1701AmountSection::Schedule4, 8, "Interest"),
    (Form1701AmountSection::Schedule4, 9, "Losses"),
    (Form1701AmountSection::Schedule4, 10, "Pension trust"),
    (Form1701AmountSection::Schedule4, 11, "Rental"),
    (
        Form1701AmountSection::Schedule4,
        12,
        "Research and development",
    ),
    (
        Form1701AmountSection::Schedule4,
        13,
        "Salaries, wages and allowances",
    ),
    (
        Form1701AmountSection::Schedule4,
        14,
        "SSS/GSIS/PhilHealth/HDMF contributions",
    ),
    (Form1701AmountSection::Schedule4, 15, "Taxes and licenses"),
    (
        Form1701AmountSection::Schedule4,
        16,
        "Transportation and travel",
    ),
    (Form1701AmountSection::Schedule6, 1, "NOLCO available"),
    (Form1701AmountSection::Schedule6, 2, "NOLCO applied"),
    (
        Form1701AmountSection::PartVi,
        2,
        "Special-rate income tax due",
    ),
    (Form1701AmountSection::PartVi, 3, "Foreign tax credits"),
    (
        Form1701AmountSection::PartVii,
        1,
        "Prior year's excess credits",
    ),
    (
        Form1701AmountSection::PartVii,
        2,
        "Quarterly income-tax payments",
    ),
    (
        Form1701AmountSection::PartVii,
        3,
        "Creditable tax withheld for the first three quarters",
    ),
    (
        Form1701AmountSection::PartVii,
        4,
        "Creditable tax withheld per BIR Form 2307 for the fourth quarter",
    ),
    (
        Form1701AmountSection::PartVii,
        6,
        "Tax paid in return previously filed, if amended",
    ),
    (Form1701AmountSection::PartVii, 7, "Foreign tax credits"),
    (Form1701AmountSection::PartVii, 8, "Special tax credits"),
    (
        Form1701AmountSection::PartVii,
        9,
        "Other tax credits/payments",
    ),
    (
        Form1701AmountSection::PartViii,
        1,
        "Regular income tax otherwise due - special rate",
    ),
    (
        Form1701AmountSection::PartViii,
        2,
        "Tax relief on special allowable itemized deductions",
    ),
    (
        Form1701AmountSection::PartViii,
        4,
        "Less: income tax due under special rate",
    ),
    (
        Form1701AmountSection::PartViii,
        6,
        "Add: special tax credit",
    ),
    (
        Form1701AmountSection::PartViii,
        8,
        "Regular income tax otherwise due - exempt income",
    ),
    (
        Form1701AmountSection::PartViii,
        9,
        "Tax relief on special allowable itemized deductions",
    ),
    (Form1701AmountSection::PartIx, 1, "Net income per books"),
    (Form1701AmountSection::PartIx, 2, "Non-deductible expenses"),
    (Form1701AmountSection::PartIx, 3, "Taxable other income"),
    (Form1701AmountSection::PartIx, 4, "Special deductions"),
    (
        Form1701AmountSection::PartIx,
        6,
        "Income not subject to tax",
    ),
    (
        Form1701AmountSection::PartIx,
        7,
        "Income subject to final tax",
    ),
    (
        Form1701AmountSection::PartIx,
        8,
        "Special deductions allowed",
    ),
    (Form1701AmountSection::PartIx, 9, "Other reconciling items"),
];

#[derive(Clone)]
struct PairedAmountInputs {
    taxpayer: Entity<InputState>,
    spouse: Entity<InputState>,
}

impl PairedAmountInputs {
    fn all(&self) -> [Entity<InputState>; 2] {
        [self.taxpayer.clone(), self.spouse.clone()]
    }
}

#[derive(Clone)]
struct EmployerInputs {
    name: Entity<InputState>,
    tin: Entity<InputState>,
    compensation: Entity<InputState>,
    withheld: Entity<InputState>,
}

impl EmployerInputs {
    fn all(&self) -> [Entity<InputState>; 4] {
        [
            self.name.clone(),
            self.tin.clone(),
            self.compensation.clone(),
            self.withheld.clone(),
        ]
    }
}

#[derive(Clone)]
struct PaymentRowInputs {
    agency: Entity<InputState>,
    number: Entity<InputState>,
    date: Entity<InputState>,
    amount: Entity<InputState>,
}

impl PaymentRowInputs {
    fn all(&self) -> [Entity<InputState>; 4] {
        [
            self.agency.clone(),
            self.number.clone(),
            self.date.clone(),
            self.amount.clone(),
        ]
    }
}

pub struct Form1701View {
    pub draft: Form1701Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    period_end_month: Entity<InputState>,
    number_of_attachments: Entity<InputState>,
    registered_address: Entity<InputState>,
    zip_code: Entity<InputState>,
    date_of_birth: Entity<InputState>,
    email: Entity<InputState>,
    citizenship: Entity<InputState>,
    foreign_tax_number: Entity<InputState>,
    contact_number: Entity<InputState>,

    spouse_tin: Entity<InputState>,
    spouse_rdo_code: Entity<InputState>,
    spouse_name: Entity<InputState>,
    spouse_contact_number: Entity<InputState>,
    spouse_citizenship: Entity<InputState>,
    spouse_foreign_tax_number: Entity<InputState>,

    schedule_3_description_19: Entity<InputState>,
    schedule_3_description_20: Entity<InputState>,
    schedule_3_description_27: Entity<InputState>,
    part_vii_description_9: Entity<InputState>,
    payment_37_description: Entity<InputState>,
    machine_validation: Entity<InputState>,

    amount_inputs: BTreeMap<(Form1701AmountSection, u8), PairedAmountInputs>,
    employer_inputs: [EmployerInputs; 2],
    payment_inputs: [PaymentRowInputs; 4],
    _subscriptions: Vec<Subscription>,
}

impl Form1701View {
    pub fn new(
        mut draft: Form1701Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        draft.recompute();

        let period_end_month = text_input(cx, &draft.period_end_month.to_string(), "1-12", window);
        let number_of_attachments = optional_u8_input(
            cx,
            draft.number_of_attachments,
            "0-99; blank is empty",
            window,
        );
        let registered_address =
            text_input(cx, &draft.registered_address, "Registered address", window);
        let zip_code = text_input(cx, &draft.zip_code, "ZIP code", window);
        let date_of_birth = text_input(cx, &draft.date_of_birth, "MM/DD/YYYY", window);
        let email = text_input(cx, &draft.email, "Email address", window);
        let citizenship = text_input(cx, &draft.citizenship, "Citizenship", window);
        let foreign_tax_number = text_input(
            cx,
            &draft.foreign_tax_number,
            "Foreign tax number when applicable",
            window,
        );
        let contact_number = text_input(cx, &draft.contact_number, "Contact number", window);

        let spouse_tin = text_input(cx, &draft.spouse.tin, "Spouse TIN", window);
        let spouse_rdo_code = text_input(cx, &draft.spouse.rdo_code, "Spouse RDO", window);
        let spouse_name = text_input(cx, &draft.spouse.name, "Spouse name", window);
        let spouse_contact_number = text_input(
            cx,
            &draft.spouse.contact_number,
            "Spouse contact number",
            window,
        );
        let spouse_citizenship =
            text_input(cx, &draft.spouse.citizenship, "Spouse citizenship", window);
        let spouse_foreign_tax_number = text_input(
            cx,
            &draft.spouse.foreign_tax_number,
            "Spouse foreign tax number",
            window,
        );

        let schedule_3_description_19 = text_input(
            cx,
            draft
                .computations
                .schedule_3_descriptions
                .get(&19)
                .map(String::as_str)
                .unwrap_or(""),
            "Item 19 description",
            window,
        );
        let schedule_3_description_20 = text_input(
            cx,
            draft
                .computations
                .schedule_3_descriptions
                .get(&20)
                .map(String::as_str)
                .unwrap_or(""),
            "Item 20 description",
            window,
        );
        let schedule_3_description_27 = text_input(
            cx,
            draft
                .computations
                .schedule_3_descriptions
                .get(&27)
                .map(String::as_str)
                .unwrap_or(""),
            "Item 27 description",
            window,
        );
        let part_vii_description_9 = text_input(
            cx,
            &draft.computations.part_vii_item_9_description,
            "Item 9 other credit/payment",
            window,
        );
        let payment_37_description = text_input(
            cx,
            &draft.payment_details.item_37_others_description,
            "Specify other payment",
            window,
        );
        let machine_validation = text_input(
            cx,
            &draft.machine_validation_or_receipt_details,
            "Machine validation / revenue official receipt details",
            window,
        );

        let amount_inputs = AMOUNT_INPUT_SPECS
            .iter()
            .map(|(section, item, _)| {
                (
                    (*section, *item),
                    PairedAmountInputs {
                        taxpayer: optional_amount_input(
                            cx,
                            draft.amount(*section, *item, Form1701Party::Taxpayer),
                            window,
                        ),
                        spouse: optional_amount_input(
                            cx,
                            draft.amount(*section, *item, Form1701Party::Spouse),
                            window,
                        ),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let employer_inputs =
            std::array::from_fn(|index| employer_input(cx, &draft.employers[index], window));
        let payment_rows = [
            &draft.payment_details.item_34_cash_or_bank_debit_memo,
            &draft.payment_details.item_35_check,
            &draft.payment_details.item_36_tax_debit_memo,
            &draft.payment_details.item_37_others,
        ];
        let payment_inputs =
            std::array::from_fn(|index| payment_input(cx, payment_rows[index], window));

        let mut all_inputs = vec![
            period_end_month.clone(),
            number_of_attachments.clone(),
            registered_address.clone(),
            zip_code.clone(),
            date_of_birth.clone(),
            email.clone(),
            citizenship.clone(),
            foreign_tax_number.clone(),
            contact_number.clone(),
            spouse_tin.clone(),
            spouse_rdo_code.clone(),
            spouse_name.clone(),
            spouse_contact_number.clone(),
            spouse_citizenship.clone(),
            spouse_foreign_tax_number.clone(),
            schedule_3_description_19.clone(),
            schedule_3_description_20.clone(),
            schedule_3_description_27.clone(),
            part_vii_description_9.clone(),
            payment_37_description.clone(),
            machine_validation.clone(),
        ];
        for inputs in amount_inputs.values() {
            all_inputs.extend(inputs.all());
        }
        for inputs in &employer_inputs {
            all_inputs.extend(inputs.all());
        }
        for inputs in &payment_inputs {
            all_inputs.extend(inputs.all());
        }

        let mut subscriptions = Vec::new();
        for input in all_inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
        }

        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            input_errors: Vec::new(),
            validation_errors,
            status_message: None,
            period_end_month,
            number_of_attachments,
            registered_address,
            zip_code,
            date_of_birth,
            email,
            citizenship,
            foreign_tax_number,
            contact_number,
            spouse_tin,
            spouse_rdo_code,
            spouse_name,
            spouse_contact_number,
            spouse_citizenship,
            spouse_foreign_tax_number,
            schedule_3_description_19,
            schedule_3_description_20,
            schedule_3_description_27,
            part_vii_description_9,
            payment_37_description,
            machine_validation,
            amount_inputs,
            employer_inputs,
            payment_inputs,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.input_errors.clear();

        assign_required_month(
            &mut self.draft.period_end_month,
            &self.period_end_month,
            cx,
            &mut self.input_errors,
        );
        assign_optional_u8(
            &mut self.draft.number_of_attachments,
            &self.number_of_attachments,
            "number_of_attachments",
            99,
            cx,
            &mut self.input_errors,
        );
        self.draft.registered_address = input_text(&self.registered_address, cx);
        self.draft.zip_code = input_text(&self.zip_code, cx);
        self.draft.date_of_birth = input_text(&self.date_of_birth, cx);
        self.draft.email = input_text(&self.email, cx);
        self.draft.citizenship = input_text(&self.citizenship, cx);
        self.draft.foreign_tax_number = input_text(&self.foreign_tax_number, cx);
        self.draft.contact_number = input_text(&self.contact_number, cx);

        self.draft.spouse.tin = input_text(&self.spouse_tin, cx);
        self.draft.spouse.rdo_code = input_text(&self.spouse_rdo_code, cx);
        self.draft.spouse.name = input_text(&self.spouse_name, cx);
        self.draft.spouse.contact_number = input_text(&self.spouse_contact_number, cx);
        self.draft.spouse.citizenship = input_text(&self.spouse_citizenship, cx);
        self.draft.spouse.foreign_tax_number = input_text(&self.spouse_foreign_tax_number, cx);

        self.draft
            .computations
            .schedule_3_descriptions
            .insert(19, input_text(&self.schedule_3_description_19, cx));
        self.draft
            .computations
            .schedule_3_descriptions
            .insert(20, input_text(&self.schedule_3_description_20, cx));
        self.draft
            .computations
            .schedule_3_descriptions
            .insert(27, input_text(&self.schedule_3_description_27, cx));
        self.draft.computations.part_vii_item_9_description =
            input_text(&self.part_vii_description_9, cx);
        self.draft.payment_details.item_37_others_description =
            input_text(&self.payment_37_description, cx);
        self.draft.machine_validation_or_receipt_details = input_text(&self.machine_validation, cx);

        for ((section, item), inputs) in &self.amount_inputs {
            assign_amount(
                &mut self.draft,
                *section,
                *item,
                Form1701Party::Taxpayer,
                &inputs.taxpayer,
                cx,
                &mut self.input_errors,
            );
            assign_amount(
                &mut self.draft,
                *section,
                *item,
                Form1701Party::Spouse,
                &inputs.spouse,
                cx,
                &mut self.input_errors,
            );
        }

        for (index, inputs) in self.employer_inputs.iter().enumerate() {
            let employer = &mut self.draft.employers[index];
            employer.employer_name = input_text(&inputs.name, cx);
            employer.employer_tin = input_text(&inputs.tin, cx);
            assign_optional_amount(
                &mut employer.compensation_income,
                &inputs.compensation,
                &format!("employer_{}_compensation", index + 1),
                false,
                cx,
                &mut self.input_errors,
            );
            assign_optional_amount(
                &mut employer.tax_withheld,
                &inputs.withheld,
                &format!("employer_{}_withheld", index + 1),
                false,
                cx,
                &mut self.input_errors,
            );
        }

        sync_payment_row(
            &mut self.draft.payment_details.item_34_cash_or_bank_debit_memo,
            &self.payment_inputs[0],
            "payment_34",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_35_check,
            &self.payment_inputs[1],
            "payment_35",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_36_tax_debit_memo,
            &self.payment_inputs[2],
            "payment_36",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_37_others,
            &self.payment_inputs[3],
            "payment_37",
            cx,
            &mut self.input_errors,
        );

        self.draft.recompute();
        self.validation_errors = self.input_errors.clone();
        self.validation_errors.extend(self.draft.validate());
        cx.notify();
    }

    fn notify(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        kind: gpui_component::notification::NotificationType,
        message: impl Into<String>,
    ) {
        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message(message.into())
                .with_type(kind)
                .autohide(true),
            cx,
        );
    }

    fn render_choice(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: bool,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Self) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .px_3()
            .py_2()
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(if selected {
                cx.theme().primary.opacity(0.15)
            } else {
                cx.theme().background
            })
            .rounded_md()
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.draft.is_editable() {
                    on_click(this);
                    this.sync_from_inputs(cx);
                }
            }))
            .child(label.into())
            .into_any_element()
    }

    fn render_input_row(&self, label: &str, input: &Entity<InputState>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().w_1_2().text_sm().child(label.to_string()))
            .child(
                div()
                    .w_1_2()
                    .child(Input::new(input).disabled(!self.draft.is_editable())),
            )
            .into_any_element()
    }

    fn render_error_summary(&self, cx: &Context<Self>) -> AnyElement {
        if self.validation_errors.is_empty() {
            return div().into_any_element();
        }
        let mut list = div().mt_2().flex().flex_col().gap_1();
        for (field, message) in &self.validation_errors {
            list = list.child(div().text_xs().child(format!("{field}: {message}")));
        }
        div()
            .p_4()
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().warning.opacity(0.1))
            .rounded_lg()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("Draft needs review"),
            )
            .child(list)
            .into_any_element()
    }

    fn render_filing_and_taxpayer(&self, cx: &Context<Self>) -> AnyElement {
        let mut choices = div().flex().flex_col().gap_3();
        choices = choices.child(div().flex().flex_wrap().gap_2().children(
            Form1701TaxpayerType::ALL.into_iter().map(|value| {
                self.render_choice(
                    format!("1701_type_{value:?}"),
                    format!("6 {}", value.label()),
                    self.draft.taxpayer_type == Some(value),
                    cx,
                    move |this| this.draft.taxpayer_type = Some(value),
                )
            }),
        ));
        choices = choices.child(div().flex().flex_wrap().gap_2().children(
            Form1701Atc::ALL.into_iter().map(|value| {
                self.render_choice(
                    format!("1701_atc_{}", value.code()),
                    format!("7 {} · {}", value.code(), value.label()),
                    self.draft.atc == Some(value),
                    cx,
                    move |this| {
                        this.draft.atc = Some(value);
                        this.draft.tax_rate = value.tax_rate();
                        if this.draft.tax_rate != Some(Form1701TaxRate::Graduated) {
                            this.draft.deduction_method = None;
                        }
                    },
                )
            }),
        ));
        choices = choices.child(choice_group_taxpayer(self, cx));

        section_card(cx, "ITEMS 1-21 — FILING AND BACKGROUND INFORMATION")
            .child(div().text_sm().child(format!(
                "Taxable year {} · TIN {} · RDO {} · Taxpayer {}",
                self.draft.taxable_year,
                self.draft.tin,
                self.draft.rdo_code,
                self.draft.taxpayer_name
            )))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.render_choice(
                        "1701_amended_yes",
                        "2 Amended: Yes",
                        self.draft.is_amended,
                        cx,
                        |this| this.draft.is_amended = true,
                    ))
                    .child(self.render_choice(
                        "1701_amended_no",
                        "2 Amended: No",
                        !self.draft.is_amended,
                        cx,
                        |this| this.draft.is_amended = false,
                    ))
                    .child(self.render_choice(
                        "1701_short_yes",
                        "3 Short Period: Yes",
                        self.draft.is_short_period,
                        cx,
                        |this| this.draft.is_short_period = true,
                    ))
                    .child(self.render_choice(
                        "1701_short_no",
                        "3 Short Period: No",
                        !self.draft.is_short_period,
                        cx,
                        |this| this.draft.is_short_period = false,
                    )),
            )
            .child(self.render_input_row(
                "Return period end month (annual return is December)",
                &self.period_end_month,
            ))
            .child(self.render_input_row("9 Registered address", &self.registered_address))
            .child(self.render_input_row("9A ZIP code", &self.zip_code))
            .child(self.render_input_row("10 Date of birth", &self.date_of_birth))
            .child(self.render_input_row("11 Email address", &self.email))
            .child(self.render_input_row("12 Citizenship", &self.citizenship))
            .child(self.render_input_row("14 Foreign tax number", &self.foreign_tax_number))
            .child(self.render_input_row("15 Contact number", &self.contact_number))
            .child(choices)
            .into_any_element()
    }

    fn render_spouse(&self, cx: &Context<Self>) -> AnyElement {
        let mut card = section_card(cx, "SPOUSE BACKGROUND INFORMATION").child(
            div().flex().gap_2().child(self.render_choice(
                "1701_spouse_enabled",
                if self.draft.spouse.enabled {
                    "Spouse section enabled"
                } else {
                    "Enable spouse section"
                },
                self.draft.spouse.enabled,
                cx,
                |this| this.draft.spouse.enabled = !this.draft.spouse.enabled,
            )),
        );
        if self.draft.spouse.enabled {
            card =
                card.child(self.render_input_row("Spouse TIN", &self.spouse_tin))
                    .child(self.render_input_row("Spouse RDO", &self.spouse_rdo_code))
                    .child(self.render_input_row("Spouse name", &self.spouse_name))
                    .child(self.render_input_row("Spouse contact", &self.spouse_contact_number))
                    .child(self.render_input_row("Spouse citizenship", &self.spouse_citizenship))
                    .child(self.render_input_row(
                        "Spouse foreign tax number",
                        &self.spouse_foreign_tax_number,
                    ))
                    .child(div().flex().flex_wrap().gap_2().children(
                        Form1701SpouseType::ALL.into_iter().map(|value| {
                            self.render_choice(
                                format!("1701_spouse_type_{value:?}"),
                                value.label(),
                                self.draft.spouse.filer_type == Some(value),
                                cx,
                                move |this| this.draft.spouse.filer_type = Some(value),
                            )
                        }),
                    ))
                    .child(div().flex().flex_wrap().gap_2().children(
                        Form1701Atc::ALL.into_iter().map(|value| {
                            self.render_choice(
                                format!("1701_spouse_atc_{}", value.code()),
                                format!("{} · {}", value.code(), value.label()),
                                self.draft.spouse.atc == Some(value),
                                cx,
                                move |this| {
                                    this.draft.spouse.atc = Some(value);
                                    this.draft.spouse.tax_rate = value.tax_rate();
                                    if this.draft.spouse.tax_rate
                                        != Some(Form1701TaxRate::Graduated)
                                    {
                                        this.draft.spouse.deduction_method = None;
                                    }
                                },
                            )
                        }),
                    ))
                    .child(choice_group_spouse(self, cx));
        }
        card.into_any_element()
    }

    fn render_employers(&self, cx: &Context<Self>) -> AnyElement {
        let mut card = section_card(cx, "SCHEDULE 1 — COMPENSATION INCOME");
        for (index, inputs) in self.employer_inputs.iter().enumerate() {
            let row = &self.draft.employers[index];
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Employer row {}", index + 1)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_choice(
                                format!("1701_employer_{}_taxpayer", index),
                                "Taxpayer/Filer",
                                row.owner == Some(Form1701Party::Taxpayer),
                                cx,
                                move |this| {
                                    this.draft.employers[index].owner =
                                        Some(Form1701Party::Taxpayer)
                                },
                            ))
                            .child(self.render_choice(
                                format!("1701_employer_{}_spouse", index),
                                "Spouse",
                                row.owner == Some(Form1701Party::Spouse),
                                cx,
                                move |this| {
                                    this.draft.employers[index].owner = Some(Form1701Party::Spouse)
                                },
                            ))
                            .child(self.render_choice(
                                format!("1701_employer_{}_clear", index),
                                "Unused",
                                row.owner.is_none(),
                                cx,
                                move |this| this.draft.employers[index].owner = None,
                            )),
                    )
                    .child(self.render_input_row("Employer name", &inputs.name))
                    .child(self.render_input_row("Employer TIN", &inputs.tin))
                    .child(self.render_input_row("Compensation income", &inputs.compensation))
                    .child(self.render_input_row("Tax withheld", &inputs.withheld)),
            );
        }
        card.into_any_element()
    }

    fn render_amount_section(
        &self,
        section: Form1701AmountSection,
        title: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut card = section_card(cx, title).child(amount_header(cx));
        for (_, item, label) in AMOUNT_INPUT_SPECS
            .iter()
            .filter(|(candidate, _, _)| *candidate == section)
        {
            let inputs = &self.amount_inputs[&(section, *item)];
            card = card.child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_3()
                    .items_center()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_sm().child(format!("{item} {label}")))
                    .child(Input::new(&inputs.taxpayer).disabled(!self.draft.is_editable()))
                    .child(Input::new(&inputs.spouse).disabled(!self.draft.is_editable())),
            );
        }
        for (item, label) in computed_rows(section) {
            card = card.child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_3()
                    .items_center()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{item} {label}")),
                    )
                    .child(computed_amount(
                        self.draft.amount(section, *item, Form1701Party::Taxpayer),
                        cx,
                    ))
                    .child(computed_amount(
                        self.draft.amount(section, *item, Form1701Party::Spouse),
                        cx,
                    )),
            );
        }
        card.into_any_element()
    }

    fn render_payment_section(&self, cx: &Context<Self>) -> AnyElement {
        let labels = [
            "34 Cash/Bank Debit Memo",
            "35 Check",
            "36 Tax Debit Memo",
            "37 Others",
        ];
        let mut card = section_card(cx, "PART III — DETAILS OF PAYMENT").child(
            div()
                .text_xs()
                .child("The reviewed XML has no drawee-bank/agency field for Item 36."),
        );
        for (index, inputs) in self.payment_inputs.iter().enumerate() {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(div().font_weight(FontWeight::BOLD).child(labels[index]))
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_2()
                            .child(
                                Input::new(&inputs.agency)
                                    .disabled(index == 2 || !self.draft.is_editable()),
                            )
                            .child(Input::new(&inputs.number).disabled(!self.draft.is_editable()))
                            .child(Input::new(&inputs.date).disabled(!self.draft.is_editable()))
                            .child(Input::new(&inputs.amount).disabled(!self.draft.is_editable())),
                    ),
            );
        }
        card.child(self.render_input_row("37 Others description", &self.payment_37_description))
            .child(self.render_input_row("Machine validation / receipt", &self.machine_validation))
            .into_any_element()
    }

    fn render_editor_sections(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        vec![
            self.render_filing_and_taxpayer(cx),
            self.render_spouse(cx),
            self.render_employers(cx),
            self.render_amount_section(
                Form1701AmountSection::Schedule2,
                "SCHEDULE 2 — TAX ON COMPENSATION INCOME",
                cx,
            ),
            self.render_amount_section(
                Form1701AmountSection::Schedule3,
                "SCHEDULE 3 — BUSINESS/PROFESSION INCOME",
                cx,
            ),
            section_card(cx, "SCHEDULE 3 — SPECIFY LINES")
                .child(self.render_input_row("19 Description", &self.schedule_3_description_19))
                .child(self.render_input_row("20 Description", &self.schedule_3_description_20))
                .child(self.render_input_row("27 Description", &self.schedule_3_description_27))
                .into_any_element(),
            self.render_amount_section(
                Form1701AmountSection::Schedule4,
                "SCHEDULE 4 — ORDINARY ALLOWABLE ITEMIZED DEDUCTIONS",
                cx,
            ),
            self.render_amount_section(
                Form1701AmountSection::Schedule6,
                "SCHEDULE 6 — NOLCO SUMMARY",
                cx,
            ),
            self.render_amount_section(Form1701AmountSection::PartVi, "PART VI — TAX DUE", cx),
            self.render_amount_section(
                Form1701AmountSection::PartVii,
                "PART VII — TAX CREDITS/PAYMENTS",
                cx,
            ),
            section_card(cx, "PART VII — OTHER CREDIT")
                .child(self.render_input_row("Item 9 description", &self.part_vii_description_9))
                .into_any_element(),
            self.render_amount_section(
                Form1701AmountSection::PartViii,
                "PART VIII — TAX RELIEF",
                cx,
            ),
            self.render_amount_section(
                Form1701AmountSection::PartIx,
                "PART IX — RECONCILIATION OF NET INCOME",
                cx,
            ),
            self.render_amount_section(
                Form1701AmountSection::PartIi,
                "PART II — TOTAL TAX PAYABLE/(OVERPAYMENT)",
                cx,
            ),
            self.render_overpayment_and_attachments(cx),
            self.render_payment_section(cx),
        ]
    }

    fn render_overpayment_and_attachments(&self, cx: &Context<Self>) -> AnyElement {
        section_card(cx, "ITEMS 32-33 — OVERPAYMENT AND ATTACHMENTS")
            .child(div().text_sm().child(format!(
                "Item 32 aggregate: {}",
                format_optional_amount(self.draft.computations.part_ii_item_32_aggregate)
            )))
            .child(
                div().flex().flex_wrap().gap_2().children(
                    [
                        Form1701OverpaymentDisposition::None,
                        Form1701OverpaymentDisposition::Refund,
                        Form1701OverpaymentDisposition::TaxCreditCertificate,
                        Form1701OverpaymentDisposition::CarryOver,
                    ]
                    .into_iter()
                    .map(|value| {
                        self.render_choice(
                            format!("1701_overpayment_{value:?}"),
                            overpayment_label(value),
                            self.draft.overpayment_disposition == value,
                            cx,
                            move |this| this.draft.overpayment_disposition = value,
                        )
                    }),
                ),
            )
            .child(self.render_input_row("33 Number of attachments", &self.number_of_attachments))
            .into_any_element()
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
        self.draft.status.clone()
    }

    fn submitted_at(&self) -> Option<&str> {
        self.draft.submitted_at.as_deref()
    }

    fn confirmed_at(&self) -> Option<&str> {
        self.draft.confirmed_at.as_deref()
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.input_errors.is_empty() {
            self.status_message = Some(
                "Draft was not saved because one or more editor values cannot be parsed."
                    .to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix invalid Form 1701 input text before saving.",
            );
            return;
        }

        self.draft.updated_at = chrono::Utc::now().to_rfc3339();
        let result = self
            .db
            .lock()
            .map_err(|_| "Draft database lock is unavailable".to_string())
            .and_then(|db| {
                db.save_form_draft(
                    &self.draft.tin,
                    "1701",
                    self.draft.taxable_year,
                    None,
                    &self.draft.status,
                    &self.draft,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            });

        match result {
            Ok(()) => {
                self.status_message = Some(if self.validation_errors.is_empty() {
                    "Draft saved locally.".to_string()
                } else {
                    "Draft saved locally with unresolved review items; submission remains disabled."
                        .to_string()
                });
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Success,
                    "Form 1701 draft saved locally.",
                );
                cx.emit(Form1701Event::Saved);
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save draft: {error}"));
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    format!("Could not save Form 1701 draft: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "1701v2018 is manual/external until queue and final-flag semantics are certified."
                .to_string(),
        );
        cx.emit(Form1701Event::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This Form 1701 draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Payment status cannot be advanced automatically for a manual/external filing."
                .to_string(),
        );
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.revert_to_draft() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => {
                self.status_message = Some(error.clone());
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    error,
                );
            }
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Freeze the latest parseable editor values into one immutable
        // envelope. Preview is experimental and never changes filing state.
        self.sync_from_inputs(cx);
        if !self.input_errors.is_empty() {
            let message = format!(
                "Experimental HTML preview was not opened because {} current editor value(s) could not be parsed. No filing state was changed.",
                self.input_errors.len()
            );
            self.status_message = Some(message.clone());
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                message,
            );
            cx.notify();
            return;
        }

        let render_draft = self.draft.clone();
        let envelope = bir_print::html::RenderEnvelopeV1::from(&render_draft);
        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => {
                let message = format!(
                    "{} Form 1701 HTML parity remains experimental. No filing state was changed.",
                    launch_kind.status_message()
                );
                self.status_message = Some(message);
            }
            Err(error) => {
                let message = format!(
                    "Experimental HTML print preview could not be opened: {error}. No filing state was changed."
                );
                self.status_message = Some(message.clone());
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    message,
                );
            }
        }
        cx.notify();
    }
}

impl Render for Form1701View {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_message = self.status_message.clone();
        let evidence_warnings = self.draft.xml_evidence_warnings();
        let is_draft = self.draft.is_editable();

        let mut content = div()
            .max_w(px(1100.0))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().warning)
                    .bg(cx.theme().warning.opacity(0.1))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Evidence boundary"),
                    )
                    .children(
                        evidence_warnings
                            .into_iter()
                            .map(|warning| div().mt_1().text_sm().child(format!("• {warning}"))),
                    ),
            );
        if let Some(message) = status_message {
            content = content.child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().muted.opacity(0.5))
                    .child(message),
            );
        }
        content = content
            .child(self.render_error_summary(cx))
            .children(self.render_editor_sections(cx));

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_8()
                    .py_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui_component::button::Button::new("1701_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form1701Event::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("1701_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("1701_manual")
                                    .label("Manual / External Filing")
                                    .primary()
                                    .disabled(true),
                            ),
                    ),
            )
            .child(
                div()
                    .p_6()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().accent)
                    .child(self.render_header(cx))
                    .child(div().mt_6().child(self.render_status_pipeline(cx))),
            )
            .child(
                div()
                    .id("1701_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(content),
            )
    }
}

fn choice_group_taxpayer(view: &Form1701View, cx: &Context<Form1701View>) -> AnyElement {
    let rates = div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(Form1701TaxRate::ALL.into_iter().map(|value| {
            view.render_choice(
                format!("1701_rate_{value:?}"),
                value.label(),
                view.draft.tax_rate == Some(value),
                cx,
                move |this| {
                    this.draft.tax_rate = Some(value);
                    if value == Form1701TaxRate::EightPercent {
                        this.draft.deduction_method = None;
                    }
                },
            )
        }));
    let deductions =
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(Form1701DeductionMethod::ALL.into_iter().map(|value| {
                view.render_choice(
                    format!("1701_deduction_{value:?}"),
                    value.label(),
                    view.draft.deduction_method == Some(value),
                    cx,
                    move |this| this.draft.deduction_method = Some(value),
                )
            }));
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(rates)
        .child(deductions)
        .child(boolean_choices(
            view,
            cx,
            "1701_ftc",
            "13 Foreign tax credits",
            view.draft.claims_foreign_tax_credits,
            |this, value| this.draft.claims_foreign_tax_credits = Some(value),
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701CivilStatus::ALL.into_iter().map(|value| {
                    view.render_choice(
                        format!("1701_civil_{value:?}"),
                        format!("16 {}", value.label()),
                        view.draft.civil_status == Some(value),
                        cx,
                        move |this| this.draft.civil_status = Some(value),
                    )
                })),
        )
        .child(boolean_choices(
            view,
            cx,
            "1701_spouse_income",
            "17 Spouse has income",
            view.draft.spouse_has_income,
            |this, value| this.draft.spouse_has_income = Some(value),
        ))
        .child(
            div()
                .flex()
                .gap_2()
                .child(view.render_choice(
                    "1701_joint",
                    "18 Joint filing",
                    view.draft.joint_filing_status == Some(Form1701JointFilingStatus::Joint),
                    cx,
                    |this| this.draft.joint_filing_status = Some(Form1701JointFilingStatus::Joint),
                ))
                .child(view.render_choice(
                    "1701_separate",
                    "18 Separate filing",
                    view.draft.joint_filing_status == Some(Form1701JointFilingStatus::Separate),
                    cx,
                    |this| {
                        this.draft.joint_filing_status = Some(Form1701JointFilingStatus::Separate)
                    },
                )),
        )
        .child(boolean_choices(
            view,
            cx,
            "1701_exempt",
            "19 Exempt income",
            view.draft.has_exempt_income,
            |this, value| this.draft.has_exempt_income = Some(value),
        ))
        .child(boolean_choices(
            view,
            cx,
            "1701_special",
            "20 Special-rate income",
            view.draft.has_special_rate_income,
            |this, value| this.draft.has_special_rate_income = Some(value),
        ))
        .into_any_element()
}

fn choice_group_spouse(view: &Form1701View, cx: &Context<Form1701View>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701TaxRate::ALL.into_iter().map(|value| {
                    view.render_choice(
                        format!("1701_spouse_rate_{value:?}"),
                        value.label(),
                        view.draft.spouse.tax_rate == Some(value),
                        cx,
                        move |this| {
                            this.draft.spouse.tax_rate = Some(value);
                            if value == Form1701TaxRate::EightPercent {
                                this.draft.spouse.deduction_method = None;
                            }
                        },
                    )
                })),
        )
        .child(div().flex().flex_wrap().gap_2().children(
            Form1701DeductionMethod::ALL.into_iter().map(|value| {
                view.render_choice(
                    format!("1701_spouse_deduction_{value:?}"),
                    value.label(),
                    view.draft.spouse.deduction_method == Some(value),
                    cx,
                    move |this| this.draft.spouse.deduction_method = Some(value),
                )
            }),
        ))
        .child(boolean_choices(
            view,
            cx,
            "1701_spouse_ftc",
            "Foreign tax credits",
            view.draft.spouse.claims_foreign_tax_credits,
            |this, value| this.draft.spouse.claims_foreign_tax_credits = Some(value),
        ))
        .child(boolean_choices(
            view,
            cx,
            "1701_spouse_exempt",
            "Exempt income",
            view.draft.spouse.has_exempt_income,
            |this, value| this.draft.spouse.has_exempt_income = Some(value),
        ))
        .child(boolean_choices(
            view,
            cx,
            "1701_spouse_special",
            "Special-rate income",
            view.draft.spouse.has_special_rate_income,
            |this, value| this.draft.spouse.has_special_rate_income = Some(value),
        ))
        .into_any_element()
}

fn boolean_choices(
    view: &Form1701View,
    cx: &Context<Form1701View>,
    id: &'static str,
    label: &'static str,
    selected: Option<bool>,
    set: impl Fn(&mut Form1701View, bool) + Copy + 'static,
) -> AnyElement {
    div()
        .flex()
        .gap_2()
        .child(view.render_choice(
            format!("{id}_yes"),
            format!("{label}: Yes"),
            selected == Some(true),
            cx,
            move |this| set(this, true),
        ))
        .child(view.render_choice(
            format!("{id}_no"),
            format!("{label}: No"),
            selected == Some(false),
            cx,
            move |this| set(this, false),
        ))
        .into_any_element()
}

fn section_card(cx: &Context<Form1701View>, title: &str) -> Div {
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
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child(title.to_string()),
        )
}

fn amount_header(cx: &Context<Form1701View>) -> AnyElement {
    div()
        .grid()
        .grid_cols(3)
        .gap_3()
        .p_2()
        .bg(cx.theme().muted.opacity(0.5))
        .font_weight(FontWeight::BOLD)
        .child("Particulars")
        .child("A) Taxpayer/Filer")
        .child("B) Spouse")
        .into_any_element()
}

fn computed_amount(value: Option<f64>, cx: &Context<Form1701View>) -> AnyElement {
    div()
        .p_2()
        .rounded_md()
        .bg(cx.theme().muted.opacity(0.5))
        .child(format_optional_amount(value))
        .into_any_element()
}

fn format_optional_amount(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_string(), |amount| format!("₱ {amount:.0}"))
}

fn computed_rows(section: Form1701AmountSection) -> &'static [(u8, &'static str)] {
    match section {
        Form1701AmountSection::PartIi => &[
            (22, "Total tax due"),
            (23, "Tax credits/payments"),
            (24, "Tax payable/(overpayment)"),
            (26, "Tax payable after installment"),
            (30, "Total penalties"),
            (31, "Total amount payable/(overpayment)"),
        ],
        Form1701AmountSection::Schedule2 => &[
            (4, "Gross compensation income"),
            (6, "Taxable compensation income"),
            (7, "Tax due"),
        ],
        Form1701AmountSection::Schedule3 => &[
            (10, "Net sales/revenues/receipts/fees"),
            (12, "Gross income from operation"),
            (16, "Total itemized deductions"),
            (17, "Optional standard deduction"),
            (18, "Net income/(loss)"),
            (22, "Total other taxable income"),
            (23, "Taxable business income"),
            (24, "Aggregate taxable income"),
            (25, "Tax due under graduated rates"),
            (28, "Total 8% gross sales and other income"),
            (29, "Less: P250,000 reduction"),
            (30, "Taxable income under 8%"),
            (31, "Tax due at 8%"),
            (32, "Total income tax due"),
        ],
        Form1701AmountSection::Schedule4 => &[(18, "Total ordinary allowable itemized deductions")],
        Form1701AmountSection::Schedule6 => &[(3, "Net operating loss carry-over")],
        Form1701AmountSection::PartVi => &[
            (1, "Regular income tax due"),
            (4, "Net special-rate tax"),
            (5, "Total income tax due"),
        ],
        Form1701AmountSection::PartVii => &[
            (5, "Tax withheld on compensation"),
            (10, "Total tax credits/payments"),
        ],
        Form1701AmountSection::PartViii => &[
            (3, "Total tax due"),
            (5, "Tax payable after foreign credits"),
            (7, "Tax payable after prior payments"),
            (10, "Total relief/reduction"),
        ],
        Form1701AmountSection::PartIx => &[
            (5, "Total additions"),
            (10, "Total deductions"),
            (11, "Taxable income per return"),
        ],
    }
}

fn overpayment_label(value: Form1701OverpaymentDisposition) -> &'static str {
    match value {
        Form1701OverpaymentDisposition::None => "No disposition",
        Form1701OverpaymentDisposition::Refund => "Refund",
        Form1701OverpaymentDisposition::TaxCreditCertificate => "Tax Credit Certificate",
        Form1701OverpaymentDisposition::CarryOver => "Carry over",
    }
}

fn text_input(
    cx: &mut Context<Form1701View>,
    value: &str,
    placeholder: &str,
    window: &mut Window,
) -> Entity<InputState> {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.to_string()));
    if !value.is_empty() {
        input.update(cx, |state, cx| {
            state.set_value(value.to_string(), window, cx)
        });
    }
    input
}

fn optional_u8_input(
    cx: &mut Context<Form1701View>,
    value: Option<u8>,
    placeholder: &str,
    window: &mut Window,
) -> Entity<InputState> {
    text_input(
        cx,
        &value.map(|value| value.to_string()).unwrap_or_default(),
        placeholder,
        window,
    )
}

fn optional_amount_input(
    cx: &mut Context<Form1701View>,
    value: Option<f64>,
    window: &mut Window,
) -> Entity<InputState> {
    text_input(
        cx,
        &value.map(|value| format!("{value:.0}")).unwrap_or_default(),
        "Whole pesos; blank is empty",
        window,
    )
}

fn employer_input(
    cx: &mut Context<Form1701View>,
    row: &Form1701EmployerRow,
    window: &mut Window,
) -> EmployerInputs {
    EmployerInputs {
        name: text_input(cx, &row.employer_name, "Employer name", window),
        tin: text_input(cx, &row.employer_tin, "Employer TIN", window),
        compensation: optional_amount_input(cx, row.compensation_income, window),
        withheld: optional_amount_input(cx, row.tax_withheld, window),
    }
}

fn payment_input(
    cx: &mut Context<Form1701View>,
    row: &Form1701PaymentRow,
    window: &mut Window,
) -> PaymentRowInputs {
    PaymentRowInputs {
        agency: text_input(cx, &row.drawee_bank_or_agency, "Drawee bank/agency", window),
        number: text_input(cx, &row.number, "Number", window),
        date: text_input(cx, &row.date, "MM/DD/YYYY", window),
        amount: optional_amount_input(cx, row.amount, window),
    }
}

fn input_text(input: &Entity<InputState>, cx: &Context<Form1701View>) -> String {
    input.read(cx).value().to_string()
}

fn parse_optional_u8(raw: &str, max: u8) -> Result<Option<u8>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let value = raw
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("{raw:?} is not a whole number"))?;
    if value > max {
        return Err(format!("Value must be between 0 and {max}"));
    }
    Ok(Some(value))
}

fn parse_optional_whole_peso(raw: &str, allow_negative: bool) -> Result<Option<f64>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let value = raw
        .trim()
        .replace(',', "")
        .parse::<f64>()
        .map_err(|_| format!("{raw:?} is not a numeric amount"))?;
    if !value.is_finite() {
        return Err("Amount must be finite".to_string());
    }
    if !allow_negative && value < 0.0 {
        return Err("This line cannot contain a negative amount".to_string());
    }
    if (value - value.round()).abs() >= 0.001 {
        return Err("Form 1701 accepts whole pesos only; do not enter centavos".to_string());
    }
    Ok(Some(value.round()))
}

fn assign_required_month(
    target: &mut u8,
    input: &Entity<InputState>,
    cx: &Context<Form1701View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match raw.trim().parse::<u8>() {
        Ok(value) if (1..=12).contains(&value) => *target = value,
        _ => errors.push((
            "period_end_month".to_string(),
            "Enter a month from 1 to 12".to_string(),
        )),
    }
}

fn assign_optional_u8(
    target: &mut Option<u8>,
    input: &Entity<InputState>,
    field: &str,
    max: u8,
    cx: &Context<Form1701View>,
    errors: &mut Vec<(String, String)>,
) {
    match parse_optional_u8(&input_text(input, cx), max) {
        Ok(value) => *target = value,
        Err(message) => errors.push((field.to_string(), message)),
    }
}

fn assign_optional_amount(
    target: &mut Option<f64>,
    input: &Entity<InputState>,
    field: &str,
    allow_negative: bool,
    cx: &Context<Form1701View>,
    errors: &mut Vec<(String, String)>,
) {
    match parse_optional_whole_peso(&input_text(input, cx), allow_negative) {
        Ok(value) => *target = value,
        Err(message) => errors.push((field.to_string(), message)),
    }
}

fn assign_amount(
    draft: &mut Form1701Draft,
    section: Form1701AmountSection,
    item: u8,
    party: Form1701Party,
    input: &Entity<InputState>,
    cx: &Context<Form1701View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match parse_optional_whole_peso(&raw, true) {
        Ok(value) => draft.set_amount(section, item, party, value),
        Err(message) => errors.push((format!("{section:?}_{item}_{party:?}"), message)),
    }
}

fn sync_payment_row(
    target: &mut Form1701PaymentRow,
    inputs: &PaymentRowInputs,
    field: &str,
    cx: &Context<Form1701View>,
    errors: &mut Vec<(String, String)>,
) {
    target.drawee_bank_or_agency = input_text(&inputs.agency, cx);
    target.number = input_text(&inputs.number, cx);
    target.date = input_text(&inputs.date, cx);
    assign_optional_amount(
        &mut target.amount,
        &inputs.amount,
        &format!("{field}_amount"),
        false,
        cx,
        errors,
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_optional_u8, parse_optional_whole_peso};

    #[test]
    fn amount_parser_preserves_blank_as_none() {
        assert_eq!(parse_optional_whole_peso("", true), Ok(None));
    }

    #[test]
    fn amount_parser_rejects_invalid_text_instead_of_zeroing_it() {
        assert!(parse_optional_whole_peso("not-a-number", true).is_err());
    }

    #[test]
    fn amount_parser_enforces_whole_pesos() {
        assert!(parse_optional_whole_peso("125.50", true).is_err());
        assert_eq!(parse_optional_whole_peso("-1250", true), Ok(Some(-1_250.0)));
    }

    #[test]
    fn attachment_count_is_blank_or_two_digits() {
        assert_eq!(parse_optional_u8("", 99), Ok(None));
        assert_eq!(parse_optional_u8("12", 99), Ok(Some(12)));
        assert!(parse_optional_u8("100", 99).is_err());
    }
}
