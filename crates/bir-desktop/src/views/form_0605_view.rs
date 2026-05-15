//! BIR Form 0605 — Full in-app form view.
//!
//! Payment Form — used for annual registration fee and other tax payments.
//! Simpler than withholding forms: just Basic Tax + Penalties = Total Due.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_0605::Form0605Draft;

use crate::components::form_engine::FormViewTrait;

pub enum Form0605Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form0605Event> for Form0605View {}

pub struct Form0605View {
    draft: Form0605Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    // Tax computation inputs (user-entered)
    tax_19_basic_tax: Entity<InputState>,
    tax_20a_surcharge: Entity<InputState>,
    tax_20b_interest: Entity<InputState>,
    tax_20c_compromise: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form0605View {
    pub fn new(
        draft: Form0605Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        let create_input = |cx: &mut Context<Self>, val: f64, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let tax_19_basic_tax = create_input(cx, draft.txt_tax19, window);
        let tax_20a_surcharge = create_input(cx, draft.txt_tax20a, window);
        let tax_20b_interest = create_input(cx, draft.txt_tax20b, window);
        let tax_20c_compromise = create_input(cx, draft.txt_tax20c, window);

        let inputs = vec![
            tax_19_basic_tax.clone(),
            tax_20a_surcharge.clone(),
            tax_20b_interest.clone(),
            tax_20c_compromise.clone(),
        ];

        for input in inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                },
            ));
        }

        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            validation_errors: Vec::new(),
            status_message: None,

            tax_19_basic_tax,
            tax_20a_surcharge,
            tax_20b_interest,
            tax_20c_compromise,

            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let get_val = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<f64>().unwrap_or(0.0)
        };

        self.draft.txt_tax19 = get_val(&self.tax_19_basic_tax, cx);
        self.draft.txt_tax20a = get_val(&self.tax_20a_surcharge, cx);
        self.draft.txt_tax20b = get_val(&self.tax_20b_interest, cx);
        self.draft.txt_tax20c = get_val(&self.tax_20c_compromise, cx);

        self.draft.recompute();

        use bir_core::forms::FormValidator;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }
}

impl FormViewTrait for Form0605View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 0605"
    }

    fn form_subtitle(&self) -> &'static str {
        "Payment Form"
    }

    fn form_version(&self) -> &'static str {
        "July 2020 (ENCS)"
    }

    fn current_status(&self) -> FilingStatus {
        self.draft.status.clone()
    }

    fn submitted_at(&self) -> Option<&str> {
        self.draft.submitted_at.as_deref()
    }

    fn confirmed_at(&self) -> Option<&str> {
        self.draft.confirmed_at.as_deref()
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if let Ok(db) = self.db.lock() {
            let _ = db.save_form_draft(
                &self.draft.tin,
                "0605",
                self.draft.taxable_year,
                Some(self.draft.month),
                &self.draft.status,
                &self.draft,
            );
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message("Form saved.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Success)
                    .autohide(true),
                cx,
            );
            cx.emit(Form0605Event::Saved);
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Preview only: validation, XML submission, persistence, and print layout are not certified."
                .to_string(),
        );
        cx.emit(Form0605Event::PushNotification(
            "warning".to_string(),
            "Preview Only".to_string(),
            "Form 0605 is scaffold-only and cannot be queued for submission yet.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Paid;
        self.save_draft(window, cx);
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Draft;
        self.draft.submitted_at = None;
        self.draft.confirmed_at = None;
        self.draft.receipt_id = None;
        self.draft.submission_filename = None;
        self.save_draft(window, cx);
        cx.notify();
    }

    fn preview_pdf(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // To be implemented via typst/print module later
    }
}

impl Render for Form0605View {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            // Top bar
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
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form0605Event::BackToDashboard);
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
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("submit_btn")
                                    .label("Preview Only")
                                    .primary()
                                    .disabled(true)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.mark_submitted(window, cx);
                                    })),
                            ),
                    ),
            )
            // Header + status pipeline
            .child(
                div()
                    .p_6()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().accent)
                    .child(self.render_header(cx))
                    .child(div().mt_6().child(self.render_status_pipeline(cx))),
            )
            // Scrollable content
            .child(
                div()
                    .id("scroll_container")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(800.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_6()
                            // Taxpayer profile (read-only)
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("TAXPAYER PROFILE"),
                                            )
                                            .child(
                                                div().mt_2().flex().gap_4().child(
                                                    div()
                                                        .text_xl()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(self.draft.taxpayer_name.clone()),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .text_sm()
                                                    .child(format!("TIN: {}", self.draft.tin)),
                                            )
                                            .child(div().mt_1().text_sm().child(format!(
                                                "RDO Code: {}",
                                                self.draft.rdo_code
                                            )))
                                            .child(div().mt_1().text_sm().child(format!(
                                                "Address: {}",
                                                self.draft.registered_address
                                            ))),
                                    ),
                            )
                            // Tax Computation
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Computation of Tax"),
                                            )
                                            .child(self.render_input_row(
                                                "19 Basic Tax / Amount of Payment",
                                                &self.tax_19_basic_tax,
                                                cx,
                                            )),
                                    ),
                            )
                            // Penalties
                            .child(
                                div()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_4()
                                            .p_4()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Add: Penalties"),
                                            )
                                            .child(self.render_input_row(
                                                "20A Surcharge",
                                                &self.tax_20a_surcharge,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "20B Interest",
                                                &self.tax_20b_interest,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "20C Compromise",
                                                &self.tax_20c_compromise,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "20D Total Penalties (20A + 20B + 20C)",
                                                self.draft.txt_tax20d,
                                                cx,
                                            )),
                                    ),
                            )
                            // Total Amount Due
                            .child(
                                div()
                                    .bg(cx.theme().primary.opacity(0.1))
                                    .border_1()
                                    .border_color(cx.theme().primary.opacity(0.2))
                                    .rounded_lg()
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .p_6()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(cx.theme().primary)
                                                    .child("21 Total Amount Due (19 + 20D)"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "₱ {:.2}",
                                                        self.draft.txt_tax21
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Form0605View {
    fn render_input_row(
        &self,
        label: &str,
        input: &Entity<InputState>,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w_1_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(div().w_1_2().child(Input::new(input).disabled(is_disabled)))
    }

    fn render_computed_row(&self, label: &str, value: f64, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .w_1_2()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .w_1_2()
                    .text_right()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{:.2}", value)),
            )
    }
}
