#![allow(dead_code)]
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::{PrintResult, render_2551q_print};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;
use std::path::PathBuf;

/// Email metadata for the confirmation email, displayed in the preview.
#[derive(Clone)]
pub struct ConfirmationInfo {
    pub subject: String,
    pub from: String,
    pub to: String,
    pub received_date: String,
    pub received_time: String,
    pub body: String,
}

/// Prefix used for all ephemeral PDF output directories.
/// Shared with startup cleanup in app.rs.
pub const TEMP_DIR_PREFIX: &str = "taxman-ebir-pdf-";

pub struct PdfViewerView {
    draft: Form2551QDraft,
    result: PrintResult,
    output_dir: PathBuf,
    scroll_handle: ScrollHandle,
    status_message: Option<String>,
    raw_html: Option<String>,
    confirmation: Option<ConfirmationInfo>,
    /// Path to the combined PDF (form + confirmation text pages).
    effective_pdf_path: PathBuf,
    focus_handle: FocusHandle,
}

impl PdfViewerView {
    pub fn new(
        draft: Form2551QDraft,
        result: PrintResult,
        output_dir: PathBuf,
        raw_html: Option<String>,
        confirmation: Option<ConfirmationInfo>,
        cx: &mut Context<Self>,
    ) -> Self {
        // If we have confirmation info, append text pages directly into the form PDF
        let effective_pdf_path = if let Some(ref info) = confirmation {
            // Build the full text including email header
            let mut lines = Vec::new();
            lines.push(format!("Subject: {}", info.subject));
            lines.push(String::new());
            lines.push(format!("From: {}", info.from));
            lines.push(format!("To: {}", info.to));
            lines.push(format!("Date: {} {}", info.received_date, info.received_time));
            lines.push(String::new());
            lines.push("---".to_string());
            lines.push(String::new());
            for line in info.body.lines() {
                lines.push(line.to_string());
            }

            match std::fs::read(&result.pdf_path) {
                Ok(form_bytes) => {
                    match bir_print::append_text_pages_to_pdf(&form_bytes, &lines) {
                        Ok(combined_bytes) => {
                            let combined_path = output_dir.join("print-preview-combined.pdf");
                            if std::fs::write(&combined_path, combined_bytes).is_ok() {
                                combined_path
                            } else {
                                result.pdf_path.clone()
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to append confirmation pages: {e}");
                            result.pdf_path.clone()
                        }
                    }
                }
                Err(_) => result.pdf_path.clone(),
            }
        } else {
            result.pdf_path.clone()
        };

        Self {
            draft,
            result,
            output_dir,
            scroll_handle: ScrollHandle::new(),
            status_message: None,
            raw_html,
            confirmation,
            effective_pdf_path,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Create a unique ephemeral output directory name.
    pub fn unique_output_dir() -> PathBuf {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        bir_core::platform::temp_dir().join(format!("{}{}", TEMP_DIR_PREFIX, unique_id))
    }

    /// Remove all `taxman-ebir-pdf-*` directories from the system temp folder.
    /// Called at app startup and can be called on demand.
    pub fn cleanup_all_temp_dirs() {
        let temp = bir_core::platform::temp_dir();
        if let Ok(entries) = std::fs::read_dir(&temp) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(TEMP_DIR_PREFIX) && entry.path().is_dir() {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
        // Also clean legacy static directory
        let legacy = temp.join("taxman-ebir-pdf");
        if legacy.exists() {
            let _ = std::fs::remove_dir_all(&legacy);
        }
    }

    fn regenerate(&mut self, cx: &mut Context<Self>) {
        // Clean up previous output before regenerating
        if self.output_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.output_dir);
        }
        self.output_dir = Self::unique_output_dir();
        let formtypes_dir = crate::platform::find_resource_dir("formtypes");
        match render_2551q_print(&self.draft, &self.output_dir, Some(formtypes_dir)) {
            Ok(result) => {
                self.result = result;
            }
            Err(err) => {
                self.status_message = Some(format!("PDF generation failed: {err}"));
            }
        }
        cx.notify();
    }



    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let default_name = self
            .draft
            .default_submission_filename()
            .trim_end_matches(".xml")
            .to_string()
            + ".pdf";
        let source_path = self.effective_pdf_path.clone();

        cx.spawn(async move |this, cx| {
            let Some(target_handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&default_name)
                .add_filter("PDF", &["pdf"])
                .save_file()
                .await
            else {
                return;
            };
            let target = target_handle.path().to_path_buf();

            if let Err(err) = std::fs::copy(&source_path, &target) {
                let _ = this.update(cx, |this, cx| {
                    this.status_message = Some(format!("Export failed: {err}"));
                    cx.notify();
                });
            }
        }).detach();
    }

    fn print_pdf(&mut self, _cx: &mut Context<Self>) {
        crate::platform::print_pdf(&self.effective_pdf_path);
    }

    fn render_page(&self, path: PathBuf) -> impl IntoElement {
        let mut page = div()
            .w_full()
            .max_w(px(900.))
            .mx_auto()
            .bg(gpui::rgb(0xffffff))
            .shadow_sm();
        page.style().aspect_ratio = Some(612.0 / 936.0);
        page.child(img(path).size_full().object_fit(ObjectFit::Contain))
    }
}

/// Cleanup: delete the ephemeral output directory when the viewer is dropped
/// (i.e. when the window is closed).
impl Drop for PdfViewerView {
    fn drop(&mut self) {
        if self.output_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.output_dir);
        }
    }
}

impl Render for PdfViewerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let is_mobile = window.viewport_size().width < px(600.);

        // Ensure the view is focused so it can receive key events
        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window, cx);
        }

        let mut pages = div()
            .id("pdf-viewer-pages")
            .w_full()
            .max_w(px(980.))
            .mx_auto()
            .px_10()
            .py_6()
            .flex()
            .flex_col()
            .gap_8();

        if self.result.preview_png_paths.is_empty() {
            pages = pages.child(
                div()
                    .w_full()
                    .max_w(px(900.))
                    .mx_auto()
                    .px_5()
                    .py_4()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(cx.theme().muted_foreground)
                    .child("Preview images are unavailable. Use Export to save the generated PDF."),
            );
        } else {
            for path in &self.result.preview_png_paths {
                pages = pages.child(self.render_page(path.clone()));
            }
        }

        // Append confirmation email section if available
        if let Some(info) = &self.confirmation {
            pages = pages.child(
                div()
                    .w_full()
                    .max_w(px(900.))
                    .mx_auto()
                    .mt_4()
                    .p_6()
                    .bg(gpui::rgb(0xffffff))
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .gap_1()
                    // Email header
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(info.subject.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("From: {}", info.from)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("To: {}", info.to)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .pb_3()
                            .child(format!("Date: {} {}", info.received_date, info.received_time)),
                    )
                    // Separator
                    .child(
                        div()
                            .h(px(1.))
                            .w_full()
                            .bg(cx.theme().border)
                            .mb_3(),
                    )
                    // Body text — plain, not monospaced
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .whitespace_nowrap()
                            .child(info.body.clone()),
                    ),
            );
        }

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
            .key_context("PdfViewerView")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|_, _: &crate::global_actions::CloseWindow, window, _| {
                    window.remove_window();
                }),
            )
            .on_action(
                cx.listener(|_, _: &crate::global_actions::QuitApplication, window, _| {
                    window.remove_window();
                }),
            )
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
                                    .child("Print Preview"),
                            )
                            .child(status),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(self.draft.payment_receipt_path.is_some(), |this| {
                                this.child(
                                    gpui_component::button::Button::new("pdf_viewer_receipt_btn")
                                        .outline()
                                        .small()
                                        .tooltip("View Payment Receipt")
                                        .icon(Icon::empty().path("svg/receipt.svg").small())
                                        .when(!is_mobile, |this| this.label("Receipt"))
                                        .on_click(cx.listener(|this, _, _, _cx| {
                                            if let Some(path) = &this.draft.payment_receipt_path {
                                                crate::platform::open_in_system(std::path::Path::new(path));
                                            }
                                        })),
                                )
                            })
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_export_btn")
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
                                gpui_component::button::Button::new("pdf_viewer_print_btn")
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
                        .id("pdf-viewer-scroll")
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("pdf-viewer-scroll-area")
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll_handle)
                                .child(pages),
                        )
                        .vertical_scrollbar(&self.scroll_handle),
                ),
            )
    }
}
