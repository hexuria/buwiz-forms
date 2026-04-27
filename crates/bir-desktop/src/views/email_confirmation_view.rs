use bir_core::db::SubmissionReceipt;
use bir_core::forms::form_2551q::Form2551QDraft;
use gpui::prelude::*;
use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;

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
        let lines: Vec<String> = self
            .receipt
            .raw_text
            .lines()
            .map(|s| s.to_string())
            .collect();
        bir_print::build_simple_confirmation_pdf(&lines)
    }

    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let default_name = format!(
            "confirmation-{}.pdf",
            self.draft
                .default_submission_filename()
                .trim_end_matches(".xml")
        );
        let Some(target) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };

        let pdf_bytes = self.generate_pdf_bytes();
        if let Err(err) = std::fs::write(&target, pdf_bytes) {
            self.status_message = Some(format!("Export failed: {}", err));
            cx.notify();
        }
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

        let script = r#"
import AppKit
import PDFKit

func printPDF(path: String) {
    let url = URL(fileURLWithPath: path)
    guard let pdfDoc = PDFDocument(url: url) else { exit(1) }
    
    let printInfo = NSPrintInfo.shared
    printInfo.isHorizontallyCentered = true
    printInfo.isVerticallyCentered = true
    
    let printOp = pdfDoc.printOperation(for: printInfo, scalingMode: .pageScaleDownToFit, autoRotate: true)
    
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    app.activate(ignoringOtherApps: true)
    
    printOp?.showsPrintPanel = true
    printOp?.showsProgressPanel = true
    printOp?.run()
}

let args = CommandLine.arguments
if args.count > 1 {
    printPDF(path: args[1])
}
"#;

        std::thread::spawn(move || {
            use std::io::Write;
            let mut child = match std::process::Command::new("swift")
                .arg("-")
                .arg(&path)
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(_) => {
                    let _ = open::that(&path);
                    return;
                }
            };

            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(script.as_bytes());
            }

            let output = child.wait();
            if output.is_err() || !output.unwrap().success() {
                let _ = open::that(&path);
            }
        });
    }
}

impl Render for EmailConfirmationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let is_mobile = window.viewport_size().width < px(600.);

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
            .bg(cx.theme().background)
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
                                gpui_component::button::Button::new("email_copy_btn")
                                    .outline()
                                    .small()
                                    .tooltip("Copy Text")
                                    .icon(Icon::empty().path("svg/copy.svg").small())
                                    .when(!is_mobile, |this| this.label("Copy Text"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(this.receipt.raw_text.clone()));
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("email_export_btn")
                                    .outline()
                                    .small()
                                    .tooltip("Export PDF")
                                    .icon(Icon::empty().path("svg/download.svg").small())
                                    .when(!is_mobile, |this| this.label("Export"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_pdf(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("email_print_btn")
                                    .outline()
                                    .small()
                                    .tooltip("Print")
                                    .icon(Icon::empty().path("svg/printer.svg").small())
                                    .when(!is_mobile, |this| this.label("Print"))
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
                                        .p_6()
                                        .child({
                                            let highlight_theme = std::sync::Arc::new(gpui_component::highlighter::HighlightTheme {
                                                name: "plain".into(),
                                                appearance: gpui_component::ThemeMode::Dark,
                                                style: gpui_component::highlighter::HighlightThemeStyle {
                                                    editor_background: Some(gpui::transparent_black()),
                                                    editor_foreground: Some(cx.theme().foreground),
                                                    editor_active_line: None,
                                                    editor_line_number: None,
                                                    editor_active_line_number: None,
                                                    editor_invisible: None,
                                                    status: gpui_component::highlighter::StatusColors::default(),
                                                    syntax: gpui_component::highlighter::SyntaxColors::default(),
                                                },
                                            });

                                            let mut text_style = gpui_component::text::TextViewStyle::default();
                                            text_style.highlight_theme = highlight_theme;
                                            text_style.code_block = gpui::StyleRefinement::default()
                                                .bg(gpui::transparent_black())
                                                .text_color(cx.theme().foreground);

                                            div()
                                                .text_sm()
                                                .font_family(".SF NS Mono")
                                                .text_color(cx.theme().foreground)
                                                .child(
                                                    gpui_component::text::TextView::markdown(
                                                        "email-text",
                                                        format!("```\n{}\n```", self.receipt.raw_text.clone())
                                                    )
                                                    .selectable(true)
                                                    .style(text_style)
                                                )
                                        }),
                                ),
                        )
                        .vertical_scrollbar(&self.scroll_handle),
                ),
            )
    }
}
