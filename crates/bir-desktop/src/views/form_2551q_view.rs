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
use bir_core::validation::{validate_email, validate_ph_phone, validate_zip};
use bir_core::parse_bir_receipt_email;
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
                        this.is_validated = false;
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
                    this.is_validated = false;
                    this.sync_from_inputs(cx);
                }
            },
        );
        let sub2 = cx.subscribe_in(
            &tax_paid_previous_input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.is_validated = false;
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
            is_validated: false,
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
                        this.is_validated = false;
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

        self.status_message = Some("Submitting to BIR Remote Gateway...".to_string());
        cx.notify();

        let draft = self.draft.clone();
        let db = self.db.clone();

                cx.spawn(async move |this, cx| {
            let form_type = "2551Qv2018";
            let filename = format!("{}-{}#{}#{}#.xml", draft.tin, form_type, draft.period_code(), draft.email);
            
            let xml_payload = draft.to_bir_xml_payload();
            
            let passphrase = "T0081gP45sy0rd-To+R3m3m63r!@4/<>";
            let encrypted = match bir_core::crypto::compress_and_encrypt(xml_payload.as_bytes(), passphrase) {
                Ok(enc) => enc,
                Err(e) => {
                    cx.update(|cx| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |this, cx| {
                                this.status_message = Some(format!("Encryption failed: {}", e));
                                cx.notify();
                            });
                        }
                    });
                    return;
                }
            };
            
            // Transmit
            match bir_core::transport::submit_iaf(form_type, &filename, &encrypted).await {
                Ok(_) => {
                    let mut final_draft = draft.clone();
                    final_draft.status = FilingStatus::Submitted;
                    final_draft.submitted_at = Some(chrono::Utc::now().to_rfc3339());
                    final_draft.submission_filename = Some(filename);
                    
                    if let Ok(db) = db.lock() {
                        let _ = db.save_2551q_draft(&final_draft);
                    }
                    
                    cx.update(|cx| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |this, cx| {
                                this.draft = final_draft;
                                this.status_message = Some("Successfully submitted to BIR!".to_string());
                                cx.emit(Form2551QEvent::Submitted);
                                cx.notify();
                            });
                        }
                    });
                }
                Err(e) => {
                    cx.update(|cx| {
                        if let Some(this) = this.upgrade() {
                            this.update(cx, |this, cx| {
                                this.status_message = Some(format!("Submission failed: {}", e));
                                cx.notify();
                            });
                        }
                    });
                }
            }
        })
        .detach();
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
        if !(1900..=9999).contains(&self.draft.taxable_year) {
            errors.push("Taxable year must be a 4-digit year".to_string());
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
            errors.push("ZIP Code must be exactly 4 digits".to_string());
        }
        if !self.draft.contact_number.trim().is_empty()
            && !validate_ph_phone(&self.draft.contact_number)
        {
            errors.push(
                "Contact number must be a valid Philippine mobile or landline number".to_string(),
            );
        }
        if !self.draft.email.trim().is_empty() && !validate_email(&self.draft.email) {
            errors.push("Email Address must be a valid email".to_string());
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
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
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
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
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
                            Input::new(&row_in.taxable_amount).into_any_element()
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
                            .child(Input::new(&self.creditable_withheld_input)),
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
                            .child(Input::new(&self.tax_paid_previous_input)),
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
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("17. Total Amount Payable / (Overpayment)"),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BLACK)
                            .text_color(if tax_payable > 0.0 {
                                cx.theme().primary
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(format!("\u{20b1} {:.2}", tax_payable)),
                    ),
            );

        let validation_block = if self.validation_errors.is_empty() {
            div().into_any_element()
        } else {
            div()
                .bg(gpui::rgba(0x7f1d1d33))
                .border_1()
                .border_color(gpui::rgba(0xff6b6bff))
                .rounded_lg()
                .p_4()
                .flex()
                .flex_col()
                .gap_1()
                .children(self.validation_errors.iter().map(|err| {
                    div()
                        .text_sm()
                        .text_color(gpui::rgba(0xffb4b4ff))
                        .child(err.clone())
                }))
                .into_any_element()
        };

        let actions = div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .pb_12()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status_message.clone().unwrap_or_default()),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        gpui_component::button::Button::new("validate_btn")
                            .label("Validate")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sync_from_inputs(cx);
                                this.validation_errors = this.validate_for_submit(cx);
                                if this.validation_errors.is_empty() {
                                    this.is_validated = true;
                                    this.status_message = Some("Validation successful. Form is ready to submit.".to_string());
                                } else {
                                    this.is_validated = false;
                                    this.status_message = Some("Fix validation errors to continue.".to_string());
                                }
                                cx.notify();
                            }))
                    )
                    .child(
                        gpui_component::button::Button::new("mark_submitted_btn")
                            .label("Mark as Submitted")
                            .disabled(!self.is_validated)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mark_submitted(cx);
                            }))
                    ),
            );

        macro_rules! build_accordion {
            ($id:expr, $label:expr, $is_expanded:expr, $is_valid:expr, $on_click:expr, $content:expr $(,)?) => {
                {
                    let mut card = div()
                        .bg(cx.theme().secondary)
                        .rounded_xl()
                        .border_1()
                        .border_color(cx.theme().border)
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
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child($label),
                                        )
                                        .child(if $is_valid {
                                            div()
                                                .px_2()
                                                .py_1()
                                                .rounded_md()
                                                .bg(gpui::rgba(0x22c55e33))
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gpui::rgba(0x22c55eff))
                                                .child("Verified")
                                        } else {
                                            div()
                                        })
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

        let is_filing_period_valid = (1900..=9999).contains(&self.draft.taxable_year) && (1..=4).contains(&self.quarter);

        let is_background_info_valid = !self.draft.tin.trim().is_empty() &&
            !self.draft.rdo_code.trim().is_empty() &&
            !self.draft.taxpayer_name.trim().is_empty() &&
            !self.draft.registered_address.trim().is_empty() &&
            !self.draft.zip_code.trim().is_empty() && validate_zip(self.draft.zip_code.trim()) &&
            !self.draft.contact_number.trim().is_empty() && validate_ph_phone(&self.draft.contact_number) &&
            !self.draft.email.trim().is_empty() && validate_email(&self.draft.email);

        let is_schedule_1_valid = !self.draft.schedule_1.is_empty() && self.row_inputs.iter().all(|r| {
            let val = r.taxable_amount.read(cx).value();
            !val.trim().is_empty() && val.parse::<f64>().map(|n| n >= 0.0).unwrap_or(false)
        });

        let is_tax_computation_valid = {
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
                cx.listener(|this, _, _, cx| {
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
                cx.listener(|this, _, _, cx| {
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
                cx.listener(|this, _, _, cx| {
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
                cx.listener(|this, _, _, cx| {
                    this.show_tax_computation = !this.show_tax_computation;
                    cx.notify();
                }),
                tax_computation_content,
            ))

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
