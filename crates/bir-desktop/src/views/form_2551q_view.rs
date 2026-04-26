//! BIR Form 2551Q — Full in-app form view.
//!
//! Single scrollable view that mimics the actual BIR form layout.
//! No wizards. Profile data is pre-filled and read-only.
//! Schedule 1 is editable. Part II auto-computes.

use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_2551q::{FilingStatus, Form2551QDraft, Schedule1Row};
use bir_core::parse_bir_receipt_email;
use bir_core::validation::{validate_email, validate_ph_phone, validate_zip};
use bir_print::render_2551q_print;

use super::pdf_viewer::PdfViewerView;
use super::email_confirmation_view::EmailConfirmationView;
use super::receipt_viewer::{ReceiptViewerView, ReceiptViewerEvent};

pub enum Form2551QEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form2551QEvent> for Form2551QView {}

struct ScheduleRowInputs {
    taxable_amount: Entity<InputState>,
}

pub struct Form2551QView {
    draft: Form2551QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,

    // Editable inputs
    quarter: u8,
    is_amended: bool,
    tax_relief: bool,

    // Schedule 1 row inputs (parallel to draft.schedule_1)
    row_inputs: Vec<ScheduleRowInputs>,

    // Part II inputs
    creditable_withheld_input: Entity<InputState>,
    tax_paid_previous_input: Entity<InputState>,
    receipt_input: Entity<InputState>,

    validation_errors: Vec<(String, String)>,
    suppressed_sections: HashSet<&'static str>,
    status_message: Option<String>,
    show_filing_period: bool,
    show_background_info: bool,
    show_schedule_1: bool,
    show_tax_computation: bool,
    show_receipt: bool,

    _subscriptions: Vec<Subscription>,
}

impl Form2551QView {
    pub fn new(
        draft: Form2551QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let cred_str = if draft.creditable_tax_withheld >= 0.0 {
            format!("{:.2}", draft.creditable_tax_withheld)
        } else {
            String::new()
        };
        let prev_str = if draft.tax_paid_previous >= 0.0 {
            format!("{:.2}", draft.tax_paid_previous)
        } else {
            String::new()
        };
        let creditable_withheld_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("0.00")
        });
        creditable_withheld_input.update(cx, |input, cx| {
            input.set_value(cred_str, window, cx);
        });

        let tax_paid_previous_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("0.00")
        });
        tax_paid_previous_input.update(cx, |input, cx| {
            input.set_value(prev_str, window, cx);
        });

        let receipt_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste BIR receipt email text, then click Import Receipt")
        });

        let quarter = draft.quarter;
        let is_amended = draft.is_amended;
        let tax_relief = draft.tax_relief;

        let mut row_inputs = Vec::new();
        let mut subscriptions = Vec::new();

        for row in draft.schedule_1.iter() {
            let amt_str = if row.taxable_amount >= 0.0 {
                format!("{:.2}", row.taxable_amount)
            } else {
                String::new()
            };
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder("0.00")
            });
            input.update(cx, |input, cx| {
                input.set_value(amt_str, window, cx);
            });

            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    match event {
                        InputEvent::Change => {
                            this.is_validated = false;
                            this.sync_from_inputs(cx);
                        }
                        InputEvent::Focus => {
                            this.suppressed_sections.insert("schedule_1");
                            cx.notify();
                        }
                        _ => {}
                    }
                },
            ));

            row_inputs.push(ScheduleRowInputs {
                taxable_amount: input,
            });
        }

        // Subscribe to creditable withheld changes
        let sub1 = cx.subscribe_in(
            &creditable_withheld_input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Change => {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                    InputEvent::Focus => {
                        this.suppressed_sections.insert("tax_computation");
                        cx.notify();
                    }
                    _ => {}
                }
            },
        );
        let sub2 = cx.subscribe_in(
            &tax_paid_previous_input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Change => {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                    InputEvent::Focus => {
                        this.suppressed_sections.insert("tax_computation");
                        cx.notify();
                    }
                    _ => {}
                }
            },
        );

        subscriptions.push(sub1);
        subscriptions.push(sub2);

        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            quarter,
            is_amended,
            tax_relief,
            row_inputs,
            creditable_withheld_input,
            tax_paid_previous_input,
            receipt_input,
            validation_errors: Vec::new(),
            suppressed_sections: HashSet::new(),
            status_message: None,
            show_filing_period: true,
            show_background_info: false,
            show_schedule_1: true,
            show_tax_computation: true,
            show_receipt: false,
            _subscriptions: subscriptions,
        }
    }

    /// Pull editable field values back into the draft and recompute.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.draft.quarter = self.quarter;
        self.draft.is_amended = self.is_amended;
        self.draft.tax_relief = self.tax_relief;

        // Sync schedule rows
        for (i, row_state) in self.row_inputs.iter().enumerate() {
            if let Some(row) = self.draft.schedule_1.get_mut(i) {
                let val_str = row_state.taxable_amount.read(cx).value();
                row.taxable_amount = val_str.parse::<f64>().unwrap_or(0.0);
            }
        }

        // Sync Part II inputs
        self.draft.creditable_tax_withheld = self
            .creditable_withheld_input
            .read(cx)
            .value()
            .parse::<f64>()
            .unwrap_or(0.0);
        self.draft.tax_paid_previous = if self.is_amended {
            self.tax_paid_previous_input
                .read(cx)
                .value()
                .parse::<f64>()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        self.draft.recompute();
        cx.notify();
    }

    fn add_schedule_row(&mut self, atc_code: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = Schedule1Row::new(atc_code) {
            self.draft.schedule_1.push(row);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            self._subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    match event {
                        InputEvent::Change => {
                            this.is_validated = false;
                            this.sync_from_inputs(cx);
                        }
                        InputEvent::Focus => {
                            this.suppressed_sections.insert("schedule_1");
                            cx.notify();
                        }
                        _ => {}
                    }
                },
            ));

            self.row_inputs.push(ScheduleRowInputs {
                taxable_amount: input,
            });
            cx.notify();
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message("Form saved.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Success)
                    .autohide(true),
                cx,
            );
            cx.emit(Form2551QEvent::Saved);
        }
    }

    fn mark_submitted(&mut self, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        self.validation_errors = self.validate_for_submit(cx);
        self.suppressed_sections.clear();
        if !self.validation_errors.is_empty() {
            self.status_message = Some("Fix validation errors before submitting".to_string());
            cx.notify();
            return;
        }

        self.status_message = Some("Queuing for background submission...".to_string());
        self.draft.status = FilingStatus::Queued;
        self.draft.submission_attempts = 0;
        self.draft.next_retry_at = Some(chrono::Utc::now().to_rfc3339());
        self.draft.last_error = None;
        
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
        }

        cx.emit(Form2551QEvent::PushNotification(
            "info".to_string(),
            "Form Queued".to_string(),
            "Your form has been queued for background submission.".to_string(),
        ));
        cx.emit(Form2551QEvent::Saved);
        self.status_message = None;
        cx.notify();
    }

    fn on_submit_action(
        &mut self,
        _: &crate::SubmitCurrentForm,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mark_submitted(cx);
    }

    fn mark_as_paid(&mut self, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Paid;
        self.draft.updated_at = chrono::Utc::now().to_rfc3339();
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
        }
        self.status_message = Some("Paid. Filing complete.".to_string());
        cx.emit(Form2551QEvent::Saved);
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Draft;
        self.draft.submitted_at = None;
        self.draft.confirmed_at = None;
        self.draft.receipt_id = None;
        self.draft.submission_filename = None;
        self.draft.updated_at = chrono::Utc::now().to_rfc3339();
        self.is_validated = false;
        self.validation_errors.clear();
        self.status_message = None;
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
        }
        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message("Form reverted to Draft. You may edit and resubmit.".to_string())
                .with_type(gpui_component::notification::NotificationType::Info)
                .autohide(true),
            cx,
        );
        cx.emit(Form2551QEvent::Saved);
        cx.notify();
    }

    fn check_confirmation_email(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.status != FilingStatus::Submitted {
            return;
        }

        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message("Checking email for BIR confirmation...".to_string())
                .with_type(gpui_component::notification::NotificationType::Info)
                .autohide(true),
            cx,
        );

        let db = self.db.clone();
        let draft = self.draft.clone();

        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                let db_guard = db.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
                let profile = db_guard.get_profile_by_tin(&draft.tin)?
                    .ok_or_else(|| anyhow::anyhow!("Profile not found for TIN {}", draft.tin))?;

                if !profile.is_email_tracking_active() {
                    return Err(anyhow::anyhow!("Email tracking is not enabled. Go to Email Settings in your profile to set up App Password or Google OAuth2."));
                }

                // Use existing email fetcher infrastructure
                let _receipts = bir_core::email::fetch_and_process_emails(&profile, &db_guard)?;

                // Check if our draft was updated to Confirmed
                let our_filename = draft.default_submission_filename();
                if let Some(updated) = db_guard.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)? {
                    if updated.status == FilingStatus::Confirmed {
                        return Ok(Some(updated));
                    }
                }
                // Also check by submission filename match in receipts table
                if let Some(_receipt) = db_guard.get_submission_receipt_by_filename(&our_filename)? {
                    // Receipt exists, reload draft
                    if let Some(updated) = db_guard.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)? {
                        return Ok(Some(updated));
                    }
                }
                Ok(None)
            }).await;

            let _ = cx.update(|cx| {
                if let Some(this) = this.upgrade() {
                    this.update(cx, |this, cx| {
                        match result {
                            Ok(Some(updated_draft)) => {
                                this.draft = updated_draft;
                                this.status_message = None;
                                cx.emit(Form2551QEvent::PushNotification(
                                    "success".to_string(),
                                    "Confirmation Received!".to_string(),
                                    "BIR confirmation email processed successfully.".to_string(),
                                ));
                                cx.emit(Form2551QEvent::Confirmed);
                            }
                            Ok(None) => {
                                this.status_message = None;
                                cx.emit(Form2551QEvent::PushNotification(
                                    "info".to_string(),
                                    "No confirmation yet".to_string(),
                                    "We couldn't find a new confirmation email from BIR. Please try again later.".to_string(),
                                ));
                            }
                            Err(e) => {
                                this.status_message = None;
                                cx.emit(Form2551QEvent::PushNotification(
                                    "error".to_string(),
                                    "Email Check Failed".to_string(),
                                    e.to_string(),
                                ));
                            }
                        }
                        cx.notify();
                    });
                }
            });
        }).detach();
    }

    fn render_status_pipeline(&self, cx: &Context<Self>) -> gpui::Div {
        let steps: [(&str, FilingStatus, u8); 5] = [
            ("Draft", FilingStatus::Draft, 1),
            ("Queued", FilingStatus::Queued, 2),
            ("Submitted", FilingStatus::Submitted, 3),
            ("Confirmed", FilingStatus::Confirmed, 4),
            ("Paid", FilingStatus::Paid, 5),
        ];

        let step_order = |s: &FilingStatus| -> u8 {
            match s {
                FilingStatus::Draft => 0,
                FilingStatus::Queued => 1,
                FilingStatus::Submitted => 2,
                FilingStatus::Confirmed => 3,
                FilingStatus::Paid => 4,
            }
        };
        let current_idx = step_order(&self.draft.status);

        let mut row = div().flex().items_center().w_full().justify_center();

        for (i, (label, step_status, step_num)) in steps.iter().enumerate() {
            let idx = step_order(step_status);
            let is_current = idx == current_idx;
            let is_completed = idx < current_idx;

            // Connector line before this step (not before the first)
            if i > 0 {
                if idx <= current_idx {
                    row = row.child(
                        div()
                            .flex_1()
                            .max_w(px(80.))
                            .h(px(2.))
                            .bg(cx.theme().success.opacity(0.5))
                    );
                } else {
                    row = row.child(
                        div()
                            .flex_1()
                            .max_w(px(80.))
                            .border_t_2()
                            .border_dashed()
                            .border_color(cx.theme().muted_foreground)
                    );
                }
            }

            // Circle content: checkmark for completed, number for current/future
            let circle_content = if is_completed {
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().success_foreground)
                    .child("✓")
                    .into_any_element()
            } else {
                let num_color = if is_current {
                    match step_status {
                        FilingStatus::Draft => cx.theme().warning_foreground,
                        FilingStatus::Queued => cx.theme().primary_foreground,
                        FilingStatus::Submitted => cx.theme().info_foreground,
                        FilingStatus::Confirmed => cx.theme().success_foreground,
                        FilingStatus::Paid => cx.theme().success_foreground,
                    }
                } else {
                    cx.theme().muted_foreground
                };
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(num_color)
                    .child(format!("{}", step_num))
                    .into_any_element()
            };

            // Circle colors
            let (circle_bg, circle_border) = if is_completed {
                (cx.theme().success, cx.theme().success)
            } else if is_current {
                let color = match step_status {
                    FilingStatus::Draft => cx.theme().warning,
                    FilingStatus::Queued => cx.theme().primary,
                    FilingStatus::Submitted => cx.theme().info,
                    FilingStatus::Confirmed => cx.theme().success,
                    FilingStatus::Paid => cx.theme().success,
                };
                (color, color)
            } else {
                (gpui::rgba(0x00000000).into(), cx.theme().muted_foreground)
            };

            // Label color
            let label_color = if is_completed {
                cx.theme().success
            } else if is_current {
                match step_status {
                    FilingStatus::Draft => cx.theme().warning,
                    FilingStatus::Queued => cx.theme().primary,
                    FilingStatus::Submitted => cx.theme().info,
                    FilingStatus::Confirmed => cx.theme().success,
                    FilingStatus::Paid => cx.theme().success,
                }
            } else {
                cx.theme().muted_foreground
            };

            let mut circle = div()
                .size(px(28.))
                .rounded_full()
                .border_2()
                .border_color(circle_border)
                .bg(circle_bg)
                .flex()
                .items_center()
                .justify_center()
                .child(circle_content);

            if !is_completed && !is_current {
                circle = circle.border_dashed();
            }

            // Build step group: circle + label inline
            let mut step_group = div()
                .id(format!("step_{}", idx))
                .flex()
                .items_center()
                .gap_2();

            step_group = step_group.child(circle).child(
                div()
                    .text_xs()
                    .text_color(label_color)
                    .font_weight(if is_current { FontWeight::BOLD } else { FontWeight::MEDIUM })
                    .child(*label)
            );

            // Add tooltip with submission date on the active Submitted step or Confirmed step
            if matches!(step_status, FilingStatus::Submitted) || matches!(step_status, FilingStatus::Confirmed) {
                let date_opt = match step_status {
                    FilingStatus::Submitted => self.draft.submitted_at.as_deref(),
                    FilingStatus::Confirmed => self.draft.confirmed_at.as_deref(),
                    _ => None,
                };
                if let Some(date_str) = date_opt {
                    let formatted_date = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
                        dt.format("%B %e, %Y %I:%M %p").to_string()
                    } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S") {
                        dt.format("%B %e, %Y %I:%M %p").to_string()
                    } else {
                        date_str.to_string()
                    };
                    
                    let tooltip_text = match step_status {
                        FilingStatus::Submitted => format!("Submitted on {}", formatted_date),
                        FilingStatus::Confirmed => format!("Confirmed on {}", formatted_date),
                        _ => unreachable!(),
                    };
                    
                    step_group = step_group.tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tooltip_text.clone()).build(window, cx)
                    });
                }
            }

            row = row.child(step_group);
        }

        row
    }

    fn validate_for_submit(&self, cx: &mut Context<Self>) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        if !(1900..=9999).contains(&self.draft.taxable_year) {
            errors.push(("taxable_year".to_string(), "Taxable year must be a 4-digit year".to_string()));
        }

        if !(1..=4).contains(&self.quarter) {
            errors.push(("quarter".to_string(), "Quarter is required".to_string()));
        }

        for (label, value) in [
            ("TIN", self.draft.tin.as_str()),
            ("RDO Code", self.draft.rdo_code.as_str()),
            ("Taxpayer Name", self.draft.taxpayer_name.as_str()),
            ("Registered Address", self.draft.registered_address.as_str()),
            ("ZIP Code", self.draft.zip_code.as_str()),
            ("Contact Number", self.draft.contact_number.as_str()),
            ("Email Address", self.draft.email.as_str()),
        ] {
            if value.trim().is_empty() {
                errors.push((label.to_string(), format!("{label} is required")));
            }
        }

        if self.draft.zip_code.trim().is_empty() {
            errors.push(("zip_code".to_string(), "Zip Code is required".to_string()));
        } else if !validate_zip(&self.draft.zip_code) {
            errors.push(("zip_code".to_string(), "Zip Code must be 4 digits".to_string()));
        }
        if self.draft.contact_number.trim().is_empty() {
            errors.push(("contact_number".to_string(), "Contact Number is required".to_string()));
        } else if !validate_ph_phone(&self.draft.contact_number) {
            errors.push(("contact_number".to_string(), "Contact Number must be valid".to_string()));
        }
        if !self.draft.email.trim().is_empty() && !validate_email(&self.draft.email) {
            errors.push(("email".to_string(), "Email Address must be a valid email".to_string()));
        }

        if self.draft.schedule_1.is_empty() {
            errors.push(("schedule_1".to_string(), "Schedule 1 requires at least one ATC row".to_string()));
        }
        for (i, row_input) in self.row_inputs.iter().enumerate() {
            let value = row_input.taxable_amount.read(cx).value();
            if value.trim().is_empty() {
                errors.push((format!("schedule_1_row_{}", i + 1), format!(
                    "Schedule 1 row {} taxable amount is required",
                    i + 1
                )));
            } else if value.parse::<f64>().map(|n| n < 0.0).unwrap_or(true) {
                errors.push((format!("schedule_1_row_{}", i + 1), format!(
                    "Schedule 1 row {} taxable amount must be non-negative",
                    i + 1
                )));
            }
        }

        for (label, input) in [
            (
                "Creditable percentage tax withheld",
                &self.creditable_withheld_input,
            ),
            (
                "Tax paid in return previously filed",
                &self.tax_paid_previous_input,
            ),
        ] {
            let value = input.read(cx).value();
            if label.starts_with("Tax paid") && !self.is_amended {
                continue;
            }
            if value.trim().is_empty() {
                let field = if label.starts_with("Creditable") { "creditable_withheld" } else { "tax_paid_previous" };
                errors.push((field.to_string(), format!("{label} is required")));
            } else if value.parse::<f64>().map(|n| n < 0.0).unwrap_or(true) {
                let field = if label.starts_with("Creditable") { "creditable_withheld" } else { "tax_paid_previous" };
                errors.push((field.to_string(), format!("{label} must be non-negative")));
            }
        }

        errors
    }

    fn import_receipt(&mut self, cx: &mut Context<Self>) {
        let raw = self.receipt_input.read(cx).value().to_string();
        match parse_bir_receipt_email(&raw) {
            Ok(receipt) => {
                if let Ok(db) = self.db.lock() {
                    match db.save_submission_receipt(&receipt) {
                        Ok(saved) => {
                            if saved.filename == self.draft.default_submission_filename()
                                || self.draft.submission_filename.as_deref()
                                    == Some(&saved.filename)
                            {
                                self.draft.status = FilingStatus::Confirmed;
                                self.draft.confirmed_at = Some(format!(
                                    "{}T{}",
                                    saved.received_date, saved.received_time
                                ));
                                self.draft.receipt_id = saved.id;
                                self.draft.submission_filename = Some(saved.filename);
                                let _ = db.save_2551q_draft(&self.draft);
                                cx.emit(Form2551QEvent::Confirmed);
                            }
                            self.status_message = Some("Receipt imported".to_string());
                        }
                        Err(err) => {
                            self.status_message = Some(format!("Receipt save failed: {err}"));
                        }
                    }
                }
            }
            Err(err) => {
                self.status_message = Some(format!("Receipt parse failed: {err}"));
            }
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        let dir = std::env::temp_dir().join("taxman-ebir-pdf");
        match render_2551q_print(&self.draft, &dir) {
            Ok(result) => {
                let draft = self.draft.clone();
                let output_dir = dir.clone();
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(1200.), px(900.)), cx)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("PDF Viewer".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                };

                if let Err(err) = cx.open_window(options, move |_window, cx| {
                    cx.new(|_cx| PdfViewerView::new(draft, result, output_dir))
                }) {
                    use gpui_component::WindowExt;
                    window.push_notification(
                        gpui_component::notification::Notification::new()
                            .message(format!("PDF viewer failed to open: {err}"))
                            .with_type(gpui_component::notification::NotificationType::Error)
                            .autohide(true),
                        cx,
                    );
                    return;
                }

                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("PDF viewer opened".to_string())
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
            }
            Err(err) => {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("PDF generation failed: {err}"))
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn print_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _receipt_id = match self.draft.receipt_id {
            Some(id) => id,
            None => {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("No confirmation receipt available.".to_string())
                        .with_type(gpui_component::notification::NotificationType::Warning)
                        .autohide(true),
                    cx,
                );
                return;
            }
        };

        // Load receipt from DB by filename match
        let receipt = if let Ok(db) = self.db.lock() {
            let filename = self.draft.submission_filename.clone()
                .unwrap_or_else(|| self.draft.default_submission_filename());
            db.get_submission_receipt_by_filename(&filename).ok().flatten()
        } else {
            None
        };

        let Some(receipt) = receipt else {
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message("Receipt not found in database.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            );
            return;
        };

        let draft = self.draft.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1000.), px(800.)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some("Email Confirmation".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        if let Err(err) = cx.open_window(options, move |_window, cx| {
            cx.new(|_cx| EmailConfirmationView::new(receipt, draft))
        }) {
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message(format!("Email Confirmation viewer failed to open: {err}"))
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            );
            return;
        }

        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message("Email Confirmation viewer opened".to_string())
                .with_type(gpui_component::notification::NotificationType::Success)
                .autohide(true),
            cx,
        );
        cx.notify();
    }

    fn upload_receipt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = rfd::FileDialog::new()
            .add_filter("Images/PDF", &["png", "jpg", "jpeg", "pdf"])
            .pick_file()
        else {
            return;
        };

        let data_dir = bir_core::db::default_database_path()
            .parent()
            .unwrap()
            .join("receipts");
        let _ = std::fs::create_dir_all(&data_dir);
        let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("bin");
        let new_filename = format!(
            "receipt-{}-{}-{}.{}",
            self.draft.tin, self.draft.taxable_year, self.draft.quarter, ext
        );
        let new_path = data_dir.join(new_filename);

        match std::fs::copy(&file, &new_path) {
            Ok(_) => {
                self.draft.payment_receipt_path = Some(new_path.to_string_lossy().to_string());
                if let Ok(db) = self.db.lock() {
                    let _ = db.save_2551q_draft(&self.draft);
                }
                
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("Receipt uploaded successfully.".to_string())
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
                cx.notify();
            }
            Err(e) => {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("Failed to copy receipt: {}", e))
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
            }
        }
    }

    fn view_receipt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.draft.payment_receipt_path {
            let draft = self.draft.clone();
            let path_clone = path.clone();
            
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(800.), px(800.)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Payment Receipt Viewer".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            let view = cx.new(|_cx| ReceiptViewerView::new(draft, path_clone));
            
            cx.subscribe(&view, |this: &mut Self, _, event: &ReceiptViewerEvent, cx| {
                match event {
                    ReceiptViewerEvent::ReUploaded(new_path) => {
                        this.draft.payment_receipt_path = Some(new_path.clone());
                        if let Ok(db) = this.db.lock() {
                            let _ = db.save_2551q_draft(&this.draft);
                        }
                        cx.notify();
                    }
                }
            }).detach();

            if let Err(err) = cx.open_window(options, move |_window, _cx| view.clone()) {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("Failed to open receipt viewer: {}", err))
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(true),
                    cx,
                );
            }
        }
    }

    fn get_error(&self, field_id: &str) -> Option<&String> {
        self.validation_errors.iter().find(|(f, _)| f == field_id).map(|(_, msg)| msg)
    }

    fn error_icon(_message: &str, cx: &Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_center()
            .text_color(cx.theme().danger)
            .child(Icon::new(IconName::TriangleAlert).small())
    }

    fn has_section_error(&self, section_id: &'static str) -> bool {
        if self.suppressed_sections.contains(section_id) {
            return false;
        }
        self.validation_errors.iter().any(|(f, _)| {
            match section_id {
                "filing_period" => f == "taxable_year" || f == "quarter",
                "background_info" => f == "tin" || f == "rdo_code" || f == "taxpayer_name" || f == "registered_address" || f == "zip_code" || f == "contact_number" || f == "email",
                "schedule_1" => f == "schedule_1" || f.starts_with("schedule_1_row_"),
                "tax_computation" => f == "creditable_withheld" || f == "tax_paid_previous",
                _ => false,
            }
        })
    }

    /// Returns true if the form fields should be editable (only in Draft status).
    fn is_editable(&self) -> bool {
        matches!(self.draft.status, FilingStatus::Draft)
    }

    /// Returns a persistent contextual hint based on current status.
    fn status_hint(&self) -> Option<String> {
        match &self.draft.status {
            FilingStatus::Draft => None,
            FilingStatus::Queued => {
                let attempts = self.draft.submission_attempts;
                let text = if attempts > 0 {
                    format!("Submission failed {} times. Waiting for next retry.", attempts)
                } else {
                    "Queued for background submission.".to_string()
                };
                Some(text)
            }
            FilingStatus::Submitted => {
                let date = self.draft.submitted_at.as_deref().unwrap_or("unknown date");
                Some(format!("Submitted on {}. Waiting for BIR confirmation email.", date))
            }
            FilingStatus::Confirmed => {
                let date = self.draft.confirmed_at.as_deref().unwrap_or("unknown date");
                Some(format!("Confirmed on {}. Print confirmation and proceed to bank payment.", date))
            }
            FilingStatus::Paid => Some("Paid. Filing complete.".to_string()),
        }
    }

    fn field_label(label: &str, cx: &Context<Self>) -> gpui::Div {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .mb_1()
            .child(label.to_string())
    }

    fn readonly_field(label: &str, value: &str, cx: &Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Self::field_label(label, cx))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .text_sm()
                    .child(if value.is_empty() {
                        "—".to_string()
                    } else {
                        value.to_string()
                    }),
            )
    }

    fn currency_display(amount: f64, cx: &Context<Self>) -> gpui::Div {
        div()
            .font_weight(FontWeight::BOLD)
            .text_color(cx.theme().primary)
            .text_sm()
            .child(format!("\u{20b1} {:.2}", amount))
    }

}

impl Render for Form2551QView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let carry_label = self.draft.carry_forward_label();
        let is_amended = self.is_amended;
        let total_due = self.draft.total_tax_due;
        let tax_payable = self.draft.tax_payable;
        let is_editable = self.is_editable();

        let title_block = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Republic of the Philippines — Department of Finance — Bureau of Internal Revenue"),
            )
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BLACK)
                    .text_color(cx.theme().foreground)
                    .child("BIR Form No. 2551Q"),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child("Quarterly Percentage Tax Return"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("January 2018 (ENCS)"),
            );

        let carry_banner = if let Some(label) = carry_label {
            div()
                .px_4()
                .py_3()
                .bg(cx.theme().accent)
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().primary)
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(label)
                .into_any_element()
        } else {
            div().into_any_element()
        };

        // Accordion wrapper macro/helper logic inline
        // filing_period content
        let filing_period_content = div()
            .flex()
            .gap_8()
            .items_center()
            .child(Self::readonly_field("Taxable Year", &self.draft.taxable_year.to_string(), cx).w(px(120.)))
            .child(Self::readonly_field("Quarter", &format!("Q{}", self.quarter), cx).w(px(80.)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Self::field_label("Options", cx))
                    .child(
                        div()
                            .flex()
                            .gap_6()
                            .child(
                                div()
                                    .id("amended_toggle")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(is_editable, |el| el.cursor_pointer())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.is_editable() { return; }
                                        this.is_amended = !this.is_amended;
                                        if !this.is_amended {
                                            this.draft.tax_paid_previous = 0.0;
                                        }
                                        this.is_validated = false;
                                        this.sync_from_inputs(cx);
                                    }))
                                    .child(
                                        div()
                                            .w_4()
                                            .h_4()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(if is_amended {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().background
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if is_amended {
                                                div()
                                                    .text_xs()
                                                    .text_color(
                                                        cx.theme().primary_foreground,
                                                    )
                                                    .child("✓")
                                            } else {
                                                div()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("Amended Return"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("tax_relief_toggle")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(is_editable, |el| el.cursor_pointer())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.is_editable() { return; }
                                        this.tax_relief = !this.tax_relief;
                                        this.draft.tax_relief = this.tax_relief;
                                        this.is_validated = false;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .w_4()
                                            .h_4()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(if self.tax_relief {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().background
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if self.tax_relief {
                                                div()
                                                    .text_xs()
                                                    .text_color(
                                                        cx.theme().primary_foreground,
                                                    )
                                                    .child("✓")
                                            } else {
                                                div()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("Tax Relief"),
                                    ),
                            ),
                    ),
            );

        // background_info content
        let background_info_content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .gap_8()
                    .child(Self::readonly_field("1. TIN", &self.draft.tin, cx).flex_1())
                    .child(Self::readonly_field("2. RDO Code", &self.draft.rdo_code, cx).flex_1()),
            )
            .child(Self::readonly_field(
                "3. Taxpayer's Name",
                &self.draft.taxpayer_name,
                cx,
            ))
            .child(Self::readonly_field(
                "4. Registered Address",
                &self.draft.registered_address,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_8()
                    .child(Self::readonly_field("5. Zip Code", &self.draft.zip_code, cx).flex_1())
                    .child(
                        Self::readonly_field("6. Contact Number", &self.draft.contact_number, cx)
                            .flex_1(),
                    ),
            )
            .child(Self::readonly_field(
                "7. Email Address",
                &self.draft.email,
                cx,
            ));

        // schedule_one content
        let schedule_one_content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .w(px(80.))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("ATC"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("DESCRIPTION"),
                    )
                    .child(
                        div()
                            .w(px(140.))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("TAXABLE AMOUNT (₱)"),
                    )
                    .child(
                        div()
                            .w(px(50.))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("RATE"),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("TAX DUE (₱)"),
                    ),
            )
            .children(self.draft.schedule_1.iter().enumerate().map(|(i, row)| {
                let atc = row.atc.clone();
                let desc = row.atc_description.clone();
                let rate_pct = format!("{:.1}%", row.tax_rate * 100.0);
                let tax_due = row.tax_due;
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .w(px(80.))
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().primary)
                            .child(atc),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().foreground)
                            .child(desc),
                    )
                    .child(div().w(px(140.)).child(
                        if let Some(row_in) = self.row_inputs.get(i) {
                            Input::new(&row_in.taxable_amount).disabled(!is_editable).into_any_element()
                        } else {
                            div().child("—").into_any_element()
                        },
                    ))
                    .child(
                        div()
                            .w(px(50.))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(rate_pct),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().primary)
                            .text_sm()
                            .child(format!("{:.2}", tax_due)),
                    )
            }));

        // tax_computation content
        let tax_computation_content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("14. Total Tax Due (from Schedule 1)"),
                    )
                    .child(Self::currency_display(total_due, cx)),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("15. Less: Creditable Percentage Tax Withheld (BIR Form 2307)"),
                    )
                    .child(
                        div()
                            .w(px(180.))
                            .child(Input::new(&self.creditable_withheld_input).disabled(!is_editable)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if is_amended {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child("16. Less: Tax Paid in Return Previously Filed"),
                            )
                            .child(if !is_amended {
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Check Amended Return to unlock")
                            } else {
                                div()
                            }),
                    )
                    .child(
                        div()
                            .w(px(180.))
                            .opacity(if is_amended { 1.0 } else { 0.4 })
                            .child(Input::new(&self.tax_paid_previous_input).disabled(!is_editable)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pt_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("17. Tax Still Payable / (Overpayment)"),
                    )
                    .child(Self::currency_display(tax_payable, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_4()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child("18. Add: Penalties"),
                    )
                    .child(
                        div()
                            .flex()
                            .pl_6()
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("18A. Surcharge"))
                            .child(Self::currency_display(self.draft.surcharge, cx))
                    )
                    .child(
                        div()
                            .flex()
                            .pl_6()
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("18B. Interest"))
                            .child(Self::currency_display(self.draft.interest, cx))
                    )
                    .child(
                        div()
                            .flex()
                            .pl_6()
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("18C. Compromise"))
                            .child(Self::currency_display(self.draft.compromise, cx))
                    )
                    .child(
                        div()
                            .flex()
                            .pl_6()
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child("18D. Total Penalties"))
                            .child(Self::currency_display(self.draft.total_penalties, cx))
                    )
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pt_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BLACK)
                            .text_color(cx.theme().foreground)
                            .child("19. Total Amount Payable / (Overpayment)"),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BLACK)
                            .text_color(if self.draft.total_amount_payable > 0.0 {
                                cx.theme().primary
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(format!("\u{20b1} {:.2}", self.draft.total_amount_payable)),
                    ),
            );

        let validation_block = if self.validation_errors.is_empty() {
            div().into_any_element()
        } else {
            div()
                .bg(cx.theme().danger.opacity(0.1))
                .border_1()
                .border_color(cx.theme().danger)
                .rounded_lg()
                .p_4()
                .flex()
                .flex_col()
                .gap_1()
                .children(self.validation_errors.iter().map(|(_field, msg)| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(msg.clone())
                }))
                .into_any_element()
        };

        // Actions moved to toolbar

        macro_rules! build_accordion {
            ($id:expr, $label:expr, $is_expanded:expr, $is_valid:expr, $has_error:expr, $on_click:expr, $content:expr $(,)?) => {
                {
                    let mut card = div()
                        .bg(cx.theme().secondary)
                        .rounded_xl()
                        .border_1()
                        .border_color(if $has_error { cx.theme().danger } else { cx.theme().border })
                        .p_6()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id($id)
                                .flex()
                                .justify_between()
                                .items_center()
                                .cursor_pointer()
                                .w_full()
                                .p_2()
                                .rounded_md()
                                .hover(|style| style.bg(cx.theme().muted.opacity(0.5)))
                                .on_click($on_click)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div()
                                                .w_5()
                                                .h_5()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(if $is_valid {
                                                    div()
                                                        .text_color(gpui::rgba(0x22c55eff))
                                                        .font_weight(FontWeight::BLACK)
                                                        .text_lg()
                                                        .child("✓")
                                                        .into_any_element()
                                                } else {
                                                    div().into_any_element()
                                                })
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child($label),
                                        )
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .w_6()
                                        .h_6()
                                        .rounded_full()
                                        .bg(cx.theme().muted)
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(if $is_expanded { "▲" } else { "▼" }),
                                        ),
                                ),
                        );
                        
                    if $is_expanded {
                        card = card.child(
                            div()
                                .mt_4()
                                .pt_4()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child($content),
                        );
                    }
                    card
                }
            };
        }

        let is_submitted = !matches!(self.draft.status, FilingStatus::Draft);

        let is_filing_period_valid = is_submitted || ((1900..=9999).contains(&self.draft.taxable_year) && (1..=4).contains(&self.quarter));

        let is_background_info_valid = is_submitted || (!self.draft.tin.trim().is_empty() &&
            !self.draft.rdo_code.trim().is_empty() &&
            !self.draft.taxpayer_name.trim().is_empty() &&
            !self.draft.registered_address.trim().is_empty() &&
            !self.draft.zip_code.trim().is_empty() && validate_zip(self.draft.zip_code.trim()) &&
            !self.draft.contact_number.trim().is_empty() && validate_ph_phone(&self.draft.contact_number) &&
            !self.draft.email.trim().is_empty() && validate_email(&self.draft.email));

        let is_schedule_1_valid = is_submitted || (!self.draft.schedule_1.is_empty() && self.row_inputs.iter().all(|r| {
            let val = r.taxable_amount.read(cx).value();
            !val.trim().is_empty() && val.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false)
        }));

        let is_tax_computation_valid = is_submitted || {
            let cw = self.creditable_withheld_input.read(cx).value();
            let tp = self.tax_paid_previous_input.read(cx).value();
            let cw_valid = !cw.trim().is_empty() && cw.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false);
            let tp_valid = if self.is_amended {
                !tp.trim().is_empty() && tp.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false)
            } else { true };
            cw_valid && tp_valid
        };

        let form_content = div()
            .w_full()
            .max_w(px(900.))
            .mx_auto()
            .px_8()
            .py_10()
            .flex()
            .flex_col()
            .gap_8()
            .child(title_block)
            .child(carry_banner)
            .child(validation_block)
            .child(build_accordion!(
                "acc_filing_period",
                "FILING PERIOD",
                self.show_filing_period,
                is_filing_period_valid,
                self.has_section_error("filing_period"),
                cx.listener(|this, _, _, cx| {
                    this.suppressed_sections.insert("filing_period");
                    this.show_filing_period = !this.show_filing_period;
                    cx.notify();
                }),
                filing_period_content,
            ))
            .child(build_accordion!(
                "acc_background_info",
                "PART I — BACKGROUND INFORMATION (pre-filled from profile)",
                self.show_background_info,
                is_background_info_valid,
                self.has_section_error("background_info"),
                cx.listener(|this, _, _, cx| {
                    this.suppressed_sections.insert("background_info");
                    this.show_background_info = !this.show_background_info;
                    cx.notify();
                }),
                background_info_content,
            ))
            .child(build_accordion!(
                "acc_schedule_1",
                "SCHEDULE 1 — COMPUTATION OF TAX",
                self.show_schedule_1,
                is_schedule_1_valid,
                self.has_section_error("schedule_1"),
                cx.listener(|this, _, _, cx| {
                    this.suppressed_sections.insert("schedule_1");
                    this.show_schedule_1 = !this.show_schedule_1;
                    cx.notify();
                }),
                schedule_one_content,
            ))
            .child(build_accordion!(
                "acc_tax_computation",
                "PART II — COMPUTATION OF TAX",
                self.show_tax_computation,
                is_tax_computation_valid,
                self.has_section_error("tax_computation"),
                cx.listener(|this, _, _, cx| {
                    this.suppressed_sections.insert("tax_computation");
                    this.show_tax_computation = !this.show_tax_computation;
                    cx.notify();
                }),
                tax_computation_content,
            ));

        let status_pipeline = self.render_status_pipeline(cx);

        let status_banner = div()
            .flex()
            .items_center()
            .justify_center()
            .px_8()
            .py_4()
            .bg(cx.theme().secondary)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(status_pipeline);

        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .on_action(cx.listener(Self::on_submit_action))
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
                            .on_click(cx.listener(|_this, _, _, cx| {
                                cx.emit(Form2551QEvent::BackToDashboard);
                            })),
                    )
                    .child({
                        let mut toolbar = div().flex().items_center().gap_3();

                        match &self.draft.status {
                            FilingStatus::Draft => {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("save_btn")
                                        .label("Save")
                                        .outline()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.save(window, cx);
                                        })),
                                );
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("validate_btn")
                                        .label("Validate")
                                        .outline()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.sync_from_inputs(cx);
                                            this.validation_errors = this.validate_for_submit(cx);
                                            this.suppressed_sections.clear();
                                            if this.validation_errors.is_empty() {
                                                this.is_validated = true;
                                                this.status_message = None;
                                                use gpui_component::WindowExt;
                                                window.push_notification(
                                                    gpui_component::notification::Notification::new()
                                                        .message("Validation successful. Form is ready to submit.".to_string())
                                                        .with_type(gpui_component::notification::NotificationType::Success)
                                                        .autohide(true),
                                                    cx,
                                                );
                                            } else {
                                                this.is_validated = false;
                                                this.status_message = Some("Fix validation errors to continue.".to_string());
                                            }
                                            cx.notify();
                                        }))
                                );
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("submit_btn")
                                        .label("Queue for Submission")
                                        .disabled(!self.is_validated)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.mark_submitted(cx);
                                        }))
                                );
                            }
                            FilingStatus::Queued => {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("cancel_queue_btn")
                                        .label("Cancel Submission Queue")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.revert_to_draft(window, cx);
                                        }))
                                );
                            }
                            FilingStatus::Submitted => {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("revert_draft_btn")
                                        .label("Revert Draft")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.revert_to_draft(window, cx);
                                        }))
                                );
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("check_email_btn")
                                        .label("Check Confirmation")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.check_confirmation_email(window, cx);
                                        }))
                                );
                            }
                            FilingStatus::Confirmed => {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("revert_draft_btn")
                                        .label("Revert Draft")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.revert_to_draft(window, cx);
                                        }))
                                );
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("mark_paid_btn")
                                        .label("Mark as Paid")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.mark_as_paid(cx);
                                        }))
                                );
                            }
                            FilingStatus::Paid => {}
                        }

                        if matches!(self.draft.status, FilingStatus::Confirmed | FilingStatus::Paid) {
                            toolbar = toolbar.child(
                                gpui_component::button::Button::new("print_confirmation_btn")
                                    .label("View Confirmation Email")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.print_confirmation(window, cx);
                                    })),
                            );

                            if self.draft.payment_receipt_path.is_some() {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("view_receipt_btn")
                                        .label("View Receipt")
                                        .outline()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.view_receipt(window, cx);
                                        })),
                                );
                            } else {
                                toolbar = toolbar.child(
                                    gpui_component::button::Button::new("upload_receipt_btn")
                                        .label("Upload Receipt")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.upload_receipt(window, cx);
                                        })),
                                );
                            }
                        }

                        toolbar = toolbar.child(
                            gpui_component::button::Button::new("print_btn")
                                .label("PDF Viewer")
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.preview_pdf(window, cx);
                                })),
                        );

                        toolbar
                    }),
            )
            .child(status_banner)
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    div()
                        .id("form-2551q-scroll")
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("form-2551q-scroll-area")
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .flex()
                                .flex_col()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll_handle)
                                .child(form_content),
                        )
                        .vertical_scrollbar(&self.scroll_handle),
                ),
            )
    }
}
