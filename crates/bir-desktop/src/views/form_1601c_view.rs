//! BIR Form 1601C — Full in-app form view.
#![allow(dead_code)]

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::forms::form_1601c::{
    FORM_1601C_ATC, Form1601CDraft, Form1601CSchedule1Row, MAX_SCHEDULE_1_ROWS,
};
use bir_core::forms::{FilingPeriod, FilingStatus, FormValidator, can_queue_for_submission};
use bir_core::official_xml::{official_amount, parse_official_amount};

use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::components::tax_relief_select::{
    TaxReliefOption, TaxReliefSelectState, new_tax_relief_select, selected_tax_relief_code,
};
use crate::views::event_form_kit::Layout;
use crate::views::form_controls::{
    self, field_error, messages_for, select_field, with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, separate_validate_button,
};
use bir_core::forms::queueable::can_user_revert;
use gpui_component::select::{Select, SelectEvent};

use crate::agent::assist::{self, AssistState};

pub enum Form1601CEvent {
    BackToDashboard,
    /// Close this form without saving (the Dismiss button, after confirming
    /// when there are unsaved changes).
    Dismissed,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1601CEvent> for Form1601CView {}

/// Item 5 ATC: the one code this form accepts (value, label).
const ATC_OPTIONS: &[(&str, &str)] = &[(FORM_1601C_ATC, FORM_1601C_ATC)];

const SCAFFOLD_MESSAGE: &str = "1601-C January 2018 is available for draft preparation only in this build. Electronic queue submission remains unavailable.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmissionDisposition {
    QueueInApp,
    ManualExternal,
}

fn submission_disposition() -> SubmissionDisposition {
    if can_queue_for_submission("1601C") {
        SubmissionDisposition::QueueInApp
    } else {
        SubmissionDisposition::ManualExternal
    }
}

/// Item 11 Category of Withholding Agent. Copy-sized so it can live on
/// `Agent1601CHostPatch` without dropping `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Agent1601CCategory {
    Private,
    Government,
}

impl Agent1601CCategory {
    pub(crate) fn as_code(self) -> &'static str {
        match self {
            Self::Private => "P",
            Self::Government => "G",
        }
    }

    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "P" => Some(Self::Private),
            "G" => Some(Self::Government),
            _ => None,
        }
    }
}

/// Host-side 1601-C field patch applied on the UI thread. Kept as one value
/// so the always-compiled accessor stays under clippy's argument limit.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Agent1601CHostPatch {
    pub tax_14: Option<f64>,
    pub tax_25: Option<f64>,
    pub sheets: Option<u32>,
    pub any_taxes_withheld: Option<bool>,
    pub category_of_agent: Option<Agent1601CCategory>,
    pub save: bool,
    pub validate: bool,
}

/// Writes header flags onto the painted controls and the draft `validate` reads.
/// Same fields the UI toggles update before `sync_from_inputs`.
pub(crate) fn apply_1601c_host_header_patch(
    patch: &Agent1601CHostPatch,
    any_taxes_withheld: &mut Option<bool>,
    category_of_agent: &mut String,
    draft: &mut Form1601CDraft,
) -> bool {
    let mut dirty = false;
    if let Some(value) = patch.any_taxes_withheld {
        if *any_taxes_withheld != Some(value) || draft.taxes_withheld_answer() != Some(value) {
            *any_taxes_withheld = Some(value);
            draft.set_any_taxes_withheld(value);
            dirty = true;
        }
    }
    if let Some(value) = patch.category_of_agent {
        let code = value.as_code().to_string();
        if *category_of_agent != code || draft.category_of_agent != code {
            *category_of_agent = code.clone();
            draft.category_of_agent = code;
            dirty = true;
        }
    }
    dirty
}

struct ScheduleRowInputs {
    previous_month: Entity<InputState>,
    date_paid: Entity<InputState>,
    drawee_bank_code_or_agency: Entity<InputState>,
    payment_number: Entity<InputState>,
    tax_paid: Entity<InputState>,
    should_be_tax_due: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

const CONTENT_MAX_W: Pixels = px(800.);

pub struct Form1601CView {
    draft: Form1601CDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,
    /// Shared header: ⋯ menu and status box state.
    header: FormHeaderState,
    release_claim_confirm_open: bool,

    // Header Inputs
    is_amended: bool,
    /// Item 3: `None` while unanswered (the official page's start).
    any_taxes_withheld: Option<bool>,
    /// Item 11: `"P"` Private, `"G"` Government, `""` unanswered.
    category_of_agent: String,
    number_of_sheets: Entity<InputState>,
    atc: Entity<InputState>,
    /// Item 5 ATC picker; the only ATC the form allows is `FORM_1601C_ATC`.
    atc_select: Entity<ComboboxState>,
    tax_relief: bool,
    tax_relief_specification: Entity<TaxReliefSelectState>,

    // Part IV Schedule I inputs (parallel to draft.schedule_1)
    schedule_row_inputs: Vec<ScheduleRowInputs>,

    // Part II Inputs
    tax_14_total_compensation: Entity<InputState>,
    tax_15_statutory_minimum_wage: Entity<InputState>,
    tax_16_holiday_pay: Entity<InputState>,
    tax_17_13th_month_pay: Entity<InputState>,
    tax_18_de_minimis: Entity<InputState>,
    tax_19_sss_gsis: Entity<InputState>,
    tax_20_other_name: Entity<InputState>,
    tax_20_other_amount: Entity<InputState>,

    tax_23_not_subject: Entity<InputState>,
    tax_25_total_taxes_withheld: Entity<InputState>,
    tax_28_tax_remitted_previously: Entity<InputState>,
    tax_29_other_remittances_name: Entity<InputState>,
    tax_29_other_remittances_amount: Entity<InputState>,

    tax_32_surcharge: Entity<InputState>,
    tax_33_interest: Entity<InputState>,
    tax_34_compromise: Entity<InputState>,

    /// Dismiss was pressed with unsaved changes: ask before discarding.
    dismiss_confirm_open: bool,
    /// Per-box sources, unsaved-edit flags and the one-form lock (shared
    /// with the agent host through the drain).
    assist: AssistState,
    /// Fillable box values as last seen, so a change found by
    /// `sync_from_inputs` can be attributed to the user.
    known_values: std::collections::BTreeMap<String, serde_json::Value>,
    /// Set while the agent's draft is being written into the inputs.
    applying_agent_draft: bool,

    _subscriptions: Vec<Subscription>,
}

impl Form1601CView {
    fn schedule_text_input(
        value: &str,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Entity<InputState> {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx);
        });
        input
    }

    fn schedule_money_input(
        value: f64,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Entity<InputState> {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        input.update(cx, |input, cx| {
            input.set_value(format!("{value:.2}"), window, cx);
        });
        input
    }

    fn new_schedule_row_inputs(
        row: &Form1601CSchedule1Row,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> ScheduleRowInputs {
        let previous_month = Self::schedule_text_input(&row.previous_month, "MM/YYYY", window, cx);
        let date_paid = Self::schedule_text_input(&row.date_paid, "MM/DD/YYYY", window, cx);
        let drawee_bank_code_or_agency = Self::schedule_text_input(
            &row.drawee_bank_code_or_agency,
            "Bank code / agency",
            window,
            cx,
        );
        let payment_number =
            Self::schedule_text_input(&row.payment_number, "Reference number", window, cx);
        let tax_paid = Self::schedule_money_input(row.tax_paid, window, cx);
        let should_be_tax_due = Self::schedule_money_input(row.should_be_tax_due, window, cx);

        let mut subscriptions = Vec::new();
        for input in [
            previous_month.clone(),
            date_paid.clone(),
            drawee_bank_code_or_agency.clone(),
            payment_number.clone(),
            tax_paid.clone(),
            should_be_tax_due.clone(),
        ] {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                },
            ));
        }

        ScheduleRowInputs {
            previous_month,
            date_paid,
            drawee_bank_code_or_agency,
            payment_number,
            tax_paid,
            should_be_tax_due,
            _subscriptions: subscriptions,
        }
    }

    pub fn new(
        mut draft: Form1601CDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        if let Ok(guard) = db.lock()
            && let Ok(Some(stored)) =
                guard.get_1601c_draft(&draft.tin, draft.taxable_year, draft.month)
        {
            draft = stored;
        }
        let mut subscriptions = Vec::new();
        let tax_relief = draft.tax_relief;

        let create_input = |cx: &mut Context<Self>, val: f64, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let create_text_input = |cx: &mut Context<Self>, val: &str, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx));
            input.update(cx, |i, cx| i.set_value(val.to_string(), window, cx));
            input
        };

        let number_of_sheets = create_text_input(cx, &draft.number_of_sheets.to_string(), window);
        let atc = create_text_input(cx, &draft.atc, window);
        // Item 13A is a select on the official form: 1 Special Rate,
        // 2 International Tax Treaty, 3 Both.
        let tax_relief_specification =
            new_tax_relief_select(&draft.tax_relief_specification, true, window, cx);
        let schedule_row_inputs = draft
            .schedule_1
            .iter()
            .map(|row| Self::new_schedule_row_inputs(row, window, cx))
            .collect();

        let tax_14_total_compensation = create_input(cx, draft.tax_14_total_compensation, window);
        let tax_15_statutory_minimum_wage =
            create_input(cx, draft.tax_15_statutory_minimum_wage, window);
        let tax_16_holiday_pay = create_input(cx, draft.tax_16_holiday_pay, window);
        let tax_17_13th_month_pay = create_input(cx, draft.tax_17_13th_month_pay, window);
        let tax_18_de_minimis = create_input(cx, draft.tax_18_de_minimis, window);
        let tax_19_sss_gsis = create_input(cx, draft.tax_19_sss_gsis, window);
        let tax_20_other_name = create_text_input(cx, &draft.tax_20_other_name, window);
        let tax_20_other_amount = create_input(cx, draft.tax_20_other_amount, window);

        let tax_23_not_subject = create_input(cx, draft.tax_23_not_subject, window);
        let tax_25_total_taxes_withheld =
            create_input(cx, draft.tax_25_total_taxes_withheld, window);
        let tax_28_tax_remitted_previously =
            create_input(cx, draft.tax_28_tax_remitted_previously, window);
        let tax_29_other_remittances_name =
            create_text_input(cx, &draft.tax_29_other_remittances_name, window);
        let tax_29_other_remittances_amount =
            create_input(cx, draft.tax_29_other_remittances_amount, window);

        let tax_32_surcharge = create_input(cx, draft.tax_32_surcharge, window);
        let tax_33_interest = create_input(cx, draft.tax_33_interest, window);
        let tax_34_compromise = create_input(cx, draft.tax_34_compromise, window);

        let inputs = vec![
            number_of_sheets.clone(),
            atc.clone(),
            tax_14_total_compensation.clone(),
            tax_15_statutory_minimum_wage.clone(),
            tax_16_holiday_pay.clone(),
            tax_17_13th_month_pay.clone(),
            tax_18_de_minimis.clone(),
            tax_19_sss_gsis.clone(),
            tax_20_other_name.clone(),
            tax_20_other_amount.clone(),
            tax_23_not_subject.clone(),
            tax_25_total_taxes_withheld.clone(),
            tax_28_tax_remitted_previously.clone(),
            tax_29_other_remittances_name.clone(),
            tax_29_other_remittances_amount.clone(),
            tax_32_surcharge.clone(),
            tax_33_interest.clone(),
            tax_34_compromise.clone(),
        ];

        subscriptions.push(cx.subscribe_in(
            &tax_relief_specification,
            window,
            |this: &mut Self, _, _: &SelectEvent<Vec<TaxReliefOption>>, _, cx| {
                this.is_validated = false;
                this.sync_from_inputs(cx);
            },
        ));

        let atc_select = form_controls::new_select_state(ATC_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &atc_select,
            ATC_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, window, cx| {
                this.atc
                    .update(cx, |input, cx| input.set_value(value, window, cx));
                this.is_validated = false;
                this.sync_from_inputs(cx);
            },
        ));

        for input in inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                },
            ));
        }

        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        subscriptions.push(cx.subscribe(
            &bus,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| {
                if !matches!(event, crate::events::AppEvent::DatabaseChanged) {
                    return;
                }
                let tin = this.draft.tin.clone();
                let year = this.draft.taxable_year;
                let month = this.draft.month;
                if let Ok(db) = this.db.lock()
                    && let Ok(Some(updated)) = db.get_1601c_draft(&tin, year, month)
                    && (this.draft.status != updated.status
                        || this.draft.submission_claim_token != updated.submission_claim_token
                        || this.draft.submission_error != updated.submission_error)
                {
                    let became_submitted = this.draft.status != FilingStatus::Submitted
                        && updated.status == FilingStatus::Submitted;
                    this.status_message = updated.submission_error.clone();
                    this.draft = updated;
                    this.known_values = assist::fillable_values_1601c(&this.draft);
                    this.release_claim_confirm_open = false;
                    if became_submitted {
                        cx.emit(Form1601CEvent::Submitted);
                    }
                    cx.notify();
                }
            },
        ));

        Self {
            is_amended: draft.is_amended,
            any_taxes_withheld: draft.taxes_withheld_answer(),
            category_of_agent: draft.category_of_agent.clone(),
            known_values: assist::fillable_values_1601c(&draft),
            dismiss_confirm_open: false,
            assist: AssistState::default(),
            applying_agent_draft: false,
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            validation_errors: Vec::new(),
            status_message: None,
            header: FormHeaderState::new(),
            release_claim_confirm_open: false,

            number_of_sheets,
            atc,
            atc_select,
            tax_relief,
            tax_relief_specification,
            schedule_row_inputs,

            tax_14_total_compensation,
            tax_15_statutory_minimum_wage,
            tax_16_holiday_pay,
            tax_17_13th_month_pay,
            tax_18_de_minimis,
            tax_19_sss_gsis,
            tax_20_other_name,
            tax_20_other_amount,

            tax_23_not_subject,
            tax_25_total_taxes_withheld,
            tax_28_tax_remitted_previously,
            tax_29_other_remittances_name,
            tax_29_other_remittances_amount,

            tax_32_surcharge,
            tax_33_interest,
            tax_34_compromise,

            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let mut input_errors = Vec::new();
        let parse_money = |field: &str,
                           input: &Entity<InputState>,
                           cx: &Context<Self>,
                           errors: &mut Vec<(String, String)>| {
            let value = input.read(cx).value();
            if value.trim().is_empty() {
                0.0
            } else {
                // Official `round(this,2)` on blur: two decimals, half up.
                value
                    .parse::<f64>()
                    .map(|v| parse_official_amount(&official_amount(v)).unwrap_or(v))
                    .unwrap_or_else(|_| {
                        errors.push((
                            field.to_string(),
                            format!("{field} must contain a valid amount"),
                        ));
                        0.0
                    })
            }
        };
        let get_text =
            |input: &Entity<InputState>, cx: &Context<Self>| input.read(cx).value().to_string();

        self.draft.is_amended = self.is_amended;
        match self.any_taxes_withheld {
            Some(value) => self.draft.set_any_taxes_withheld(value),
            None => {
                self.draft.any_taxes_withheld = false;
                self.draft.any_taxes_withheld_unanswered = true;
            }
        }
        self.draft.category_of_agent = self.category_of_agent.clone();
        let number_of_sheets = get_text(&self.number_of_sheets, cx);
        self.draft.number_of_sheets = if number_of_sheets.trim().is_empty() {
            0
        } else {
            number_of_sheets.parse::<u32>().unwrap_or_else(|_| {
                input_errors.push((
                    "number_of_sheets".to_string(),
                    "Number of sheets must be a whole number".to_string(),
                ));
                0
            })
        };
        self.draft.atc = get_text(&self.atc, cx);
        self.draft.tax_relief = self.tax_relief;
        self.draft.tax_relief_specification = if self.tax_relief {
            selected_tax_relief_code(&self.tax_relief_specification, cx)
        } else {
            String::new()
        };

        for (index, (row, inputs)) in self
            .draft
            .schedule_1
            .iter_mut()
            .zip(&self.schedule_row_inputs)
            .enumerate()
        {
            row.previous_month = get_text(&inputs.previous_month, cx);
            row.date_paid = get_text(&inputs.date_paid, cx);
            row.drawee_bank_code_or_agency = get_text(&inputs.drawee_bank_code_or_agency, cx);
            row.payment_number = get_text(&inputs.payment_number, cx);
            row.tax_paid = parse_money(
                &format!("schedule_1_row_{}_tax_paid", index + 1),
                &inputs.tax_paid,
                cx,
                &mut input_errors,
            );
            row.should_be_tax_due = parse_money(
                &format!("schedule_1_row_{}_should_be_tax_due", index + 1),
                &inputs.should_be_tax_due,
                cx,
                &mut input_errors,
            );
        }

        self.draft.tax_14_total_compensation = parse_money(
            "tax_14_total_compensation",
            &self.tax_14_total_compensation,
            cx,
            &mut input_errors,
        );
        self.draft.tax_15_statutory_minimum_wage = parse_money(
            "tax_15_statutory_minimum_wage",
            &self.tax_15_statutory_minimum_wage,
            cx,
            &mut input_errors,
        );
        self.draft.tax_16_holiday_pay = parse_money(
            "tax_16_holiday_pay",
            &self.tax_16_holiday_pay,
            cx,
            &mut input_errors,
        );
        self.draft.tax_17_13th_month_pay = parse_money(
            "tax_17_13th_month_pay",
            &self.tax_17_13th_month_pay,
            cx,
            &mut input_errors,
        );
        self.draft.tax_18_de_minimis = parse_money(
            "tax_18_de_minimis",
            &self.tax_18_de_minimis,
            cx,
            &mut input_errors,
        );
        self.draft.tax_19_sss_gsis = parse_money(
            "tax_19_sss_gsis",
            &self.tax_19_sss_gsis,
            cx,
            &mut input_errors,
        );
        self.draft.tax_20_other_name = get_text(&self.tax_20_other_name, cx);
        self.draft.tax_20_other_amount = parse_money(
            "tax_20_other_amount",
            &self.tax_20_other_amount,
            cx,
            &mut input_errors,
        );

        self.draft.tax_23_not_subject = parse_money(
            "tax_23_not_subject",
            &self.tax_23_not_subject,
            cx,
            &mut input_errors,
        );
        self.draft.tax_25_total_taxes_withheld = parse_money(
            "tax_25_total_taxes_withheld",
            &self.tax_25_total_taxes_withheld,
            cx,
            &mut input_errors,
        );
        self.draft.tax_28_tax_remitted_previously = parse_money(
            "tax_28_tax_remitted_previously",
            &self.tax_28_tax_remitted_previously,
            cx,
            &mut input_errors,
        );
        self.draft.tax_29_other_remittances_name =
            get_text(&self.tax_29_other_remittances_name, cx);
        self.draft.tax_29_other_remittances_amount = parse_money(
            "tax_29_other_remittances_amount",
            &self.tax_29_other_remittances_amount,
            cx,
            &mut input_errors,
        );

        self.draft.tax_32_surcharge = parse_money(
            "tax_32_surcharge",
            &self.tax_32_surcharge,
            cx,
            &mut input_errors,
        );
        self.draft.tax_33_interest = parse_money(
            "tax_33_interest",
            &self.tax_33_interest,
            cx,
            &mut input_errors,
        );
        self.draft.tax_34_compromise = parse_money(
            "tax_34_compromise",
            &self.tax_34_compromise,
            cx,
            &mut input_errors,
        );

        self.draft.compute();
        self.note_box_changes();

        use bir_core::forms::FormValidator;
        input_errors.extend(self.draft.validate());
        self.validation_errors = input_errors;
        cx.notify();
    }

    /// Attribute every fillable box that changed since last seen to the user,
    /// unless the change is the agent's draft being written in.
    fn note_box_changes(&mut self) {
        let now = assist::fillable_values_1601c(&self.draft);
        if !self.applying_agent_draft && self.draft.is_editable() {
            let changed = assist::changed_keys(&self.known_values, &now);
            if !changed.is_empty() {
                self.assist.note_user_edits(changed);
                self.dismiss_confirm_open = false;
            }
        }
        self.known_values = now;
    }

    /// Forget box changes made by a reload (status refresh): they are not the
    /// user's typing.
    fn rebase_known_values(&mut self) {
        self.known_values = assist::fillable_values_1601c(&self.draft);
    }

    pub(crate) fn agent_assist(&self) -> &AssistState {
        &self.assist
    }

    pub(crate) fn agent_set_assist(&mut self, assist: AssistState, cx: &mut Context<Self>) {
        if self.assist != assist {
            self.assist = assist;
            cx.notify();
        }
    }

    /// Write the agent host's draft into the form when its fillable boxes
    /// differ from what the form shows (a `form.fill` of any box).
    pub(crate) fn agent_apply_draft(
        &mut self,
        draft: &Form1601CDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.is_editable()
            || !draft.is_editable()
            || assist::fillable_values_1601c(&self.draft) == assist::fillable_values_1601c(draft)
        {
            return;
        }
        self.draft = draft.clone();
        self.is_amended = draft.is_amended;
        self.any_taxes_withheld = draft.taxes_withheld_answer();
        self.category_of_agent = draft.category_of_agent.clone();
        self.tax_relief = draft.tax_relief;
        Self::set_input_if_changed(
            &self.number_of_sheets,
            &draft.number_of_sheets.to_string(),
            window,
            cx,
        );
        Self::set_input_if_changed(&self.atc, &draft.atc, window, cx);
        let relief =
            bir_core::validation::official_tax_relief_code(&draft.tax_relief_specification, true);
        if selected_tax_relief_code(&self.tax_relief_specification, cx)
            != relief.clone().unwrap_or_default()
        {
            self.tax_relief_specification
                .update(cx, |select, cx| match &relief {
                    Some(code) => select.set_selected_value(code, window, cx),
                    None => select.set_selected_index(None, window, cx),
                });
        }
        if self.schedule_row_inputs.len() != draft.schedule_1.len() {
            self.schedule_row_inputs = draft
                .schedule_1
                .iter()
                .map(|row| Self::new_schedule_row_inputs(row, window, cx))
                .collect();
        } else {
            for (row, inputs) in draft.schedule_1.iter().zip(&self.schedule_row_inputs) {
                Self::set_input_if_changed(&inputs.previous_month, &row.previous_month, window, cx);
                Self::set_input_if_changed(&inputs.date_paid, &row.date_paid, window, cx);
                Self::set_input_if_changed(
                    &inputs.drawee_bank_code_or_agency,
                    &row.drawee_bank_code_or_agency,
                    window,
                    cx,
                );
                Self::set_input_if_changed(&inputs.payment_number, &row.payment_number, window, cx);
                Self::set_money_if_changed(&inputs.tax_paid, row.tax_paid, window, cx);
                Self::set_money_if_changed(
                    &inputs.should_be_tax_due,
                    row.should_be_tax_due,
                    window,
                    cx,
                );
            }
        }
        for (input, value) in [
            (
                &self.tax_14_total_compensation,
                draft.tax_14_total_compensation,
            ),
            (
                &self.tax_15_statutory_minimum_wage,
                draft.tax_15_statutory_minimum_wage,
            ),
            (&self.tax_16_holiday_pay, draft.tax_16_holiday_pay),
            (&self.tax_17_13th_month_pay, draft.tax_17_13th_month_pay),
            (&self.tax_18_de_minimis, draft.tax_18_de_minimis),
            (&self.tax_19_sss_gsis, draft.tax_19_sss_gsis),
            (&self.tax_20_other_amount, draft.tax_20_other_amount),
            (&self.tax_23_not_subject, draft.tax_23_not_subject),
            (
                &self.tax_25_total_taxes_withheld,
                draft.tax_25_total_taxes_withheld,
            ),
            (
                &self.tax_28_tax_remitted_previously,
                draft.tax_28_tax_remitted_previously,
            ),
            (
                &self.tax_29_other_remittances_amount,
                draft.tax_29_other_remittances_amount,
            ),
            (&self.tax_32_surcharge, draft.tax_32_surcharge),
            (&self.tax_33_interest, draft.tax_33_interest),
            (&self.tax_34_compromise, draft.tax_34_compromise),
        ] {
            Self::set_money_if_changed(input, value, window, cx);
        }
        Self::set_input_if_changed(
            &self.tax_20_other_name,
            &draft.tax_20_other_name,
            window,
            cx,
        );
        Self::set_input_if_changed(
            &self.tax_29_other_remittances_name,
            &draft.tax_29_other_remittances_name,
            window,
            cx,
        );
        self.is_validated = false;
        self.applying_agent_draft = true;
        self.sync_from_inputs(cx);
        self.applying_agent_draft = false;
    }

    /// Write an amount only when the parsed input differs, keeping the user's
    /// own formatting of an equal amount.
    fn set_money_if_changed(
        input: &Entity<InputState>,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = input.read(cx).value().trim().replace(',', "");
        let current = if current.is_empty() {
            Some(0.0)
        } else {
            current.parse::<f64>().ok()
        };
        if current != Some(value) {
            input.update(cx, |input, cx| {
                input.set_value(format!("{value:.2}"), window, cx);
            });
        }
    }

    /// The Dismiss button: close without saving, asking first when there are
    /// unsaved changes (the user's or the agent's).
    fn request_dismiss(&mut self, cx: &mut Context<Self>) {
        if self.assist.unsaved_changes && self.draft.is_editable() {
            self.dismiss_confirm_open = true;
            cx.notify();
        } else {
            self.dismiss_confirm_open = false;
            cx.emit(Form1601CEvent::Dismissed);
        }
    }

    fn add_schedule_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.draft.status, FilingStatus::Draft)
            || self.draft.schedule_1.len() >= MAX_SCHEDULE_1_ROWS
        {
            return;
        }

        self.sync_from_inputs(cx);
        let row = Form1601CSchedule1Row::default();
        self.schedule_row_inputs
            .push(Self::new_schedule_row_inputs(&row, window, cx));
        self.draft.schedule_1.push(row);
        self.finish_schedule_mutation(cx);
    }

    fn remove_schedule_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if !matches!(self.draft.status, FilingStatus::Draft)
            || index >= self.draft.schedule_1.len()
            || index >= self.schedule_row_inputs.len()
        {
            return;
        }

        self.sync_from_inputs(cx);
        self.draft.schedule_1.remove(index);
        self.schedule_row_inputs.remove(index);
        self.finish_schedule_mutation(cx);
    }

    fn finish_schedule_mutation(&mut self, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        self.is_validated = false;
    }

    pub(crate) fn agent_draft(&self) -> &Form1601CDraft {
        &self.draft
    }

    pub(crate) fn agent_validated(&self) -> bool {
        self.is_validated
    }

    pub(crate) fn agent_validation_errors(&self) -> Vec<(String, String)> {
        self.validation_errors.clone()
    }

    pub(crate) fn agent_scroll_handle(&self) -> ScrollHandle {
        self.scroll_handle.clone()
    }

    /// Write `value` into `input` only when it differs, and report whether it
    /// was written. `InputState::set_value` emits `InputEvent::Change` even for
    /// an identical value, and this view answers that with `sync_from_inputs`.
    fn set_input_if_changed(
        input: &Entity<InputState>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if input.read(cx).value() == value {
            return false;
        }
        input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx);
        });
        true
    }

    pub(crate) fn agent_apply_from_host(
        &mut self,
        patch: Agent1601CHostPatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Queued / Submitted / Confirmed is an immutable snapshot: there is
        // nothing to fill, save, or re-validate. `apply_host` runs for every
        // `Op::Invoke`, read-only ones included, so re-applying the patch here
        // re-ran `sync_from_inputs` (and an `InputEvent::Change` per field) on
        // every `form.fields` poll for a return that cannot change.
        if !self.draft.is_editable() {
            return;
        }
        let mut dirty = apply_1601c_host_header_patch(
            &patch,
            &mut self.any_taxes_withheld,
            &mut self.category_of_agent,
            &mut self.draft,
        );
        // Only write an input when the text actually differs. `set_value`
        // emits `InputEvent::Change` unconditionally, and each one costs a
        // full `sync_from_inputs` + validate pass.
        if let Some(value) = patch.tax_14
            && Self::set_input_if_changed(
                &self.tax_14_total_compensation,
                &format!("{value:.2}"),
                window,
                cx,
            )
        {
            dirty = true;
        }
        if let Some(value) = patch.tax_25
            && Self::set_input_if_changed(
                &self.tax_25_total_taxes_withheld,
                &format!("{value:.2}"),
                window,
                cx,
            )
        {
            dirty = true;
        }
        if let Some(value) = patch.sheets
            && Self::set_input_if_changed(&self.number_of_sheets, &value.to_string(), window, cx)
        {
            dirty = true;
        }
        if dirty || patch.validate {
            self.sync_from_inputs(cx);
        }
        if patch.validate {
            self.draft.compute();
            self.validation_errors = self.draft.validate();
            self.is_validated = true;
        }
        if patch.save {
            // Queued/Submitted/Confirmed must never take the generic Draft save
            // path (DB rejects it and the UI toaster spams on every agent drain).
            if self.draft.is_editable() {
                self.save_draft(window, cx);
            }
        }
    }

    pub(crate) fn agent_sync_filing_snapshot(
        &mut self,
        draft: &Form1601CDraft,
        cx: &mut Context<Self>,
    ) {
        let claim_changed = self.draft.submission_claim_token != draft.submission_claim_token
            || self.draft.submission_claimed_at != draft.submission_claimed_at;
        if self.draft.status == draft.status
            && !claim_changed
            && self.draft.submission_error == draft.submission_error
        {
            return;
        }
        self.status_message = draft.submission_error.clone();
        self.is_amended = draft.is_amended;
        self.any_taxes_withheld = draft.taxes_withheld_answer();
        self.category_of_agent = draft.category_of_agent.clone();
        self.draft = draft.clone();
        self.rebase_known_values();
        self.is_validated = false;
        self.validation_errors.clear();
        self.release_claim_confirm_open = false;
        cx.notify();
    }

    pub(crate) fn agent_reload_filing_from_db(&mut self, cx: &mut Context<Self>) {
        let tin = self.draft.tin.clone();
        let year = self.draft.taxable_year;
        let month = self.draft.month;
        let Some(updated) = self
            .db
            .lock()
            .ok()
            .and_then(|db| db.get_1601c_draft(&tin, year, month).ok().flatten())
        else {
            return;
        };
        self.agent_sync_filing_snapshot(&updated, cx);
    }

    pub(crate) fn agent_preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_pdf(window, cx);
    }

    fn show_scaffold_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(SCAFFOLD_MESSAGE.to_string());
        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message(SCAFFOLD_MESSAGE.to_string())
                .with_type(gpui_component::notification::NotificationType::Warning)
                .autohide(true),
            cx,
        );
        cx.notify();
    }

    fn confirm_release_abandoned_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tin = self.draft.tin.clone();
        let year = self.draft.taxable_year;
        let month = self.draft.month;
        let outcome = match self.db.lock() {
            Ok(db) => db.release_abandoned_claimed_1601c_submission(
                &tin,
                year,
                month,
                ABANDONED_CLAIM_RELEASE_REASON,
            ),
            Err(error) => {
                self.status_message = Some(format!("Could not access the form database: {error}"));
                cx.notify();
                return;
            }
        };
        match outcome {
            Ok(AbandonedClaimRelease::Released { draft, .. })
            | Ok(AbandonedClaimRelease::AlreadyClear {
                draft: Some(draft), ..
            }) => {
                self.is_amended = draft.is_amended;
                self.any_taxes_withheld = draft.taxes_withheld_answer();
                self.category_of_agent = draft.category_of_agent.clone();
                self.draft = draft;
                self.release_claim_confirm_open = false;
                self.is_validated = false;
                self.validation_errors.clear();
                self.status_message =
                    Some("Returned to editable Draft. Nothing was filed.".to_string());
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("Returned to editable Draft. Nothing was filed.".to_string())
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
                cx.emit(Form1601CEvent::Saved);
            }
            Ok(AbandonedClaimRelease::AlreadyClear { draft: None, .. }) => {
                self.release_claim_confirm_open = false;
                self.status_message = Some("No 1601-C row to release.".to_string());
            }
            Err(error) => {
                self.status_message = Some(error.to_string());
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(error.to_string())
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
            }
        }
        cx.notify();
    }
}

impl FormViewTrait for Form1601CView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1601C"
    }

    fn form_subtitle(&self) -> &'static str {
        "Monthly Remittance Return of Income Taxes Withheld on Compensation"
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
        None
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.draft.is_editable() {
            // Immutable snapshot — ignore Save Draft / agent save patches quietly.
            cx.notify();
            return;
        }
        self.sync_from_inputs(cx);
        let save_result = match self.db.lock() {
            Ok(db) => db
                .save_1601c_draft(&self.draft)
                .map_err(|error| error.to_string()),
            Err(error) => Err(format!("Could not access the form database: {error}")),
        };

        use gpui_component::WindowExt;
        match save_result {
            Ok(_) => {
                self.assist.note_saved();
                self.dismiss_confirm_open = false;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("Form saved.".to_string())
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
                cx.emit(Form1601CEvent::Saved);
            }
            Err(error) => {
                self.status_message = Some(error.clone());
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("Could not save Form 1601-C: {error}"))
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
                cx.notify();
            }
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            submission_disposition(),
            SubmissionDisposition::ManualExternal
        ) {
            self.show_scaffold_message(window, cx);
            return;
        }
        if !self.draft.is_editable() {
            self.status_message = Some(
                "This return is already queued or filed and cannot be queued again.".to_string(),
            );
            cx.notify();
            return;
        }
        self.sync_from_inputs(cx);
        if !self.validation_errors.is_empty() {
            self.status_message = Some("Fix validation errors before submitting.".to_string());
            cx.notify();
            return;
        }

        let before_queue = self.draft.clone();
        if let Err(errors) = self.draft.transition_to_queued() {
            self.validation_errors = errors;
            self.status_message = Some("Fix validation errors before submitting.".to_string());
            cx.notify();
            return;
        }
        let save_result = match self.db.lock() {
            Ok(db) => db
                .save_queued_1601c_draft(&self.draft)
                .map_err(|error| error.to_string()),
            Err(error) => Err(format!("Could not access the form database: {error}")),
        };
        if let Err(error) = save_result {
            self.draft = before_queue;
            self.status_message = Some(bir_core::forms::queueable::queue_failure_message(
                "Form 1601-C",
                &error,
            ));
            cx.notify();
            return;
        }

        // D5C: the toast and the stepper are the only confirmation.
        self.status_message = None;
        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message("Form 1601-C queued.".to_string())
                .with_type(gpui_component::notification::NotificationType::Success)
                .autohide(true),
            cx,
        );
        cx.emit(Form1601CEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "1601-C payment status requires a separately verified confirmation workflow."
                .to_string(),
        );
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // D7A: Queued, Submitted and Confirmed revert; Paid never does.
        if !can_user_revert(&self.draft.status) {
            return;
        }
        let claimed = self.draft.submission_claim_token.is_some()
            || self.draft.submission_claimed_at.is_some();
        if claimed && matches!(self.draft.status, FilingStatus::Queued) {
            self.release_claim_confirm_open = true;
            self.status_message = Some(
                "Confirm nothing reached BIR to return this claimed queue to an editable Draft. This does not file."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        let tin = self.draft.tin.clone();
        let year = self.draft.taxable_year;
        let month = self.draft.month;
        // D8A: a Submitted or Confirmed return keeps a read-only snapshot of
        // what BIR received and becomes an amended Draft.
        let outcome = match self.db.lock() {
            Ok(db) => db
                .revert_1601c_for_amendment(&tin, year, month)
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        use gpui_component::WindowExt;
        let reverted = match outcome {
            Ok(reverted) => reverted,
            Err(error) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_1601c_draft(&tin, year, month)
                {
                    self.draft = current;
                }
                self.status_message = Some(format!("Could not revert to Draft: {error}"));
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("Could not revert to Draft: {error}"))
                        .with_type(gpui_component::notification::NotificationType::Warning)
                        .autohide(false),
                    cx,
                );
                cx.notify();
                return;
            }
        };
        let amended = reverted.snapshot_id.is_some();
        let draft = reverted.draft;
        // Refresh the editors so Amended Return shows Yes.
        self.is_amended = draft.is_amended;
        self.any_taxes_withheld = draft.taxes_withheld_answer();
        self.category_of_agent = draft.category_of_agent.clone();
        self.draft = draft;
        self.release_claim_confirm_open = false;
        self.status_message = amended.then(|| AMENDED_REVERT_MESSAGE.to_string());
        self.is_validated = false;
        self.validation_errors.clear();
        if amended {
            self.header.load_sent_snapshot_for(
                &self.db,
                "1601C",
                &tin,
                year,
                &FilingPeriod::Monthly(month),
            );
        }
        cx.emit(Form1601CEvent::Saved);
        cx.notify();
    }

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Synchronize the editor first, then hand the preview host an immutable
        // snapshot. Opening or retrying a preview never changes filing status.
        self.sync_from_inputs(cx);
        let render_draft = self.draft.clone();
        let receipt = super::form_html_preview_launcher::receipt_page_for(
            &self.db,
            render_draft.receipt_id,
            &render_draft.email_address,
        );
        match super::form_html_preview_launcher::launch_frozen_form_preview_with_receipt(
            "1601c-2018",
            &render_draft.to_bir_field_map(),
            receipt,
            "1601-C — Print Preview",
            cx,
        ) {
            Ok(launch_kind) => {
                self.status_message = Some(launch_kind.status_message().to_string());
                launch_kind.observe_close(cx, |this, cx| {
                    this.status_message = None;
                    cx.notify();
                });
                cx.notify();
            }
            Err(error) => {
                let message = format!(
                    "HTML print preview could not be opened: {error}. No filing state was changed."
                );
                self.status_message = Some(message.clone());
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(message.clone())
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
                cx.emit(Form1601CEvent::PushNotification(
                    "error".to_string(),
                    "HTML preview unavailable".to_string(),
                    message,
                ));
                cx.notify();
            }
        }
    }
}

/// Error keys that have no input on this page; they come from the taxpayer profile.
const PROFILE_SOURCED_FIELDS: [&str; 4] = [
    "email_address",
    "zip_code",
    "contact_number",
    "line_of_business",
];

fn is_profile_sourced(field: &str) -> bool {
    PROFILE_SOURCED_FIELDS.contains(&field)
}

impl Form1601CView {}

impl Render for Form1601CView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);
        let is_queued = matches!(self.draft.status, FilingStatus::Queued);
        let claim_active = self.draft.submission_claim_token.is_some()
            || self.draft.submission_claimed_at.is_some();
        let release_confirm =
            (is_queued && claim_active && self.release_claim_confirm_open).then(|| {
                div()
                    .mt_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_component::button::Button::new("form-1601c-release-claim-confirm")
                            .label("Confirm nothing reached BIR")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_release_abandoned_claim(window, cx);
                            })),
                    )
                    .child(
                        gpui_component::button::Button::new("form-1601c-keep-queued")
                            .label("Keep queued")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.release_claim_confirm_open = false;
                                cx.notify();
                            })),
                    )
            });
        self.header.ensure_sent_snapshot_for(
            &self.db,
            "1601C",
            &self.draft.tin,
            self.draft.taxable_year,
            &FilingPeriod::Monthly(self.draft.month),
        );
        let props = self.header_props();
        let header_buttons =
            render_header_buttons(self, &props, HEADER_ACTIONS, layout == Layout::Phone, cx);
        let status_box = render_status_box(self, &props, HEADER_ACTIONS, cx)
            .map(|b| div().mt_3().child(b).into_any_element())
            .unwrap_or_else(|| div().into_any_element());
        let atc_field = self.render_atc_select(window, cx);
        let profile_errors = self.render_profile_errors();
        let assist_panel = self
            .draft
            .is_editable()
            .then(|| {
                crate::components::assist_panel::assist_panel(
                    "form-1601c-assist",
                    &self.assist.agent_filled(),
                    &assist::needs_you_1601c(&self.draft),
                    cx,
                )
            })
            .flatten();
        let dismiss_actions = if self.dismiss_confirm_open {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Discard unsaved changes?"),
                )
                .child(
                    gpui_component::button::Button::new("form-1601c-dismiss-confirm")
                        .label("Discard and close")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dismiss_confirm_open = false;
                            cx.emit(Form1601CEvent::Dismissed);
                        })),
                )
                .child(
                    gpui_component::button::Button::new("form-1601c-dismiss-keep")
                        .label("Keep editing")
                        .ghost()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dismiss_confirm_open = false;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            gpui_component::button::Button::new("form-1601c-dismiss")
                .label("Dismiss")
                .ghost()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_dismiss(cx);
                }))
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(rsx! {
                <div flex items_center justify_between flex_wrap gap_2 py_4
                    px={pad}
                    bg={cx.theme().background}
                    border_b_1
                    border_color={cx.theme().border}>
                    {gpui_component::button::Button::new("back_btn")
                        .label("← Back")
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(Form1601CEvent::BackToDashboard);
                        }))}
                    <div flex items_center gap_3>
                        {dismiss_actions}
                        {header_buttons}
                    </div>
                </div>
            })
            .when_some(assist_panel, |view, panel| view.child(panel))
            .child(rsx! {
                <div p={pad} border_b_1
                    border_color={cx.theme().border}
                    bg={cx.theme().background}>
                    {self.render_header_with_preview(cx)}
                    <div id={crate::agent::ids::FORM_1601C_STATUS} mt_6>{self.render_status_pipeline(cx)}</div>
                    {status_box}
                    {div().children(release_confirm)}
                </div>
            })
            .child(rsx! {
                <div id={crate::agent::ids::FORM_1601C_SCROLL} flex_1 w_full overflow_y_scroll
                    track_scroll={&self.scroll_handle}
                    p={pad}>
                    <div max_w={CONTENT_MAX_W} w_full mx_auto flex flex_col gap_6>
                        <div bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_lg>
                            <div flex flex_col p_4>
                                <div text_sm
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().muted_foreground}>
                                    {"TAXPAYER PROFILE"}
                                </div>
                                <div mt_2 flex flex_wrap gap_4 text_sm>
                                    <div>{format!("1 For the Month (MM): {:02}", self.draft.month)}</div>
                                    <div>{format!("For the Year (YYYY): {}", self.draft.taxable_year)}</div>
                                </div>
                                <div mt_2 flex gap_4>
                                    <div text_xl font_weight={FontWeight::BOLD}>
                                        {self.draft.taxpayer_name.clone()}
                                    </div>
                                </div>
                                <div mt_1 text_sm>{format!("TIN: {}", self.draft.tin)}</div>
                                <div mt_1 text_sm>{format!(
                                    "RDO Code: {}",
                                    self.draft.rdo_code
                                )}</div>
                                <div mt_1 text_sm>{format!(
                                    "Address: {}",
                                    self.draft.registered_address
                                )}</div>
                                {profile_errors}
                            </div>
                        </div>
                        {self.render_schedule_section(layout, is_draft, cx)}
                        <div bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_lg>
                            {div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(rsx! {
                                                <div text_xl font_weight={FontWeight::BOLD}>
                                                    {"Part I - Background Information"}
                                                </div>
                                            })
                                            .child(self.render_yes_no(
                                                layout,
                                                "Amended Return?",
                                                "amended_btn",
                                                Some(self.is_amended),
                                                |this, value| this.is_amended = value,
                                                cx,
                                            ))
                                            .child(self.render_yes_no(
                                                layout,
                                                "Any Taxes Withheld?",
                                                crate::agent::ids::FORM_1601C_WITHHELD,
                                                self.any_taxes_withheld,
                                                |this, value| this.any_taxes_withheld = Some(value),
                                                cx,
                                            ))
                                            .child(div().id(crate::agent::ids::FORM_1601C_SHEETS).child(self.render_input_row(layout,
                                                "Number of Sheets Attached",
                                                &self.number_of_sheets,
                                                &["number_of_sheets"],
                                                cx,
                                            )))
                                            .child(atc_field)
                                            .child(self.render_category_of_agent(layout, cx))
                                            .child(rsx! {
                                                <div flex flex_wrap gap_2 items_center>
                                                    <div>{
                                                        "13 Payees availing of tax relief under Special Law or International Tax Treaty?"
                                                    }</div>
                                                    {div()
                                                            .id("tax_relief_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(if self.tax_relief {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().border
                                                            })
                                                            .bg(if self.tax_relief {
                                                                cx.theme().primary.opacity(0.2)
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .rounded_md()
                                                            .when(is_draft, |button| {
                                                                button.cursor_pointer()
                                                            })
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    if !matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        return;
                                                                    }
                                                                    this.tax_relief =
                                                                        !this.tax_relief;
                                                                    if !this.tax_relief {
                                                                        this.tax_relief_specification.update(
                                                                            cx,
                                                                            |select, cx| {
                                                                                select.set_selected_index(
                                                                                    None, window, cx,
                                                                                );
                                                                            },
                                                                        );
                                                                    }
                                                                    this.sync_from_inputs(cx);
                                                                },
                                                            ))
                                                            .child(if self.tax_relief {
                                                                "Yes"
                                                            } else {
                                                                "No"
                                                            })}
                                                </div>
                                            })
                                            .when(self.tax_relief, |content| {
                                                content.child(self.render_tax_relief_row(layout))
                                            })}
                        </div>
                        <div bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_lg>
                            <div flex flex_col gap_4 p_4>
                                <div text_xl font_weight={FontWeight::BOLD}>
                                    {"Part II - Computation of Tax"}
                                </div>
                                {div().id(crate::agent::ids::FORM_1601C_TAX_14).child(self.render_input_row(layout,
                                    "14 Total Amount of Compensation",
                                    &self.tax_14_total_compensation,
                                    &["tax_14_total_compensation"],
                                    cx,
                                ))}
                                <div text_lg font_weight={FontWeight::SEMIBOLD} mt_4>
                                    {"Less: Non-Taxable/Exempt Compensation"}
                                </div>
                                {self.render_input_row(layout,
                                    "15 Statutory Minimum Wage",
                                    &self.tax_15_statutory_minimum_wage,
                                    &["tax_15_statutory_minimum_wage"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "16 Holiday Pay, Overtime Pay, Night Shift",
                                    &self.tax_16_holiday_pay,
                                    &["tax_16_holiday_pay"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "17 13th Month Pay and Other Benefits",
                                    &self.tax_17_13th_month_pay,
                                    &["tax_17_13th_month_pay"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "18 De Minimis Benefits",
                                    &self.tax_18_de_minimis,
                                    &["tax_18_de_minimis"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "19 SSS, GSIS, PHIC, HDMF Contributions",
                                    &self.tax_19_sss_gsis,
                                    &["tax_19_sss_gsis"],
                                    cx,
                                )}
                                {self.render_input_with_text_row(layout,
                                    "20 Other Non-Taxable Compensation",
                                    &self.tax_20_other_name,
                                    &self.tax_20_other_amount,
                                    &["tax_20_other_amount"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "21 Total Non-Taxable Compensation",
                                    self.draft.tax_21_total_non_taxable,
                                    &["tax_21_total_non_taxable"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "22 Total Taxable Compensation (14 - 21)",
                                    self.draft.tax_22_total_taxable,
                                    &["tax_22_total_taxable"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "23 Less: Taxable comp not subject to withholding",
                                    &self.tax_23_not_subject,
                                    &["tax_23_not_subject"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "24 Net Taxable Compensation (22 - 23)",
                                    self.draft.tax_24_net_taxable,
                                    &["tax_24_net_taxable"],
                                    cx,
                                )}
                                {div().id(crate::agent::ids::FORM_1601C_TAX_25).child(self.render_input_row(layout,
                                    "25 Total Taxes Withheld",
                                    &self.tax_25_total_taxes_withheld,
                                    &["tax_25_total_taxes_withheld"],
                                    cx,
                                ))}
                                {self.render_computed_row(layout,
                                    "26 Add/Less: Adjustment from Schedule I",
                                    self.draft.tax_26_adjustment,
                                    &["tax_26_adjustment"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "27 Taxes Withheld for Remittance",
                                    self.draft.tax_27_taxes_withheld_for_remittance,
                                    &["tax_27_taxes_withheld_for_remittance"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "28 Less: Tax Remitted in Return Previously Filed",
                                    &self.tax_28_tax_remitted_previously,
                                    &["tax_28_tax_remitted_previously"],
                                    cx,
                                )}
                                {self.render_input_with_text_row(layout,
                                    "29 Other Remittances Made",
                                    &self.tax_29_other_remittances_name,
                                    &self.tax_29_other_remittances_amount,
                                    &["tax_29_other_remittances_amount"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "30 Total Tax Remittances Made",
                                    self.draft.tax_30_total_tax_remittances,
                                    &["tax_30_total_tax_remittances"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "31 Tax Still Due/(Overremittance)",
                                    self.draft.tax_31_tax_still_due,
                                    &["tax_31_tax_still_due"],
                                    cx,
                                )}
                            </div>
                        </div>
                        <div bg={cx.theme().background}
                            border_1
                            border_color={cx.theme().border}
                            rounded_lg>
                            <div flex flex_col gap_4 p_4>
                                <div text_xl font_weight={FontWeight::BOLD}>
                                    {"Add: Penalties"}
                                </div>
                                {self.render_input_row(layout,
                                    "32 Surcharge",
                                    &self.tax_32_surcharge,
                                    &["tax_32_surcharge"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "33 Interest",
                                    &self.tax_33_interest,
                                    &["tax_33_interest"],
                                    cx,
                                )}
                                {self.render_input_row(layout,
                                    "34 Compromise",
                                    &self.tax_34_compromise,
                                    &["tax_34_compromise"],
                                    cx,
                                )}
                                {self.render_computed_row(layout,
                                    "35 Total Penalties",
                                    self.draft.tax_35_total_penalties,
                                    &["tax_35_total_penalties"],
                                    cx,
                                )}
                            </div>
                        </div>
                        <div bg={cx.theme().primary.opacity(0.1)}
                            border_1
                            border_color={cx.theme().primary.opacity(0.2)}
                            rounded_lg>
                            <div flex justify_between items_center flex_wrap gap_2 p_6>
                                <div text_2xl
                                    font_weight={FontWeight::BOLD}
                                    text_color={cx.theme().primary}>
                                    {"36 Total Amount Payable"}
                                </div>
                                <div text_2xl
                                    font_weight={FontWeight::BLACK}
                                    text_color={cx.theme().primary}>
                                    {format!(
                                        "P {}",
                                        official_amount(self.draft.tax_36_total_amount_payable)
                                    )}
                                </div>
                                {div().px_6().pb_4().children(
                                    self.error_for(&["tax_36_total_amount_payable"])
                                        .map(field_error),
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            })
    }
}

impl Form1601CView {
    fn render_schedule_section(
        &self,
        layout: Layout,
        is_editable: bool,
        cx: &Context<Self>,
    ) -> Div {
        let row_count = self.draft.schedule_1.len();
        let rows = self
            .schedule_row_inputs
            .iter()
            .enumerate()
            .map(|(index, inputs)| {
                let adjustment = self
                    .draft
                    .schedule_1
                    .get(index)
                    .map(|row| row.adjustment)
                    .unwrap_or(0.0);
                // Row-level messages (month, date, bank, reference, adjustment)
                // are keyed by the row; the two money inputs also have parse keys.
                let row_key = format!("schedule_1_row_{}", index + 1);
                let tax_paid_key = format!("schedule_1_row_{}_tax_paid", index + 1);
                let should_be_key = format!("schedule_1_row_{}_should_be_tax_due", index + 1);
                rsx! {
                    <div flex flex_col gap_3 p_4 border_1
                        border_color={cx.theme().border}
                        rounded_md>
                        {div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(rsx! {
                                <div font_weight={FontWeight::BOLD}>
                                    {format!("Schedule I row {}", index + 1)}
                                </div>
                            })
                            .when(is_editable, |header| {
                                header.child(
                                    gpui_component::button::Button::new(format!(
                                        "remove_1601c_schedule_row_{}",
                                        index + 1
                                    ))
                                    .label("Remove")
                                    .small()
                                    .outline()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.remove_schedule_row(index, cx);
                                        },
                                    )),
                                )
                            })}
                        {self.render_input_row(layout,
                            "1 Previous Month (MM/YYYY)",
                            &inputs.previous_month,
                            &[],
                            cx,
                        )}
                        {self.render_input_row(layout,"2 Date Paid (MM/DD/YYYY)", &inputs.date_paid, &[], cx)}
                        {self.render_input_row(layout,
                            "3 Drawee Bank / Bank Code / Agency",
                            &inputs.drawee_bank_code_or_agency,
                            &[],
                            cx,
                        )}
                        {self.render_input_row(layout,
                            "4 Payment Reference Number",
                            &inputs.payment_number,
                            &[],
                            cx,
                        )}
                        {self.render_input_row(layout,
                            "5 Tax Paid (excluding penalties)",
                            &inputs.tax_paid,
                            &[tax_paid_key.as_str()],
                            cx,
                        )}
                        {self.render_input_row(layout,
                            "6 Should Be Tax Due for the Month",
                            &inputs.should_be_tax_due,
                            &[should_be_key.as_str()],
                            cx,
                        )}
                        {self.render_computed_row(layout,
                            "7 Adjustment (Item 6 less Item 5)",
                            adjustment,
                            &[row_key.as_str()],
                            cx,
                        )}
                    </div>
                }
            })
            .collect::<Vec<_>>();

        rsx! {
            <div bg={cx.theme().background}
                border_1
                border_color={cx.theme().border}
                rounded_lg>
                <div flex flex_col gap_4 p_4>
                    <div flex items_center justify_between flex_wrap gap_2>
                        <div flex flex_col gap_1 flex_1 min_w_0>
                            <div text_xl font_weight={FontWeight::BOLD}>
                                {"Part IV - Schedule I"}
                            </div>
                            <div text_sm text_color={cx.theme().muted_foreground}>
                                {
                                    "Adjustment of Taxes Withheld on Compensation from Previous Months"
                                }
                            </div>
                        </div>
                        {div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(rsx! {
                                        <div text_xs text_color={cx.theme().muted_foreground}>
                                            {format!("{row_count} of {MAX_SCHEDULE_1_ROWS} rows")}
                                        </div>
                                    })
                                    .when(is_editable, |controls| {
                                        controls.child(
                                            gpui_component::button::Button::new(
                                                "add_1601c_schedule_row",
                                            )
                                            .label("Add Schedule Row")
                                            .outline()
                                            .disabled(row_count >= MAX_SCHEDULE_1_ROWS)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.add_schedule_row(window, cx);
                                            })),
                                        )
                                    })}
                    </div>
                    {...rows}
                    {self.render_computed_row(layout,
                        "4 Total Adjustment (to Part II, Item 26)",
                        self.draft.tax_26_adjustment,
                        &["schedule_1"],
                        cx,
                    )}
                </div>
            </div>
        }
    }

    /// Label/control pair: side by side above Phone, stacked on Phone.
    fn pair_row(layout: Layout) -> Div {
        if layout == Layout::Phone {
            div().flex().flex_col().gap_1().w_full()
        } else {
            div().flex().justify_between().items_center().gap_4()
        }
    }

    fn half(layout: Layout) -> Div {
        if layout == Layout::Phone {
            div().w_full()
        } else {
            div().w_1_2()
        }
    }

    /// Messages recorded for any of `keys`, joined into one line.
    fn error_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(self.validation_errors.iter(), keys)
    }

    /// Put the messages for `keys` directly under an already-built row.
    fn with_errors(&self, row: impl IntoElement, keys: &[&str]) -> AnyElement {
        with_field_error(row.into_any_element(), self.error_for(keys))
    }

    /// Messages for the values this page shows read-only or not at all (they
    /// come from the taxpayer profile), one line each under the profile block.
    fn render_profile_errors(&self) -> AnyElement {
        const LINES: [(&str, &[&str]); 9] = [
            ("Period", &["taxable_year", "month"]),
            ("TIN", &["tin"]),
            ("RDO", &["rdo_code"]),
            ("Name", &["taxpayer_name"]),
            ("Address", &["registered_address", "registered_address_2"]),
            ("ZIP code", &["zip_code"]),
            ("Telephone", &["contact_number"]),
            ("Email address", &["email_address"]),
            ("Line of business", &["line_of_business"]),
        ];
        let mut column = div().flex().flex_col().gap_1().mt_2();
        let has_profile_error = self
            .validation_errors
            .iter()
            .any(|(field, _)| is_profile_sourced(field));
        for (label, keys) in LINES {
            if let Some(message) = self.error_for(keys) {
                column = column.child(field_error(format!("{label}: {message}")));
            }
        }
        if has_profile_error {
            column =
                column.child(div().mt_2().text_sm().child(
                    "These come from your taxpayer profile. Edit them in Profile Settings.",
                ));
        }
        column.into_any_element()
    }

    /// Item 5 ATC: a searchable select over the one allowed code.
    fn render_atc_select(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let editable = matches!(self.draft.status, FilingStatus::Draft);
        let shown = (!self.draft.atc.is_empty()).then_some(self.draft.atc.as_str());
        form_controls::sync_select(&self.atc_select, ATC_OPTIONS, shown, window, cx);
        select_field(
            "ATC",
            &self.atc_select,
            ATC_OPTIONS,
            shown,
            !editable,
            self.error_for(&["atc"]).as_deref(),
            cx,
        )
    }

    fn render_tax_relief_row(&self, layout: Layout) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        let row = Self::pair_row(layout)
            .child(
                Self::half(layout)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("13A If yes, specify"),
            )
            .child(
                Self::half(layout).child(
                    Select::new(&self.tax_relief_specification)
                        .placeholder("Choose Special Rate, Treaty, or Both")
                        .disabled(is_disabled),
                ),
            );
        self.with_errors(row, &["tax_relief_specification"])
    }

    /// A labelled input row with the messages for `keys` under it.
    fn render_input_row(
        &self,
        layout: Layout,
        label: &str,
        input: &Entity<InputState>,
        keys: &[&str],
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        let row = Self::pair_row(layout)
            .child(
                Self::half(layout)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(Self::half(layout).child(Input::new(input).disabled(is_disabled)));
        self.with_errors(row, keys)
    }

    fn render_input_with_text_row(
        &self,
        layout: Layout,
        label: &str,
        text_input: &Entity<InputState>,
        amount_input: &Entity<InputState>,
        keys: &[&str],
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        let label_and_text = Self::half(layout)
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(Input::new(text_input).disabled(is_disabled));
        let row = if layout == Layout::Phone {
            div().flex().flex_col().gap_2().w_full()
        } else {
            div().flex().justify_between().items_center().gap_4()
        };
        let row = row
            .child(label_and_text)
            .child(Self::half(layout).child(Input::new(amount_input).disabled(is_disabled)));
        self.with_errors(row, keys)
    }

    /// A computed row; the messages for `keys` appear under it.
    fn render_computed_row(
        &self,
        layout: Layout,
        label: &str,
        value: f64,
        keys: &[&str],
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let row = Self::pair_row(layout)
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                Self::half(layout)
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(label.to_string()),
            )
            .child(
                Self::half(layout)
                    .when(layout != Layout::Phone, |d| d.text_right())
                    .font_weight(FontWeight::BOLD)
                    .child(official_amount(value)),
            );
        self.with_errors(row, keys)
    }

    /// One labelled question with both answers visible (radio-style), the
    /// chosen one primary, as the 2551Q `choice()` helper draws them.
    fn render_yes_no(
        &self,
        layout: Layout,
        label: &'static str,
        id: &'static str,
        value: Option<bool>,
        set: fn(&mut Self, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editable = matches!(self.draft.status, FilingStatus::Draft);
        let choice = |suffix: &'static str, text: &'static str, answer: bool| {
            let selected = value == Some(answer);
            let button =
                gpui_component::button::Button::new(SharedString::from(format!("{id}_{suffix}")))
                    .label(if selected {
                        format!("✓ {text}")
                    } else {
                        text.to_string()
                    });
            let button = if selected {
                button.primary()
            } else {
                button.outline()
            };
            button
                .disabled(!editable)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if matches!(this.draft.status, FilingStatus::Draft) {
                        set(this, answer);
                        this.sync_from_inputs(cx);
                    }
                }))
        };
        let row = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(
                div()
                    .when(layout == Layout::Phone, |d| d.w_full())
                    .child(label),
            )
            .child(
                div()
                    .id(id)
                    .flex()
                    .gap_2()
                    .child(choice("yes", "Yes", true))
                    .child(choice("no", "No", false)),
            );
        // Item 3's "Please select an option for Item 3." shows under it.
        self.with_errors(
            row,
            if id == "amended_btn" {
                &[]
            } else {
                &["any_taxes_withheld"]
            },
        )
    }

    fn render_category_of_agent(&self, layout: Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let is_private = self.category_of_agent == "P";
        let is_government = self.category_of_agent == "G";
        let outer = if layout == Layout::Phone {
            div().flex().flex_col().gap_2()
        } else {
            div().flex().gap_4().items_center()
        };
        let row = outer
            .child(div().child("11 Category of Withholding Agent"))
            .child(
                div()
                    .id(crate::agent::ids::FORM_1601C_CATEGORY)
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(Self::category_choice_chip(
                        "1601c_cat_p",
                        "Private",
                        is_private,
                        "P",
                        cx,
                    ))
                    .child(Self::category_choice_chip(
                        "1601c_cat_g",
                        "Government",
                        is_government,
                        "G",
                        cx,
                    )),
            );
        self.with_errors(row, &["category_of_agent"])
    }

    fn category_choice_chip(
        id: &'static str,
        label: &'static str,
        selected: bool,
        code: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .p_2()
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(if selected {
                cx.theme().primary.opacity(0.2)
            } else {
                cx.theme().background
            })
            .rounded_md()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if matches!(this.draft.status, FilingStatus::Draft) {
                    this.category_of_agent = code.to_string();
                    this.sync_from_inputs(cx);
                }
            }))
            .child(label)
    }
}

impl Form1601CView {
    /// The status box carries the cron submitter's error, then any page
    /// message; without in-app queueing, the scaffold notice.
    fn header_message(&self) -> Option<String> {
        if !matches!(submission_disposition(), SubmissionDisposition::QueueInApp) {
            return Some(
                self.status_message
                    .clone()
                    .unwrap_or_else(|| SCAFFOLD_MESSAGE.to_string()),
            );
        }
        self.draft
            .submission_error
            .clone()
            .or_else(|| self.status_message.clone())
    }

    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1601c",
            status: self.draft.status.clone(),
            editor_safe: true,
            validated: self.is_validated,
            errors: self
                .validation_errors
                .iter()
                .map(|(_, message)| message.replace('\n', " "))
                .collect(),
            content_max_w: CONTENT_MAX_W,
            success_text: DEFAULT_SUCCESS_MESSAGE,
            message: self.header_message(),
            separate_validate: separate_validate_button(&self.db),
        }
    }

    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
        self.draft.compute();
        self.validation_errors = self.draft.validate();
        self.is_validated = true;
        cx.notify();
    }

    fn header_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_draft(window, cx);
    }

    fn header_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mark_submitted(window, cx);
    }

    fn header_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_pdf(window, cx);
    }

    fn header_revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revert_to_draft(window, cx);
    }

    #[cfg(debug_assertions)]
    fn header_simulate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filename = self.draft.submission_filename.clone();
        use gpui_component::WindowExt;
        match crate::dev_mode::simulate_confirmation(&self.db, filename.as_deref()) {
            Ok(()) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_1601c_draft(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        self.draft.month,
                    )
                {
                    self.draft = current;
                }
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(
                            "1601C marked Confirmed (simulated, nothing was sent to BIR)."
                                .to_string(),
                        )
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
                cx.emit(Form1601CEvent::Confirmed);
            }
            Err(error) => window.push_notification(
                gpui_component::notification::Notification::new()
                    .message(format!("Simulated confirmation failed: {error}"))
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            ),
        }
        cx.notify();
    }
}

const HEADER_ACTIONS: FormHeaderActions<Form1601CView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: |this| this.validation_errors.len(),
    on_validate: Form1601CView::header_validate,
    on_submit: Form1601CView::header_submit,
    on_save: Form1601CView::header_save,
    on_print: Form1601CView::header_print,
    on_revert: Form1601CView::header_revert,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1601CView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};
