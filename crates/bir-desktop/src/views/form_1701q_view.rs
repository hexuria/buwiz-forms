//! Evidence-safe editor for exact form `1701Qv2018`.
//!
//! The editor persists semantic local drafts and previews the owned HTML form.
//! The exact-revision editable XML contract round-trips locally. Queueing and
//! submission stay disabled until the reviewed encrypt/transport helpers,
//! credential handling, and endpoint acceptance semantics are certified.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_1701q::{
    Form1701QAtc, Form1701QDeductionMethod, Form1701QDraft, Form1701QFilerType, Form1701QParty,
    Form1701QPaymentRow, Form1701QSpouseType, Form1701QTaxRate, USER_ENTERED_AMOUNT_ITEMS,
};
use bir_core::forms::{FilingStatus, FormValidator};
use bir_print::html::RenderEnvelopeV1;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;
use crate::components::form_parts::readonly_field;

pub enum Form1701QEvent {
    BackToDashboard,
    Saved,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1701QEvent> for Form1701QView {}

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

pub struct Form1701QView {
    pub draft: Form1701QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    number_of_sheets: Entity<InputState>,
    taxpayer_last_name: Entity<InputState>,
    registered_address_2: Entity<InputState>,
    date_of_birth: Entity<InputState>,
    citizenship: Entity<InputState>,
    foreign_tax_number: Entity<InputState>,

    spouse_tin: Entity<InputState>,
    spouse_rdo_code: Entity<InputState>,
    spouse_name: Entity<InputState>,
    spouse_citizenship: Entity<InputState>,
    spouse_foreign_tax_number: Entity<InputState>,

    item_43_description: Entity<InputState>,
    item_48_description: Entity<InputState>,
    item_61_description: Entity<InputState>,
    amount_inputs: BTreeMap<u8, PairedAmountInputs>,

    payment_32: PaymentRowInputs,
    payment_33: PaymentRowInputs,
    payment_34: PaymentRowInputs,
    payment_35: PaymentRowInputs,
    payment_35_description: Entity<InputState>,
    machine_validation: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form1701QView {
    pub fn new(
        mut draft: Form1701QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        draft.recompute();
        let number_of_sheets = text_input(cx, &draft.number_of_sheets.to_string(), "0-99", window);
        let taxpayer_last_name = text_input(
            cx,
            &draft.taxpayer_last_name,
            "Last name exactly as printed on page 2",
            window,
        );
        let registered_address_2 = text_input(
            cx,
            &draft.registered_address_2,
            "Address continuation (optional)",
            window,
        );
        let date_of_birth = text_input(cx, &draft.date_of_birth, "MM/DD/YYYY", window);
        let citizenship = text_input(cx, &draft.citizenship, "Citizenship", window);
        let foreign_tax_number = text_input(
            cx,
            &draft.foreign_tax_number,
            "Foreign tax number (optional)",
            window,
        );

        let spouse_tin = text_input(cx, &draft.spouse_tin, "Spouse TIN", window);
        let spouse_rdo_code = text_input(cx, &draft.spouse_rdo_code, "RDO", window);
        let spouse_name = text_input(cx, &draft.spouse_name, "Spouse name", window);
        let spouse_citizenship =
            text_input(cx, &draft.spouse_citizenship, "Spouse citizenship", window);
        let spouse_foreign_tax_number = text_input(
            cx,
            &draft.spouse_foreign_tax_number,
            "Spouse foreign tax number",
            window,
        );

        let item_43_description = text_input(
            cx,
            &draft.item_43_non_operating_income_description,
            "Item 43 description",
            window,
        );
        let item_48_description = text_input(
            cx,
            &draft.item_48_non_operating_income_description,
            "Item 48 description",
            window,
        );
        let item_61_description = text_input(
            cx,
            &draft.item_61_other_tax_credit_description,
            "Item 61 description",
            window,
        );

        let amount_inputs = USER_ENTERED_AMOUNT_ITEMS
            .iter()
            .map(|item| {
                (
                    *item,
                    PairedAmountInputs {
                        taxpayer: optional_amount_input(
                            cx,
                            draft.amount(*item, Form1701QParty::Taxpayer),
                            window,
                        ),
                        spouse: optional_amount_input(
                            cx,
                            draft.amount(*item, Form1701QParty::Spouse),
                            window,
                        ),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let payment_32 = payment_inputs(
            cx,
            &draft.payment_details.item_32_cash_or_bank_debit_memo,
            window,
        );
        let payment_33 = payment_inputs(cx, &draft.payment_details.item_33_check, window);
        let payment_34 = payment_inputs(cx, &draft.payment_details.item_34_tax_debit_memo, window);
        let payment_35 = payment_inputs(cx, &draft.payment_details.item_35_others, window);
        let payment_35_description = text_input(
            cx,
            &draft.payment_details.item_35_others_description,
            "Specify other payment",
            window,
        );
        let machine_validation = text_input(
            cx,
            &draft.payment_details.machine_validation_or_receipt_details,
            "Machine validation / receipt details",
            window,
        );

        let mut all_inputs = vec![
            number_of_sheets.clone(),
            taxpayer_last_name.clone(),
            registered_address_2.clone(),
            date_of_birth.clone(),
            citizenship.clone(),
            foreign_tax_number.clone(),
            spouse_tin.clone(),
            spouse_rdo_code.clone(),
            spouse_name.clone(),
            spouse_citizenship.clone(),
            spouse_foreign_tax_number.clone(),
            item_43_description.clone(),
            item_48_description.clone(),
            item_61_description.clone(),
            payment_35_description.clone(),
            machine_validation.clone(),
        ];
        for pair in amount_inputs.values() {
            all_inputs.extend(pair.all());
        }
        for row in [&payment_32, &payment_33, &payment_34, &payment_35] {
            all_inputs.extend(row.all());
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
            number_of_sheets,
            taxpayer_last_name,
            registered_address_2,
            date_of_birth,
            citizenship,
            foreign_tax_number,
            spouse_tin,
            spouse_rdo_code,
            spouse_name,
            spouse_citizenship,
            spouse_foreign_tax_number,
            item_43_description,
            item_48_description,
            item_61_description,
            amount_inputs,
            payment_32,
            payment_33,
            payment_34,
            payment_35,
            payment_35_description,
            machine_validation,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.input_errors.clear();

        let raw_sheets = input_text(&self.number_of_sheets, cx);
        match parse_sheet_count(&raw_sheets) {
            Ok(value) => self.draft.number_of_sheets = value,
            Err(message) => self
                .input_errors
                .push(("number_of_sheets".to_string(), message)),
        }
        self.draft.taxpayer_last_name = input_text(&self.taxpayer_last_name, cx);
        self.draft.registered_address_2 = input_text(&self.registered_address_2, cx);
        self.draft.date_of_birth = input_text(&self.date_of_birth, cx);
        self.draft.citizenship = input_text(&self.citizenship, cx);
        self.draft.foreign_tax_number = input_text(&self.foreign_tax_number, cx);

        self.draft.spouse_tin = input_text(&self.spouse_tin, cx);
        self.draft.spouse_rdo_code = input_text(&self.spouse_rdo_code, cx);
        self.draft.spouse_name = input_text(&self.spouse_name, cx);
        self.draft.spouse_citizenship = input_text(&self.spouse_citizenship, cx);
        self.draft.spouse_foreign_tax_number = input_text(&self.spouse_foreign_tax_number, cx);

        self.draft.item_43_non_operating_income_description =
            input_text(&self.item_43_description, cx);
        self.draft.item_48_non_operating_income_description =
            input_text(&self.item_48_description, cx);
        self.draft.item_61_other_tax_credit_description = input_text(&self.item_61_description, cx);

        for (item, pair) in &self.amount_inputs {
            let allow_negative = matches!(*item, 42 | 50);
            assign_amount(
                &mut self.draft,
                *item,
                Form1701QParty::Taxpayer,
                &pair.taxpayer,
                allow_negative,
                cx,
                &mut self.input_errors,
            );
            assign_amount(
                &mut self.draft,
                *item,
                Form1701QParty::Spouse,
                &pair.spouse,
                allow_negative,
                cx,
                &mut self.input_errors,
            );
        }

        sync_payment_row(
            &mut self.draft.payment_details.item_32_cash_or_bank_debit_memo,
            &self.payment_32,
            "payment_32_cash_or_bank_debit_memo",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_33_check,
            &self.payment_33,
            "payment_33_check",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_34_tax_debit_memo,
            &self.payment_34,
            "payment_34_tax_debit_memo",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.item_35_others,
            &self.payment_35,
            "payment_35_others",
            cx,
            &mut self.input_errors,
        );
        self.draft.payment_details.item_35_others_description =
            input_text(&self.payment_35_description, cx);
        self.draft
            .payment_details
            .machine_validation_or_receipt_details = input_text(&self.machine_validation, cx);

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
            .when(self.draft.is_editable(), |element| element.cursor_pointer())
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

    fn render_amount_row(&self, item: u8, label: &str, cx: &Context<Self>) -> AnyElement {
        let values = self.amount_inputs.get(&item);
        let mut row = div()
            .grid()
            .grid_cols(3)
            .gap_3()
            .items_center()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().text_sm().child(format!("{item} {label}")));
        if let Some(inputs) = values {
            row = row
                .child(Input::new(&inputs.taxpayer).disabled(!self.draft.is_editable()))
                .child(Input::new(&inputs.spouse).disabled(!self.draft.is_editable()));
        } else {
            row = row
                .child(computed_amount(
                    self.draft.amount(item, Form1701QParty::Taxpayer),
                    cx,
                ))
                .child(computed_amount(
                    self.draft.amount(item, Form1701QParty::Spouse),
                    cx,
                ));
        }
        row.into_any_element()
    }

    fn render_choice_sections(&self, cx: &Context<Self>) -> AnyElement {
        let taxpayer_types =
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701QFilerType::ALL.into_iter().map(|value| {
                    self.render_choice(
                        format!("1701q_filer_{value:?}"),
                        format!("7 {}", value.label()),
                        self.draft.filer_type == Some(value),
                        cx,
                        move |this| this.draft.filer_type = Some(value),
                    )
                }));
        let taxpayer_atcs = div().flex().flex_wrap().gap_2().children(
            Form1701QAtc::TAXPAYER_CHOICES.into_iter().map(|value| {
                self.render_choice(
                    format!("1701q_atc_{}", value.code()),
                    format!("8 {} · {}", value.code(), value.label()),
                    self.draft.atc == Some(value),
                    cx,
                    move |this| this.draft.atc = Some(value),
                )
            }),
        );
        let taxpayer_rates =
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701QTaxRate::ALL.into_iter().map(|value| {
                    self.render_choice(
                        format!("1701q_rate_{value:?}"),
                        format!("16 {}", value.label()),
                        self.draft.tax_rate == Some(value),
                        cx,
                        move |this| {
                            this.draft.tax_rate = Some(value);
                            if value == Form1701QTaxRate::EightPercent {
                                this.draft.deduction_method = None;
                            }
                        },
                    )
                }));
        let taxpayer_deductions = div().flex().flex_wrap().gap_2().children(
            Form1701QDeductionMethod::ALL.into_iter().map(|value| {
                self.render_choice(
                    format!("1701q_deduction_{value:?}"),
                    format!("16A {}", value.label()),
                    self.draft.deduction_method == Some(value),
                    cx,
                    move |this| this.draft.deduction_method = Some(value),
                )
            }),
        );

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(taxpayer_types)
            .child(taxpayer_atcs)
            .child(taxpayer_rates)
            .child(taxpayer_deductions)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(self.render_choice(
                        "1701q_ftc_yes",
                        "15 Foreign Tax Credits: Yes",
                        self.draft.claims_foreign_tax_credits == Some(true),
                        cx,
                        |this| this.draft.claims_foreign_tax_credits = Some(true),
                    ))
                    .child(self.render_choice(
                        "1701q_ftc_no",
                        "15 Foreign Tax Credits: No",
                        self.draft.claims_foreign_tax_credits == Some(false),
                        cx,
                        |this| this.draft.claims_foreign_tax_credits = Some(false),
                    )),
            )
            .into_any_element()
    }

    fn render_spouse_choices(&self, cx: &Context<Self>) -> AnyElement {
        let spouse_types =
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701QSpouseType::ALL.into_iter().map(|value| {
                    self.render_choice(
                        format!("1701q_spouse_type_{value:?}"),
                        format!("19 {}", value.label()),
                        self.draft.spouse_type == Some(value),
                        cx,
                        move |this| this.draft.spouse_type = Some(value),
                    )
                }));
        let spouse_atcs = div().flex().flex_wrap().gap_2().children(
            Form1701QAtc::SPOUSE_CHOICES.into_iter().map(|value| {
                self.render_choice(
                    format!("1701q_spouse_atc_{}", value.code()),
                    format!("20 {} · {}", value.code(), value.label()),
                    self.draft.spouse_atc == Some(value),
                    cx,
                    move |this| {
                        this.draft.spouse_atc = Some(value);
                        if value == Form1701QAtc::Ii011 {
                            this.draft.spouse_tax_rate = None;
                            this.draft.spouse_deduction_method = None;
                        }
                    },
                )
            }),
        );
        let spouse_rates =
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(Form1701QTaxRate::ALL.into_iter().map(|value| {
                    self.render_choice(
                        format!("1701q_spouse_rate_{value:?}"),
                        format!("25 {}", value.label()),
                        self.draft.spouse_tax_rate == Some(value),
                        cx,
                        move |this| {
                            this.draft.spouse_tax_rate = Some(value);
                            if value == Form1701QTaxRate::EightPercent {
                                this.draft.spouse_deduction_method = None;
                            }
                        },
                    )
                }));
        let spouse_deductions = div().flex().flex_wrap().gap_2().children(
            Form1701QDeductionMethod::ALL.into_iter().map(|value| {
                self.render_choice(
                    format!("1701q_spouse_deduction_{value:?}"),
                    format!("25A {}", value.label()),
                    self.draft.spouse_deduction_method == Some(value),
                    cx,
                    move |this| this.draft.spouse_deduction_method = Some(value),
                )
            }),
        );

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().gap_2().child(self.render_choice(
                "1701q_has_spouse",
                if self.draft.has_spouse {
                    "Spouse section enabled"
                } else {
                    "Enable spouse section"
                },
                self.draft.has_spouse,
                cx,
                |this| this.draft.has_spouse = !this.draft.has_spouse,
            )))
            .when(self.draft.has_spouse, |element| {
                element
                    .child(spouse_types)
                    .child(spouse_atcs)
                    .child(spouse_rates)
                    .child(spouse_deductions)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_choice(
                                "1701q_spouse_ftc_yes",
                                "24 Foreign Tax Credits: Yes",
                                self.draft.spouse_claims_foreign_tax_credits == Some(true),
                                cx,
                                |this| this.draft.spouse_claims_foreign_tax_credits = Some(true),
                            ))
                            .child(self.render_choice(
                                "1701q_spouse_ftc_no",
                                "24 Foreign Tax Credits: No",
                                self.draft.spouse_claims_foreign_tax_credits == Some(false),
                                cx,
                                |this| this.draft.spouse_claims_foreign_tax_credits = Some(false),
                            )),
                    )
            })
            .into_any_element()
    }

    fn render_payment_row(
        &self,
        item: u8,
        label: &str,
        row: &PaymentRowInputs,
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
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{item} {label}")),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(4)
                    .gap_2()
                    .child(Input::new(&row.agency).disabled(!self.draft.is_editable()))
                    .child(Input::new(&row.number).disabled(!self.draft.is_editable()))
                    .child(Input::new(&row.date).disabled(!self.draft.is_editable()))
                    .child(Input::new(&row.amount).disabled(!self.draft.is_editable())),
            )
            .into_any_element()
    }

    fn render_filing_section(&self, cx: &Context<Self>) -> AnyElement {
        section_card(cx, "ITEMS 1-4 — FILING PERIOD")
            .child(div().text_sm().child(format!(
                "1 Taxable Year: {} · 2 Quarter: Q{} · 3 Amended: {}",
                self.draft.taxable_year,
                self.draft.quarter,
                if self.draft.is_amended { "Yes" } else { "No" }
            )))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_choice(
                        "1701q_amended_yes",
                        "3 Amended Return: Yes",
                        self.draft.is_amended,
                        cx,
                        |this| this.draft.is_amended = true,
                    ))
                    .child(self.render_choice(
                        "1701q_amended_no",
                        "3 Amended Return: No",
                        !self.draft.is_amended,
                        cx,
                        |this| this.draft.is_amended = false,
                    )),
            )
            .child(self.render_input_row("4 Number of Sheets Attached", &self.number_of_sheets))
            .into_any_element()
    }

    fn render_taxpayer_section(&self, cx: &Context<Self>) -> AnyElement {
        section_card(cx, "PART I — ITEMS 5-16A")
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_4()
                    .child(readonly_field(
                        "5 Taxpayer Identification Number (TIN)",
                        &self.draft.tin,
                        None,
                        cx,
                    ))
                    .child(readonly_field("6 RDO Code", &self.draft.rdo_code, None, cx)),
            )
            .child(readonly_field(
                "9 Taxpayer/Filer's Name",
                &self.draft.taxpayer_name,
                None,
                cx,
            ))
            .child(self.render_input_row(
                "Page 2 Taxpayer/Filer's Last Name",
                &self.taxpayer_last_name,
            ))
            .child(readonly_field(
                "10 Registered Address",
                &self.draft.registered_address,
                None,
                cx,
            ))
            .child(self.render_input_row(
                "10 Registered Address continuation",
                &self.registered_address_2,
            ))
            .child(readonly_field(
                "10A ZIP Code",
                &self.draft.zip_code,
                None,
                cx,
            ))
            .child(self.render_input_row("11 Date of Birth", &self.date_of_birth))
            .child(readonly_field(
                "12 Email Address",
                &self.draft.email,
                None,
                cx,
            ))
            .child(self.render_input_row("13 Citizenship", &self.citizenship))
            .child(self.render_input_row("14 Foreign Tax Number", &self.foreign_tax_number))
            .child(self.render_choice_sections(cx))
            .into_any_element()
    }

    fn render_spouse_section(&self, cx: &Context<Self>) -> AnyElement {
        section_card(cx, "PART II — ITEMS 17-25A (SPOUSE)")
            .child(self.render_spouse_choices(cx))
            .when(self.draft.has_spouse, |element| {
                element
                    .child(self.render_input_row("17 Spouse TIN", &self.spouse_tin))
                    .child(self.render_input_row("18 Spouse RDO Code", &self.spouse_rdo_code))
                    .child(self.render_input_row("21 Spouse Name", &self.spouse_name))
                    .child(self.render_input_row("22 Spouse Citizenship", &self.spouse_citizenship))
                    .child(self.render_input_row(
                        "23 Spouse Foreign Tax Number",
                        &self.spouse_foreign_tax_number,
                    ))
            })
            .into_any_element()
    }

    fn render_amount_section(
        &self,
        title: &str,
        rows: &[(u8, &str)],
        description: Option<(&str, &Entity<InputState>)>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut card = section_card(cx, title).child(amount_header(cx));
        for (item, label) in rows {
            card = card.child(self.render_amount_row(*item, label, cx));
        }
        if let Some((label, input)) = description {
            card = card.child(self.render_input_row(label, input));
        }
        card.into_any_element()
    }

    fn render_part_iii_section(&self, cx: &Context<Self>) -> AnyElement {
        let rows = [
            (26, "Tax Due"),
            (27, "Less: Tax Credits/Payments"),
            (28, "Tax Payable/(Overpayment)"),
            (29, "Add: Total Penalties"),
            (30, "Total Amount Payable/(Overpayment)"),
        ];
        let mut card = section_card(cx, "PART III — TOTAL TAX PAYABLE").child(amount_header(cx));
        for (item, label) in rows {
            card = card.child(self.render_amount_row(item, label, cx));
        }
        card.child(div().p_3().font_weight(FontWeight::BOLD).child(format!(
            "31 Aggregate Amount Payable/(Overpayment): {}",
            format_optional_amount(self.draft.item_31_aggregate_amount_payable)
        )))
        .into_any_element()
    }

    fn render_payment_section(&self, cx: &Context<Self>) -> AnyElement {
        section_card(cx, "PART IV — DETAILS OF PAYMENT")
            .child(
                div()
                    .text_xs()
                    .child("Columns: Drawee Bank/Agency · Number · Date (MM/DD/YYYY) · Amount"),
            )
            .child(self.render_payment_row(32, "Cash/Bank Debit Memo", &self.payment_32, cx))
            .child(self.render_payment_row(33, "Check", &self.payment_33, cx))
            .child(self.render_payment_row(34, "Tax Debit Memo", &self.payment_34, cx))
            .child(self.render_payment_row(35, "Others", &self.payment_35, cx))
            .child(self.render_input_row("35 Others description", &self.payment_35_description))
            .child(self.render_input_row(
                "Machine validation / receipt details",
                &self.machine_validation,
            ))
            .into_any_element()
    }

    fn render_editor_sections(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        const GRADUATED_ROWS: &[(u8, &str)] = &[
            (36, "Sales/Revenues/Receipts/Fees"),
            (37, "Less: Cost of Sales/Services"),
            (38, "Gross Income/(Loss) from Operation"),
            (39, "Total Allowable Itemized Deductions"),
            (40, "Optional Standard Deduction (40% of Item 36)"),
            (41, "Net Income/(Loss) This Quarter"),
            (42, "Taxable Income/(Loss) Previous Quarter/s"),
            (43, "Non-Operating Income"),
            (44, "GPP Share in Income"),
            (45, "Total Taxable Income/(Loss) To Date"),
            (46, "TAX DUE"),
        ];
        const EIGHT_PERCENT_ROWS: &[(u8, &str)] = &[
            (47, "Sales/Revenues/Receipts/Fees"),
            (48, "Add: Non-Operating Income"),
            (49, "Total Income for the Quarter"),
            (50, "Taxable Income/(Loss) Previous Quarter"),
            (51, "Cumulative Taxable Income/(Loss)"),
            (52, "Less: Allowable P250,000 Reduction"),
            (53, "Taxable Income/(Loss) To Date"),
            (54, "TAX DUE at 8%"),
        ];
        const CREDIT_ROWS: &[(u8, &str)] = &[
            (55, "Prior Year's Excess Credits"),
            (56, "Tax Payments for Previous Quarters"),
            (57, "Creditable Tax Withheld for Previous Quarters"),
            (58, "Creditable Tax Withheld per BIR Form 2307"),
            (59, "Tax Paid in Previously Filed Amended Return"),
            (60, "Foreign Tax Credits"),
            (61, "Other Tax Credits/Payments"),
            (62, "Total Tax Credits/Payments"),
        ];
        const FINAL_ROWS: &[(u8, &str)] = &[
            (63, "Tax Payable/(Overpayment)"),
            (64, "Surcharge"),
            (65, "Interest"),
            (66, "Compromise"),
            (67, "Total Penalties"),
            (68, "Total Amount Payable/(Overpayment)"),
        ];

        vec![
            self.render_filing_section(cx),
            self.render_taxpayer_section(cx),
            self.render_spouse_section(cx),
            self.render_part_iii_section(cx),
            self.render_amount_section(
                "PART V — SCHEDULE I: GRADUATED IT RATE",
                GRADUATED_ROWS,
                Some(("43 Description", &self.item_43_description)),
                cx,
            ),
            self.render_amount_section(
                "PART V — SCHEDULE II: 8% IT RATE",
                EIGHT_PERCENT_ROWS,
                Some(("48 Description", &self.item_48_description)),
                cx,
            ),
            self.render_amount_section(
                "PART V — SCHEDULE III: TAX CREDITS/PAYMENTS",
                CREDIT_ROWS,
                Some(("61 Description", &self.item_61_description)),
                cx,
            ),
            self.render_amount_section("PART V — ITEMS 63-68", FINAL_ROWS, None, cx),
            self.render_payment_section(cx),
        ]
    }
}

impl FormViewTrait for Form1701QView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701Q"
    }

    fn form_subtitle(&self) -> &'static str {
        "Quarterly Income Tax Return for Individuals, Estates and Trusts"
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
                "Draft was not saved because one or more fields contain invalid text.".to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix the invalid 1701Q input text before saving.",
            );
            return;
        }

        self.draft.updated_at = Some(chrono::Utc::now().to_rfc3339());
        let save_result = self
            .db
            .lock()
            .map_err(|_| "Draft database lock is unavailable".to_string())
            .and_then(|db| {
                db.save_form_draft(
                    &self.draft.tin,
                    "1701Q",
                    self.draft.taxable_year,
                    Some(self.draft.quarter),
                    &self.draft.status,
                    &self.draft,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            });

        match save_result {
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
                    "1701Q draft saved locally.",
                );
                cx.emit(Form1701QEvent::Saved);
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save draft: {error}"));
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    format!("Could not save 1701Q draft: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "1701Qv2018 filing is manual/external until exact XML and submission contracts are certified."
                .to_string(),
        );
        cx.emit(Form1701QEvent::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This 1701Q draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Payment status cannot be advanced automatically for a manual/external 1701Q filing."
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
        self.sync_from_inputs(cx);
        if !self.input_errors.is_empty() {
            self.status_message = Some(
                "HTML preview was not opened because one or more editor values cannot be parsed."
                    .to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix the invalid 1701Q input text before opening preview.",
            );
            return;
        }
        let envelope = RenderEnvelopeV1::from(&self.draft);
        match crate::views::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(kind) => self.status_message = Some(kind.status_message().to_string()),
            Err(error) => {
                self.status_message = Some(format!("Could not open HTML preview: {error}"))
            }
        }
        cx.notify();
    }
}

impl Render for Form1701QView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = self.draft.is_editable();
        let status_message = self.status_message.clone();
        let evidence_warnings = self.draft.evidence_warnings();

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
                        gpui_component::button::Button::new("1701q_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form1701QEvent::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("1701q_preview")
                                    .label("HTML Preview")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.preview_pdf(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("1701q_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("1701q_manual")
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
                    .id("1701q_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
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
                                    .children(evidence_warnings.into_iter().map(|warning| {
                                        div().mt_1().text_sm().child(format!("• {warning}"))
                                    })),
                            )
                            .when_some(status_message, |element, message| {
                                element.child(
                                    div()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().muted.opacity(0.5))
                                        .child(message),
                                )
                            })
                            .child(self.render_error_summary(cx))
                            .children(self.render_editor_sections(cx)),
                    ),
            )
    }
}

fn section_card(cx: &Context<Form1701QView>, title: &str) -> Div {
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

fn amount_header(cx: &Context<Form1701QView>) -> AnyElement {
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

fn computed_amount(value: Option<f64>, cx: &Context<Form1701QView>) -> AnyElement {
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

fn text_input(
    cx: &mut Context<Form1701QView>,
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

fn optional_amount_input(
    cx: &mut Context<Form1701QView>,
    value: Option<f64>,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "Whole pesos; blank is empty", window);
    if let Some(value) = value {
        input.update(cx, |state, cx| {
            state.set_value(format!("{value:.0}"), window, cx)
        });
    }
    input
}

fn optional_payment_amount_input(
    cx: &mut Context<Form1701QView>,
    value: Option<f64>,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "Payment amount; blank is empty", window);
    if let Some(value) = value {
        input.update(cx, |state, cx| {
            state.set_value(value.to_string(), window, cx)
        });
    }
    input
}

fn payment_inputs(
    cx: &mut Context<Form1701QView>,
    row: &Form1701QPaymentRow,
    window: &mut Window,
) -> PaymentRowInputs {
    PaymentRowInputs {
        agency: text_input(cx, &row.drawee_bank_or_agency, "Drawee bank/agency", window),
        number: text_input(cx, &row.number, "Number", window),
        date: text_input(cx, &row.date, "MM/DD/YYYY", window),
        amount: optional_payment_amount_input(cx, row.amount, window),
    }
}

fn input_text(input: &Entity<InputState>, cx: &Context<Form1701QView>) -> String {
    input.read(cx).value().to_string()
}

fn parse_sheet_count(raw: &str) -> Result<u8, String> {
    raw.trim()
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= 99)
        .ok_or_else(|| "Enter a whole number from 0 to 99".to_string())
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
        return Err("Form 1701Q accepts whole pesos only; do not enter centavos".to_string());
    }
    Ok(Some(value.round()))
}

fn parse_optional_payment_amount(raw: &str) -> Result<Option<f64>, String> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let value = raw
        .trim()
        .replace(',', "")
        .parse::<f64>()
        .map_err(|_| format!("{raw:?} is not a numeric payment amount"))?;
    if !value.is_finite() || value < 0.0 {
        return Err("Payment amount must be finite and non-negative".to_string());
    }
    Ok(Some(value))
}

fn assign_amount(
    draft: &mut Form1701QDraft,
    item: u8,
    party: Form1701QParty,
    input: &Entity<InputState>,
    allow_negative: bool,
    cx: &Context<Form1701QView>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match parse_optional_whole_peso(&raw, allow_negative) {
        Ok(value) => draft.set_amount(item, party, value),
        Err(message) => {
            let party_name = match party {
                Form1701QParty::Taxpayer => "taxpayer",
                Form1701QParty::Spouse => "spouse",
            };
            errors.push((format!("item_{item}_{party_name}"), message));
        }
    }
}

fn sync_payment_row(
    target: &mut Form1701QPaymentRow,
    inputs: &PaymentRowInputs,
    field: &str,
    cx: &Context<Form1701QView>,
    errors: &mut Vec<(String, String)>,
) {
    target.drawee_bank_or_agency = input_text(&inputs.agency, cx);
    target.number = input_text(&inputs.number, cx);
    target.date = input_text(&inputs.date, cx);
    let raw = input_text(&inputs.amount, cx);
    match parse_optional_payment_amount(&raw) {
        Ok(value) => target.amount = value,
        Err(message) => errors.push((format!("{field}.amount"), message)),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_optional_payment_amount, parse_optional_whole_peso, parse_sheet_count};

    #[test]
    fn amount_parser_should_preserve_blank_as_none() {
        assert_eq!(parse_optional_whole_peso("", false), Ok(None));
    }

    #[test]
    fn amount_parser_should_reject_invalid_text_instead_of_zeroing_it() {
        assert!(parse_optional_whole_peso("not-a-number", false).is_err());
    }

    #[test]
    fn amount_parser_should_allow_signed_loss_lines_only_when_requested() {
        assert_eq!(parse_optional_whole_peso("-1250", true), Ok(Some(-1_250.0)));
        assert!(parse_optional_whole_peso("-1250", false).is_err());
    }

    #[test]
    fn sheet_parser_should_reject_more_than_two_digits() {
        assert!(parse_sheet_count("100").is_err());
    }

    #[test]
    fn payment_parser_should_not_apply_part_iii_whole_peso_rule() {
        assert_eq!(parse_optional_payment_amount("125.50"), Ok(Some(125.5)));
        assert!(parse_optional_payment_amount("-1").is_err());
    }
}
