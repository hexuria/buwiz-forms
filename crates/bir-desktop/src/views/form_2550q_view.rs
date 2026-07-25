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
use bir_core::form_rules::{
    Form2550QLiveValidationFacade, Form2550QRawFieldKey, Form2550QRepoDiagnosticSetupOutcome,
    Form2550QRepoLiveValidationState, InputRevision, RawValue, StableInstanceId,
    prepare_form_2550q_live_row_identities,
};
use bir_core::forms::form_2550q::{
    FORM_CODE, FORM_VERSION_LABEL, Form2550QAdvanceVatRow, Form2550QCapitalGoodRow,
    Form2550QCreditableVatRow, Form2550QDate, Form2550QDraft, Form2550QFilingBasis,
    Form2550QQuarter, Form2550QRowFamily, Form2550QTaxpayerClassification,
};
use bir_core::forms::{FilingPeriod, FilingStatus, FormValidator};
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;
use crate::components::form_validation::SemanticFieldTargets;

const YEAR_END_MONTH: &str = "year_end_month";
const RAW_TAXABLE_YEAR: &str = "raw_taxable_year";
const RETURN_PERIOD_FROM: &str = "return_period_from";
const RETURN_PERIOD_TO: &str = "return_period_to";
const TAX_RELIEF_DETAILS: &str = "tax_relief_details";
const FILING_BASIS_CALENDAR: &str = "filing_basis_calendar";
const FILING_BASIS_FISCAL: &str = "filing_basis_fiscal";
const QUARTER_1: &str = "quarter_1";
const QUARTER_2: &str = "quarter_2";
const QUARTER_3: &str = "quarter_3";
const QUARTER_4: &str = "quarter_4";
const AMENDED_YES: &str = "amended_yes";
const AMENDED_NO: &str = "amended_no";
const SHORT_PERIOD_YES: &str = "short_period_yes";
const SHORT_PERIOD_NO: &str = "short_period_no";
const CLASSIFICATION_1: &str = "classification_1";
const CLASSIFICATION_2: &str = "classification_2";
const CLASSIFICATION_3: &str = "classification_3";
const CLASSIFICATION_4: &str = "classification_4";
const TAX_RELIEF_YES: &str = "tax_relief_yes";
const TAX_RELIEF_NO: &str = "tax_relief_no";
const RAW_TIN_1: &str = "raw_tin_1";
const RAW_TIN_2: &str = "raw_tin_2";
const RAW_TIN_3: &str = "raw_tin_3";
const RAW_BRANCH_CODE: &str = "raw_branch_code";
const RAW_RDO_CODE: &str = "raw_rdo_code";
const RAW_TAXPAYER_NAME: &str = "raw_taxpayer_name";
const RAW_TAXPAYER_ADDRESS: &str = "raw_taxpayer_address";
const RAW_TAXPAYER_ZIP: &str = "raw_taxpayer_zip";
const RAW_TAXPAYER_CONTACT_NUMBER: &str = "raw_taxpayer_contact_number";
const RAW_TAXPAYER_EMAIL_ADDRESS: &str = "raw_taxpayer_email_address";

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
    instance_id: StableInstanceId,
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
    fn bound_inputs(&self) -> Vec<(Entity<InputState>, RawControlBinding)> {
        vec![
            self.bound(&self.date, "txtDatePurchase1"),
            self.bound(&self.source_code, "txtSourceCode1"),
            self.bound(&self.description, "txtDescription1"),
            self.bound(&self.purchase_amount, "txtAmountPurchase1"),
            self.bound(&self.input_tax, "txtInputTax1"),
            self.bound(&self.estimated_life, "txtEstimatedLife1"),
            self.bound(&self.recognized_life, "txtRecognizedLife1"),
            self.bound(&self.allowable_input_tax, "txtAllowedInputTax1"),
            self.bound(&self.balance_next_period, "txtBalanceInputTax1"),
        ]
    }

    fn bound(
        &self,
        input: &Entity<InputState>,
        member_key: &'static str,
    ) -> (Entity<InputState>, RawControlBinding) {
        (
            input.clone(),
            RawControlBinding::Repeated {
                family: Form2550QRowFamily::Schedule1,
                instance_id: self.instance_id.clone(),
                member_key,
            },
        )
    }
}

#[derive(Clone)]
struct CreditableVatInputs {
    instance_id: StableInstanceId,
    period_from: Entity<InputState>,
    period_to: Entity<InputState>,
    agent_name: Entity<InputState>,
    income_payment: Entity<InputState>,
    tax_withheld: Entity<InputState>,
}

impl CreditableVatInputs {
    fn bound_inputs(&self) -> Vec<(Entity<InputState>, RawControlBinding)> {
        vec![
            self.bound(&self.period_from, "txtDateCovered3"),
            self.bound(&self.period_to, "txtDateCovered3To"),
            self.bound(&self.agent_name, "txtNameWithHoldingAgent3"),
            self.bound(&self.income_payment, "txtIncomePayment3"),
            self.bound(&self.tax_withheld, "txtTotalTaxWithHeld3"),
        ]
    }

    fn bound(
        &self,
        input: &Entity<InputState>,
        member_key: &'static str,
    ) -> (Entity<InputState>, RawControlBinding) {
        (
            input.clone(),
            RawControlBinding::Repeated {
                family: Form2550QRowFamily::Schedule3,
                instance_id: self.instance_id.clone(),
                member_key,
            },
        )
    }
}

#[derive(Clone)]
struct AdvanceVatInputs {
    instance_id: StableInstanceId,
    period_from: Entity<InputState>,
    period_to: Entity<InputState>,
    miller_name: Entity<InputState>,
    taxpayer_name: Entity<InputState>,
    receipt_number: Entity<InputState>,
    amount_paid: Entity<InputState>,
}

impl AdvanceVatInputs {
    fn bound_inputs(&self) -> Vec<(Entity<InputState>, RawControlBinding)> {
        vec![
            self.bound(&self.period_from, "txtDate4"),
            self.bound(&self.period_to, "txtDate4To"),
            self.bound(&self.miller_name, "txtNameOfMiller4"),
            self.bound(&self.taxpayer_name, "txtNameOfTaxpayer4"),
            self.bound(&self.receipt_number, "txtOfficialReceiptNumber4"),
            self.bound(&self.amount_paid, "txtAmountPaid4"),
        ]
    }

    fn bound(
        &self,
        input: &Entity<InputState>,
        member_key: &'static str,
    ) -> (Entity<InputState>, RawControlBinding) {
        (
            input.clone(),
            RawControlBinding::Repeated {
                family: Form2550QRowFamily::Schedule4,
                instance_id: self.instance_id.clone(),
                member_key,
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RawControlBinding {
    Singleton {
        ui_key: &'static str,
        raw_key: &'static str,
    },
    Repeated {
        family: Form2550QRowFamily,
        instance_id: StableInstanceId,
        member_key: &'static str,
    },
}

impl RawControlBinding {
    fn raw_field_key(&self) -> Result<Form2550QRawFieldKey, String> {
        match self {
            Self::Singleton { raw_key, .. } => Form2550QRawFieldKey::try_singleton(raw_key),
            Self::Repeated {
                family,
                instance_id,
                member_key,
            } => Form2550QRawFieldKey::try_repeated(*family, instance_id.clone(), member_key),
        }
        .map_err(|error| error.to_string())
    }

    fn is_parsed(&self) -> bool {
        match self {
            Self::Singleton { ui_key, .. } => singleton_is_parsed(ui_key),
            Self::Repeated { member_key, .. } => repeated_member_is_parsed(member_key),
        }
    }
}

#[derive(Clone, Copy)]
enum ChoiceAction {
    FilingBasis(Form2550QFilingBasis),
    Quarter(Form2550QQuarter),
    Classification(Form2550QTaxpayerClassification),
    Amended(bool),
    ShortPeriod(bool),
    TaxRelief(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Form2550QDiagnosticState {
    NotRun,
    Unavailable(String),
    Incomplete(String),
}

pub struct Form2550QV2View {
    draft: Form2550QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    editor_state_error: Option<String>,
    status_message: Option<String>,
    fields: BTreeMap<&'static str, Entity<InputState>>,
    candidate_choice_fields: BTreeMap<&'static str, Entity<InputState>>,
    candidate_raw_text_fields: BTreeMap<&'static str, Entity<InputState>>,
    schedule_1_inputs: Vec<CapitalGoodInputs>,
    schedule_3_inputs: Vec<CreditableVatInputs>,
    schedule_4_inputs: Vec<AdvanceVatInputs>,
    validation_setup: Form2550QRepoDiagnosticSetupOutcome,
    validation_state: Form2550QDiagnosticState,
    validation_input_revision: InputRevision,
    semantic_field_targets: SemanticFieldTargets<Entity<InputState>>,
    semantic_focus_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl Form2550QV2View {
    pub fn new(
        mut draft: Form2550QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut editor_state_error = prepare_form_2550q_live_row_identities(&mut draft)
            .err()
            .map(|error| format!("Persisted raw editor identity is unsafe: {error}"));
        let mut migrated_legacy_draft = false;
        if editor_state_error.is_none() {
            match draft.migrate_legacy_flat_draft() {
                Ok(migrated) => migrated_legacy_draft = migrated,
                Err(error) => {
                    editor_state_error = Some(format!(
                        "Persisted legacy draft shape is ambiguous: {error}"
                    ));
                }
            }
        }
        if editor_state_error.is_none()
            && let Err(error) = prepare_form_2550q_live_row_identities(&mut draft)
        {
            editor_state_error = Some(format!("Persisted raw editor identity is unsafe: {error}"));
        }
        if editor_state_error.is_none() {
            editor_state_error = raw_editor_binding_safety_error(&draft);
        }
        let mut fields = BTreeMap::new();
        insert_text(
            &mut fields,
            YEAR_END_MONTH,
            &draft.year_end_month.to_string(),
            "1-12",
            window,
            cx,
        );
        // Item 2 is a candidate-only raw input. Do not initialize it from
        // draft.taxable_year: a typed projection is not evidence of the
        // exact bytes present in the live control.
        insert_text(&mut fields, RAW_TAXABLE_YEAR, "", "YYYY", window, cx);
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

        // These sixteen candidate booleans intentionally start blank. A typed
        // draft choice is not evidence of the raw bytes that produced it.
        // Existing raw state is restored below, and an explicit radio click
        // materializes the complete mutually-exclusive group.
        let candidate_choice_fields = candidate_choice_inputs(window, cx);
        // Profile projections are not evidence of live candidate bytes.
        // Reviewed imports restore exact raw state below; new and legacy
        // drafts intentionally keep these four controls blank.
        let candidate_raw_text_fields = candidate_raw_text_inputs(window, cx);

        let (schedule_1_inputs, schedule_3_inputs, schedule_4_inputs) =
            if editor_state_error.is_none() {
                (
                    draft
                        .schedule_1
                        .iter()
                        .map(|row| capital_good_inputs(row, window, cx))
                        .collect::<Vec<_>>(),
                    draft
                        .schedule_3
                        .iter()
                        .map(|row| creditable_vat_inputs(row, window, cx))
                        .collect::<Vec<_>>(),
                    draft
                        .schedule_4
                        .iter()
                        .map(|row| advance_vat_inputs(row, window, cx))
                        .collect::<Vec<_>>(),
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        if editor_state_error.is_none() {
            restore_singleton_raw_values(&fields, &draft.raw_editor_state, window, cx);
            restore_singleton_raw_values(
                &candidate_choice_fields,
                &draft.raw_editor_state,
                window,
                cx,
            );
            restore_singleton_raw_values(
                &candidate_raw_text_fields,
                &draft.raw_editor_state,
                window,
                cx,
            );
            restore_repeated_raw_values(
                &schedule_1_inputs,
                &schedule_3_inputs,
                &schedule_4_inputs,
                &draft.raw_editor_state,
                window,
                cx,
            );
        }

        let mut subscribed_inputs = if editor_state_error.is_none() {
            let mut inputs = singleton_bound_inputs(&fields);
            inputs.extend(singleton_bound_inputs(&candidate_raw_text_fields));
            inputs
        } else {
            Vec::new()
        };
        for row in &schedule_1_inputs {
            subscribed_inputs.extend(row.bound_inputs());
        }
        for row in &schedule_3_inputs {
            subscribed_inputs.extend(row.bound_inputs());
        }
        for row in &schedule_4_inputs {
            subscribed_inputs.extend(row.bound_inputs());
        }

        // Candidate choice buffers are changed only by apply_choice, which
        // updates the whole radio group and advances the revision exactly
        // once. They still participate in raw capture and semantic focus
        // mapping as live InputState-backed controls.
        let mut semantic_inputs = subscribed_inputs.clone();
        if editor_state_error.is_none() {
            semantic_inputs.extend(singleton_bound_inputs(&candidate_choice_fields));
        }
        let (semantic_field_targets, semantic_focus_error) =
            match semantic_targets_from_raw_controls(&semantic_inputs) {
                Ok(targets) => (targets, None),
                Err(error) => (
                    SemanticFieldTargets::default(),
                    Some(format!(
                        "Validation focus mapping is unavailable: {error}. No fallback field will be focused."
                    )),
                ),
            };

        let mut subscriptions = Vec::new();
        for (input, binding) in subscribed_inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.raw_control_changed();
                        this.sync_from_inputs(Some(std::slice::from_ref(&binding)), cx);
                    }
                },
            ));
        }

        let validation_setup = Form2550QLiveValidationFacade::setup_repo_default_diagnostic();
        let validation_state = diagnostic_state_from_setup(&validation_setup);
        let validation_errors = draft.validate();
        let status_message = editor_state_error.as_ref().map(|error| {
            format!(
                "2550Q editor is locked because persisted control identity could not be validated. No positional fallback was used. {error}"
            )
        }).or_else(|| migrated_legacy_draft.then(|| {
            "The scaffold-era 2550Q draft was migrated to the reviewed April 2024 model. Review any migration warnings before filing externally."
                .to_string()
        }));
        let mut view = Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            input_errors: Vec::new(),
            validation_errors,
            editor_state_error,
            status_message,
            fields,
            candidate_choice_fields,
            candidate_raw_text_fields,
            schedule_1_inputs,
            schedule_3_inputs,
            schedule_4_inputs,
            validation_setup,
            validation_state,
            validation_input_revision: InputRevision::default(),
            semantic_field_targets,
            semantic_focus_error,
            _subscriptions: subscriptions,
        };
        if view.editor_state_error.is_none() {
            view.sync_from_inputs(None, cx);
        }
        view
    }

    fn bound_inputs(&self) -> Vec<(Entity<InputState>, RawControlBinding)> {
        let mut inputs = singleton_bound_inputs(&self.fields);
        inputs.extend(singleton_bound_inputs(&self.candidate_raw_text_fields));
        inputs.extend(singleton_bound_inputs(&self.candidate_choice_fields));
        for row in &self.schedule_1_inputs {
            inputs.extend(row.bound_inputs());
        }
        for row in &self.schedule_3_inputs {
            inputs.extend(row.bound_inputs());
        }
        for row in &self.schedule_4_inputs {
            inputs.extend(row.bound_inputs());
        }
        inputs
    }

    /// Copy materialized editor bytes into the preservation boundary before
    /// any parser can update the typed draft. Untouched missing/Absent buffers
    /// remain uncaptured even though their controls may show typed fallback.
    fn capture_raw_before_parse(
        &mut self,
        changed: Option<&[RawControlBinding]>,
        cx: &Context<Self>,
    ) -> Result<Vec<(String, Form2550QRawFieldKey)>, String> {
        let visible_values = self
            .bound_inputs()
            .into_iter()
            .map(|(input, binding)| (binding, input_text(&input, cx)))
            .collect::<Vec<_>>();
        let mut parsed_paths = Vec::new();

        for (binding, raw) in visible_values {
            let existing = match &binding {
                RawControlBinding::Singleton { raw_key, .. } => {
                    self.draft.raw_editor_state.singleton_value(raw_key)
                }
                RawControlBinding::Repeated {
                    family,
                    instance_id,
                    member_key,
                } => self
                    .draft
                    .raw_editor_state
                    .repeated_value(*family, instance_id, member_key),
            };
            let was_changed = changed.is_some_and(|bindings| bindings.contains(&binding));
            if !should_capture_raw(existing, was_changed) {
                continue;
            }

            let raw_field_key = binding
                .is_parsed()
                .then(|| binding.raw_field_key())
                .transpose()?;
            match &binding {
                RawControlBinding::Singleton { raw_key, .. } => self
                    .draft
                    .raw_editor_state
                    .set_singleton(*raw_key, RawValue::Text(raw)),
                RawControlBinding::Repeated {
                    family,
                    instance_id,
                    member_key,
                } => self.draft.raw_editor_state.set_repeated(
                    *family,
                    instance_id.clone(),
                    *member_key,
                    RawValue::Text(raw),
                ),
            }
            if let Some(raw_field_key) = raw_field_key {
                parsed_paths.push((raw_field_key.stable_path(), raw_field_key));
            }
        }
        Ok(parsed_paths)
    }

    fn reconcile_raw_parse_results(&mut self, parsed_paths: Vec<(String, Form2550QRawFieldKey)>) {
        for (stable_path, raw_field_key) in parsed_paths {
            let message = self
                .input_errors
                .iter()
                .find(|(field, _)| field == &stable_path)
                .map(|(_, message)| message.clone());
            if let Some(message) = message {
                raw_field_key.mark_malformed(&mut self.draft.raw_editor_state, message);
            } else {
                raw_field_key.clear_malformed(&mut self.draft.raw_editor_state);
            }
        }
    }

    fn sync_from_inputs(&mut self, changed: Option<&[RawControlBinding]>, cx: &mut Context<Self>) {
        if self.editor_state_error.is_some() {
            cx.notify();
            return;
        }
        let parsed_paths = match self.capture_raw_before_parse(changed, cx) {
            Ok(paths) => paths,
            Err(error) => {
                let message = format!("Raw editor binding is unsafe: {error}");
                self.editor_state_error = Some(message.clone());
                self.status_message = Some(message);
                cx.notify();
                return;
            }
        };
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

        for inputs in &self.schedule_1_inputs {
            if let Some(row) = self
                .draft
                .schedule_1
                .iter_mut()
                .find(|row| row.instance_id.as_ref() == Some(&inputs.instance_id))
            {
                sync_capital_good_row(row, inputs, cx, &mut self.input_errors);
            } else {
                self.input_errors.push((
                    format!("schedule-1/{}", inputs.instance_id),
                    "Persisted row identity disappeared while the editor was open".to_string(),
                ));
            }
        }
        for inputs in &self.schedule_3_inputs {
            if let Some(row) = self
                .draft
                .schedule_3
                .iter_mut()
                .find(|row| row.instance_id.as_ref() == Some(&inputs.instance_id))
            {
                sync_creditable_vat_row(row, inputs, cx, &mut self.input_errors);
            } else {
                self.input_errors.push((
                    format!("schedule-3/{}", inputs.instance_id),
                    "Persisted row identity disappeared while the editor was open".to_string(),
                ));
            }
        }
        for inputs in &self.schedule_4_inputs {
            if let Some(row) = self
                .draft
                .schedule_4
                .iter_mut()
                .find(|row| row.instance_id.as_ref() == Some(&inputs.instance_id))
            {
                sync_advance_vat_row(row, inputs, cx, &mut self.input_errors);
            } else {
                self.input_errors.push((
                    format!("schedule-4/{}", inputs.instance_id),
                    "Persisted row identity disappeared while the editor was open".to_string(),
                ));
            }
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

        self.reconcile_raw_parse_results(parsed_paths);
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

    fn editor_is_editable(&self) -> bool {
        self.draft.is_editable() && self.editor_state_error.is_none()
    }

    fn raw_control_changed(&mut self) {
        match advance_input_revision(&mut self.validation_input_revision) {
            Ok(_) => {
                self.validation_state = diagnostic_state_from_setup(&self.validation_setup);
            }
            Err(()) => {
                self.validation_state = Form2550QDiagnosticState::Unavailable(
                    "Validation unavailable because the input revision counter is exhausted. Restart the editor before relying on any later diagnostic."
                        .to_string(),
                );
            }
        }
    }

    fn run_validation_diagnostic(&mut self, cx: &mut Context<Self>) {
        let setup = self.validation_setup.clone();
        match setup {
            Form2550QRepoDiagnosticSetupOutcome::Unavailable { reason } => {
                self.validation_state = Form2550QDiagnosticState::Unavailable(format!(
                    "Validation did not run: {reason}. This is not a valid or filing-ready result."
                ));
            }
            Form2550QRepoDiagnosticSetupOutcome::ExactRegistrationAvailable(diagnostic) => {
                if let Some(error) = &self.semantic_focus_error {
                    self.validation_state = Form2550QDiagnosticState::Unavailable(format!(
                        "{error} Validation did not run and no field was focused."
                    ));
                    cx.notify();
                    return;
                }
                if let Some(error) = &self.editor_state_error {
                    self.validation_state = Form2550QDiagnosticState::Unavailable(format!(
                        "Validation did not run because editor identity is unsafe: {error}"
                    ));
                    cx.notify();
                    return;
                }

                // Synchronization preserves raw bytes before parsing and does
                // not advance the user-edit revision.
                self.sync_from_inputs(None, cx);
                if let Some(error) = &self.editor_state_error {
                    self.validation_state = Form2550QDiagnosticState::Unavailable(format!(
                        "Validation did not run because raw capture failed closed: {error}"
                    ));
                    cx.notify();
                    return;
                }
                self.validation_state = match diagnostic.evaluate(&mut self.draft) {
                    Form2550QRepoLiveValidationState::Unavailable { reason, .. } => {
                        Form2550QDiagnosticState::Unavailable(format!(
                            "Validation did not run: {reason}. This is not a valid or filing-ready result."
                        ))
                    }
                    Form2550QRepoLiveValidationState::IncompleteCapture { gaps, .. } => {
                        Form2550QDiagnosticState::Incomplete(format!(
                            "Validation is incomplete at input revision {}: {} required raw capture gap(s) remain. No validity or filing-readiness conclusion was produced.",
                            self.validation_input_revision.get(),
                            gaps.len()
                        ))
                    }
                };
            }
        }
        cx.notify();
    }

    fn apply_choice(&mut self, action: ChoiceAction, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editor_is_editable() {
            return;
        }
        let candidate_updates = candidate_radio_values(action);
        for (ui_key, raw_value) in &candidate_updates {
            set_input_value(
                field(&self.candidate_choice_fields, ui_key),
                raw_value,
                window,
                cx,
            );
        }
        let changed_bindings = candidate_updates
            .iter()
            .map(|(ui_key, _)| RawControlBinding::Singleton {
                ui_key: *ui_key,
                raw_key: singleton_raw_key(ui_key),
            })
            .collect::<Vec<_>>();
        self.raw_control_changed();
        match action {
            ChoiceAction::FilingBasis(value) => self.draft.filing_basis = value,
            ChoiceAction::Quarter(value) => self.draft.quarter = value,
            ChoiceAction::Classification(value) => self.draft.taxpayer_classification = Some(value),
            ChoiceAction::Amended(value) => self.draft.is_amended = value,
            ChoiceAction::ShortPeriod(value) => self.draft.is_short_period_return = value,
            ChoiceAction::TaxRelief(value) => self.draft.is_availing_tax_relief = value,
        }
        if changed_bindings.is_empty() {
            self.sync_from_inputs(None, cx);
        } else {
            self.sync_from_inputs(Some(&changed_bindings), cx);
        }
    }

    fn render_choice(
        &self,
        id: String,
        label: String,
        selected: bool,
        action: ChoiceAction,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut choice = div()
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
            .cursor_pointer();
        if let Some(ui_key) = candidate_radio_ui_key(action) {
            choice =
                choice.track_focus(&field(&self.candidate_choice_fields, ui_key).focus_handle(cx));
        }
        choice
            .on_click(cx.listener(move |this, _, window, cx| {
                this.apply_choice(action, window, cx);
            }))
            .child(label)
            .into_any_element()
    }

    fn candidate_choice_is_selected(&self, action: ChoiceAction, cx: &Context<Self>) -> bool {
        candidate_radio_ui_key(action).is_some_and(|ui_key| {
            candidate_radio_raw_is_selected(&input_text(
                field(&self.candidate_choice_fields, ui_key),
                cx,
            ))
        })
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
                    Input::new(field(&self.fields, key)).disabled(!self.editor_is_editable()),
                ),
            )
            .into_any_element()
    }

    fn render_candidate_raw_text_row(&self, label: &str, key: &'static str) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().w_1_2().text_sm().child(label.to_string()))
            .child(
                div().w_1_2().child(
                    Input::new(field(&self.candidate_raw_text_fields, key))
                        .disabled(!self.editor_is_editable()),
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

    fn render_validation_diagnostic(&self, cx: &Context<Self>) -> AnyElement {
        let (title, message) = match &self.validation_state {
            Form2550QDiagnosticState::NotRun => (
                "Validation not run",
                "No validation conclusion is available. This does not mean the draft is valid or filing-ready."
                    .to_string(),
            ),
            Form2550QDiagnosticState::Unavailable(message) => {
                ("Validation unavailable", message.clone())
            }
            Form2550QDiagnosticState::Incomplete(message) => {
                ("Validation incomplete", message.clone())
            }
        };
        div()
            .p_4()
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().warning.opacity(0.1))
            .rounded_lg()
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(div().mt_1().text_sm().child(message))
            .into_any_element()
    }

    fn render_capital_good_row(
        &self,
        index: usize,
        row: &CapitalGoodInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        let editable = self.editor_is_editable();
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
        let editable = self.editor_is_editable();
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
        let editable = self.editor_is_editable();
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
        if let Some(error) = &self.editor_state_error {
            let message = format!(
                "Draft was not changed or saved because the editor identity boundary is unsafe. {error}"
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

        self.sync_from_inputs(None, cx);
        if let Some(error) = &self.editor_state_error {
            let message = format!(
                "Draft was not saved because a raw editor binding failed closed during synchronization. {error}"
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
        // Q0 is a local preservation bucket for an unresolved imported
        // quarter. It is deliberately not a valid filing period and cannot
        // authorize queueing, submission, or final-copy output.
        let quarter = self.draft.quarter_number().unwrap_or(0);
        let period = FilingPeriod::Quarterly(quarter);
        let has_unresolved_issues = !self.validation_errors.is_empty();
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
                self.status_message = Some(if !has_unresolved_issues {
                    "Draft saved locally for preservation. Filing and submission remain manual/external."
                        .to_string()
                } else {
                    "Draft saved locally for preservation with unresolved issues, including any malformed visible text shown below. Filing and submission remain disabled."
                        .to_string()
                });
                self.notify(
                    window,
                    cx,
                    if !has_unresolved_issues {
                        gpui_component::notification::NotificationType::Success
                    } else {
                        gpui_component::notification::NotificationType::Warning
                    },
                    if !has_unresolved_issues {
                        "2550Q April 2024 draft saved locally for preservation.".to_string()
                    } else {
                        "2550Q draft was preserved locally with unresolved issues.".to_string()
                    },
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
        if let Some(error) = &self.editor_state_error {
            let message = format!(
                "HTML preview was not opened because persisted editor identity is unsafe. {error}"
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
        self.sync_from_inputs(None, cx);
        if let Some(error) = &self.editor_state_error {
            let message = format!(
                "HTML preview was not opened because a raw editor binding failed closed during synchronization. {error}"
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
        let editor_is_editable = self.editor_is_editable();
        let mut basis_choices = div().flex().gap_2();
        for (label, value) in [
            ("Calendar", Form2550QFilingBasis::Calendar),
            ("Fiscal", Form2550QFilingBasis::Fiscal),
        ] {
            let action = ChoiceAction::FilingBasis(value);
            basis_choices = basis_choices.child(self.render_choice(
                format!("2550q_basis_{label}"),
                label.to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
                cx,
            ));
        }

        let mut quarter_choices = div().flex().gap_2();
        for value in Form2550QQuarter::ALL {
            let action = ChoiceAction::Quarter(value);
            quarter_choices = quarter_choices.child(self.render_choice(
                format!("2550q_quarter_{}", value.label()),
                value.label().to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
                cx,
            ));
        }

        let mut amended_choices = div().flex().gap_2();
        for (label, value) in [("Yes", true), ("No", false)] {
            let action = ChoiceAction::Amended(value);
            amended_choices = amended_choices.child(self.render_choice(
                format!("2550q_amended_{}", label.to_ascii_lowercase()),
                label.to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
                cx,
            ));
        }

        let mut short_period_choices = div().flex().gap_2();
        for (label, value) in [("Yes", true), ("No", false)] {
            let action = ChoiceAction::ShortPeriod(value);
            short_period_choices = short_period_choices.child(self.render_choice(
                format!("2550q_short_period_{}", label.to_ascii_lowercase()),
                label.to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
                cx,
            ));
        }

        let mut class_choices = div().flex().gap_2();
        for value in Form2550QTaxpayerClassification::ALL {
            let action = ChoiceAction::Classification(value);
            class_choices = class_choices.child(self.render_choice(
                format!("2550q_class_{}", value.label()),
                value.label().to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
                cx,
            ));
        }

        let mut relief_choices = div().flex().gap_2();
        for (label, value) in [("Yes", true), ("No", false)] {
            let action = ChoiceAction::TaxRelief(value);
            relief_choices = relief_choices.child(self.render_choice(
                format!("2550q_relief_{}", label.to_ascii_lowercase()),
                label.to_string(),
                self.candidate_choice_is_selected(action, cx),
                action,
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
        if let Some(error) = &self.editor_state_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .bg(cx.theme().danger.opacity(0.1))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Editor locked — unsafe persisted row identity"),
                    )
                    .child(div().mt_1().text_sm().child(format!(
                        "{error} The controls are read-only, repeated rows were not bound by position, and Save/Preview are disabled."
                    ))),
            );
        }
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
            .child(self.render_validation_diagnostic(cx))
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
                    .child(div().text_sm().child(
                        "Candidate raw identity/profile controls are blank on profile-derived drafts and restore only exact reviewed-import or explicitly captured text.",
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 6 TIN segment 1 (raw)",
                        RAW_TIN_1,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 6 TIN segment 2 (raw)",
                        RAW_TIN_2,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 6 TIN segment 3 (raw)",
                        RAW_TIN_3,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 6 branch code (raw)",
                        RAW_BRANCH_CODE,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 7 RDO code (raw)",
                        RAW_RDO_CODE,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 8 taxpayer name (raw)",
                        RAW_TAXPAYER_NAME,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 10 registered address (raw)",
                        RAW_TAXPAYER_ADDRESS,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 10A ZIP code (raw)",
                        RAW_TAXPAYER_ZIP,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 11 contact number (raw)",
                        RAW_TAXPAYER_CONTACT_NUMBER,
                    ))
                    .child(self.render_candidate_raw_text_row(
                        "Item 12 email address (raw)",
                        RAW_TAXPAYER_EMAIL_ADDRESS,
                    ))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Filing basis"))
                    .child(basis_choices)
                    .child(self.render_input_row("Year-end month", YEAR_END_MONTH))
                    .child(self.render_input_row("Item 2 taxable year (raw)", RAW_TAXABLE_YEAR))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Quarter"))
                    .child(quarter_choices)
                    .child(self.render_input_row("Return period from", RETURN_PERIOD_FROM))
                    .child(self.render_input_row("Return period to", RETURN_PERIOD_TO))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Amended return"))
                    .child(amended_choices)
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Short-period return"))
                    .child(short_period_choices)
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Item 13 taxpayer classification"))
                    .child(class_choices)
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Item 14 tax relief"))
                    .child(relief_choices)
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
                                gpui_component::button::Button::new("2550q_validate")
                                    .label("Validate")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_validation_diagnostic(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("2550q_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!editor_is_editable)
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

fn semantic_targets_from_raw_controls(
    controls: &[(Entity<InputState>, RawControlBinding)],
) -> Result<SemanticFieldTargets<Entity<InputState>>, String> {
    let mut targets = Vec::new();
    for (input, binding) in controls {
        let raw_key = binding.raw_field_key()?;
        if let Some(field) = raw_key.semantic_field_instance() {
            targets.push((field, input.clone()));
        }
    }
    SemanticFieldTargets::try_new(targets).map_err(|error| error.to_string())
}

fn diagnostic_state_from_setup(
    setup: &Form2550QRepoDiagnosticSetupOutcome,
) -> Form2550QDiagnosticState {
    match setup {
        Form2550QRepoDiagnosticSetupOutcome::ExactRegistrationAvailable(_) => {
            Form2550QDiagnosticState::NotRun
        }
        Form2550QRepoDiagnosticSetupOutcome::Unavailable { reason } => {
            Form2550QDiagnosticState::Unavailable(format!(
                "{reason}. No candidate validator was evaluated, and this does not mean the draft is valid or filing-ready."
            ))
        }
    }
}

fn advance_input_revision(revision: &mut InputRevision) -> Result<InputRevision, ()> {
    let next = revision.get().checked_add(1).ok_or(())?;
    *revision = InputRevision::new(next);
    Ok(*revision)
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

fn candidate_choice_inputs(
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> BTreeMap<&'static str, Entity<InputState>> {
    let mut fields = BTreeMap::new();
    for key in [
        FILING_BASIS_CALENDAR,
        FILING_BASIS_FISCAL,
        QUARTER_1,
        QUARTER_2,
        QUARTER_3,
        QUARTER_4,
        AMENDED_YES,
        AMENDED_NO,
        SHORT_PERIOD_YES,
        SHORT_PERIOD_NO,
        CLASSIFICATION_1,
        CLASSIFICATION_2,
        CLASSIFICATION_3,
        CLASSIFICATION_4,
        TAX_RELIEF_YES,
        TAX_RELIEF_NO,
    ] {
        insert_text(&mut fields, key, "", "", window, cx);
    }
    fields
}

fn candidate_raw_text_inputs(
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> BTreeMap<&'static str, Entity<InputState>> {
    let mut fields = BTreeMap::new();
    for (key, placeholder) in [
        (RAW_TIN_1, "Exact Item 6 TIN segment 1"),
        (RAW_TIN_2, "Exact Item 6 TIN segment 2"),
        (RAW_TIN_3, "Exact Item 6 TIN segment 3"),
        (RAW_BRANCH_CODE, "Exact Item 6 branch code"),
        (RAW_RDO_CODE, "Exact Item 7 RDO code"),
        (RAW_TAXPAYER_NAME, "Exact Item 8 taxpayer name"),
        (RAW_TAXPAYER_ADDRESS, "Exact Item 10 text"),
        (RAW_TAXPAYER_ZIP, "Exact Item 10A text"),
        (RAW_TAXPAYER_CONTACT_NUMBER, "Exact Item 11 text"),
        (RAW_TAXPAYER_EMAIL_ADDRESS, "Exact Item 12 text"),
    ] {
        insert_text(&mut fields, key, "", placeholder, window, cx);
    }
    fields
}

fn singleton_bound_inputs(
    fields: &BTreeMap<&'static str, Entity<InputState>>,
) -> Vec<(Entity<InputState>, RawControlBinding)> {
    fields
        .iter()
        .map(|(ui_key, input)| {
            (
                input.clone(),
                RawControlBinding::Singleton {
                    ui_key: *ui_key,
                    raw_key: singleton_raw_key(ui_key),
                },
            )
        })
        .collect()
}

fn candidate_radio_values(action: ChoiceAction) -> Vec<(&'static str, &'static str)> {
    match action {
        ChoiceAction::FilingBasis(selected) => [
            (
                FILING_BASIS_CALENDAR,
                if selected == Form2550QFilingBasis::Calendar {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                FILING_BASIS_FISCAL,
                if selected == Form2550QFilingBasis::Fiscal {
                    "true"
                } else {
                    "false"
                },
            ),
        ]
        .into(),
        ChoiceAction::Quarter(selected) => Form2550QQuarter::ALL
            .into_iter()
            .zip([QUARTER_1, QUARTER_2, QUARTER_3, QUARTER_4])
            .map(|(candidate, key)| {
                (
                    key,
                    if candidate == selected {
                        "true"
                    } else {
                        "false"
                    },
                )
            })
            .collect(),
        ChoiceAction::Amended(selected) => [
            (AMENDED_YES, if selected { "true" } else { "false" }),
            (AMENDED_NO, if selected { "false" } else { "true" }),
        ]
        .into(),
        ChoiceAction::ShortPeriod(selected) => [
            (SHORT_PERIOD_YES, if selected { "true" } else { "false" }),
            (SHORT_PERIOD_NO, if selected { "false" } else { "true" }),
        ]
        .into(),
        ChoiceAction::Classification(selected) => Form2550QTaxpayerClassification::ALL
            .into_iter()
            .zip([
                CLASSIFICATION_1,
                CLASSIFICATION_2,
                CLASSIFICATION_3,
                CLASSIFICATION_4,
            ])
            .map(|(candidate, key)| {
                (
                    key,
                    if candidate == selected {
                        "true"
                    } else {
                        "false"
                    },
                )
            })
            .collect(),
        ChoiceAction::TaxRelief(selected) => [
            (TAX_RELIEF_YES, if selected { "true" } else { "false" }),
            (TAX_RELIEF_NO, if selected { "false" } else { "true" }),
        ]
        .into(),
    }
}

fn candidate_radio_ui_key(action: ChoiceAction) -> Option<&'static str> {
    Some(match action {
        ChoiceAction::FilingBasis(Form2550QFilingBasis::Calendar) => FILING_BASIS_CALENDAR,
        ChoiceAction::FilingBasis(Form2550QFilingBasis::Fiscal) => FILING_BASIS_FISCAL,
        ChoiceAction::Quarter(Form2550QQuarter::First) => QUARTER_1,
        ChoiceAction::Quarter(Form2550QQuarter::Second) => QUARTER_2,
        ChoiceAction::Quarter(Form2550QQuarter::Third) => QUARTER_3,
        ChoiceAction::Quarter(Form2550QQuarter::Fourth) => QUARTER_4,
        ChoiceAction::Amended(true) => AMENDED_YES,
        ChoiceAction::Amended(false) => AMENDED_NO,
        ChoiceAction::ShortPeriod(true) => SHORT_PERIOD_YES,
        ChoiceAction::ShortPeriod(false) => SHORT_PERIOD_NO,
        ChoiceAction::Classification(Form2550QTaxpayerClassification::Micro) => CLASSIFICATION_1,
        ChoiceAction::Classification(Form2550QTaxpayerClassification::Small) => CLASSIFICATION_2,
        ChoiceAction::Classification(Form2550QTaxpayerClassification::Medium) => CLASSIFICATION_3,
        ChoiceAction::Classification(Form2550QTaxpayerClassification::Large) => CLASSIFICATION_4,
        ChoiceAction::TaxRelief(true) => TAX_RELIEF_YES,
        ChoiceAction::TaxRelief(false) => TAX_RELIEF_NO,
        ChoiceAction::Quarter(Form2550QQuarter::Unresolved(_)) => return None,
    })
}

fn candidate_radio_raw_is_selected(raw: &str) -> bool {
    raw == "true"
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

fn raw_text_override(value: Option<&RawValue>) -> Option<&str> {
    match value {
        Some(RawValue::Text(text)) => Some(text.as_str()),
        Some(RawValue::Absent) => Some(""),
        None => None,
    }
}

fn raw_editor_binding_safety_error(draft: &Form2550QDraft) -> Option<String> {
    draft
        .validate()
        .into_iter()
        .find(|(field, _)| {
            matches!(
                field.as_str(),
                "repeated_row_identity" | "raw_editor_state.version" | "raw_editor_state.bindings"
            )
        })
        .map(|(_, message)| message)
}

fn should_capture_raw(existing: Option<&RawValue>, changed: bool) -> bool {
    changed || matches!(existing, Some(RawValue::Text(_)))
}

fn set_input_value(
    input: &Entity<InputState>,
    value: &str,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) {
    input.update(cx, |state, cx| {
        state.set_value(value.to_string(), window, cx)
    });
}

fn restore_singleton_raw_values(
    fields: &BTreeMap<&'static str, Entity<InputState>>,
    raw_state: &bir_core::forms::form_2550q::Form2550QRawEditorState,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) {
    for (ui_key, input) in fields {
        let raw_key = singleton_raw_key(ui_key);
        if let Some(value) = raw_text_override(raw_state.singleton_value(raw_key)) {
            set_input_value(input, value, window, cx);
        }
    }
}

fn restore_repeated_raw_values(
    schedule_1: &[CapitalGoodInputs],
    schedule_3: &[CreditableVatInputs],
    schedule_4: &[AdvanceVatInputs],
    raw_state: &bir_core::forms::form_2550q::Form2550QRawEditorState,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) {
    let bindings = schedule_1
        .iter()
        .flat_map(CapitalGoodInputs::bound_inputs)
        .chain(
            schedule_3
                .iter()
                .flat_map(CreditableVatInputs::bound_inputs),
        )
        .chain(schedule_4.iter().flat_map(AdvanceVatInputs::bound_inputs));
    for (input, binding) in bindings {
        let RawControlBinding::Repeated {
            family,
            instance_id,
            member_key,
        } = binding
        else {
            continue;
        };
        if let Some(value) =
            raw_text_override(raw_state.repeated_value(family, &instance_id, member_key))
        {
            set_input_value(&input, value, window, cx);
        }
    }
}

fn singleton_raw_key(ui_key: &str) -> &'static str {
    singleton_raw_key_if_known(ui_key)
        .unwrap_or_else(|| panic!("2550Q input {ui_key} has no reviewed raw-state binding"))
}

fn singleton_raw_key_if_known(ui_key: &str) -> Option<&'static str> {
    Some(match ui_key {
        FILING_BASIS_CALENDAR => "frm2550qv2024:calendarNo1",
        FILING_BASIS_FISCAL => "frm2550qv2024:fiscalNo1",
        QUARTER_1 => "frm2550qv2024:OptQuarter1",
        QUARTER_2 => "frm2550qv2024:OptQuarter2",
        QUARTER_3 => "frm2550qv2024:OptQuarter3",
        QUARTER_4 => "frm2550qv2024:OptQuarter4",
        AMENDED_YES => "frm2550qv2024:amendedReturnYesNo5",
        AMENDED_NO => "frm2550qv2024:amendedReturnNo5",
        SHORT_PERIOD_YES => "frm2550qv2024:OptShortPrd1",
        SHORT_PERIOD_NO => "frm2550qv2024:OptShortPrd2",
        CLASSIFICATION_1 => "frm2550qv2024:taxPayerClassification1",
        CLASSIFICATION_2 => "frm2550qv2024:taxPayerClassification2",
        CLASSIFICATION_3 => "frm2550qv2024:taxPayerClassification3",
        CLASSIFICATION_4 => "frm2550qv2024:taxPayerClassification4",
        TAX_RELIEF_YES => "frm2550qv2024:internationalTreatyYn",
        TAX_RELIEF_NO => "frm2550qv2024:specialRateYn",
        YEAR_END_MONTH => "frm2550qv2024:selectedMonthNo2",
        RAW_TAXABLE_YEAR => "frm2550qv2024:txtYearNo2",
        RAW_TIN_1 => "frm2550qv2024:txtTIN1",
        RAW_TIN_2 => "frm2550qv2024:txtTIN2",
        RAW_TIN_3 => "frm2550qv2024:txtTIN3",
        RAW_BRANCH_CODE => "frm2550qv2024:branchCode",
        RAW_RDO_CODE => "frm2550qv2024:txtRDOCode",
        RAW_TAXPAYER_NAME => "frm2550qv2024:taxpayerName",
        RAW_TAXPAYER_ADDRESS => "frm2550qv2024:taxpayerAddress",
        RAW_TAXPAYER_ZIP => "frm2550qv2024:taxpayerZip",
        RAW_TAXPAYER_CONTACT_NUMBER => "frm2550qv2024:taxpayerContactNumber",
        RAW_TAXPAYER_EMAIL_ADDRESS => "frm2550qv2024:taxpayerEmailAddress",
        RETURN_PERIOD_FROM => "frm2550qv2024:RtnPeriodFromNo4",
        RETURN_PERIOD_TO => "frm2550qv2024:RtnPeriodToNo4",
        TAX_RELIEF_DETAILS => "frm2550qv2024:specifyInternationalTreaty",
        ITEM_18 => "frm2550qv2024:vatPaidReturn",
        ITEM_19_DESCRIPTION => "frm2550qv2024:addSpecifyNo19",
        ITEM_19 => "frm2550qv2024:otherCreditsNo19",
        ITEM_22 => "frm2550qv2024:surcharge",
        ITEM_23 => "frm2550qv2024:interest",
        ITEM_24 => "frm2550qv2024:compromise",
        ITEM_31A => "frm2550qv2024:vatableSales",
        ITEM_32A => "frm2550qv2024:zeroRatedSales",
        ITEM_33A => "frm2550qv2024:exemptSales",
        ITEM_35B => "frm2550qv2024:lessOutputVat",
        ITEM_36B => "frm2550qv2024:addOutputVat",
        ITEM_38B => "frm2550qv2024:inputTaxCarried",
        ITEM_40B => "frm2550qv2024:transitionalInputTax",
        ITEM_41B => "frm2550qv2024:presumptiveInputTax",
        ITEM_42_DESCRIPTION => "frm2550qv2024:addSpecifyNo42",
        ITEM_42B => "frm2550qv2024:otherSpecify42",
        ITEM_44A => "frm2550qv2024:domesticPurchase",
        ITEM_44B => "frm2550qv2024:domesticInputTax",
        ITEM_45A => "frm2550qv2024:servicesPurchase",
        ITEM_45B => "frm2550qv2024:serviceInputTax",
        ITEM_46A => "frm2550qv2024:importPurchase",
        ITEM_46B => "frm2550qv2024:importInputTax",
        ITEM_47_DESCRIPTION => "frm2550qv2024:addSpecifyNo47",
        ITEM_47A => "frm2550qv2024:otherSpecify47",
        ITEM_47B => "otherSpecify47B",
        ITEM_48A => "frm2550qv2024:domesticPurchaseNoTax",
        ITEM_49A => "frm2550qv2024:vatExemptImports",
        ITEM_54B => "frm2550qv2024:vatRefund",
        ITEM_55B => "frm2550qv2024:inputVatUnpaid",
        ITEM_56_DESCRIPTION => "frm2550qv2024:addSpecifyNo56",
        ITEM_56B => "frm2550qv2024:otherSpecify56",
        ITEM_58B => "frm2550qv2024:addInputVat",
        SCHEDULE_2_DIRECT => "frm2550qv2024:sched2InputTaxDirect",
        SCHEDULE_2_EXEMPT_SALES => "frm2550qv2024:sched2VatExemptSale",
        SCHEDULE_2_NOT_DIRECT => "frm2550qv2024:sched2AmountInputTax",
        SIGNATORY => "signatory",
        SIGNATORY_TITLE => "signatory_title",
        NON_INDIVIDUAL_OFFICER => "non_individual_officer",
        TAX_AGENT_NUMBER => "tax_agent_number",
        TAX_AGENT_ISSUE => "tax_agent_issue",
        TAX_AGENT_EXPIRY => "tax_agent_expiry",
        CASH_AMOUNT => "cash_amount",
        CHECK_BANK => "check_bank",
        CHECK_NUMBER => "check_number",
        CHECK_DATE => "check_date",
        CHECK_AMOUNT => "check_amount",
        TDM_NUMBER => "tdm_number",
        TDM_DATE => "tdm_date",
        TDM_AMOUNT => "tdm_amount",
        OTHER_PAYMENT_DESCRIPTION => "other_payment_description",
        OTHER_PAYMENT_BANK => "other_payment_bank",
        OTHER_PAYMENT_NUMBER => "other_payment_number",
        OTHER_PAYMENT_DATE => "other_payment_date",
        OTHER_PAYMENT_AMOUNT => "other_payment_amount",
        MACHINE_VALIDATION => "machine_validation",
        _ => return None,
    })
}

fn singleton_is_parsed(ui_key: &str) -> bool {
    ![
        FILING_BASIS_CALENDAR,
        FILING_BASIS_FISCAL,
        QUARTER_1,
        QUARTER_2,
        QUARTER_3,
        QUARTER_4,
        AMENDED_YES,
        AMENDED_NO,
        SHORT_PERIOD_YES,
        SHORT_PERIOD_NO,
        CLASSIFICATION_1,
        CLASSIFICATION_2,
        CLASSIFICATION_3,
        CLASSIFICATION_4,
        TAX_RELIEF_YES,
        TAX_RELIEF_NO,
        RAW_TAXABLE_YEAR,
        RAW_TIN_1,
        RAW_TIN_2,
        RAW_TIN_3,
        RAW_BRANCH_CODE,
        RAW_RDO_CODE,
        RAW_TAXPAYER_NAME,
        RAW_TAXPAYER_ADDRESS,
        RAW_TAXPAYER_ZIP,
        RAW_TAXPAYER_CONTACT_NUMBER,
        RAW_TAXPAYER_EMAIL_ADDRESS,
        TAX_RELIEF_DETAILS,
        ITEM_19_DESCRIPTION,
        ITEM_42_DESCRIPTION,
        ITEM_47_DESCRIPTION,
        ITEM_56_DESCRIPTION,
        SIGNATORY,
        SIGNATORY_TITLE,
        NON_INDIVIDUAL_OFFICER,
        TAX_AGENT_NUMBER,
        TAX_AGENT_ISSUE,
        TAX_AGENT_EXPIRY,
        CHECK_BANK,
        CHECK_NUMBER,
        CHECK_DATE,
        TDM_NUMBER,
        TDM_DATE,
        OTHER_PAYMENT_DESCRIPTION,
        OTHER_PAYMENT_BANK,
        OTHER_PAYMENT_NUMBER,
        OTHER_PAYMENT_DATE,
        MACHINE_VALIDATION,
    ]
    .contains(&ui_key)
}

fn repeated_member_is_parsed(member_key: &str) -> bool {
    ![
        "txtSourceCode1",
        "txtDescription1",
        "txtNameWithHoldingAgent3",
        "txtNameOfMiller4",
        "txtNameOfTaxpayer4",
        "txtOfficialReceiptNumber4",
    ]
    .contains(&member_key)
}

fn stable_parse_path(field_name: &str) -> String {
    singleton_raw_key_if_known(field_name)
        .unwrap_or(field_name)
        .to_string()
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
            stable_parse_path(field_name),
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
            stable_parse_path(field_name),
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
            stable_parse_path(field_name),
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
        Err(message) => errors.push((stable_parse_path(field_name), message)),
    }
}

fn capital_good_inputs(
    row: &Form2550QCapitalGoodRow,
    window: &mut Window,
    cx: &mut Context<Form2550QV2View>,
) -> CapitalGoodInputs {
    CapitalGoodInputs {
        instance_id: row
            .instance_id
            .clone()
            .expect("validated Schedule 1 row must retain its stable ID"),
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
        instance_id: row
            .instance_id
            .clone()
            .expect("validated Schedule 3 row must retain its stable ID"),
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
        instance_id: row
            .instance_id
            .clone()
            .expect("validated Schedule 4 row must retain its stable ID"),
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
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule-1/{}", inputs.instance_id);
    assign_return_date(
        &mut row.purchase_or_import_date,
        &inputs.date,
        &format!("{prefix}/txtDatePurchase1"),
        cx,
        errors,
    );
    row.source_code = input_text(&inputs.source_code, cx);
    row.description = input_text(&inputs.description, cx);
    assign_money(
        &mut row.purchase_or_import_amount,
        &inputs.purchase_amount,
        &format!("{prefix}/txtAmountPurchase1"),
        cx,
        errors,
    );
    assign_money(
        &mut row.input_tax,
        &inputs.input_tax,
        &format!("{prefix}/txtInputTax1"),
        cx,
        errors,
    );
    assign_optional_u16(
        &mut row.estimated_life_months,
        &inputs.estimated_life,
        &format!("{prefix}/txtEstimatedLife1"),
        cx,
        errors,
    );
    assign_optional_u16(
        &mut row.recognized_life_months,
        &inputs.recognized_life,
        &format!("{prefix}/txtRecognizedLife1"),
        cx,
        errors,
    );
    assign_money(
        &mut row.allowable_input_tax_for_period,
        &inputs.allowable_input_tax,
        &format!("{prefix}/txtAllowedInputTax1"),
        cx,
        errors,
    );
    assign_money(
        &mut row.balance_to_next_period,
        &inputs.balance_next_period,
        &format!("{prefix}/txtBalanceInputTax1"),
        cx,
        errors,
    );
}

fn sync_creditable_vat_row(
    row: &mut Form2550QCreditableVatRow,
    inputs: &CreditableVatInputs,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule-3/{}", inputs.instance_id);
    assign_return_date(
        &mut row.period_from,
        &inputs.period_from,
        &format!("{prefix}/txtDateCovered3"),
        cx,
        errors,
    );
    assign_return_date(
        &mut row.period_to,
        &inputs.period_to,
        &format!("{prefix}/txtDateCovered3To"),
        cx,
        errors,
    );
    row.withholding_agent_name = input_text(&inputs.agent_name, cx);
    assign_money(
        &mut row.income_payment,
        &inputs.income_payment,
        &format!("{prefix}/txtIncomePayment3"),
        cx,
        errors,
    );
    assign_money(
        &mut row.tax_withheld,
        &inputs.tax_withheld,
        &format!("{prefix}/txtTotalTaxWithHeld3"),
        cx,
        errors,
    );
}

fn sync_advance_vat_row(
    row: &mut Form2550QAdvanceVatRow,
    inputs: &AdvanceVatInputs,
    cx: &Context<Form2550QV2View>,
    errors: &mut Vec<(String, String)>,
) {
    let prefix = format!("schedule-4/{}", inputs.instance_id);
    assign_return_date(
        &mut row.period_from,
        &inputs.period_from,
        &format!("{prefix}/txtDate4"),
        cx,
        errors,
    );
    assign_return_date(
        &mut row.period_to,
        &inputs.period_to,
        &format!("{prefix}/txtDate4To"),
        cx,
        errors,
    );
    row.miller_name = input_text(&inputs.miller_name, cx);
    row.taxpayer_name = input_text(&inputs.taxpayer_name, cx);
    row.official_receipt_number = input_text(&inputs.receipt_number, cx);
    assign_money(
        &mut row.amount_paid,
        &inputs.amount_paid,
        &format!("{prefix}/txtAmountPaid4"),
        cx,
        errors,
    );
}

fn format_optional_money(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_string(), |amount| format!("₱ {amount:.2}"))
}

#[cfg(test)]
mod tests {
    use super::{
        AMENDED_NO, AMENDED_YES, CASH_AMOUNT, CHECK_DATE, CLASSIFICATION_1, CLASSIFICATION_2,
        CLASSIFICATION_3, CLASSIFICATION_4, ChoiceAction, FILING_BASIS_CALENDAR,
        FILING_BASIS_FISCAL, Form2550QDiagnosticState, Form2550QFilingBasis,
        Form2550QLiveValidationFacade, Form2550QQuarter, Form2550QRawFieldKey, Form2550QRowFamily,
        Form2550QTaxpayerClassification, ITEM_18, ITEM_19_DESCRIPTION, ITEM_31A, ITEM_47B,
        InputRevision, QUARTER_1, QUARTER_2, QUARTER_3, QUARTER_4, RAW_BRANCH_CODE, RAW_RDO_CODE,
        RAW_TAXABLE_YEAR, RAW_TAXPAYER_ADDRESS, RAW_TAXPAYER_CONTACT_NUMBER,
        RAW_TAXPAYER_EMAIL_ADDRESS, RAW_TAXPAYER_NAME, RAW_TAXPAYER_ZIP, RAW_TIN_1, RAW_TIN_2,
        RAW_TIN_3, RETURN_PERIOD_FROM, RawControlBinding, RawValue, SHORT_PERIOD_NO,
        SHORT_PERIOD_YES, SIGNATORY, StableInstanceId, TAX_RELIEF_NO, TAX_RELIEF_YES,
        YEAR_END_MONTH, advance_input_revision, candidate_radio_raw_is_selected,
        candidate_radio_ui_key, candidate_radio_values, diagnostic_state_from_setup,
        raw_text_override, repeated_member_is_parsed, should_capture_raw, singleton_is_parsed,
        singleton_raw_key, stable_parse_path,
    };

    #[test]
    fn raw_restore_distinguishes_missing_absent_blank_and_text() {
        let blank = RawValue::Text(String::new());
        let text = RawValue::Text("  1,234.500  ".to_string());

        assert_eq!(raw_text_override(None), None);
        assert_eq!(raw_text_override(Some(&RawValue::Absent)), Some(""));
        assert_eq!(raw_text_override(Some(&blank)), Some(""));
        assert_eq!(raw_text_override(Some(&text)), Some("  1,234.500  "));

        assert!(!should_capture_raw(None, false));
        assert!(!should_capture_raw(Some(&RawValue::Absent), false));
        assert!(should_capture_raw(Some(&blank), false));
        assert!(should_capture_raw(None, true));
        assert!(should_capture_raw(Some(&RawValue::Absent), true));
    }

    #[test]
    fn singleton_bindings_use_the_reviewed_and_local_raw_inventories() {
        assert_eq!(
            singleton_raw_key(FILING_BASIS_CALENDAR),
            "frm2550qv2024:calendarNo1"
        );
        assert_eq!(
            singleton_raw_key(FILING_BASIS_FISCAL),
            "frm2550qv2024:fiscalNo1"
        );
        assert_eq!(
            [
                singleton_raw_key(QUARTER_1),
                singleton_raw_key(QUARTER_2),
                singleton_raw_key(QUARTER_3),
                singleton_raw_key(QUARTER_4),
            ],
            [
                "frm2550qv2024:OptQuarter1",
                "frm2550qv2024:OptQuarter2",
                "frm2550qv2024:OptQuarter3",
                "frm2550qv2024:OptQuarter4",
            ]
        );
        assert_eq!(
            [
                singleton_raw_key(CLASSIFICATION_1),
                singleton_raw_key(CLASSIFICATION_2),
                singleton_raw_key(CLASSIFICATION_3),
                singleton_raw_key(CLASSIFICATION_4),
            ],
            [
                "frm2550qv2024:taxPayerClassification1",
                "frm2550qv2024:taxPayerClassification2",
                "frm2550qv2024:taxPayerClassification3",
                "frm2550qv2024:taxPayerClassification4",
            ]
        );
        assert_eq!(
            [
                singleton_raw_key(TAX_RELIEF_YES),
                singleton_raw_key(TAX_RELIEF_NO),
            ],
            [
                "frm2550qv2024:internationalTreatyYn",
                "frm2550qv2024:specialRateYn",
            ]
        );
        assert_eq!(
            [
                singleton_raw_key(AMENDED_YES),
                singleton_raw_key(AMENDED_NO),
                singleton_raw_key(SHORT_PERIOD_YES),
                singleton_raw_key(SHORT_PERIOD_NO),
            ],
            [
                "frm2550qv2024:amendedReturnYesNo5",
                "frm2550qv2024:amendedReturnNo5",
                "frm2550qv2024:OptShortPrd1",
                "frm2550qv2024:OptShortPrd2",
            ]
        );
        assert_eq!(
            singleton_raw_key(YEAR_END_MONTH),
            "frm2550qv2024:selectedMonthNo2"
        );
        assert_eq!(
            singleton_raw_key(RAW_TAXABLE_YEAR),
            "frm2550qv2024:txtYearNo2"
        );
        for (ui_key, raw_key) in [
            (RAW_TIN_1, "frm2550qv2024:txtTIN1"),
            (RAW_TIN_2, "frm2550qv2024:txtTIN2"),
            (RAW_TIN_3, "frm2550qv2024:txtTIN3"),
            (RAW_BRANCH_CODE, "frm2550qv2024:branchCode"),
            (RAW_RDO_CODE, "frm2550qv2024:txtRDOCode"),
            (RAW_TAXPAYER_NAME, "frm2550qv2024:taxpayerName"),
            (RAW_TAXPAYER_ADDRESS, "frm2550qv2024:taxpayerAddress"),
            (RAW_TAXPAYER_ZIP, "frm2550qv2024:taxpayerZip"),
            (
                RAW_TAXPAYER_CONTACT_NUMBER,
                "frm2550qv2024:taxpayerContactNumber",
            ),
            (
                RAW_TAXPAYER_EMAIL_ADDRESS,
                "frm2550qv2024:taxpayerEmailAddress",
            ),
        ] {
            assert_eq!(singleton_raw_key(ui_key), raw_key);
            let semantic = Form2550QRawFieldKey::try_singleton(raw_key)
                .expect("candidate raw profile key is closed and reviewed")
                .semantic_field_instance()
                .expect("candidate raw profile key has a semantic focus identity");
            assert_eq!(semantic.field_id().as_str(), raw_key);
            assert!(semantic.group_path().is_empty());
        }
        assert_eq!(
            singleton_raw_key(ITEM_47B),
            "otherSpecify47B",
            "the reviewed source's non-namespaced key must remain exact"
        );
        assert_eq!(singleton_raw_key(SIGNATORY), "signatory");
        assert_eq!(stable_parse_path(ITEM_18), "frm2550qv2024:vatPaidReturn");
    }

    #[test]
    fn only_parser_backed_controls_participate_in_malformed_tracking() {
        assert!(singleton_is_parsed(RETURN_PERIOD_FROM));
        assert!(singleton_is_parsed(ITEM_31A));
        assert!(!singleton_is_parsed(FILING_BASIS_CALENDAR));
        assert!(!singleton_is_parsed(QUARTER_1));
        assert!(!singleton_is_parsed(CLASSIFICATION_1));
        assert!(!singleton_is_parsed(TAX_RELIEF_YES));
        assert!(!singleton_is_parsed(TAX_RELIEF_NO));
        assert!(!singleton_is_parsed(AMENDED_YES));
        assert!(!singleton_is_parsed(AMENDED_NO));
        assert!(!singleton_is_parsed(SHORT_PERIOD_YES));
        assert!(!singleton_is_parsed(SHORT_PERIOD_NO));
        assert!(!singleton_is_parsed(RAW_TAXABLE_YEAR));
        for ui_key in [
            RAW_TIN_1,
            RAW_TIN_2,
            RAW_TIN_3,
            RAW_BRANCH_CODE,
            RAW_RDO_CODE,
            RAW_TAXPAYER_NAME,
            RAW_TAXPAYER_ADDRESS,
            RAW_TAXPAYER_ZIP,
            RAW_TAXPAYER_CONTACT_NUMBER,
            RAW_TAXPAYER_EMAIL_ADDRESS,
        ] {
            assert!(
                !singleton_is_parsed(ui_key),
                "{ui_key} must remain raw-only instead of mutating profile-backed typed fields"
            );
        }
        assert!(!singleton_is_parsed(ITEM_19_DESCRIPTION));
        assert!(!singleton_is_parsed(CHECK_DATE));
        assert!(repeated_member_is_parsed("txtInputTax1"));
        assert!(!repeated_member_is_parsed("txtDescription1"));
        assert!(
            Form2550QRawFieldKey::try_singleton(singleton_raw_key(CASH_AMOUNT)).is_ok(),
            "local-print money buffers need canonical malformed tracking without entering rules"
        );
    }

    #[test]
    fn repeated_malformed_paths_are_derived_from_full_persisted_ids() {
        let instance_id = StableInstanceId::parse("row-00000000000000000042")
            .expect("fixed-width test row identity");
        let binding = RawControlBinding::Repeated {
            family: Form2550QRowFamily::Schedule1,
            instance_id,
            member_key: "txtInputTax1",
        };
        assert_eq!(
            binding
                .raw_field_key()
                .expect("declared repeated member")
                .stable_path(),
            "schedule-1/row-00000000000000000042/txtInputTax1"
        );
    }

    #[test]
    fn edit_revision_advances_once_per_explicit_change_and_fails_closed_at_overflow() {
        let mut revision = InputRevision::default();
        assert_eq!(
            advance_input_revision(&mut revision),
            Ok(InputRevision::new(1))
        );
        assert_eq!(revision, InputRevision::new(1));

        revision = InputRevision::new(u64::MAX);
        assert_eq!(advance_input_revision(&mut revision), Err(()));
        assert_eq!(revision, InputRevision::new(u64::MAX));
    }

    #[test]
    fn candidate_radio_actions_materialize_exact_mutually_exclusive_groups() {
        assert_eq!(
            candidate_radio_values(ChoiceAction::FilingBasis(Form2550QFilingBasis::Fiscal)),
            vec![
                (FILING_BASIS_CALENDAR, "false"),
                (FILING_BASIS_FISCAL, "true"),
            ]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::Quarter(Form2550QQuarter::Third)),
            vec![
                (QUARTER_1, "false"),
                (QUARTER_2, "false"),
                (QUARTER_3, "true"),
                (QUARTER_4, "false"),
            ]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::Amended(true)),
            vec![(AMENDED_YES, "true"), (AMENDED_NO, "false")]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::Amended(false)),
            vec![(AMENDED_YES, "false"), (AMENDED_NO, "true")]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::ShortPeriod(true)),
            vec![(SHORT_PERIOD_YES, "true"), (SHORT_PERIOD_NO, "false")]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::ShortPeriod(false)),
            vec![(SHORT_PERIOD_YES, "false"), (SHORT_PERIOD_NO, "true")]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::Classification(
                Form2550QTaxpayerClassification::Large,
            )),
            vec![
                (CLASSIFICATION_1, "false"),
                (CLASSIFICATION_2, "false"),
                (CLASSIFICATION_3, "false"),
                (CLASSIFICATION_4, "true"),
            ]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::TaxRelief(true)),
            vec![(TAX_RELIEF_YES, "true"), (TAX_RELIEF_NO, "false")]
        );
        assert_eq!(
            candidate_radio_values(ChoiceAction::TaxRelief(false)),
            vec![(TAX_RELIEF_YES, "false"), (TAX_RELIEF_NO, "true")]
        );

        for action in [
            ChoiceAction::FilingBasis(Form2550QFilingBasis::Calendar),
            ChoiceAction::FilingBasis(Form2550QFilingBasis::Fiscal),
        ] {
            let values = candidate_radio_values(action);
            assert_eq!(values.len(), 2);
            assert_eq!(
                values.iter().filter(|(_, value)| *value == "true").count(),
                1
            );
        }
        for quarter in Form2550QQuarter::ALL {
            let values = candidate_radio_values(ChoiceAction::Quarter(quarter));
            assert_eq!(values.len(), 4);
            assert_eq!(
                values.iter().filter(|(_, value)| *value == "true").count(),
                1
            );
        }
        for classification in Form2550QTaxpayerClassification::ALL {
            let values = candidate_radio_values(ChoiceAction::Classification(classification));
            assert_eq!(values.len(), 4);
            assert_eq!(
                values.iter().filter(|(_, value)| *value == "true").count(),
                1
            );
        }
        for selected in [true, false] {
            let values = candidate_radio_values(ChoiceAction::TaxRelief(selected));
            assert_eq!(values.len(), 2);
            assert_eq!(
                values.iter().filter(|(_, value)| *value == "true").count(),
                1
            );
        }
        for action in [
            ChoiceAction::Amended(true),
            ChoiceAction::Amended(false),
            ChoiceAction::ShortPeriod(true),
            ChoiceAction::ShortPeriod(false),
        ] {
            let values = candidate_radio_values(action);
            assert_eq!(values.len(), 2);
            assert_eq!(
                values.iter().filter(|(_, value)| *value == "true").count(),
                1
            );
        }
        assert_eq!(
            Form2550QQuarter::ALL
                .into_iter()
                .map(|quarter| candidate_radio_ui_key(ChoiceAction::Quarter(quarter)))
                .collect::<Vec<_>>(),
            vec![
                Some(QUARTER_1),
                Some(QUARTER_2),
                Some(QUARTER_3),
                Some(QUARTER_4),
            ],
            "each semantic raw flag must focus its own visible radio control"
        );
        assert_eq!(
            Form2550QTaxpayerClassification::ALL
                .into_iter()
                .map(|classification| {
                    candidate_radio_ui_key(ChoiceAction::Classification(classification))
                })
                .collect::<Vec<_>>(),
            vec![
                Some(CLASSIFICATION_1),
                Some(CLASSIFICATION_2),
                Some(CLASSIFICATION_3),
                Some(CLASSIFICATION_4),
            ]
        );
        assert_eq!(
            [
                candidate_radio_ui_key(ChoiceAction::TaxRelief(true)),
                candidate_radio_ui_key(ChoiceAction::TaxRelief(false)),
            ],
            [Some(TAX_RELIEF_YES), Some(TAX_RELIEF_NO)]
        );
        assert_eq!(
            [
                candidate_radio_ui_key(ChoiceAction::Amended(true)),
                candidate_radio_ui_key(ChoiceAction::Amended(false)),
                candidate_radio_ui_key(ChoiceAction::ShortPeriod(true)),
                candidate_radio_ui_key(ChoiceAction::ShortPeriod(false)),
            ],
            [
                Some(AMENDED_YES),
                Some(AMENDED_NO),
                Some(SHORT_PERIOD_YES),
                Some(SHORT_PERIOD_NO),
            ]
        );
    }

    #[test]
    fn candidate_radio_edit_advances_one_revision_without_default_raw_fallback() {
        assert_eq!(
            raw_text_override(None),
            None,
            "typed draft defaults must not materialize candidate raw controls"
        );

        let mut revision = InputRevision::default();
        let changed = candidate_radio_values(ChoiceAction::Quarter(Form2550QQuarter::Second));
        assert_eq!(changed.len(), 4);
        advance_input_revision(&mut revision).expect("one explicit radio edit advances");
        assert_eq!(revision, InputRevision::new(1));

        assert!(candidate_radio_raw_is_selected("true"));
        for raw in ["", "false", "TRUE", " true "] {
            assert!(
                !candidate_radio_raw_is_selected(raw),
                "only exact raw true may render as selected"
            );
        }
    }

    #[test]
    fn candidate_taxable_year_edit_advances_one_revision_without_typed_fallback() {
        assert_eq!(
            raw_text_override(None),
            None,
            "draft.taxable_year must not materialize the candidate raw control"
        );
        assert_eq!(
            singleton_raw_key(RAW_TAXABLE_YEAR),
            "frm2550qv2024:txtYearNo2"
        );
        assert!(
            !singleton_is_parsed(RAW_TAXABLE_YEAR),
            "the candidate buffer must not write through to the typed draft"
        );

        let mut revision = InputRevision::default();
        advance_input_revision(&mut revision).expect("one explicit text edit advances");
        assert_eq!(revision, InputRevision::new(1));
    }

    #[test]
    fn candidate_profile_text_edits_each_advance_one_revision_without_typed_fallback() {
        assert_eq!(
            raw_text_override(None),
            None,
            "profile-backed typed values must not materialize candidate raw controls"
        );
        for ui_key in [
            RAW_TAXPAYER_ADDRESS,
            RAW_TAXPAYER_ZIP,
            RAW_TAXPAYER_CONTACT_NUMBER,
            RAW_TAXPAYER_EMAIL_ADDRESS,
        ] {
            assert!(!singleton_is_parsed(ui_key));
            let mut revision = InputRevision::default();
            advance_input_revision(&mut revision).expect("one explicit text edit advances");
            assert_eq!(
                revision,
                InputRevision::new(1),
                "{ui_key} must advance exactly once for its subscribed change"
            );
        }
    }

    #[test]
    fn production_ui_reports_no_reviewed_validator_without_running_candidate() {
        let setup = Form2550QLiveValidationFacade::setup_repo_default_diagnostic();
        let state = diagnostic_state_from_setup(&setup);
        let Form2550QDiagnosticState::Unavailable(message) = state else {
            panic!("the empty reviewed registry must be unavailable");
        };
        assert!(message.contains("no review-controlled exact 2550Q"));
        assert!(message.contains("reviewed registry entries alone do not authorize activation"));
        assert!(message.contains("No candidate validator was evaluated"));
        assert!(message.contains("does not mean the draft is valid or filing-ready"));
    }
}
