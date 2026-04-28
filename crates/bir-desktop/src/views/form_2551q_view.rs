//! BIR Form 2551Q — Full in-app form view.
//!
//! Single scrollable view that mimics the actual BIR form layout.
//! No wizards. Profile data is pre-filled and read-only.
//! Schedule 1 is editable. Part II auto-computes.
#![allow(dead_code)]

use gpui::prelude::FluentBuilder;
use gpui::*;
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

use super::email_confirmation_view::EmailConfirmationView;
use super::pdf_viewer::PdfViewerView;
use super::receipt_viewer::{ReceiptViewerEvent, ReceiptViewerView};
use crate::components::form_engine::FormViewTrait;

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
        let creditable_withheld_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        creditable_withheld_input.update(cx, |input, cx| {
            input.set_value(cred_str, window, cx);
        });

        let tax_paid_previous_input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
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
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            input.update(cx, |input, cx| {
                input.set_value(amt_str, window, cx);
            });

            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                    InputEvent::Focus => {
                        this.suppressed_sections.insert("schedule_1");
                        cx.notify();
                    }
                    _ => {}
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
            |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.is_validated = false;
                    this.sync_from_inputs(cx);
                }
                InputEvent::Focus => {
                    this.suppressed_sections.insert("tax_computation");
                    cx.notify();
                }
                _ => {}
            },
        );
        let sub2 = cx.subscribe_in(
            &tax_paid_previous_input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                InputEvent::Change => {
                    this.is_validated = false;
                    this.sync_from_inputs(cx);
                }
                InputEvent::Focus => {
                    this.suppressed_sections.insert("tax_computation");
                    cx.notify();
                }
                _ => {}
            },
        );

        subscriptions.push(sub1);
        subscriptions.push(sub2);

        let bus = cx.global::<crate::events::GlobalEventBus>().0.clone();
        let sub_bus = cx.subscribe(
            &bus,
            |this: &mut Self, _bus, event: &crate::events::AppEvent, cx| match event {
                crate::events::AppEvent::DatabaseChanged => {
                    let tin = this.draft.tin.clone();
                    let year = this.draft.taxable_year;
                    let quarter = this.draft.quarter;
                    if let Ok(db_guard) = this.db.lock()
                        && let Ok(Some(updated)) = db_guard.get_2551q_draft(&tin, year, quarter)
                            && this.draft.status != updated.status {
                                this.draft = updated;
                                if this.draft.status == FilingStatus::Confirmed {
                                    cx.emit(Form2551QEvent::Confirmed);
                                }
                                cx.notify();
                            }
                }
            },
        );
        subscriptions.push(sub_bus);

        let mut view = Self {
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
        };
        view.validation_errors = view.validate_for_submit(cx);
        view
    }

    // Polling removed in favor of AppEvent::DatabaseChanged

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
        self.validation_errors = self.validate_for_submit(cx);
        cx.notify();
    }

    fn add_schedule_row(&mut self, atc_code: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = Schedule1Row::new(atc_code) {
            self.draft.schedule_1.push(row);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            self._subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                    InputEvent::Focus => {
                        this.suppressed_sections.insert("schedule_1");
                        cx.notify();
                    }
                    _ => {}
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
                
                drop(db_guard);

                // Use existing email fetcher infrastructure
                let _receipts = bir_core::email::fetch_and_process_emails(&profile, db.clone())?;

                let db_guard = db.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
                // Check if our draft was updated to Confirmed
                let our_filename = draft.default_submission_filename();
                if let Some(updated) = db_guard.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)?
                    && updated.status == FilingStatus::Confirmed {
                        return Ok(Some(updated));
                    }
                // Also check by submission filename match in receipts table (BIR strips the #email# suffix)
                let stripped_filename = bir_core::receipt::split_bir_filename(&our_filename)
                    .map(|(t, f, p)| format!("{}-{}-{}.xml", t, f, p.split('#').next().unwrap_or(&p)))
                    .unwrap_or(our_filename);

                if let Some(_receipt) = db_guard.get_submission_receipt_by_filename(&stripped_filename)? {
                    // Receipt exists, ensure the draft status is Confirmed
                    let _ = db_guard.confirm_2551q_from_receipt(&_receipt);
                    if let Some(updated) = db_guard.get_2551q_draft(&draft.tin, draft.taxable_year, draft.quarter)? {
                        return Ok(Some(updated));
                    }
                }
                Ok(None)
            }).await;

            cx.update(|cx| {
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

    fn validate_for_submit(&self, cx: &Context<Self>) -> Vec<(String, String)> {
        use bir_core::forms::FormValidator;
        let mut errors = FormValidator::validate(&self.draft);

        for (i, row_state) in self.row_inputs.iter().enumerate() {
            let val_str = row_state.taxable_amount.read(cx).value();
            if val_str.trim().is_empty() {
                errors.push((
                    format!("schedule_1_row_{}", i + 1),
                    format!("Schedule 1 row {} taxable amount is required", i + 1),
                ));
            } else if val_str.trim().parse::<f64>().is_err() {
                errors.push((
                    format!("schedule_1_row_{}", i + 1),
                    format!(
                        "Schedule 1 row {} taxable amount must be a valid number",
                        i + 1
                    ),
                ));
            }
        }

        let cred_str = self.creditable_withheld_input.read(cx).value();
        if cred_str.trim().is_empty() {
            errors.push((
                "creditable_withheld".to_string(),
                "Creditable percentage tax withheld is required".to_string(),
            ));
        } else if cred_str.trim().parse::<f64>().is_err() {
            errors.push((
                "creditable_withheld".to_string(),
                "Creditable percentage tax withheld must be a valid number".to_string(),
            ));
        }

        if self.is_amended {
            let prev_str = self.tax_paid_previous_input.read(cx).value();
            if prev_str.trim().is_empty() {
                errors.push((
                    "tax_paid_previous".to_string(),
                    "Tax paid in return previously filed is required".to_string(),
                ));
            } else if prev_str.trim().parse::<f64>().is_err() {
                errors.push((
                    "tax_paid_previous".to_string(),
                    "Tax paid in return previously filed must be a valid number".to_string(),
                ));
            }
        }

        errors
    }

    fn import_receipt(&mut self, cx: &mut Context<Self>) {
        let raw = self.receipt_input.read(cx).value().to_string();
        match parse_bir_receipt_email(&raw, None) {
            Ok(receipt) => {
                if let Ok(db) = self.db.lock() {
                    match db.save_submission_receipt(&receipt) {
                        Ok((saved, _is_new)) => {
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
        let dir = bir_core::platform::temp_dir().join("taxman-ebir-pdf");
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

                let mut raw_html = None;
                if let Some(receipt_id) = draft.receipt_id
                    && let Ok(db) = self.db.lock()
                        && let Ok(Some(receipt)) = db.get_submission_receipt_by_id(receipt_id) {
                            raw_html = receipt.raw_html;
                        }

                if let Err(err) = cx.open_window(options, move |_window, cx| {
                    cx.new(|_cx| PdfViewerView::new(draft, result, output_dir, raw_html))
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
            let filename = self
                .draft
                .submission_filename
                .clone()
                .unwrap_or_else(|| self.draft.default_submission_filename());
            db.get_submission_receipt_by_filename(&filename)
                .ok()
                .flatten()
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

            cx.subscribe(
                &view,
                |this: &mut Self, _, event: &ReceiptViewerEvent, cx| match event {
                    ReceiptViewerEvent::ReUploaded(new_path) => {
                        this.draft.payment_receipt_path = Some(new_path.clone());
                        if let Ok(db) = this.db.lock() {
                            let _ = db.save_2551q_draft(&this.draft);
                        }
                        cx.notify();
                    }
                },
            )
            .detach();

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
        self.validation_errors
            .iter()
            .find(|(f, _)| f == field_id)
            .map(|(_, msg)| msg)
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
        self.validation_errors
            .iter()
            .any(|(f, _)| match section_id {
                "filing_period" => f == "taxable_year" || f == "quarter",
                "background_info" => {
                    f == "tin"
                        || f == "rdo_code"
                        || f == "taxpayer_name"
                        || f == "registered_address"
                        || f == "zip_code"
                        || f == "contact_number"
                        || f == "email"
                }
                "schedule_1" => f == "schedule_1" || f.starts_with("schedule_1_row_"),
                "tax_computation" => f == "creditable_withheld" || f == "tax_paid_previous",
                _ => false,
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
                    format!(
                        "Submission failed {} times. Waiting for next retry.",
                        attempts
                    )
                } else {
                    "Queued for background submission.".to_string()
                };
                Some(text)
            }
            FilingStatus::Submitted => {
                let date = self.draft.submitted_at.as_deref().unwrap_or("unknown date");
                Some(format!(
                    "Submitted on {}. Waiting for BIR confirmation email.",
                    date
                ))
            }
            FilingStatus::Confirmed => {
                let date = self.draft.confirmed_at.as_deref().unwrap_or("unknown date");
                Some(format!(
                    "Confirmed on {}. Print confirmation and proceed to bank payment.",
                    date
                ))
            }
            FilingStatus::Paid => Some("Paid. Filing complete.".to_string()),
        }
    }

    fn field_label(label: &str, cx: &Context<Self>) -> gpui::Div {
        crate::components::form_parts::field_label(label, cx)
    }

    fn readonly_field(
        label: &str,
        value: &str,
        error: Option<&String>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        crate::components::form_parts::readonly_field(label, value, error, cx)
    }

    fn currency_display(amount: f64, cx: &Context<Self>) -> gpui::Div {
        crate::components::form_parts::currency_display(amount, cx)
    }
}

impl FormViewTrait for Form2551QView {
    fn form_title(&self) -> &'static str { "BIR Form No. 2551Q" }
    fn form_subtitle(&self) -> &'static str { "Quarterly Percentage Tax Return" }
    fn form_version(&self) -> &'static str { "January 2018 (ENCS)" }
    fn current_status(&self) -> FilingStatus { self.draft.status.clone() }
    fn submitted_at(&self) -> Option<&str> { self.draft.submitted_at.as_deref() }
    fn confirmed_at(&self) -> Option<&str> { self.draft.confirmed_at.as_deref() }
    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) { self.save(window, cx); }
    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) { self.mark_submitted(cx); }
    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) { self.mark_as_paid(cx); }
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) { self.revert_to_draft(window, cx); }
    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) { self.preview_pdf(window, cx); }
    fn print_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) { self.print_confirmation(window, cx); }
}

impl Render for Form2551QView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let is_mobile = window.viewport_size().width < px(1100.);
        let carry_label = self.draft.carry_forward_label();
        let is_amended = self.is_amended;
        let total_due = self.draft.total_tax_due;
        let tax_payable = self.draft.tax_payable;
        let is_editable = self.is_editable();

        let title_block = <Self as FormViewTrait>::render_header(self, cx);

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
            .flex_wrap()
            .gap_x_8()
            .gap_y_4()
            .items_center()
            .child(
                Self::readonly_field(
                    "Taxable Year",
                    &self.draft.taxable_year.to_string(),
                    self.get_error("taxable_year"),
                    cx,
                )
                .w(px(120.)),
            )
            .child(
                Self::readonly_field(
                    "Quarter",
                    &format!("Q{}", self.quarter),
                    self.get_error("quarter"),
                    cx,
                )
                .w(px(80.)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Self::field_label("Options", cx))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_x_6()
                            .gap_y_3()
                            .child(
                                div()
                                    .id("amended_toggle")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(is_editable, |el| el.cursor_pointer())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.is_editable() {
                                            return;
                                        }
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
                                                    .text_color(cx.theme().primary_foreground)
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
                                        if !this.is_editable() {
                                            return;
                                        }
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
                                                    .text_color(cx.theme().primary_foreground)
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
        let background_info_content = crate::components::form_parts::taxpayer_info_section(
            crate::components::form_parts::TaxpayerInfoProps {
                tin: &self.draft.tin,
                tin_err: self.get_error("tin"),
                rdo: &self.draft.rdo_code,
                rdo_err: self.get_error("rdo_code"),
                name: &self.draft.taxpayer_name,
                name_err: self.get_error("taxpayer_name"),
                address: &self.draft.registered_address,
                address_err: self.get_error("registered_address"),
                zip: &self.draft.zip_code,
                zip_err: self.get_error("zip_code"),
                contact: &self.draft.contact_number,
                contact_err: self.get_error("contact_number"),
                email: &self.draft.email,
                email_err: self.get_error("email"),
            },
            cx,
        );

        // schedule_one content
        let schedule_rows = self.draft.schedule_1.iter().enumerate().map(|(i, row)| {
            let err_id = format!("schedule_1_row_{}", i + 1);
            let has_err = self.get_error(&err_id).is_some();
            let input_component = if let Some(row_in) = self.row_inputs.get(i) {
                div()
                    .bg(cx.theme().background)
                    .border_1()
                    .rounded_md()
                    .border_color(if has_err { cx.theme().danger } else { cx.theme().border })
                    .px_2()
                    .py_1()
                    .child(Input::new(&row_in.taxable_amount).disabled(!is_editable).appearance(false))
                    .into_any_element()
            } else {
                div().child("—").into_any_element()
            };
            crate::components::form_parts::ScheduleRowProps {
                atc: row.atc.clone(),
                description: row.atc_description.clone(),
                amount_label: "TAXABLE AMOUNT (₱)".to_string(),
                rate: format!("{:.1}%", row.tax_rate * 100.0),
                tax_due: row.tax_due,
                error_message: self.get_error(&err_id),
                input_component,
            }
        }).collect::<Vec<_>>();

        let schedule_one_content = crate::components::form_parts::atc_schedule_table(
            crate::components::form_parts::AtcScheduleTableProps {
                title: "",
                amount_col_label: "TAXABLE AMOUNT (₱)",
                is_mobile,
                rows: schedule_rows,
            },
            cx,
        );

        // tax_computation content
        let tax_computation_content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(crate::components::form_parts::computation_row_readonly(
                "14. Total Tax Due (from Schedule 1)",
                total_due,
                false,
                cx,
            ))
            .child(crate::components::form_parts::computation_row_input(
                crate::components::form_parts::ComputationRowInputProps {
                    label: "15. Less: Creditable Percentage Tax Withheld (BIR Form 2307)",
                    input_component: div()
                        .bg(cx.theme().background)
                        .border_1()
                        .rounded_md()
                        .border_color(if self.get_error("creditable_withheld").is_some() { cx.theme().danger } else { cx.theme().border })
                        .px_2()
                        .py_1()
                        .child(Input::new(&self.creditable_withheld_input).disabled(!is_editable).appearance(false))
                        .into_any_element(),
                    error_message: self.get_error("creditable_withheld"),
                    locked_message: None,
                    is_mobile,
                },
                cx,
            ))
            .child(crate::components::form_parts::computation_row_input(
                crate::components::form_parts::ComputationRowInputProps {
                    label: "16. Less: Tax Paid in Return Previously Filed",
                    input_component: div()
                        .bg(cx.theme().background)
                        .border_1()
                        .rounded_md()
                        .border_color(if self.get_error("tax_paid_previous").is_some() { cx.theme().danger } else { cx.theme().border })
                        .px_2()
                        .py_1()
                        .child(Input::new(&self.tax_paid_previous_input).disabled(!is_editable).appearance(false))
                        .into_any_element(),
                    error_message: self.get_error("tax_paid_previous"),
                    locked_message: if is_amended { None } else { Some("Check Amended Return to unlock") },
                    is_mobile,
                },
                cx,
            ))
            .child(
                div()
                    .pt_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(crate::components::form_parts::computation_row_readonly(
                        "17. Tax Still Payable / (Overpayment)",
                        tax_payable,
                        true,
                        cx,
                    ))
            )
            .child(
                div()
                    .pt_4()
                    .child(
                        crate::components::form_parts::penalty_summary_section(
                            self.draft.surcharge,
                            self.draft.interest,
                            self.draft.compromise,
                            self.draft.total_penalties,
                            self.draft.total_amount_payable,
                            cx,
                        )
                    )
            );

        // Actions moved to toolbar

        let is_submitted = !matches!(self.draft.status, FilingStatus::Draft);

        let is_filing_period_valid = is_submitted
            || ((1900..=9999).contains(&self.draft.taxable_year)
                && (1..=4).contains(&self.quarter));

        let is_background_info_valid = is_submitted
            || (!self.draft.tin.trim().is_empty()
                && !self.draft.rdo_code.trim().is_empty()
                && !self.draft.taxpayer_name.trim().is_empty()
                && !self.draft.registered_address.trim().is_empty()
                && !self.draft.zip_code.trim().is_empty()
                && validate_zip(self.draft.zip_code.trim())
                && !self.draft.contact_number.trim().is_empty()
                && validate_ph_phone(&self.draft.contact_number)
                && !self.draft.email.trim().is_empty()
                && validate_email(&self.draft.email));

        let is_schedule_1_valid = is_submitted
            || (!self.draft.schedule_1.is_empty()
                && self.row_inputs.iter().all(|r| {
                    let val = r.taxable_amount.read(cx).value();
                    !val.trim().is_empty() && val.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false)
                }));

        let is_tax_computation_valid = is_submitted || {
            let cw = self.creditable_withheld_input.read(cx).value();
            let tp = self.tax_paid_previous_input.read(cx).value();
            let cw_valid =
                !cw.trim().is_empty() && cw.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false);
            let tp_valid = if self.is_amended {
                !tp.trim().is_empty() && tp.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false)
            } else {
                true
            };
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
            .child(crate::components::form_parts::form_accordion(
                "acc_filing_period",
                "FILING PERIOD",
                self.show_filing_period,
                is_filing_period_valid,
                self.has_section_error("filing_period"),
                cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_filing_period = !this.show_filing_period;
                    cx.notify();
                }),
                filing_period_content.into_any_element(),
                cx,
            ))
            .child(crate::components::form_parts::form_accordion(
                "acc_background_info",
                "PART I — BACKGROUND INFORMATION (pre-filled from profile)",
                self.show_background_info,
                is_background_info_valid,
                self.has_section_error("background_info"),
                cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_background_info = !this.show_background_info;
                    cx.notify();
                }),
                background_info_content.into_any_element(),
                cx,
            ))
            .child(crate::components::form_parts::form_accordion(
                "acc_schedule_1",
                "SCHEDULE 1 — COMPUTATION OF TAX",
                self.show_schedule_1,
                is_schedule_1_valid,
                self.has_section_error("schedule_1"),
                cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_schedule_1 = !this.show_schedule_1;
                    cx.notify();
                }),
                schedule_one_content.into_any_element(),
                cx,
            ))
            .child(crate::components::form_parts::form_accordion(
                "acc_tax_computation",
                "PART II — COMPUTATION OF TAX",
                self.show_tax_computation,
                is_tax_computation_valid,
                self.has_section_error("tax_computation"),
                cx.listener(|this: &mut Self, _, _, cx| {
                    this.show_tax_computation = !this.show_tax_computation;
                    cx.notify();
                }),
                tax_computation_content.into_any_element(),
                cx,
            ));

        let status_pipeline = <Self as FormViewTrait>::render_status_pipeline(self, cx);

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
                                    gpui_component::button::Button::new("submit_btn")
                                        .label("Submit")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.sync_from_inputs(cx);
                                            this.validation_errors = this.validate_for_submit(cx);
                                            this.suppressed_sections.clear();
                                            if this.validation_errors.is_empty() {
                                                this.is_validated = true;
                                                this.status_message = None;
                                                this.mark_submitted(cx);
                                            } else {
                                                this.is_validated = false;
                                                this.status_message = Some("Fix validation errors to continue.".to_string());
                                                use gpui_component::WindowExt;
                                                window.push_notification(
                                                    gpui_component::notification::Notification::new()
                                                        .message("Please check the form for missing or invalid entries before submitting.".to_string())
                                                        .with_type(gpui_component::notification::NotificationType::Error)
                                                        .autohide(true),
                                                    cx,
                                                );
                                                cx.notify();
                                            }
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
