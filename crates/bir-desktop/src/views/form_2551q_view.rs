//! BIR Form 2551Q — Full in-app form view.
//!
//! Single scrollable view that mimics the actual BIR form layout.
//! No wizards. Profile data is pre-filled and read-only.
//! Schedule 1 is editable. Part II auto-computes.

use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_2551q::{FilingStatus, Form2551QDraft, Schedule1Row};
use bir_core::{parse_bir_receipt_email, validate_ph_phone, validate_zip};
use bir_print::{PaperSize, write_2551q_pdf};

pub enum Form2551QEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
}

impl EventEmitter<Form2551QEvent> for Form2551QView {}

struct ScheduleRowInputs {
    taxable_amount: Entity<InputState>,
}

pub struct Form2551QView {
    draft: Form2551QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    // Editable inputs
    year_input: Entity<InputState>,
    quarter: u8,
    is_amended: bool,
    tax_relief: bool,

    // Schedule 1 row inputs (parallel to draft.schedule_1)
    row_inputs: Vec<ScheduleRowInputs>,

    // Part II inputs
    creditable_withheld_input: Entity<InputState>,
    tax_paid_previous_input: Entity<InputState>,
    receipt_input: Entity<InputState>,

    validation_errors: Vec<String>,
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
        let year_str = draft.taxable_year.to_string();
        let cred_str = if draft.creditable_tax_withheld > 0.0 {
            format!("{:.2}", draft.creditable_tax_withheld)
        } else {
            String::new()
        };
        let prev_str = if draft.tax_paid_previous > 0.0 {
            format!("{:.2}", draft.tax_paid_previous)
        } else {
            String::new()
        };

        let year_input = cx.new(|cx| InputState::new(window, cx).default_value(year_str));
        let creditable_withheld_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(cred_str)
                .placeholder("0.00")
        });
        let tax_paid_previous_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(prev_str)
                .placeholder("0.00")
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
            let amt_str = if row.taxable_amount > 0.0 {
                format!("{:.2}", row.taxable_amount)
            } else {
                String::new()
            };
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(amt_str)
                    .placeholder("0.00")
            });

            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
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
                if matches!(event, InputEvent::Change) {
                    this.sync_from_inputs(cx);
                }
            },
        );
        let sub2 = cx.subscribe_in(
            &tax_paid_previous_input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.sync_from_inputs(cx);
                }
            },
        );

        subscriptions.push(sub1);
        subscriptions.push(sub2);

        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            year_input,
            quarter,
            is_amended,
            tax_relief,
            row_inputs,
            creditable_withheld_input,
            tax_paid_previous_input,
            receipt_input,
            validation_errors: Vec::new(),
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
        // Sync year
        if let Ok(y) = self.year_input.read(cx).value().parse::<u16>() {
            self.draft.taxable_year = y;
        }
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
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
                    }
                },
            ));

            self.row_inputs.push(ScheduleRowInputs {
                taxable_amount: input,
            });
            cx.notify();
        }
    }

    fn save_draft(&mut self, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        self.validation_errors = self.validate_for_submit(cx);
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
            self.status_message = Some("Draft saved".to_string());
            cx.emit(Form2551QEvent::Saved);
        }
    }

    fn mark_submitted(&mut self, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        self.validation_errors = self.validate_for_submit(cx);
        if !self.validation_errors.is_empty() {
            self.status_message = Some("Fix validation errors before submitting".to_string());
            cx.notify();
            return;
        }

        self.draft.status = FilingStatus::Submitted;
        self.draft.submitted_at = Some(chrono::Utc::now().to_rfc3339());
        self.draft.submission_filename = Some(self.draft.default_submission_filename());
        if let Ok(db) = self.db.lock() {
            let _ = db.save_2551q_draft(&self.draft);
            self.status_message =
                Some("Marked as submitted. Waiting for BIR confirmation.".to_string());
            cx.emit(Form2551QEvent::Submitted);
        }
    }

    fn on_submit_action(
        &mut self,
        _: &crate::SubmitCurrentForm,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mark_submitted(cx);
    }

    fn validate_for_submit(&self, cx: &mut Context<Self>) -> Vec<String> {
        let mut errors = Vec::new();
        let year_value = self.year_input.read(cx).value();
        match year_value.parse::<u16>() {
            Ok(year) if (1900..=9999).contains(&year) => {}
            _ => errors.push("Taxable year must be a 4-digit year".to_string()),
        }

        if !(1..=4).contains(&self.quarter) {
            errors.push("Quarter is required".to_string());
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
                errors.push(format!("{label} is required"));
            }
        }

        if !self.draft.zip_code.trim().is_empty() && !validate_zip(self.draft.zip_code.trim()) {
            errors.push("ZIP Code must be 4 digits".to_string());
        }
        if !self.draft.contact_number.trim().is_empty()
            && !validate_ph_phone(&self.draft.contact_number)
        {
            errors.push(
                "Contact number must be a valid Philippine mobile or landline number".to_string(),
            );
        }

        if self.draft.schedule_1.is_empty() {
            errors.push("Schedule 1 requires at least one ATC row".to_string());
        }
        for (i, row_input) in self.row_inputs.iter().enumerate() {
            let value = row_input.taxable_amount.read(cx).value();
            if value.trim().is_empty() {
                errors.push(format!(
                    "Schedule 1 row {} taxable amount is required",
                    i + 1
                ));
            } else if value.parse::<f64>().map(|n| n < 0.0).unwrap_or(true) {
                errors.push(format!(
                    "Schedule 1 row {} taxable amount must be non-negative",
                    i + 1
                ));
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
                errors.push(format!("{label} is required"));
            } else if value.parse::<f64>().map(|n| n < 0.0).unwrap_or(true) {
                errors.push(format!("{label} must be non-negative"));
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

    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        let dir = std::env::temp_dir().join("taxman-ebir-pdf");
        let filename = format!(
            "{}.pdf",
            self.draft
                .default_submission_filename()
                .trim_end_matches(".xml")
        );
        let path = dir.join(filename);
        match write_2551q_pdf(&self.draft, PaperSize::A4, &path) {
            Ok(path) => {
                let _ = std::process::Command::new("open").arg(&path).spawn();
                self.status_message = Some(format!("PDF generated: {}", path.display()));
            }
            Err(err) => {
                self.status_message = Some(format!("PDF generation failed: {err}"));
            }
        }
        cx.notify();
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

        let filing_period = div()
            .bg(cx.theme().secondary)
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .id("header_filing_period")
                    .flex()
                    .justify_between()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_filing_period = !this.show_filing_period
            });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("FILING PERIOD"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(filing_period)
            .child(background_info)
            .child(schedule_one)
            .child(tax_computation)
            .child(receipt_section)
            .child(actions);

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
                            .label("← Dashboard")
                            .on_click(cx.listener(|_this, _, _, cx| {
                                cx.emit(Form2551QEvent::BackToDashboard);
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
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_draft(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("print_btn")
                                    .label("Print PDF")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_pdf(cx);
                                    })),
                            ),
                    ),
            )
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
