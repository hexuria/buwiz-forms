//! Evidence-safe editor for exact form `2550Qv2024`.
//!
//! The supplied official PDF and reviewed 160-field editable-save pair prove
//! draft persistence and the form's arithmetic. They do not prove an online
//! submission transport, so this view saves local drafts but never queues or
//! submits them.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_2550q::{
    FORM_CODE, FORM_VERSION_LABEL, Form2550QAdvanceVatRow, Form2550QCapitalGoodRow,
    Form2550QCreditableVatRow, Form2550QDate, Form2550QDraft, Form2550QFilingBasis,
    Form2550QQuarter, Form2550QTaxpayerClassification,
};
use bir_core::forms::{FilingPeriod, FilingStatus, FormValidator};
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;

const YEAR_END_MONTH: &str = "year_end_month";
const RETURN_PERIOD_FROM: &str = "return_period_from";
const RETURN_PERIOD_TO: &str = "return_period_to";
const TAX_RELIEF_DETAILS: &str = "tax_relief_details";

const ITEM_18: &str = "item_18";
const ITEM_19_DESCRIPTION: &str = "item_19_description";
const ITEM_19: &str = "item_19";
const ITEM_22: &str = "item_22";
const ITEM_23: &str = "item_23";
const ITEM_24: &str = "item_24";

const ITEM_31A: &str = "item_31a";
const ITEM_32A: &str = "item_32a";
const ITEM_33A: &str = "item_33a";
const ITEM_35B: &str = "item_35b";
const ITEM_36B: &str = "item_36b";
const ITEM_38B: &str = "item_38b";
const ITEM_40B: &str = "item_40b";
const ITEM_41B: &str = "item_41b";
const ITEM_42_DESCRIPTION: &str = "item_42_description";
const ITEM_42B: &str = "item_42b";
const ITEM_44A: &str = "item_44a";
const ITEM_44B: &str = "item_44b";
const ITEM_45A: &str = "item_45a";
const ITEM_45B: &str = "item_45b";
const ITEM_46A: &str = "item_46a";
const ITEM_46B: &str = "item_46b";
const ITEM_47_DESCRIPTION: &str = "item_47_description";
const ITEM_47A: &str = "item_47a";
const ITEM_47B: &str = "item_47b";
const ITEM_48A: &str = "item_48a";
const ITEM_49A: &str = "item_49a";
const ITEM_54B: &str = "item_54b";
const ITEM_55B: &str = "item_55b";
const ITEM_56_DESCRIPTION: &str = "item_56_description";
const ITEM_56B: &str = "item_56b";
const ITEM_58B: &str = "item_58b";

const SCHEDULE_2_DIRECT: &str = "schedule_2_direct";
const SCHEDULE_2_EXEMPT_SALES: &str = "schedule_2_exempt_sales";
const SCHEDULE_2_NOT_DIRECT: &str = "schedule_2_not_direct";

const SIGNATORY: &str = "signatory";
const SIGNATORY_TITLE: &str = "signatory_title";
const NON_INDIVIDUAL_OFFICER: &str = "non_individual_officer";
const TAX_AGENT_NUMBER: &str = "tax_agent_number";
const TAX_AGENT_ISSUE: &str = "tax_agent_issue";
const TAX_AGENT_EXPIRY: &str = "tax_agent_expiry";
const CASH_AMOUNT: &str = "cash_amount";
const CHECK_BANK: &str = "check_bank";
const CHECK_NUMBER: &str = "check_number";
const CHECK_DATE: &str = "check_date";
const CHECK_AMOUNT: &str = "check_amount";
const TDM_NUMBER: &str = "tdm_number";
const TDM_DATE: &str = "tdm_date";
const TDM_AMOUNT: &str = "tdm_amount";
const OTHER_PAYMENT_DESCRIPTION: &str = "other_payment_description";
const OTHER_PAYMENT_BANK: &str = "other_payment_bank";
const OTHER_PAYMENT_NUMBER: &str = "other_payment_number";
const OTHER_PAYMENT_DATE: &str = "other_payment_date";
const OTHER_PAYMENT_AMOUNT: &str = "other_payment_amount";
const MACHINE_VALIDATION: &str = "machine_validation";

pub enum Form2550QV2Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form2550QV2Event> for Form2550QV2View {}

#[derive(Clone)]
struct CapitalGoodInputs {
    date: Entity<InputState>,
    source_code: Entity<InputState>,
    description: Entity<InputState>,
    purchase_amount: Entity<InputState>,
    input_tax: Entity<InputState>,
    estimated_life: Entity<InputState>,
    recognized_life: Entity<InputState>,
    allowable_input_tax: Entity<InputState>,
    balance_next_period: Entity<InputState>,
}

impl CapitalGoodInputs {
    fn all(&self) -> Vec<Entity<InputState>> {
        vec![
            self.date.clone(),
            self.source_code.clone(),
            self.description.clone(),
            self.purchase_amount.clone(),
            self.input_tax.clone(),
            self.estimated_life.clone(),
            self.recognized_life.clone(),
            self.allowable_input_tax.clone(),
            self.balance_next_period.clone(),
        ]
    }
}

#[derive(Clone)]
struct CreditableVatInputs {
    period_from: Entity<InputState>,
    period_to: Entity<InputState>,
    agent_name: Entity<InputState>,
    income_payment: Entity<InputState>,
    tax_withheld: Entity<InputState>,
}

impl CreditableVatInputs {
    fn all(&self) -> Vec<Entity<InputState>> {
        vec![
            self.period_from.clone(),
            self.period_to.clone(),
            self.agent_name.clone(),
            self.income_payment.clone(),
            self.tax_withheld.clone(),
        ]
    }
}

#[derive(Clone)]
struct AdvanceVatInputs {
    period_from: Entity<InputState>,
    period_to: Entity<InputState>,
    miller_name: Entity<InputState>,
    taxpayer_name: Entity<InputState>,
    receipt_number: Entity<InputState>,
    amount_paid: Entity<InputState>,
}

impl AdvanceVatInputs {
    fn all(&self) -> Vec<Entity<InputState>> {
        vec![
            self.period_from.clone(),
            self.period_to.clone(),
            self.miller_name.clone(),
            self.taxpayer_name.clone(),
            self.receipt_number.clone(),
            self.amount_paid.clone(),
        ]
    }
}

#[derive(Clone, Copy)]
enum ChoiceAction {
    FilingBasis(Form2550QFilingBasis),
    Quarter(Form2550QQuarter),
    Classification(Form2550QTaxpayerClassification),
    ToggleAmended,
    ToggleShortPeriod,
    ToggleTaxRelief,
}

pub struct Form2550QV2View {
    draft: Form2550QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,
    fields: BTreeMap<&'static str, Entity<InputState>>,
    schedule_1_inputs: Vec<CapitalGoodInputs>,
    schedule_3_inputs: Vec<CreditableVatInputs>,
    schedule_4_inputs: Vec<AdvanceVatInputs>,
    _subscriptions: Vec<Subscription>,
}

impl Form2550QV2View {
    pub fn new(
        mut draft: Form2550QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let migrated_legacy_draft = draft.migrate_legacy_flat_draft();
        let mut fields = BTreeMap::new();
        insert_text(
            &mut fields,
            YEAR_END_MONTH,
            &draft.year_end_month.to_string(),
            "1-12",
            window,
            cx,
        );
        insert_text(
            &mut fields,
            RETURN_PERIOD_FROM,
            &draft
                .return_period_from
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
            cx,
        );
        insert_text(
            &mut fields,
            RETURN_PERIOD_TO,
            &draft
                .return_period_to
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
            cx,
        );
        insert_text(
            &mut fields,
            TAX_RELIEF_DETAILS,
            &draft.tax_relief_details,
            "Special law / treaty",
            window,
            cx,
        );

        let p2 = &draft.part_ii;
        insert_money(
            &mut fields,
            ITEM_18,
            p2.item_18_paid_on_previous_return,
            window,
            cx,
        );
        insert_text(
            &mut fields,
            ITEM_19_DESCRIPTION,
            &p2.item_19_description,
            "Specify other credit/payment",
            window,
            cx,
        );
        insert_money(
            &mut fields,
            ITEM_19,
            p2.item_19_other_credit_or_payment,
            window,
            cx,
        );
        insert_money(&mut fields, ITEM_22, p2.item_22_surcharge, window, cx);
        insert_money(&mut fields, ITEM_23, p2.item_23_interest, window, cx);
        insert_money(&mut fields, ITEM_24, p2.item_24_compromise, window, cx);

        let p4 = &draft.part_iv;
        for (key, value) in [
            (ITEM_31A, p4.item_31a_vatable_sales),
            (ITEM_32A, p4.item_32a_zero_rated_sales),
            (ITEM_33A, p4.item_33a_exempt_sales),
            (ITEM_35B, p4.item_35b_less_output_vat_uncollected),
            (ITEM_36B, p4.item_36b_add_output_vat_recovered),
            (ITEM_38B, p4.item_38b_input_tax_carried),
            (ITEM_40B, p4.item_40b_transitional_input_tax),
            (ITEM_41B, p4.item_41b_presumptive_input_tax),
            (ITEM_42B, p4.item_42b_other_input_tax),
            (ITEM_44A, p4.item_44a_domestic_purchases),
            (ITEM_44B, p4.item_44b_domestic_input_tax),
            (ITEM_45A, p4.item_45a_nonresident_services),
            (ITEM_45B, p4.item_45b_nonresident_service_input_tax),
            (ITEM_46A, p4.item_46a_importations),
            (ITEM_46B, p4.item_46b_import_input_tax),
            (ITEM_47A, p4.item_47a_other_purchases),
            (ITEM_47B, p4.item_47b_other_input_tax),
            (ITEM_48A, p4.item_48a_domestic_purchases_no_input_tax),
            (ITEM_49A, p4.item_49a_vat_exempt_importations),
            (ITEM_54B, p4.item_54b_vat_refund_or_tcc_claimed),
            (ITEM_55B, p4.item_55b_input_vat_on_unpaid_payables),
            (ITEM_56B, p4.item_56b_other_deduction),
            (ITEM_58B, p4.item_58b_input_vat_on_settled_payables),
        ] {
            insert_money(&mut fields, key, value, window, cx);
        }
        insert_text(
            &mut fields,
            ITEM_42_DESCRIPTION,
            &p4.item_42_description,
            "Specify other prior input tax",
            window,
            cx,
        );
        insert_text(
            &mut fields,
            ITEM_47_DESCRIPTION,
            &p4.item_47_description,
            "Specify other purchases",
            window,
            cx,
        );
        insert_text(
            &mut fields,
            ITEM_56_DESCRIPTION,
            &p4.item_56_description,
            "Specify other deduction",
            window,
            cx,
        );

        insert_money(
            &mut fields,
            SCHEDULE_2_DIRECT,
            draft
                .schedule_2
                .input_tax_directly_attributable_to_exempt_sales,
            window,
            cx,
        );
        insert_money(
            &mut fields,
            SCHEDULE_2_EXEMPT_SALES,
            draft.schedule_2.vat_exempt_sales,
            window,
            cx,
        );
        insert_money(
            &mut fields,
            SCHEDULE_2_NOT_DIRECT,
            draft.schedule_2.input_tax_not_directly_attributable,
            window,
            cx,
        );

        let local = &draft.local_print_fields;
        for (key, value, placeholder) in [
            (
                SIGNATORY,
                local.taxpayer_or_authorized_representative.as_str(),
                "Taxpayer / representative",
            ),
            (
                SIGNATORY_TITLE,
                local.representative_title.as_str(),
                "Title / designation",
            ),
            (
                NON_INDIVIDUAL_OFFICER,
                local.non_individual_authorized_officer.as_str(),
                "Authorized officer",
            ),
            (
                TAX_AGENT_NUMBER,
                local.tax_agent_accreditation_or_roll_number.as_str(),
                "Accreditation / roll no.",
            ),
            (
                TAX_AGENT_ISSUE,
                local.tax_agent_date_of_issue.as_str(),
                "MM/DD/YYYY",
            ),
            (
                TAX_AGENT_EXPIRY,
                local.tax_agent_date_of_expiry.as_str(),
                "MM/DD/YYYY",
            ),
            (CHECK_BANK, local.check_bank.as_str(), "Drawee bank"),
            (CHECK_NUMBER, local.check_number.as_str(), "Check number"),
            (CHECK_DATE, local.check_date.as_str(), "MM/DD/YYYY"),
            (
                TDM_NUMBER,
                local.tax_debit_memo_number.as_str(),
                "Tax debit memo no.",
            ),
            (TDM_DATE, local.tax_debit_memo_date.as_str(), "MM/DD/YYYY"),
            (
                OTHER_PAYMENT_DESCRIPTION,
                local.other_payment_description.as_str(),
                "Describe other payment",
            ),
            (
                OTHER_PAYMENT_BANK,
                local.other_payment_bank.as_str(),
                "Bank / agency",
            ),
            (
                OTHER_PAYMENT_NUMBER,
                local.other_payment_number.as_str(),
                "Reference number",
            ),
            (
                OTHER_PAYMENT_DATE,
                local.other_payment_date.as_str(),
                "MM/DD/YYYY",
            ),
            (
                MACHINE_VALIDATION,
                local.machine_validation_or_receipt_details.as_str(),
                "Machine validation / receipt details",
            ),
        ] {
            insert_text(&mut fields, key, value, placeholder, window, cx);
        }
        for (key, value) in [
            (CASH_AMOUNT, local.cash_or_bank_debit_advice_amount),
            (CHECK_AMOUNT, local.check_amount),
            (TDM_AMOUNT, local.tax_debit_memo_amount),
            (OTHER_PAYMENT_AMOUNT, local.other_payment_amount),
        ] {
            insert_money(&mut fields, key, value, window, cx);
        }

        let schedule_1_inputs = draft
            .schedule_1
            .iter()
            .map(|row| capital_good_inputs(row, window, cx))
            .collect::<Vec<_>>();
        let schedule_3_inputs = draft
            .schedule_3
            .iter()
            .map(|row| creditable_vat_inputs(row, window, cx))
            .collect::<Vec<_>>();
        let schedule_4_inputs = draft
            .schedule_4
            .iter()
            .map(|row| advance_vat_inputs(row, window, cx))
            .collect::<Vec<_>>();

        let mut all_inputs = fields.values().cloned().collect::<Vec<_>>();
        for row in &schedule_1_inputs {
            all_inputs.extend(row.all());
        }
        for row in &schedule_3_inputs {
            all_inputs.extend(row.all());
        }
        for row in &schedule_4_inputs {
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
            status_message: migrated_legacy_draft.then(|| {
                "The scaffold-era 2550Q draft was migrated to the reviewed April 2024 model. Review any migration warnings before filing externally."
                    .to_string()
            }),
            fields,
            schedule_1_inputs,
            schedule_3_inputs,
            schedule_4_inputs,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.input_errors.clear();
        let fields = &self.fields;

        assign_required_u8(
            &mut self.draft.year_end_month,
            field(fields, YEAR_END_MONTH),
            YEAR_END_MONTH,
            cx,
            &mut self.input_errors,
        );
        assign_return_date(
            &mut self.draft.return_period_from,
            field(fields, RETURN_PERIOD_FROM),
            RETURN_PERIOD_FROM,
            cx,
            &mut self.input_errors,
        );
        assign_return_date(
            &mut self.draft.return_period_to,
            field(fields, RETURN_PERIOD_TO),
            RETURN_PERIOD_TO,
            cx,
            &mut self.input_errors,
        );
        self.draft.tax_relief_details = input_text(field(fields, TAX_RELIEF_DETAILS), cx);

        let p2 = &mut self.draft.part_ii;
        assign_money(
            &mut p2.item_18_paid_on_previous_return,
            field(fields, ITEM_18),
            ITEM_18,
            cx,
            &mut self.input_errors,
        );
        p2.item_19_description = input_text(field(fields, ITEM_19_DESCRIPTION), cx);
        assign_money(
            &mut p2.item_19_other_credit_or_payment,
            field(fields, ITEM_19),
            ITEM_19,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p2.item_22_surcharge,
            field(fields, ITEM_22),
            ITEM_22,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p2.item_23_interest,
            field(fields, ITEM_23),
            ITEM_23,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p2.item_24_compromise,
            field(fields, ITEM_24),
            ITEM_24,
            cx,
            &mut self.input_errors,
        );

        let p4 = &mut self.draft.part_iv;
        assign_money(
            &mut p4.item_31a_vatable_sales,
            field(fields, ITEM_31A),
            ITEM_31A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_32a_zero_rated_sales,
            field(fields, ITEM_32A),
            ITEM_32A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_33a_exempt_sales,
            field(fields, ITEM_33A),
            ITEM_33A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_35b_less_output_vat_uncollected,
            field(fields, ITEM_35B),
            ITEM_35B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_36b_add_output_vat_recovered,
            field(fields, ITEM_36B),
            ITEM_36B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_38b_input_tax_carried,
            field(fields, ITEM_38B),
            ITEM_38B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_40b_transitional_input_tax,
            field(fields, ITEM_40B),
            ITEM_40B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_41b_presumptive_input_tax,
            field(fields, ITEM_41B),
            ITEM_41B,
            cx,
            &mut self.input_errors,
        );
        p4.item_42_description = input_text(field(fields, ITEM_42_DESCRIPTION), cx);
        assign_money(
            &mut p4.item_42b_other_input_tax,
            field(fields, ITEM_42B),
            ITEM_42B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_44a_domestic_purchases,
            field(fields, ITEM_44A),
            ITEM_44A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_44b_domestic_input_tax,
            field(fields, ITEM_44B),
            ITEM_44B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_45a_nonresident_services,
            field(fields, ITEM_45A),
            ITEM_45A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_45b_nonresident_service_input_tax,
            field(fields, ITEM_45B),
            ITEM_45B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_46a_importations,
            field(fields, ITEM_46A),
            ITEM_46A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_46b_import_input_tax,
            field(fields, ITEM_46B),
            ITEM_46B,
            cx,
            &mut self.input_errors,
        );
        p4.item_47_description = input_text(field(fields, ITEM_47_DESCRIPTION), cx);
        assign_money(
            &mut p4.item_47a_other_purchases,
            field(fields, ITEM_47A),
            ITEM_47A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_47b_other_input_tax,
            field(fields, ITEM_47B),
            ITEM_47B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_48a_domestic_purchases_no_input_tax,
            field(fields, ITEM_48A),
            ITEM_48A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_49a_vat_exempt_importations,
            field(fields, ITEM_49A),
            ITEM_49A,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_54b_vat_refund_or_tcc_claimed,
            field(fields, ITEM_54B),
            ITEM_54B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_55b_input_vat_on_unpaid_payables,
            field(fields, ITEM_55B),
            ITEM_55B,
            cx,
            &mut self.input_errors,
        );
        p4.item_56_description = input_text(field(fields, ITEM_56_DESCRIPTION), cx);
        assign_money(
            &mut p4.item_56b_other_deduction,
            field(fields, ITEM_56B),
            ITEM_56B,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut p4.item_58b_input_vat_on_settled_payables,
            field(fields, ITEM_58B),
            ITEM_58B,
            cx,
            &mut self.input_errors,
        );

        assign_money(
            &mut self
                .draft
                .schedule_2
                .input_tax_directly_attributable_to_exempt_sales,
            field(fields, SCHEDULE_2_DIRECT),
            SCHEDULE_2_DIRECT,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.schedule_2.vat_exempt_sales,
            field(fields, SCHEDULE_2_EXEMPT_SALES),
            SCHEDULE_2_EXEMPT_SALES,
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.schedule_2.input_tax_not_directly_attributable,
            field(fields, SCHEDULE_2_NOT_DIRECT),
            SCHEDULE_2_NOT_DIRECT,
            cx,
            &mut self.input_errors,
        );

        for (index, (row, inputs)) in self
            .draft
            .schedule_1
            .iter_mut()
            .zip(&self.schedule_1_inputs)
            .enumerate()
        {
            sync_capital_good_row(row, inputs, index, cx, &mut self.input_errors);
        }
        for (index, (row, inputs)) in self
            .draft
            .schedule_3
            .iter_mut()
            .zip(&self.schedule_3_inputs)
            .enumerate()
        {
            sync_creditable_vat_row(row, inputs, index, cx, &mut self.input_errors);
        }
        for (index, (row, inputs)) in self
            .draft
            .schedule_4
            .iter_mut()
            .zip(&self.schedule_4_inputs)
            .enumerate()
        {
            sync_advance_vat_row(row, inputs, index, cx, &mut self.input_errors);
        }

        let local = &mut self.draft.local_print_fields;
        local.taxpayer_or_authorized_representative = input_text(field(fields, SIGNATORY), cx);
        local.representative_title = input_text(field(fields, SIGNATORY_TITLE), cx);
        local.non_individual_authorized_officer =
            input_text(field(fields, NON_INDIVIDUAL_OFFICER), cx);
        local.tax_agent_accreditation_or_roll_number =
            input_text(field(fields, TAX_AGENT_NUMBER), cx);
        local.tax_agent_date_of_issue = input_text(field(fields, TAX_AGENT_ISSUE), cx);
        local.tax_agent_date_of_expiry = input_text(field(fields, TAX_AGENT_EXPIRY), cx);
        assign_money(
            &mut local.cash_or_bank_debit_advice_amount,
            field(fields, CASH_AMOUNT),
            CASH_AMOUNT,
            cx,
            &mut self.input_errors,
        );
        local.check_bank = input_text(field(fields, CHECK_BANK), cx);
        local.check_number = input_text(field(fields, CHECK_NUMBER), cx);
        local.check_date = input_text(field(fields, CHECK_DATE), cx);
        assign_money(
            &mut local.check_amount,
            field(fields, CHECK_AMOUNT),
            CHECK_AMOUNT,
            cx,
            &mut self.input_errors,
        );
        local.tax_debit_memo_number = input_text(field(fields, TDM_NUMBER), cx);
        local.tax_debit_memo_date = input_text(field(fields, TDM_DATE), cx);
        assign_money(
            &mut local.tax_debit_memo_amount,
            field(fields, TDM_AMOUNT),
            TDM_AMOUNT,
            cx,
            &mut self.input_errors,
        );
        local.other_payment_description = input_text(field(fields, OTHER_PAYMENT_DESCRIPTION), cx);
        local.other_payment_bank = input_text(field(fields, OTHER_PAYMENT_BANK), cx);
        local.other_payment_number = input_text(field(fields, OTHER_PAYMENT_NUMBER), cx);
        local.other_payment_date = input_text(field(fields, OTHER_PAYMENT_DATE), cx);
        assign_money(
            &mut local.other_payment_amount,
            field(fields, OTHER_PAYMENT_AMOUNT),
            OTHER_PAYMENT_AMOUNT,
            cx,
            &mut self.input_errors,
        );
        local.machine_validation_or_receipt_details =
            input_text(field(fields, MACHINE_VALIDATION), cx);

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

    fn apply_choice(&mut self, action: ChoiceAction, cx: &mut Context<Self>) {
        if !self.draft.is_editable() {
            return;
        }
        match action {
            ChoiceAction::FilingBasis(value) => self.draft.filing_basis = value,
            ChoiceAction::Quarter(value) => self.draft.quarter = value,
            ChoiceAction::Classification(value) => self.draft.taxpayer_classification = Some(value),
            ChoiceAction::ToggleAmended => self.draft.is_amended = !self.draft.is_amended,
            ChoiceAction::ToggleShortPeriod => {
                self.draft.is_short_period_return = !self.draft.is_short_period_return
            }
            ChoiceAction::ToggleTaxRelief => {
                self.draft.is_availing_tax_relief = !self.draft.is_availing_tax_relief
            }
        }
        self.sync_from_inputs(cx);
    }

    fn render_choice(
        &self,
        id: String,
        label: String,
        selected: bool,
        action: ChoiceAction,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_md()
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
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.apply_choice(action, cx);
            }))
            .child(label)
            .into_any_element()
    }

    fn render_input_row(&self, label: &str, key: &'static str) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().w_1_2().text_sm().child(label.to_string()))
            .child(
                div().w_1_2().child(
                    Input::new(field(&self.fields, key)).disabled(!self.draft.is_editable()),
                ),
            )
            .into_any_element()
    }

    fn render_computed_row(
        &self,
        label: &str,
        value: Option<f64>,
        cx: &Context<Self>,
    ) -> AnyElement {
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
                    .child(format_optional_money(value)),
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

    fn render_capital_good_row(
        &self,
        index: usize,
        row: &CapitalGoodInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.is_editable();
        row_card(cx, &format!("Schedule 1 row {}", index + 1))
            .child(input_pair("Purchase/import date", &row.date, editable))
            .child(input_pair(
                "Source code (D or I)",
                &row.source_code,
                editable,
            ))
            .child(input_pair("Description", &row.description, editable))
            .child(input_pair(
                "Purchase/import amount",
                &row.purchase_amount,
                editable,
            ))
            .child(input_pair("Input tax", &row.input_tax, editable))
            .child(input_pair(
                "Estimated useful life (months)",
                &row.estimated_life,
                editable,
            ))
            .child(input_pair(
                "Recognized useful life (months)",
                &row.recognized_life,
                editable,
            ))
            .child(input_pair(
                "Allowable input tax this period (manual)",
                &row.allowable_input_tax,
                editable,
            ))
            .child(input_pair(
                "Balance to next period",
                &row.balance_next_period,
                editable,
            ))
            .into_any_element()
    }

    fn render_creditable_vat_row(
        &self,
        index: usize,
        row: &CreditableVatInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.is_editable();
        row_card(cx, &format!("Schedule 3 row {}", index + 1))
            .child(input_pair("Period from", &row.period_from, editable))
            .child(input_pair("Period to", &row.period_to, editable))
            .child(input_pair("Withholding agent", &row.agent_name, editable))
            .child(input_pair("Income payment", &row.income_payment, editable))
            .child(input_pair(
                "Creditable VAT withheld",
                &row.tax_withheld,
                editable,
            ))
            .into_any_element()
    }

    fn render_advance_vat_row(
        &self,
        index: usize,
        row: &AdvanceVatInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.draft.is_editable();
        row_card(cx, &format!("Schedule 4 row {}", index + 1))
            .child(input_pair("Period from", &row.period_from, editable))
            .child(input_pair("Period to", &row.period_to, editable))
            .child(input_pair("Miller name", &row.miller_name, editable))
            .child(input_pair("Taxpayer name", &row.taxpayer_name, editable))
            .child(input_pair(
                "Official receipt number",
                &row.receipt_number,
                editable,
            ))
            .child(input_pair("Amount paid", &row.amount_paid, editable))
            .into_any_element()
    }
}

impl FormViewTrait for Form2550QV2View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2550Q"
    }

    fn form_subtitle(&self) -> &'static str {
        "Quarterly Value-Added Tax Return"
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
                "Draft was not saved because one or more numeric/date fields are invalid."
                    .to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix the invalid 2550Q numeric/date fields before saving.",
            );
            return;
        }

        let Some(quarter) = self.draft.quarter_number() else {
            self.status_message = Some("Select a reviewed quarter before saving.".to_string());
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Select quarter 1, 2, 3, or 4 before saving 2550Q.",
            );
            return;
        };
        let period = FilingPeriod::Quarterly(quarter);
        let save_result = self
            .db
            .lock()
            .map_err(|_| "Draft database lock is unavailable".to_string())
            .and_then(|db| {
                db.save_form_draft_v2(
                    &self.draft.tin,
                    FORM_CODE,
                    self.draft.taxable_year,
                    &period,
                    &self.draft.status,
                    &self.draft,
                )
                .map_err(|error| error.to_string())
            });

        match save_result {
            Ok(id) => {
                self.draft.id = Some(id);
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
                    "2550Q April 2024 draft saved locally.",
                );
                cx.emit(Form2550QV2Event::Saved);
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save draft: {error}"));
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    format!("Could not save 2550Q draft: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "2550Qv2024 has reviewed editable-save evidence only. Electronic queue/submission is not certified."
                .to_string(),
        );
        cx.emit(Form2550QV2Event::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This 2550Q draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Payment status cannot advance automatically for a manual/external 2550Q filing."
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
        // Freeze the latest editor values into one immutable renderer envelope.
        // Preview never changes filing status or enables the submission queue.
        self.sync_from_inputs(cx);
        if !self.input_errors.is_empty() {
            let message =
                "HTML preview was not opened because one or more numeric/date fields are invalid."
                    .to_string();
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
                self.status_message = Some(format!(
                    "{} Preview is available for review; filing remains manual/external.",
                    launch_kind.status_message()
                ));
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

impl Render for Form2550QV2View {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = self.draft.is_editable();
        let mut basis_choices = div().flex().gap_2();
        for (label, value) in [
            ("Calendar", Form2550QFilingBasis::Calendar),
            ("Fiscal", Form2550QFilingBasis::Fiscal),
        ] {
            basis_choices = basis_choices.child(self.render_choice(
                format!("2550q_basis_{label}"),
                label.to_string(),
                self.draft.filing_basis == value,
                ChoiceAction::FilingBasis(value),
                cx,
            ));
        }

        let mut quarter_choices = div().flex().gap_2();
        for value in Form2550QQuarter::ALL {
            quarter_choices = quarter_choices.child(self.render_choice(
                format!("2550q_quarter_{}", value.label()),
                value.label().to_string(),
                self.draft.quarter == value,
                ChoiceAction::Quarter(value),
                cx,
            ));
        }

        let mut class_choices = div().flex().gap_2();
        for value in Form2550QTaxpayerClassification::ALL {
            class_choices = class_choices.child(self.render_choice(
                format!("2550q_class_{}", value.label()),
                value.label().to_string(),
                self.draft.taxpayer_classification == Some(value),
                ChoiceAction::Classification(value),
                cx,
            ));
        }

        let mut schedule_1 = section_card(cx, "SCHEDULE 1 — CAPITAL GOODS (2 reviewed rows)")
            .child(div().text_sm().child(
                "Allowable input tax is manual: the reviewed source does not contain the number-of-months-in-use input required to derive it safely. Additional sheets are not yet supported.",
            ));
        for (index, row) in self.schedule_1_inputs.iter().enumerate() {
            schedule_1 = schedule_1.child(self.render_capital_good_row(index, row, cx));
        }

        let mut schedule_3 =
            section_card(cx, "SCHEDULE 3 — CREDITABLE VAT WITHHELD (2 reviewed rows)");
        for (index, row) in self.schedule_3_inputs.iter().enumerate() {
            schedule_3 = schedule_3.child(self.render_creditable_vat_row(index, row, cx));
        }

        let mut schedule_4 =
            section_card(cx, "SCHEDULE 4 — ADVANCE VAT PAYMENTS (2 reviewed rows)");
        for (index, row) in self.schedule_4_inputs.iter().enumerate() {
            schedule_4 = schedule_4.child(self.render_advance_vat_row(index, row, cx));
        }

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
                    .child(div().font_weight(FontWeight::BOLD).child("Evidence boundary"))
                    .child(div().mt_1().text_sm().child(
                        "The April 2024 official form, guidelines, and two 160-field saves prove local draft/XML behavior. Queueing, submission transport, HTML parity, additional schedule sheets, and native PDF output remain disabled until separately certified.",
                    )),
            );
        if let Some(message) = &self.status_message {
            content = content.child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().muted.opacity(0.5))
                    .child(message.clone()),
            );
        }
        content = content
            .child(self.render_error_summary(cx))
            .child(
                section_card(cx, "FILING PERIOD AND BACKGROUND INFORMATION")
                    .child(div().text_sm().child(format!(
                        "{} · TIN {} · RDO {}",
                        self.draft.taxpayer_name, self.draft.tin, self.draft.rdo_code
                    )))
                    .child(div().text_sm().child(format!(
                        "{} · ZIP {} · {} · {}",
                        self.draft.registered_address,
                        self.draft.zip_code,
                        self.draft.contact_number,
                        self.draft.email
                    )))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Filing basis"))
                    .child(basis_choices)
                    .child(self.render_input_row("Year-end month", YEAR_END_MONTH))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Quarter"))
                    .child(quarter_choices)
                    .child(self.render_input_row("Return period from", RETURN_PERIOD_FROM))
                    .child(self.render_input_row("Return period to", RETURN_PERIOD_TO))
                    .child(self.render_choice(
                        "2550q_amended".to_string(),
                        format!("Amended return: {}", yes_no(self.draft.is_amended)),
                        self.draft.is_amended,
                        ChoiceAction::ToggleAmended,
                        cx,
                    ))
                    .child(self.render_choice(
                        "2550q_short_period".to_string(),
                        format!("Short-period return: {}", yes_no(self.draft.is_short_period_return)),
                        self.draft.is_short_period_return,
                        ChoiceAction::ToggleShortPeriod,
                        cx,
                    ))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Item 13 taxpayer classification"))
                    .child(class_choices)
                    .child(self.render_choice(
                        "2550q_relief".to_string(),
                        format!("Item 14 tax relief: {}", yes_no(self.draft.is_availing_tax_relief)),
                        self.draft.is_availing_tax_relief,
                        ChoiceAction::ToggleTaxRelief,
                        cx,
                    ))
                    .child(self.render_input_row("Item 14A special law / treaty", TAX_RELIEF_DETAILS)),
            )
            .child(
                section_card(cx, "PART IV — SALES AND OUTPUT TAX")
                    .child(self.render_input_row("31A Vatable sales/receipts", ITEM_31A))
                    .child(self.render_computed_row("31B Output tax (12%)", self.draft.part_iv.item_31b_output_tax, cx))
                    .child(self.render_input_row("32A Zero-rated sales/receipts", ITEM_32A))
                    .child(self.render_input_row("33A VAT-exempt sales/receipts", ITEM_33A))
                    .child(self.render_computed_row("34A Total sales/receipts", self.draft.part_iv.item_34a_total_sales, cx))
                    .child(self.render_computed_row("34B Output tax due", self.draft.part_iv.item_34b_output_tax_due, cx))
                    .child(self.render_input_row("35B Less: output VAT on uncollected receivables", ITEM_35B))
                    .child(self.render_input_row("36B Add: output VAT recovered", ITEM_36B))
                    .child(self.render_computed_row("37B Adjusted output tax due", self.draft.part_iv.item_37b_adjusted_output_tax_due, cx))
                    .child(self.render_input_row("38B Input tax carried over", ITEM_38B))
                    .child(self.render_computed_row("39B Deferred input tax from Schedule 1", self.draft.part_iv.item_39b_input_tax_deferred, cx))
                    .child(self.render_input_row("40B Transitional input tax", ITEM_40B))
                    .child(self.render_input_row("41B Presumptive input tax", ITEM_41B))
                    .child(self.render_input_row("42 Specify other input tax", ITEM_42_DESCRIPTION))
                    .child(self.render_input_row("42B Other input tax", ITEM_42B))
                    .child(self.render_computed_row("43B Total prior-period input tax", self.draft.part_iv.item_43b_total_prior_input_tax, cx)),
            )
            .child(
                section_card(cx, "PART IV — CURRENT PURCHASES AND ADJUSTMENTS")
                    .child(self.render_input_row("44A Domestic purchases", ITEM_44A))
                    .child(self.render_input_row("44B Domestic input tax", ITEM_44B))
                    .child(self.render_input_row("45A Services by non-residents", ITEM_45A))
                    .child(self.render_input_row("45B Non-resident service input tax", ITEM_45B))
                    .child(self.render_input_row("46A Importations", ITEM_46A))
                    .child(self.render_input_row("46B Import input tax", ITEM_46B))
                    .child(self.render_input_row("47 Specify other purchases", ITEM_47_DESCRIPTION))
                    .child(self.render_input_row("47A Other purchases", ITEM_47A))
                    .child(self.render_input_row("47B Other input tax", ITEM_47B))
                    .child(self.render_input_row("48A Domestic purchases without input tax", ITEM_48A))
                    .child(self.render_input_row("49A VAT-exempt importations", ITEM_49A))
                    .child(self.render_computed_row("50A Total current purchases", self.draft.part_iv.item_50a_total_current_purchases, cx))
                    .child(self.render_computed_row("50B Total current input tax", self.draft.part_iv.item_50b_total_current_input_tax, cx))
                    .child(self.render_computed_row("51B Total available input tax", self.draft.part_iv.item_51b_total_available_input_tax, cx))
                    .child(self.render_computed_row("52B Deferred capital-goods input tax", self.draft.part_iv.item_52b_deferred_capital_goods_input_tax, cx))
                    .child(self.render_computed_row("53B Input tax attributable to exempt sales", self.draft.part_iv.item_53b_input_tax_attributable_to_exempt_sales, cx))
                    .child(self.render_input_row("54B VAT refund/TCC claimed", ITEM_54B))
                    .child(self.render_input_row("55B Input VAT on unpaid payables", ITEM_55B))
                    .child(self.render_input_row("56 Specify other deduction", ITEM_56_DESCRIPTION))
                    .child(self.render_input_row("56B Other deduction", ITEM_56B))
                    .child(self.render_computed_row("57B Total deductions", self.draft.part_iv.item_57b_total_deductions, cx))
                    .child(self.render_input_row("58B Input VAT on settled payables", ITEM_58B))
                    .child(self.render_computed_row("59B Adjusted deductions", self.draft.part_iv.item_59b_adjusted_deductions, cx))
                    .child(self.render_computed_row("60B Total allowable input tax", self.draft.part_iv.item_60b_total_allowable_input_tax, cx))
                    .child(self.render_computed_row("61B Net VAT payable/(excess)", self.draft.part_iv.item_61b_net_vat_payable_or_excess, cx)),
            )
            .child(schedule_1)
            .child(
                section_card(cx, "SCHEDULE 2 — INPUT TAX ATTRIBUTABLE TO EXEMPT SALES")
                    .child(self.render_input_row("Directly attributable input tax", SCHEDULE_2_DIRECT))
                    .child(self.render_input_row("VAT-exempt sales", SCHEDULE_2_EXEMPT_SALES))
                    .child(self.render_input_row("Input tax not directly attributable", SCHEDULE_2_NOT_DIRECT))
                    .child(self.render_computed_row("Total sales", self.draft.schedule_2.total_sales, cx))
                    .child(self.render_computed_row("Ratable input tax", self.draft.schedule_2.ratable_input_tax, cx))
                    .child(self.render_computed_row("Total attributable to exempt sales", self.draft.schedule_2.total_input_tax_attributable_to_exempt_sales, cx)),
            )
            .child(schedule_3)
            .child(schedule_4)
            .child(
                section_card(cx, "PART II — TOTAL TAX PAYABLE")
                    .child(self.render_computed_row("15 Net VAT payable/(excess)", self.draft.part_ii.item_15_net_vat_payable_or_excess, cx))
                    .child(self.render_computed_row("16 Creditable VAT withheld (Schedule 3)", self.draft.part_ii.item_16_creditable_vat_withheld, cx))
                    .child(self.render_computed_row("17 Advance VAT payments (Schedule 4)", self.draft.part_ii.item_17_advance_vat_payments, cx))
                    .child(self.render_input_row("18 Paid on previous return (amended only)", ITEM_18))
                    .child(self.render_input_row("19 Specify other credit/payment", ITEM_19_DESCRIPTION))
                    .child(self.render_input_row("19 Other credit/payment", ITEM_19))
                    .child(self.render_computed_row("20 Total credits/payments", self.draft.part_ii.item_20_total_credits_or_payments, cx))
                    .child(self.render_computed_row("21 Tax payable/(excess credits)", self.draft.part_ii.item_21_tax_payable_or_excess_credits, cx))
                    .child(self.render_input_row("22 Surcharge (manual)", ITEM_22))
                    .child(self.render_input_row("23 Interest (manual)", ITEM_23))
                    .child(self.render_input_row("24 Compromise (manual)", ITEM_24))
                    .child(self.render_computed_row("25 Total penalties", self.draft.part_ii.item_25_total_penalties, cx))
                    .child(self.render_computed_row("26 Total amount payable/(excess)", self.draft.part_ii.item_26_total_amount_payable_or_excess, cx)),
            )
            .child(
                section_card(cx, "SIGNATURE AND LOCAL PAYMENT DETAILS")
                    .child(div().text_sm().child("These local print fields are not present in the reviewed 160-field XML payload."))
                    .child(self.render_input_row("Taxpayer / authorized representative", SIGNATORY))
                    .child(self.render_input_row("Title / designation", SIGNATORY_TITLE))
                    .child(self.render_input_row("Non-individual authorized officer", NON_INDIVIDUAL_OFFICER))
                    .child(self.render_input_row("Tax-agent accreditation / roll no.", TAX_AGENT_NUMBER))
                    .child(self.render_input_row("Tax-agent date of issue", TAX_AGENT_ISSUE))
                    .child(self.render_input_row("Tax-agent expiry", TAX_AGENT_EXPIRY))
                    .child(self.render_input_row("Cash/bank debit advice amount", CASH_AMOUNT))
                    .child(self.render_input_row("Check bank", CHECK_BANK))
                    .child(self.render_input_row("Check number", CHECK_NUMBER))
                    .child(self.render_input_row("Check date", CHECK_DATE))
                    .child(self.render_input_row("Check amount", CHECK_AMOUNT))
                    .child(self.render_input_row("Tax debit memo number", TDM_NUMBER))
                    .child(self.render_input_row("Tax debit memo date", TDM_DATE))
                    .child(self.render_input_row("Tax debit memo amount", TDM_AMOUNT))
                    .child(self.render_input_row("Other payment description", OTHER_PAYMENT_DESCRIPTION))
                    .child(self.render_input_row("Other payment bank/agency", OTHER_PAYMENT_BANK))
                    .child(self.render_input_row("Other payment number", OTHER_PAYMENT_NUMBER))
                    .child(self.render_input_row("Other payment date", OTHER_PAYMENT_DATE))
                    .child(self.render_input_row("Other payment amount", OTHER_PAYMENT_AMOUNT))
                    .child(self.render_input_row("Machine validation / receipt details", MACHINE_VALIDATION)),
            );

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
                        gpui_component::button::Button::new("2550q_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form2550QV2Event::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("2550q_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("2550q_revert")
                                    .label("Revert to Draft")
                                    .outline()
                                    .disabled(is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("2550q_manual")
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
                    .id("2550q_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(content),
            )
    }
}

fn section_card(cx: &Context<Form2550QV2View>, title: &str) -> Div {
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

fn row_card(cx: &Context<Form2550QV2View>, title: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .child(div().font_weight(FontWeight::BOLD).child(title.to_string()))
}

fn input_pair(label: &str, input: &Entity<InputState>, editable: bool) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(div().w_1_2().text_sm().child(label.to_string()))
        .child(div().w_1_2().child(Input::new(input).disabled(!editable)))
        .into_any_element()
}

fn text_input(
    cx: &mut Context<Form2550QV2View>,
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
    cx: &mut Context<Form2550QV2View>,
    value: Option<f64>,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "0.00", window);
    if let Some(value) = value {
        input.update(cx, |state, cx| {
            state.set_value(format!("{value:.2}"), window, cx)
        });
    }
    input
}

fn insert_text(
    fields: &mut BTreeMap<&'static str, Entity<InputState>>,
    key: &'static str,
    value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) {
    fields.insert(key, text_input(cx, value, placeholder, window));
}

fn insert_money(
    fields: &mut BTreeMap<&'static str, Entity<InputState>>,
    key: &'static str,
    value: Option<f64>,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) {
    fields.insert(key, money_input(cx, value, window));
}

fn field<'a>(
    fields: &'a BTreeMap<&'static str, Entity<InputState>>,
    key: &'static str,
) -> &'a Entity<InputState> {
    fields
        .get(key)
        .unwrap_or_else(|| panic!("2550Q editor field {key} was not initialized"))
}

fn input_text(input: &Entity<InputState>, cx: &Context<Form2550QV2View>) -> String {
    input.read(cx).value().to_string()
}

fn parse_optional_money(
    input: &Entity<InputState>,
    field_name: &str,
    cx: &Context<Form2550QV2View>,
) -> Result<Option<f64>, (String, String)> {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        return Ok(None);
    }
    match raw.trim().replace(',', "").parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(Some(value)),
        _ => Err((
            field_name.to_string(),
            format!("Enter a finite numeric amount; {raw:?} is not accepted"),
        )),
    }
}

fn assign_money(
    target: &mut Option<f64>,
    input: &Entity<InputState>,
    field_name: &str,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    match parse_optional_money(input, field_name, cx) {
        Ok(value) => *target = value,
        Err(error) => errors.push(error),
    }
}

fn assign_required_u8(
    target: &mut u8,
    input: &Entity<InputState>,
    field_name: &str,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    match raw.trim().parse::<u8>() {
        Ok(value) => *target = value,
        Err(_) => errors.push((
            field_name.to_string(),
            format!("Enter a whole number; {raw:?} is not accepted"),
        )),
    }
}

fn assign_optional_u16(
    target: &mut Option<u16>,
    input: &Entity<InputState>,
    field_name: &str,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        *target = None;
        return;
    }
    match raw.trim().parse::<u16>() {
        Ok(value) => *target = Some(value),
        Err(_) => errors.push((
            field_name.to_string(),
            format!("Enter a whole number of months; {raw:?} is not accepted"),
        )),
    }
}

fn assign_return_date(
    target: &mut Option<Form2550QDate>,
    input: &Entity<InputState>,
    field_name: &str,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let raw = input_text(input, cx);
    if raw.trim().is_empty() {
        *target = None;
        return;
    }
    match Form2550QDate::parse_return_period(&raw) {
        Ok(value) => *target = Some(value),
        Err(message) => errors.push((field_name.to_string(), message)),
    }
}

fn capital_good_inputs(
    row: &Form2550QCapitalGoodRow,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> CapitalGoodInputs {
    CapitalGoodInputs {
        date: text_input(
            cx,
            &row.purchase_or_import_date
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        ),
        source_code: text_input(cx, &row.source_code, "D or I", window),
        description: text_input(cx, &row.description, "Capital good", window),
        purchase_amount: money_input(cx, row.purchase_or_import_amount, window),
        input_tax: money_input(cx, row.input_tax, window),
        estimated_life: text_input(
            cx,
            &row.estimated_life_months
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "Months",
            window,
        ),
        recognized_life: text_input(
            cx,
            &row.recognized_life_months
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "Months",
            window,
        ),
        allowable_input_tax: money_input(cx, row.allowable_input_tax_for_period, window),
        balance_next_period: money_input(cx, row.balance_to_next_period, window),
    }
}

fn creditable_vat_inputs(
    row: &Form2550QCreditableVatRow,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> CreditableVatInputs {
    CreditableVatInputs {
        period_from: text_input(
            cx,
            &row.period_from
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        ),
        period_to: text_input(
            cx,
            &row.period_to
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        ),
        agent_name: text_input(cx, &row.withholding_agent_name, "Withholding agent", window),
        income_payment: money_input(cx, row.income_payment, window),
        tax_withheld: money_input(cx, row.tax_withheld, window),
    }
}

fn advance_vat_inputs(
    row: &Form2550QAdvanceVatRow,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> AdvanceVatInputs {
    AdvanceVatInputs {
        period_from: text_input(
            cx,
            &row.period_from
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        ),
        period_to: text_input(
            cx,
            &row.period_to
                .map(|value| value.to_string())
                .unwrap_or_default(),
            "MM/DD/YYYY",
            window,
        ),
        miller_name: text_input(cx, &row.miller_name, "Miller", window),
        taxpayer_name: text_input(cx, &row.taxpayer_name, "Taxpayer", window),
        receipt_number: text_input(cx, &row.official_receipt_number, "Receipt no.", window),
        amount_paid: money_input(cx, row.amount_paid, window),
    }
}

fn sync_capital_good_row(
    row: &mut Form2550QCapitalGoodRow,
    inputs: &CapitalGoodInputs,
    index: usize,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule_1[{index}]");
    assign_return_date(
        &mut row.purchase_or_import_date,
        &inputs.date,
        &format!("{prefix}.date"),
        cx,
        errors,
    );
    row.source_code = input_text(&inputs.source_code, cx);
    row.description = input_text(&inputs.description, cx);
    assign_money(
        &mut row.purchase_or_import_amount,
        &inputs.purchase_amount,
        &format!("{prefix}.purchase_amount"),
        cx,
        errors,
    );
    assign_money(
        &mut row.input_tax,
        &inputs.input_tax,
        &format!("{prefix}.input_tax"),
        cx,
        errors,
    );
    assign_optional_u16(
        &mut row.estimated_life_months,
        &inputs.estimated_life,
        &format!("{prefix}.estimated_life"),
        cx,
        errors,
    );
    assign_optional_u16(
        &mut row.recognized_life_months,
        &inputs.recognized_life,
        &format!("{prefix}.recognized_life"),
        cx,
        errors,
    );
    assign_money(
        &mut row.allowable_input_tax_for_period,
        &inputs.allowable_input_tax,
        &format!("{prefix}.allowable_input_tax"),
        cx,
        errors,
    );
    assign_money(
        &mut row.balance_to_next_period,
        &inputs.balance_next_period,
        &format!("{prefix}.balance_next_period"),
        cx,
        errors,
    );
}

fn sync_creditable_vat_row(
    row: &mut Form2550QCreditableVatRow,
    inputs: &CreditableVatInputs,
    index: usize,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule_3[{index}]");
    assign_return_date(
        &mut row.period_from,
        &inputs.period_from,
        &format!("{prefix}.period_from"),
        cx,
        errors,
    );
    assign_return_date(
        &mut row.period_to,
        &inputs.period_to,
        &format!("{prefix}.period_to"),
        cx,
        errors,
    );
    row.withholding_agent_name = input_text(&inputs.agent_name, cx);
    assign_money(
        &mut row.income_payment,
        &inputs.income_payment,
        &format!("{prefix}.income_payment"),
        cx,
        errors,
    );
    assign_money(
        &mut row.tax_withheld,
        &inputs.tax_withheld,
        &format!("{prefix}.tax_withheld"),
        cx,
        errors,
    );
}

fn sync_advance_vat_row(
    row: &mut Form2550QAdvanceVatRow,
    inputs: &AdvanceVatInputs,
    index: usize,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule_4[{index}]");
    assign_return_date(
        &mut row.period_from,
        &inputs.period_from,
        &format!("{prefix}.period_from"),
        cx,
        errors,
    );
    assign_return_date(
        &mut row.period_to,
        &inputs.period_to,
        &format!("{prefix}.period_to"),
        cx,
        errors,
    );
    row.miller_name = input_text(&inputs.miller_name, cx);
    row.taxpayer_name = input_text(&inputs.taxpayer_name, cx);
    row.official_receipt_number = input_text(&inputs.receipt_number, cx);
    assign_money(
        &mut row.amount_paid,
        &inputs.amount_paid,
        &format!("{prefix}.amount_paid"),
        cx,
        errors,
    );
}

fn format_optional_money(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_string(), |amount| format!("₱ {amount:.2}"))
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}
