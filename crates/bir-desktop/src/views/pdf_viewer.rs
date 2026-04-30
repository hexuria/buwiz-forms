#![allow(dead_code)]
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::{PrintResult, render_2551q_print};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::*;
use std::path::PathBuf;

pub struct PdfViewerView {
    draft: Form2551QDraft,
    result: PrintResult,
    output_dir: PathBuf,
    scroll_handle: ScrollHandle,
    status_message: Option<String>,
    raw_html: Option<String>,
    focus_handle: FocusHandle,
}

impl PdfViewerView {
    pub fn new(
        draft: Form2551QDraft,
        result: PrintResult,
        output_dir: PathBuf,
        raw_html: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            draft,
            result,
            output_dir,
            scroll_handle: ScrollHandle::new(),
            status_message: None,
            raw_html,
            focus_handle: cx.focus_handle(),
        }
    }

    fn regenerate(&mut self, cx: &mut Context<Self>) {
        match render_2551q_print(&self.draft, &self.output_dir) {
            Ok(result) => {
                self.result = result;
            }
            Err(err) => {
                self.status_message = Some(format!("PDF generation failed: {err}"));
            }
        }
        cx.notify();
    }

    fn reveal_pdf(&self) {
        crate::platform::reveal_in_file_manager(&self.result.pdf_path);
    }

    fn view_bank_receipt(&mut self, cx: &mut Context<Self>) {
        if let Some(html) = &self.raw_html {
            let receipt_path = self.output_dir.join("bank_presentation_receipt.html");
            if let Err(e) = std::fs::write(&receipt_path, html) {
                self.status_message = Some(format!("Failed to save receipt: {e}"));
                cx.notify();
                return;
            }
            if let Err(e) = open::that(&receipt_path) {
                self.status_message = Some(format!("Failed to open receipt: {e}"));
                cx.notify();
            }
        }
    }

    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let default_name = self
            .draft
            .default_submission_filename()
            .trim_end_matches(".xml")
            .to_string()
            + ".pdf";
        let Some(target) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };

        if let Err(err) = std::fs::copy(&self.result.pdf_path, &target) {
            self.status_message = Some(format!("Export failed: {err}"));
            cx.notify();
        }
    }

    fn print_pdf(&mut self, _cx: &mut Context<Self>) {
        crate::platform::print_pdf(&self.result.pdf_path);
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
            .on_action(cx.listener(|_, _: &crate::global_actions::CloseWindow, window, _| {
                window.remove_window();
            }))
            .on_action(cx.listener(|_, _: &crate::global_actions::QuitApplication, window, _| {
                window.remove_window();
            }))
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
                                    .child("PDF Viewer"),
                            )
                            .child(status),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_reveal_btn")
                                    .outline()
                                    .small()
                                    .tooltip("Reveal in Finder")
                                    .icon(Icon::empty().path("svg/folder.svg").small())
                                    .when(!is_mobile, |this| this.label("Reveal"))
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.reveal_pdf();
                                    })),
                            )
                            .when(self.raw_html.is_some(), |this| {
                                this.child(
                                    gpui_component::button::Button::new("pdf_viewer_receipt_btn")
                                        .outline()
                                        .small()
                                        .tooltip("View Bank Presentation Receipt")
                                        .icon(Icon::empty().path("svg/receipt.svg").small())
                                        .when(!is_mobile, |this| this.label("Receipt"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.view_bank_receipt(cx);
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
