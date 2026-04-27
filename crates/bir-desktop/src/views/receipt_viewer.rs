use bir_core::forms::form_2551q::Form2551QDraft;
use gpui::*;
use gpui_component::*;
use std::path::PathBuf;

pub enum ReceiptViewerEvent {
    ReUploaded(String), // Returns the new path
}

impl EventEmitter<ReceiptViewerEvent> for ReceiptViewerView {}

pub struct ReceiptViewerView {
    draft: Form2551QDraft,
    path: PathBuf,
    status_message: Option<String>,
}

impl ReceiptViewerView {
    pub fn new(draft: Form2551QDraft, path: String) -> Self {
        Self {
            draft,
            path: PathBuf::from(path),
            status_message: None,
        }
    }

    fn open_in_system(&self) {
        crate::platform::open_in_system(&self.path);
    }

    fn re_upload(&mut self, cx: &mut Context<Self>) {
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
                self.path = new_path.clone();
                let path_str = new_path.to_string_lossy().to_string();
                self.status_message = Some("Receipt updated".to_string());
                cx.emit(ReceiptViewerEvent::ReUploaded(path_str));
                cx.notify();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to copy receipt: {}", e));
                cx.notify();
            }
        }
    }
}

impl Render for ReceiptViewerView {
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

        let is_pdf = self.path.extension().map_or(false, |ext| {
            ext.to_string_lossy().eq_ignore_ascii_case("pdf")
        });

        let content = if is_pdf {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .gap_4()
                .child(div().text_2xl().child("📄"))
                .child(
                    div()
                        .text_lg()
                        .text_color(cx.theme().foreground)
                        .child("PDF Receipt"),
                )
                .child(
                    gpui_component::button::Button::new("open_sys_btn")
                        .label("Open in System Viewer")
                        .on_click(cx.listener(|this, _, _, _| {
                            this.open_in_system();
                        })),
                )
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_8()
                .child(
                    img(self.path.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                )
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().muted)
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
                                    .child("Payment Receipt Viewer"),
                            )
                            .child(status),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                gpui_component::button::Button::new("receipt_open_btn")
                                    .label("Open System Viewer")
                                    .outline()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.open_in_system();
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("receipt_reupload_btn")
                                    .label("Change / Re-upload")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.re_upload(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(cx.theme().background)
                    .child(content),
            )
    }
}
