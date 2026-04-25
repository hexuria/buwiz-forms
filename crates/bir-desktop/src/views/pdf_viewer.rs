use bir_core::forms::form_2551q::Form2551QDraft;
use bir_print::{render_2551q_print, PrintResult};
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
}

impl PdfViewerView {
    pub fn new(draft: Form2551QDraft, result: PrintResult, output_dir: PathBuf) -> Self {
        Self {
            draft,
            result,
            output_dir,
            scroll_handle: ScrollHandle::new(),
            status_message: None,
        }
    }

    fn regenerate(&mut self, cx: &mut Context<Self>) {
        match render_2551q_print(&self.draft, &self.output_dir) {
            Ok(result) => {
                self.result = result;
                self.status_message = Some("PDF regenerated".to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("PDF generation failed: {err}"));
            }
        }
        cx.notify();
    }

    fn open_pdf(&self) {
        let _ = std::process::Command::new("open")
            .arg(&self.result.pdf_path)
            .spawn();
    }

    fn reveal_pdf(&self) {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(&self.result.pdf_path)
            .spawn();
    }

    fn save_pdf_as(&mut self, cx: &mut Context<Self>) {
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

        match std::fs::copy(&self.result.pdf_path, &target) {
            Ok(_) => {
                self.status_message = Some(format!("PDF saved: {}", target.display()));
            }
            Err(err) => {
                self.status_message = Some(format!("PDF save failed: {err}"));
            }
        }
        cx.notify();
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
                    .child("Preview images are unavailable. Use Open PDF to inspect the generated file."),
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
                                    .child("PDF Viewer"),
                            )
                            .child(status),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_close_btn")
                                    .label("Close")
                                    .outline()
                                    .on_click(cx.listener(|_, _, window, _| {
                                        window.remove_window();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_regen_btn")
                                    .label("Regenerate")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.regenerate(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_open_btn")
                                    .label("Open PDF")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.open_pdf();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_reveal_btn")
                                    .label("Reveal in Finder")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.reveal_pdf();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_save_btn")
                                    .label("Save as PDF")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_pdf_as(cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("pdf_viewer_print_btn")
                                    .label("Print")
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.open_pdf();
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
