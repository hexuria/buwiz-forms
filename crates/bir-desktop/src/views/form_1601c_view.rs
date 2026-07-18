//! BIR Form 1601C — Full in-app form view.
#![allow(dead_code)]

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row, MAX_SCHEDULE_1_ROWS};
use bir_core::forms::{FilingStatus, FormValidator, can_queue_for_submission};
use bir_print::html::RenderEnvelopeV1;

use crate::components::form_engine::FormViewTrait;

pub enum Form1601CEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1601CEvent> for Form1601CView {}

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

struct ScheduleRowInputs {
    previous_month: Entity<InputState>,
    date_paid: Entity<InputState>,
    drawee_bank_code_or_agency: Entity<InputState>,
    payment_number: Entity<InputState>,
    tax_paid: Entity<InputState>,
    should_be_tax_due: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

pub struct Form1601CView {
    draft: Form1601CDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    // Header Inputs
    is_amended: bool,
    any_taxes_withheld: bool,
    number_of_sheets: Entity<InputState>,
    atc: Entity<InputState>,
    tax_relief: bool,
    tax_relief_specification: Entity<InputState>,

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
        draft: Form1601CDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
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
        let tax_relief_specification =
            create_text_input(cx, &draft.tax_relief_specification, window);
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
            tax_relief_specification.clone(),
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
                    if became_submitted {
                        cx.emit(Form1601CEvent::Submitted);
                    }
                    cx.notify();
                }
            },
        ));

        Self {
            is_amended: draft.is_amended,
            any_taxes_withheld: draft.any_taxes_withheld,
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            validation_errors: Vec::new(),
            status_message: None,

            number_of_sheets,
            atc,
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
                value.parse::<f64>().unwrap_or_else(|_| {
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
        self.draft.any_taxes_withheld = self.any_taxes_withheld;
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
            get_text(&self.tax_relief_specification, cx)
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

        use bir_core::forms::FormValidator;
        input_errors.extend(self.draft.validate());
        self.validation_errors = input_errors;
        cx.notify();
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
            self.status_message = Some(format!(
                "Could not queue Form 1601-C. No submission was started: {error}"
            ));
            cx.notify();
            return;
        }

        self.status_message = Some("Form queued for background submission.".to_string());
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
        if !matches!(self.draft.status, FilingStatus::Queued)
            || self.draft.submission_claim_token.is_some()
        {
            self.status_message = Some(
                "This immutable return cannot be reverted after submission has started."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        let queued = self.draft.clone();
        let canceled = match self.db.lock() {
            Ok(db) => db.cancel_queued_1601c_submission(&queued).ok(),
            Err(_) => None,
        };
        let Some(canceled) = canceled else {
            self.draft = match self.db.lock() {
                Ok(db) => db
                    .get_1601c_draft(&queued.tin, queued.taxable_year, queued.month)
                    .ok()
                    .flatten()
                    .unwrap_or(queued),
                Err(_) => queued,
            };
            self.status_message = Some(
                "Submission has already started. The queued return was not canceled.".to_string(),
            );
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message(
                        "Submission has already started. Keep any BIR confirmation or receipt and do not retry this return."
                            .to_string(),
                    )
                    .with_type(gpui_component::notification::NotificationType::Warning)
                    .autohide(false),
                cx,
            );
            cx.notify();
            return;
        };
        self.draft = canceled;
        self.status_message = None;
        self.is_validated = false;
        self.validation_errors.clear();
        cx.emit(Form1601CEvent::Saved);
        cx.notify();
    }

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Synchronize the editor first, then hand the preview host an immutable
        // snapshot. Opening or retrying a preview never changes filing status.
        self.sync_from_inputs(cx);
        let render_draft = self.draft.clone();
        let envelope = RenderEnvelopeV1::from(&render_draft);

        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => {
                self.status_message = Some(launch_kind.status_message().to_string());
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

impl Render for Form1601CView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);
        let queue_supported = matches!(submission_disposition(), SubmissionDisposition::QueueInApp);

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
                    .bg(cx.theme().background)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui_component::button::Button::new("back_btn")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form1601CEvent::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("save_draft_btn")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("submit_btn")
                                    .label(if queue_supported {
                                        "Generate XML & Submit"
                                    } else {
                                        "Manual / external filing"
                                    })
                                    .primary()
                                    .disabled(
                                        !queue_supported
                                            || !is_draft
                                            || !self.validation_errors.is_empty(),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.mark_submitted(window, cx);
                                    })),
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
            .when(!queue_supported, |view| {
                view.child(
                    div()
                        .px_8()
                        .py_3()
                        .bg(cx.theme().warning.opacity(0.12))
                        .border_b_1()
                        .border_color(cx.theme().warning.opacity(0.4))
                        .text_sm()
                        .text_color(crate::theme::warning_on_tint(cx.theme()))
                        .child(
                            self.status_message
                                .clone()
                                .unwrap_or_else(|| SCAFFOLD_MESSAGE.to_string()),
                        ),
                )
            })
            .when(queue_supported, |view| {
                let message = self
                    .draft
                    .submission_error
                    .clone()
                    .or_else(|| self.status_message.clone());
                view.when_some(message, |view, message| {
                    view.child(
                        div()
                            .px_8()
                            .py_3()
                            .bg(cx.theme().warning.opacity(0.12))
                            .border_b_1()
                            .border_color(cx.theme().warning.opacity(0.4))
                            .text_sm()
                            .text_color(crate::theme::warning_on_tint(cx.theme()))
                            .child(message),
                    )
                })
            })
            .child(
                div()
                    .id("scroll_container")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(800.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("TAXPAYER PROFILE"),
                                            )
                                            .child(
                                                div().mt_2().flex().gap_4().child(
                                                    div()
                                                        .text_xl()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(self.draft.taxpayer_name.clone()),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .text_sm()
                                                    .child(format!("TIN: {}", self.draft.tin)),
                                            )
                                            .child(div().mt_1().text_sm().child(format!(
                                                "RDO Code: {}",
                                                self.draft.rdo_code
                                            )))
                                            .child(div().mt_1().text_sm().child(format!(
                                                "Address: {}",
                                                self.draft.registered_address
                                            ))),
                                    ),
                            )
                            .child(self.render_schedule_section(is_draft, cx))
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Part I - Background Information"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child("Amended Return?"))
                                                    .child(
                                                        div()
                                                            .id("amended_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(if self.is_amended {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().border
                                                            })
                                                            .bg(if self.is_amended {
                                                                cx.theme().primary.opacity(0.2)
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    if matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        this.is_amended =
                                                                            !this.is_amended;
                                                                        this.sync_from_inputs(cx);
                                                                    }
                                                                },
                                                            ))
                                                            .child(if self.is_amended {
                                                                "Yes"
                                                            } else {
                                                                "No"
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child("Any Taxes Withheld?"))
                                                    .child(
                                                        div()
                                                            .id("withheld_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(
                                                                if self.any_taxes_withheld {
                                                                    cx.theme().primary
                                                                } else {
                                                                    cx.theme().border
                                                                },
                                                            )
                                                            .bg(if self.any_taxes_withheld {
                                                                cx.theme().primary.opacity(0.2)
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    if matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        this.any_taxes_withheld =
                                                                            !this
                                                                                .any_taxes_withheld;
                                                                        this.sync_from_inputs(cx);
                                                                    }
                                                                },
                                                            ))
                                                            .child(if self.any_taxes_withheld {
                                                                "Yes"
                                                            } else {
                                                                "No"
                                                            }),
                                                    ),
                                            )
                                            .child(self.render_input_row(
                                                "Number of Sheets Attached",
                                                &self.number_of_sheets,
                                                cx,
                                            ))
                                            .child(self.render_input_row("ATC", &self.atc, cx))
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child(
                                                        "13 Payees availing of tax relief under Special Law or International Tax Treaty?",
                                                    ))
                                                    .child(
                                                        div()
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
                                                                            |input, cx| {
                                                                                input.set_value(
                                                                                    String::new(),
                                                                                    window,
                                                                                    cx,
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
                                                            }),
                                                    ),
                                            )
                                            .when(self.tax_relief, |content| {
                                                content.child(self.render_input_row(
                                                    "13A If yes, specify",
                                                    &self.tax_relief_specification,
                                                    cx,
                                                ))
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Part II - Computation of Tax"),
                                            )
                                            .child(self.render_input_row(
                                                "14 Total Amount of Compensation",
                                                &self.tax_14_total_compensation,
                                                cx,
                                            ))
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .mt_4()
                                                    .child("Less: Non-Taxable/Exempt Compensation"),
                                            )
                                            .child(self.render_input_row(
                                                "15 Statutory Minimum Wage",
                                                &self.tax_15_statutory_minimum_wage,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "16 Holiday Pay, Overtime Pay, Night Shift",
                                                &self.tax_16_holiday_pay,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "17 13th Month Pay and Other Benefits",
                                                &self.tax_17_13th_month_pay,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "18 De Minimis Benefits",
                                                &self.tax_18_de_minimis,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "19 SSS, GSIS, PHIC, HDMF Contributions",
                                                &self.tax_19_sss_gsis,
                                                cx,
                                            ))
                                            .child(self.render_input_with_text_row(
                                                "20 Other Non-Taxable Compensation",
                                                &self.tax_20_other_name,
                                                &self.tax_20_other_amount,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "21 Total Non-Taxable Compensation",
                                                self.draft.tax_21_total_non_taxable,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "22 Total Taxable Compensation (14 - 21)",
                                                self.draft.tax_22_total_taxable,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "23 Less: Taxable comp not subject to withholding",
                                                &self.tax_23_not_subject,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "24 Net Taxable Compensation (22 - 23)",
                                                self.draft.tax_24_net_taxable,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "25 Total Taxes Withheld",
                                                &self.tax_25_total_taxes_withheld,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "26 Add/Less: Adjustment from Schedule I",
                                                self.draft.tax_26_adjustment,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "27 Taxes Withheld for Remittance",
                                                self.draft.tax_27_taxes_withheld_for_remittance,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "28 Less: Tax Remitted in Return Previously Filed",
                                                &self.tax_28_tax_remitted_previously,
                                                cx,
                                            ))
                                            .child(self.render_input_with_text_row(
                                                "29 Other Remittances Made",
                                                &self.tax_29_other_remittances_name,
                                                &self.tax_29_other_remittances_amount,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "30 Total Tax Remittances Made",
                                                self.draft.tax_30_total_tax_remittances,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "31 Tax Still Due/(Overremittance)",
                                                self.draft.tax_31_tax_still_due,
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Add: Penalties"),
                                            )
                                            .child(self.render_input_row(
                                                "32 Surcharge",
                                                &self.tax_32_surcharge,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "33 Interest",
                                                &self.tax_33_interest,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "34 Compromise",
                                                &self.tax_34_compromise,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "35 Total Penalties",
                                                self.draft.tax_35_total_penalties,
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .bg(cx.theme().primary.opacity(0.1))
                                    .border_1()
                                    .border_color(cx.theme().primary.opacity(0.2))
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .p_6()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().primary)
                                                    .child("36 Total Amount Payable"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "P {:.2}",
                                                        self.draft.tax_36_total_amount_payable
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Form1601CView {
    fn render_schedule_section(&self, is_editable: bool, cx: &Context<Self>) -> Div {
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
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("Schedule I row {}", index + 1)),
                            )
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
                            }),
                    )
                    .child(self.render_input_row(
                        "1 Previous Month (MM/YYYY)",
                        &inputs.previous_month,
                        cx,
                    ))
                    .child(self.render_input_row("2 Date Paid (MM/DD/YYYY)", &inputs.date_paid, cx))
                    .child(self.render_input_row(
                        "3 Drawee Bank / Bank Code / Agency",
                        &inputs.drawee_bank_code_or_agency,
                        cx,
                    ))
                    .child(self.render_input_row(
                        "4 Payment Reference Number",
                        &inputs.payment_number,
                        cx,
                    ))
                    .child(self.render_input_row(
                        "5 Tax Paid (excluding penalties)",
                        &inputs.tax_paid,
                        cx,
                    ))
                    .child(self.render_input_row(
                        "6 Should Be Tax Due for the Month",
                        &inputs.should_be_tax_due,
                        cx,
                    ))
                    .child(self.render_computed_row(
                        "7 Adjustment (Item 6 less Item 5)",
                        adjustment,
                        cx,
                    ))
            })
            .collect::<Vec<_>>();

        div()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Part IV - Schedule I"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Adjustment of Taxes Withheld on Compensation from Previous Months",
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{row_count} of {MAX_SCHEDULE_1_ROWS} rows"
                                            )),
                                    )
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
                                    }),
                            ),
                    )
                    .children(rows)
                    .child(self.render_computed_row(
                        "4 Total Adjustment (to Part II, Item 26)",
                        self.draft.tax_26_adjustment,
                        cx,
                    )),
            )
    }

    fn render_input_row(
        &self,
        label: &str,
        input: &Entity<InputState>,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w_1_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(div().w_1_2().child(Input::new(input).disabled(is_disabled)))
    }

    fn render_input_with_text_row(
        &self,
        label: &str,
        text_input: &Entity<InputState>,
        amount_input: &Entity<InputState>,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w_1_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(label.to_string()),
                    )
                    .child(Input::new(text_input).disabled(is_disabled)),
            )
            .child(
                div()
                    .w_1_2()
                    .child(Input::new(amount_input).disabled(is_disabled)),
            )
    }

    fn render_computed_row(&self, label: &str, value: f64, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .w_1_2()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .w_1_2()
                    .text_right()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{:.2}", value)),
            )
    }
}
