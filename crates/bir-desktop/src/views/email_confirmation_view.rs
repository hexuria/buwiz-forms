use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;
use bir_core::db::SubmissionReceipt;
use bir_core::forms::form_2551q::Form2551QDraft;

pub struct EmailConfirmationView {
    receipt: SubmissionReceipt,
    draft: Form2551QDraft,
    scroll_handle: ScrollHandle,
    status_message: Option<String>,
}

impl EmailConfirmationView {
    pub fn new(receipt: SubmissionReceipt, draft: Form2551QDraft) -> Self {
        Self {
            receipt,
            draft,
            scroll_handle: ScrollHandle::new(),
            status_message: None,
        }
    }

    fn generate_pdf_bytes(&self) -> Vec<u8> {
        let lines = vec![
            "BIR e-Filing Confirmation Receipt".to_string(),
            "=".repeat(40),
            String::new(),
            format!("Filename: {}", self.receipt.filename),
            format!("TIN: {}", self.receipt.tin),
            format!("Form Type: {}", self.receipt.form_type),
            format!("Period: {}", self.receipt.period),
            format!("Received Date: {}", self.receipt.received_date),
            format!("Received Time: {}", self.receipt.received_time),
            format!("Source: {}", self.receipt.source_from.as_deref().unwrap_or("BIR")),
            String::new(),
            "=".repeat(40),
            "Form Summary".to_string(),
            format!("Taxpayer: {}", self.draft.taxpayer_name),
            format!("TIN: {}", self.draft.tin),
            format!("Period: Q{} {}", self.draft.quarter, self.draft.taxable_year),
            format!("Total Tax Due: {:.2}", self.draft.total_tax_due),
            format!("Creditable Tax Withheld: {:.2}", self.draft.creditable_tax_withheld),
            format!("Tax Still Payable: {:.2}", self.draft.tax_payable),
            format!("Penalties: {:.2}", self.draft.total_penalties),
            format!("Total Amount Payable: {:.2}", self.draft.total_amount_payable),
            format!("Status: {:?}", self.draft.status),
        ];
        bir_print::build_simple_confirmation_pdf(&lines)
    }

    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let default_name = format!(
            "confirmation-{}.pdf",
            self.draft.default_submission_filename().trim_end_matches(".xml")
        );
        let Some(target) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };

        let pdf_bytes = self.generate_pdf_bytes();
        match std::fs::write(&target, pdf_bytes) {
            Ok(_) => {
                self.status_message = Some("Exported".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("Export failed: {}", err));
            }
        }
        cx.notify();
    }

    fn print_pdf(&mut self, cx: &mut Context<Self>) {
        let pdf_bytes = self.generate_pdf_bytes();
        let dir = std::env::temp_dir().join("taxman-ebir-pdf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("temp-print.pdf");
        
        if let Err(e) = std::fs::write(&path, pdf_bytes) {
            self.status_message = Some(format!("Failed to write temp file: {}", e));
            cx.notify();
            return;
        }

        let output = std::process::Command::new("lp").arg(&path).output();
        match output {
            Ok(o) if o.status.success() => {
                self.status_message = Some("Sent to printer".to_string());
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                self.status_message = Some(format!("Print failed: {}", stderr));
            }
            Err(e) => {
                self.status_message = Some(format!("No printer available: {}", e));
            }
        }
        cx.notify();
    }
}

impl Render for EmailConfirmationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let status = self
            .status_message
            .as_ref()
            .map(|message| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(message.clone())
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element());

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x0b0b0b))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Email Confirmation"),
                            )
                            .child(status),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                gpui_component::button::Button::new("email_export_btn")
                                    .icon(Icon::empty().path("svg/download.svg"))
                                    .outline()
                                    .small()
                                    .tooltip("Export PDF")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_pdf(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("email_print_btn")
                                    .icon(Icon::empty().path("svg/printer.svg"))
                                    .outline()
                                    .small()
                                    .tooltip("Print")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.print_pdf(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div().flex_1().min_h_0().overflow_hidden().child(
                    div()
                        .id("email-scroll")
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("email-scroll-area")
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll_handle)
                                .child(
                                    div()
                                        .w_full()
                                        .max_w(px(900.))
                                        .mx_auto()
                                        .mt_6()
                                        .mb_6()
                                        .p_6()
                                        .bg(cx.theme().background)
                                        .rounded_md()
                                        .shadow_sm()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_family(".SF NS Mono")
                                                .text_color(cx.theme().foreground)
                                                .child(self.receipt.raw_text.clone())
                                        )
                                )
                        )
                        .vertical_scrollbar(&self.scroll_handle),
                ),
            )
    }
}
