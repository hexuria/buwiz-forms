//! Evidence-safe editor for exact form `0605v1999` (July 1999 ENCS).
//!
//! The editor persists semantic drafts and preserves the reviewed 235-field
//! editable-save contract, but it never queues or submits Form 0605. Only the
//! four reviewed ATC/tax-type code-index pairs are selectable in the app.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_0605::{
    FORM_VERSION_LABEL, Form0605Date, Form0605Draft, Form0605FilingBasis, Form0605MannerOfPayment,
    Form0605ReviewedAtc, Form0605ReviewedTaxType, Form0605TaxpayerClassification,
    Form0605TypeOfPayment,
};
use bir_core::forms::{FilingStatus, FormValidator};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;

pub enum Form0605Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form0605Event> for Form0605View {}

#[derive(Clone)]
struct FullPaymentRowInputs {
    agency: Entity<InputState>,
    number: Entity<InputState>,
    date: Entity<InputState>,
    amount: Entity<InputState>,
}

impl FullPaymentRowInputs {
    fn all(&self) -> [Entity<InputState>; 4] {
        [
            self.agency.clone(),
            self.number.clone(),
            self.date.clone(),
            self.amount.clone(),
        ]
    }
}

#[derive(Clone)]
struct TaxDebitPaymentInputs {
    number: Entity<InputState>,
    date: Entity<InputState>,
    amount: Entity<InputState>,
}

impl TaxDebitPaymentInputs {
    fn all(&self) -> [Entity<InputState>; 3] {
        [self.number.clone(), self.date.clone(), self.amount.clone()]
    }
}

pub struct Form0605View {
    draft: Form0605Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    filing_basis: Form0605FilingBasis,
    quarter: u8,
    classification: Form0605TaxpayerClassification,
    reviewed_atc: Option<Form0605ReviewedAtc>,
    reviewed_tax_type: Option<Form0605ReviewedTaxType>,
    manner_of_payment: Option<Form0605MannerOfPayment>,
    type_of_payment: Option<Form0605TypeOfPayment>,

    year_end_month: Entity<InputState>,
    year_ended: Entity<InputState>,
    due_date: Entity<InputState>,
    return_period: Entity<InputState>,
    number_of_sheets: Entity<InputState>,

    rdo_code: Entity<InputState>,
    taxpayer_name: Entity<InputState>,
    line_of_business: Entity<InputState>,
    registered_address: Entity<InputState>,
    zip_code: Entity<InputState>,
    contact_number: Entity<InputState>,
    email: Entity<InputState>,

    other_manner_description: Entity<InputState>,
    number_of_installments: Entity<InputState>,

    item_19_basic_tax_or_payment: Entity<InputState>,
    item_20a_surcharge: Entity<InputState>,
    item_20b_interest: Entity<InputState>,
    item_20c_compromise: Entity<InputState>,

    signature_taxpayer_or_representative: Entity<InputState>,
    signature_title_or_position: Entity<InputState>,
    signature_head_of_office: Entity<InputState>,

    payment_23_amount: Entity<InputState>,
    payment_24_check: FullPaymentRowInputs,
    payment_25_tax_debit_memo: TaxDebitPaymentInputs,
    payment_26_others: FullPaymentRowInputs,
    machine_validation_or_receipt_details: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form0605View {
    pub fn new(
        draft: Form0605Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let reviewed_atc = draft.atc.as_ref().and_then(|selection| {
            Form0605ReviewedAtc::ALL.into_iter().find(|candidate| {
                candidate.code() == selection.code()
                    && candidate.xml_index() == selection.xml_index()
            })
        });
        let reviewed_tax_type = draft.tax_type.as_ref().and_then(|selection| {
            Form0605ReviewedTaxType::ALL.into_iter().find(|candidate| {
                candidate.code() == selection.code()
                    && candidate.xml_index() == selection.xml_index()
            })
        });

        let year_end_month = text_input(
            cx,
            &draft.year_end_month.to_string(),
            "Month (1-12)",
            window,
        );
        let year_ended = text_input(cx, &draft.taxable_year.to_string(), "YYYY", window);
        let due_date = text_input(
            cx,
            &draft
                .due_date
                .map(|date| date.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        );
        let return_period = text_input(
            cx,
            &draft
                .return_period
                .map(|date| date.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        );
        let number_of_sheets = text_input(
            cx,
            &draft.number_of_sheets.to_string(),
            "Attached sheets",
            window,
        );

        let rdo_code = text_input(cx, &draft.rdo_code, "3-digit RDO", window);
        let taxpayer_name = text_input(cx, &draft.taxpayer_name, "Taxpayer name", window);
        let line_of_business = text_input(
            cx,
            &draft.line_of_business,
            "Line of business / occupation",
            window,
        );
        let registered_address =
            text_input(cx, &draft.registered_address, "Registered address", window);
        let zip_code = text_input(cx, &draft.zip_code, "4-digit ZIP", window);
        let contact_number = text_input(cx, &draft.contact_number, "Telephone number", window);
        let email = text_input(cx, &draft.email, "Email address", window);

        let other_manner_description = text_input(
            cx,
            &draft.other_manner_description,
            "Required only for Others",
            window,
        );
        let number_of_installments = text_input(
            cx,
            &draft
                .number_of_installments
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "Positive whole number",
            window,
        );

        let item_19_basic_tax_or_payment =
            money_input(cx, Some(draft.item_19_basic_tax_or_payment), true, window);
        let item_20a_surcharge = money_input(cx, Some(draft.item_20a_surcharge), true, window);
        let item_20b_interest = money_input(cx, Some(draft.item_20b_interest), true, window);
        let item_20c_compromise = money_input(cx, Some(draft.item_20c_compromise), true, window);

        let signature_taxpayer_or_representative = text_input(
            cx,
            &draft.signatures.taxpayer_or_authorized_representative,
            "Printed name",
            window,
        );
        let signature_title_or_position = text_input(
            cx,
            &draft.signatures.title_or_position,
            "Title / position",
            window,
        );
        let signature_head_of_office = text_input(
            cx,
            &draft.signatures.head_of_office,
            "Head of office",
            window,
        );

        let payment_23_amount = money_input(
            cx,
            draft.payment_details.cash_or_bank_debit_memo_amount,
            false,
            window,
        );
        let payment_24_check = FullPaymentRowInputs {
            agency: text_input(
                cx,
                &draft.payment_details.check.drawee_bank_or_agency,
                "Drawee bank / agency",
                window,
            ),
            number: text_input(cx, &draft.payment_details.check.number, "Number", window),
            date: text_input(cx, &draft.payment_details.check.date, "MM/DD/YYYY", window),
            amount: money_input(cx, draft.payment_details.check.amount, false, window),
        };
        let payment_25_tax_debit_memo = TaxDebitPaymentInputs {
            number: text_input(
                cx,
                &draft.payment_details.tax_debit_memo.number,
                "Number",
                window,
            ),
            date: text_input(
                cx,
                &draft.payment_details.tax_debit_memo.date,
                "MM/DD/YYYY",
                window,
            ),
            amount: money_input(
                cx,
                draft.payment_details.tax_debit_memo.amount,
                false,
                window,
            ),
        };
        let payment_26_others = FullPaymentRowInputs {
            agency: text_input(
                cx,
                &draft.payment_details.others.drawee_bank_or_agency,
                "Drawee bank / agency",
                window,
            ),
            number: text_input(cx, &draft.payment_details.others.number, "Number", window),
            date: text_input(cx, &draft.payment_details.others.date, "MM/DD/YYYY", window),
            amount: money_input(cx, draft.payment_details.others.amount, false, window),
        };
        let machine_validation_or_receipt_details = text_input(
            cx,
            &draft.payment_details.machine_validation_or_receipt_details,
            "Machine validation / receipt details",
            window,
        );

        let mut all_inputs = vec![
            year_end_month.clone(),
            year_ended.clone(),
            due_date.clone(),
            return_period.clone(),
            number_of_sheets.clone(),
            rdo_code.clone(),
            taxpayer_name.clone(),
            line_of_business.clone(),
            registered_address.clone(),
            zip_code.clone(),
            contact_number.clone(),
            email.clone(),
            other_manner_description.clone(),
            number_of_installments.clone(),
            item_19_basic_tax_or_payment.clone(),
            item_20a_surcharge.clone(),
            item_20b_interest.clone(),
            item_20c_compromise.clone(),
            signature_taxpayer_or_representative.clone(),
            signature_title_or_position.clone(),
            signature_head_of_office.clone(),
            payment_23_amount.clone(),
            machine_validation_or_receipt_details.clone(),
        ];
        all_inputs.extend(payment_24_check.all());
        all_inputs.extend(payment_25_tax_debit_memo.all());
        all_inputs.extend(payment_26_others.all());

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
            filing_basis: draft.filing_basis,
            quarter: draft.quarter,
            classification: draft.classification,
            reviewed_atc,
            reviewed_tax_type,
            manner_of_payment: draft.manner_of_payment,
            type_of_payment: draft.type_of_payment,
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            input_errors: Vec::new(),
            validation_errors,
            status_message: None,
            year_end_month,
            year_ended,
            due_date,
            return_period,
            number_of_sheets,
            rdo_code,
            taxpayer_name,
            line_of_business,
            registered_address,
            zip_code,
            contact_number,
            email,
            other_manner_description,
            number_of_installments,
            item_19_basic_tax_or_payment,
            item_20a_surcharge,
            item_20b_interest,
            item_20c_compromise,
            signature_taxpayer_or_representative,
            signature_title_or_position,
            signature_head_of_office,
            payment_23_amount,
            payment_24_check,
            payment_25_tax_debit_memo,
            payment_26_others,
            machine_validation_or_receipt_details,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.input_errors.clear();
        self.draft.filing_basis = self.filing_basis;
        self.draft.quarter = self.quarter;
        self.draft.classification = self.classification;
        self.draft.manner_of_payment = self.manner_of_payment;
        self.draft.type_of_payment = self.type_of_payment;
        if let Some(value) = self.reviewed_atc {
            self.draft.select_reviewed_atc(value);
        }
        if let Some(value) = self.reviewed_tax_type {
            self.draft.select_reviewed_tax_type(value);
        }

        assign_u8(
            &mut self.draft.year_end_month,
            &self.year_end_month,
            "year_end_month",
            cx,
            &mut self.input_errors,
        );
        assign_u16(
            &mut self.draft.taxable_year,
            &self.year_ended,
            "taxable_year",
            cx,
            &mut self.input_errors,
        );
        assign_date(
            &mut self.draft.due_date,
            &self.due_date,
            "due_date",
            cx,
            &mut self.input_errors,
        );
        assign_date(
            &mut self.draft.return_period,
            &self.return_period,
            "return_period",
            cx,
            &mut self.input_errors,
        );
        assign_u16(
            &mut self.draft.number_of_sheets,
            &self.number_of_sheets,
            "number_of_sheets",
            cx,
            &mut self.input_errors,
        );

        self.draft.rdo_code = input_text(&self.rdo_code, cx);
        self.draft.taxpayer_name = input_text(&self.taxpayer_name, cx);
        self.draft.line_of_business = input_text(&self.line_of_business, cx);
        self.draft.registered_address = input_text(&self.registered_address, cx);
        self.draft.zip_code = input_text(&self.zip_code, cx);
        self.draft.contact_number = input_text(&self.contact_number, cx);
        self.draft.email = input_text(&self.email, cx);
        self.draft.other_manner_description = input_text(&self.other_manner_description, cx);
        self.draft.number_of_installments = parse_optional_u16(
            &self.number_of_installments,
            "number_of_installments",
            cx,
            &mut self.input_errors,
        );

        assign_money(
            &mut self.draft.item_19_basic_tax_or_payment,
            &self.item_19_basic_tax_or_payment,
            "item_19_basic_tax_or_payment",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_20a_surcharge,
            &self.item_20a_surcharge,
            "item_20a_surcharge",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_20b_interest,
            &self.item_20b_interest,
            "item_20b_interest",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_20c_compromise,
            &self.item_20c_compromise,
            "item_20c_compromise",
            cx,
            &mut self.input_errors,
        );

        self.draft.signatures.taxpayer_or_authorized_representative =
            input_text(&self.signature_taxpayer_or_representative, cx);
        self.draft.signatures.title_or_position = input_text(&self.signature_title_or_position, cx);
        self.draft.signatures.head_of_office = input_text(&self.signature_head_of_office, cx);

        assign_optional_money(
            &mut self.draft.payment_details.cash_or_bank_debit_memo_amount,
            &self.payment_23_amount,
            "payment_23_cash_or_bank_debit_memo.amount",
            cx,
            &mut self.input_errors,
        );
        self.draft.payment_details.check.drawee_bank_or_agency =
            input_text(&self.payment_24_check.agency, cx);
        self.draft.payment_details.check.number = input_text(&self.payment_24_check.number, cx);
        self.draft.payment_details.check.date = input_text(&self.payment_24_check.date, cx);
        assign_optional_money(
            &mut self.draft.payment_details.check.amount,
            &self.payment_24_check.amount,
            "payment_24_check.amount",
            cx,
            &mut self.input_errors,
        );
        self.draft.payment_details.tax_debit_memo.number =
            input_text(&self.payment_25_tax_debit_memo.number, cx);
        self.draft.payment_details.tax_debit_memo.date =
            input_text(&self.payment_25_tax_debit_memo.date, cx);
        assign_optional_money(
            &mut self.draft.payment_details.tax_debit_memo.amount,
            &self.payment_25_tax_debit_memo.amount,
            "payment_25_tax_debit_memo.amount",
            cx,
            &mut self.input_errors,
        );
        self.draft.payment_details.others.drawee_bank_or_agency =
            input_text(&self.payment_26_others.agency, cx);
        self.draft.payment_details.others.number = input_text(&self.payment_26_others.number, cx);
        self.draft.payment_details.others.date = input_text(&self.payment_26_others.date, cx);
        assign_optional_money(
            &mut self.draft.payment_details.others.amount,
            &self.payment_26_others.amount,
            "payment_26_others.amount",
            cx,
            &mut self.input_errors,
        );
        self.draft
            .payment_details
            .machine_validation_or_receipt_details =
            input_text(&self.machine_validation_or_receipt_details, cx);

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

    fn render_computed_row(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted.opacity(0.5))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("₱ {value:.2}")),
            )
            .into_any_element()
    }

    fn render_full_payment_row(
        &self,
        item: u8,
        label: &str,
        row: &FullPaymentRowInputs,
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
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.agency).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.number).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.date).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.amount).disabled(!self.draft.is_editable())),
                    ),
            )
            .into_any_element()
    }

    fn render_tax_debit_payment_row(&self, cx: &Context<Self>) -> AnyElement {
        let row = &self.payment_25_tax_debit_memo;
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
                    .child("25 Tax Debit Memo"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.number).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.date).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.amount).disabled(!self.draft.is_editable())),
                    ),
            )
            .into_any_element()
    }
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
                "Draft was not saved because one or more entered values are invalid.".to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix the 0605 input errors before saving; invalid values are never converted to zero.",
            );
            return;
        }

        let save_result = self
            .db
            .lock()
            .map_err(|_| "Draft database lock is unavailable".to_string())
            .and_then(|db| {
                db.save_form_draft(
                    &self.draft.tin,
                    "0605",
                    self.draft.taxable_year,
                    Some(self.draft.month),
                    &self.draft.status,
                    &self.draft,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            });

        match save_result {
            Ok(()) => {
                self.status_message = Some(if self.validation_errors.is_empty() {
                    "Draft saved.".to_string()
                } else {
                    "Draft saved locally with unresolved review items; submission remains disabled."
                        .to_string()
                });
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Success,
                    "0605 draft saved locally.",
                );
                cx.emit(Form0605Event::Saved);
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save draft: {error}"));
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    format!("Could not save 0605 draft: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "0605v1999 filing is manual/external; the reviewed saves are not a certified submission contract."
                .to_string(),
        );
        cx.emit(Form0605Event::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This 0605 draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Payment status cannot be advanced automatically for a manual/external 0605 filing."
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
        // Freeze the latest editor values into one immutable renderer
        // envelope. Preview never changes the manual/external filing state.
        self.sync_from_inputs(cx);
        let render_draft = self.draft.clone();
        let envelope = bir_print::html::RenderEnvelopeV1::from(&render_draft);

        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => {
                self.status_message = Some(launch_kind.status_message().to_string());
            }
            Err(error) => {
                let message = format!(
                    "HTML print preview could not be opened: {error}. No filing state was changed."
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

impl Render for Form0605View {
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
                        gpui_component::button::Button::new("0605_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form0605Event::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("0605_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("0605_manual")
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
                    .id("0605_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(1050.0))
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
                            .child(
                                section_card(cx, "ITEMS 1–8 — FILING AND PAYMENT IDENTITY")
                                    .child(
                                        div()
                                            .text_xs()
                                            .child(format!("Persistence slot: {} (not used to derive quarter or dates)", self.draft.month)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .child(self.render_choice(
                                                "0605_calendar",
                                                "1 Calendar",
                                                matches!(
                                                    self.filing_basis,
                                                    Form0605FilingBasis::Calendar
                                                ),
                                                cx,
                                                |this| {
                                                    this.filing_basis =
                                                        Form0605FilingBasis::Calendar
                                                },
                                            ))
                                            .child(self.render_choice(
                                                "0605_fiscal",
                                                "1 Fiscal",
                                                matches!(
                                                    self.filing_basis,
                                                    Form0605FilingBasis::Fiscal
                                                ),
                                                cx,
                                                |this| {
                                                    this.filing_basis =
                                                        Form0605FilingBasis::Fiscal
                                                },
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .children((1_u8..=4).map(|quarter| {
                                                self.render_choice(
                                                    format!("0605_quarter_{quarter}"),
                                                    format!("3 Q{quarter}"),
                                                    self.quarter == quarter,
                                                    cx,
                                                    move |this| this.quarter = quarter,
                                                )
                                            })),
                                    )
                                    .child(self.render_input_row(
                                        "2 Year Ended month (independent)",
                                        &self.year_end_month,
                                    ))
                                    .child(self.render_input_row(
                                        "2 Year Ended year (independent)",
                                        &self.year_ended,
                                    ))
                                    .child(self.render_input_row(
                                        "4 Due Date (MM/DD/YYYY)",
                                        &self.due_date,
                                    ))
                                    .child(self.render_input_row(
                                        "5 Number of Sheets Attached",
                                        &self.number_of_sheets,
                                    ))
                                    .child(self.render_input_row(
                                        "7 Return Period (MM/DD/YYYY)",
                                        &self.return_period,
                                    ))
                                    .child(
                                        div()
                                            .mt_2()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("6 ATC — only source-proven pairs"),
                                    )
                                    .child(
                                        div().flex().flex_wrap().gap_2().children(
                                            Form0605ReviewedAtc::ALL.into_iter().map(|value| {
                                                self.render_choice(
                                                    format!("0605_atc_{}", value.code()),
                                                    format!(
                                                        "{} · {}",
                                                        value.code(),
                                                        value.description()
                                                    ),
                                                    self.reviewed_atc == Some(value),
                                                    cx,
                                                    move |this| this.reviewed_atc = Some(value),
                                                )
                                            }),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .mt_2()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("8 Tax Type — only source-proven pairs"),
                                    )
                                    .child(
                                        div().flex().flex_wrap().gap_2().children(
                                            Form0605ReviewedTaxType::ALL.into_iter().map(|value| {
                                                self.render_choice(
                                                    format!("0605_tax_type_{}", value.code()),
                                                    format!(
                                                        "{} · {}",
                                                        value.code(),
                                                        value.description()
                                                    ),
                                                    self.reviewed_tax_type == Some(value),
                                                    cx,
                                                    move |this| {
                                                        this.reviewed_tax_type = Some(value)
                                                    },
                                                )
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                section_card(cx, "PART I — BACKGROUND INFORMATION")
                                    .child(div().text_sm().child(format!("9 TIN: {}", self.draft.tin)))
                                    .child(self.render_input_row("10 RDO Code", &self.rdo_code))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .child(self.render_choice(
                                                "0605_classification_individual",
                                                "11 Individual",
                                                matches!(
                                                    self.classification,
                                                    Form0605TaxpayerClassification::Individual
                                                ),
                                                cx,
                                                |this| {
                                                    this.classification =
                                                        Form0605TaxpayerClassification::Individual
                                                },
                                            ))
                                            .child(self.render_choice(
                                                "0605_classification_non_individual",
                                                "11 Non-Individual",
                                                matches!(
                                                    self.classification,
                                                    Form0605TaxpayerClassification::NonIndividual
                                                ),
                                                cx,
                                                |this| {
                                                    this.classification =
                                                        Form0605TaxpayerClassification::NonIndividual
                                                },
                                            )),
                                    )
                                    .child(self.render_input_row(
                                        "12 Line of Business / Occupation",
                                        &self.line_of_business,
                                    ))
                                    .child(self.render_input_row(
                                        "13 Taxpayer Name",
                                        &self.taxpayer_name,
                                    ))
                                    .child(self.render_input_row(
                                        "14 Telephone Number",
                                        &self.contact_number,
                                    ))
                                    .child(self.render_input_row(
                                        "15 Registered Address",
                                        &self.registered_address,
                                    ))
                                    .child(self.render_input_row("16 ZIP Code", &self.zip_code))
                                    .child(self.render_input_row(
                                        "Editable-save email",
                                        &self.email,
                                    ))
                                    .child(
                                        div()
                                            .mt_2()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("17 Manner of Payment"),
                                    )
                                    .child(
                                        div().flex().flex_wrap().gap_2().children(
                                            Form0605MannerOfPayment::ALL.into_iter().map(|value| {
                                                self.render_choice(
                                                    format!("0605_manner_{value:?}"),
                                                    value.label(),
                                                    self.manner_of_payment == Some(value),
                                                    cx,
                                                    move |this| {
                                                        this.manner_of_payment = Some(value)
                                                    },
                                                )
                                            }),
                                        ),
                                    )
                                    .child(self.render_input_row(
                                        "17 Others (Specify)",
                                        &self.other_manner_description,
                                    ))
                                    .child(
                                        div()
                                            .mt_2()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("18 Type of Payment"),
                                    )
                                    .child(
                                        div().flex().flex_wrap().gap_2().children(
                                            Form0605TypeOfPayment::ALL.into_iter().map(|value| {
                                                self.render_choice(
                                                    format!("0605_type_{value:?}"),
                                                    value.label(),
                                                    self.type_of_payment == Some(value),
                                                    cx,
                                                    move |this| {
                                                        this.type_of_payment = Some(value)
                                                    },
                                                )
                                            }),
                                        ),
                                    )
                                    .child(self.render_input_row(
                                        "18 Number of Installments",
                                        &self.number_of_installments,
                                    )),
                            )
                            .child(
                                section_card(cx, "PART II — COMPUTATION OF TAX")
                                    .child(self.render_input_row(
                                        "19 Basic Tax / Deposit / Advance Payment",
                                        &self.item_19_basic_tax_or_payment,
                                    ))
                                    .child(self.render_input_row(
                                        "20A Surcharge (manual)",
                                        &self.item_20a_surcharge,
                                    ))
                                    .child(self.render_input_row(
                                        "20B Interest (manual)",
                                        &self.item_20b_interest,
                                    ))
                                    .child(self.render_input_row(
                                        "20C Compromise (manual)",
                                        &self.item_20c_compromise,
                                    ))
                                    .child(self.render_computed_row(
                                        "20D Total Penalties (20A + 20B + 20C)",
                                        self.draft.item_20d_total_penalties,
                                        cx,
                                    ))
                                    .child(self.render_computed_row(
                                        "21 Total Amount Payable (19 + 20D)",
                                        self.draft.item_21_total_amount_payable,
                                        cx,
                                    )),
                            )
                            .child(
                                section_card(cx, "ITEM 22 — SIGNATURE DETAILS")
                                    .child(
                                        div().text_xs().child(
                                            "Official-PDF fields persisted in the app draft; absent from the reviewed 235-field editable saves.",
                                        ),
                                    )
                                    .child(self.render_input_row(
                                        "22A Taxpayer / Authorized Representative",
                                        &self.signature_taxpayer_or_representative,
                                    ))
                                    .child(self.render_input_row(
                                        "22A Title / Position",
                                        &self.signature_title_or_position,
                                    ))
                                    .child(self.render_input_row(
                                        "22B Head of Office",
                                        &self.signature_head_of_office,
                                    )),
                            )
                            .child(
                                section_card(cx, "PART III — DETAILS OF PAYMENT")
                                    .child(
                                        div().text_xs().child(
                                            "Official-PDF fields persisted in the app draft; absent from the reviewed 235-field editable saves.",
                                        ),
                                    )
                                    .child(self.render_input_row(
                                        "23 Cash / Bank Debit Memo — Amount",
                                        &self.payment_23_amount,
                                    ))
                                    .child(self.render_full_payment_row(
                                        24,
                                        "Check · Bank/Agency · Number · Date · Amount",
                                        &self.payment_24_check,
                                        cx,
                                    ))
                                    .child(self.render_tax_debit_payment_row(cx))
                                    .child(self.render_full_payment_row(
                                        26,
                                        "Others · Bank/Agency · Number · Date · Amount",
                                        &self.payment_26_others,
                                        cx,
                                    ))
                                    .child(self.render_input_row(
                                        "Machine Validation / Revenue Official Receipt Details",
                                        &self.machine_validation_or_receipt_details,
                                    )),
                            ),
                    ),
            )
    }
}

fn section_card(cx: &Context<Form0605View>, title: &str) -> Div {
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

fn text_input(
    cx: &mut Context<Form0605View>,
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

fn money_input(
    cx: &mut Context<Form0605View>,
    value: Option<f64>,
    preserve_zero: bool,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "0.00", window);
    if let Some(value) = value
        && (value != 0.0 || preserve_zero)
    {
        input.update(cx, |state, cx| {
            state.set_value(format!("{value:.2}"), window, cx)
        });
    }
    input
}

fn input_text(input: &Entity<InputState>, cx: &Context<Form0605View>) -> String {
    input.read(cx).value().to_string()
}

fn parse_required_number<T: std::str::FromStr>(
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) -> Option<T> {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        errors.push((
            field.to_string(),
            "A whole-number value is required".to_string(),
        ));
        return None;
    }
    raw.trim().parse::<T>().map(Some).unwrap_or_else(|_| {
        errors.push((
            field.to_string(),
            format!("Enter a valid whole number; {raw:?} is not accepted"),
        ));
        None
    })
}

fn assign_u8(
    target: &mut u8,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) {
    if let Some(value) = parse_required_number(input, field, cx, errors) {
        *target = value;
    }
}

fn assign_u16(
    target: &mut u16,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) {
    if let Some(value) = parse_required_number(input, field, cx, errors) {
        *target = value;
    }
}

fn parse_optional_u16(
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) -> Option<u16> {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        return None;
    }
    raw.trim().parse::<u16>().map(Some).unwrap_or_else(|_| {
        errors.push((
            field.to_string(),
            format!("Enter a valid whole number; {raw:?} is not accepted"),
        ));
        None
    })
}

fn assign_date(
    target: &mut Option<Form0605Date>,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        *target = None;
        return;
    }
    match Form0605Date::parse_mm_dd_yyyy(&raw) {
        Ok(value) => *target = Some(value),
        Err(message) => errors.push((field.to_string(), message)),
    }
}

fn parse_money_value(raw: &str, blank_is_zero: bool) -> Result<Option<f64>, String> {
    if raw.trim().is_empty() {
        return Ok(blank_is_zero.then_some(0.0));
    }
    match raw.trim().replace(',', "").parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(Some(value)),
        _ => Err(format!(
            "Enter a valid finite numeric amount; {raw:?} is not accepted"
        )),
    }
}

fn assign_money(
    target: &mut f64,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match parse_money_value(&raw, true) {
        Ok(Some(value)) => *target = value,
        Ok(None) => {}
        Err(message) => errors.push((field.to_string(), message)),
    }
}

fn assign_optional_money(
    target: &mut Option<f64>,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0605View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match parse_money_value(&raw, false) {
        Ok(value) => *target = value,
        Err(message) => errors.push((field.to_string(), message)),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_money_value;

    #[test]
    fn invalid_money_is_rejected_instead_of_becoming_zero() {
        assert!(parse_money_value("not-a-number", true).is_err());
    }

    #[test]
    fn required_blank_money_is_explicit_zero() {
        assert_eq!(parse_money_value("", true).unwrap(), Some(0.0));
    }

    #[test]
    fn optional_blank_money_stays_blank() {
        assert_eq!(parse_money_value("", false).unwrap(), None);
    }

    #[test]
    fn grouped_money_is_parsed_without_loss() {
        assert_eq!(parse_money_value("1,234.56", true).unwrap(), Some(1_234.56));
    }
}
