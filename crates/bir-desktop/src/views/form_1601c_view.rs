//! BIR Form 1601C — Full in-app form view.
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
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::FilingStatus;

use crate::components::form_engine::FormViewTrait;

pub enum Form1601CEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1601CEvent> for Form1601CView {}

pub struct Form1601CView {
    draft: Form1601CDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    // Editable Inputs
    tax_15_total_compensation: Entity<InputState>,
    tax_16a_nontaxable: Entity<InputState>,
    tax_16b_not_subject: Entity<InputState>,
    tax_16c_exempt: Entity<InputState>,
    tax_17_regular: Entity<InputState>,
    tax_18_supplementary: Entity<InputState>,
    tax_20_required_withheld: Entity<InputState>,
    tax_21a_previous_withheld: Entity<InputState>,
    tax_21b_other_payments: Entity<InputState>,
    
    // Penalties
    tax_24a_surcharge: Entity<InputState>,
    tax_24b_interest: Entity<InputState>,
    tax_24c_compromise: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form1601CView {
    pub fn new(
        draft: Form1601CDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        let mut create_input = |cx: &mut Context<Self>, val: f64| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let tax_15_total_compensation = create_input(cx, draft.tax_15_total_compensation);
        let tax_16a_nontaxable = create_input(cx, draft.tax_16a_nontaxable);
        let tax_16b_not_subject = create_input(cx, draft.tax_16b_not_subject);
        let tax_16c_exempt = create_input(cx, draft.tax_16c_exempt);
        let tax_17_regular = create_input(cx, draft.tax_17_regular);
        let tax_18_supplementary = create_input(cx, draft.tax_18_supplementary);
        let tax_20_required_withheld = create_input(cx, draft.tax_20_required_withheld);
        let tax_21a_previous_withheld = create_input(cx, draft.tax_21a_previous_withheld);
        let tax_21b_other_payments = create_input(cx, draft.tax_21b_other_payments);
        
        let tax_24a_surcharge = create_input(cx, draft.tax_24a_surcharge);
        let tax_24b_interest = create_input(cx, draft.tax_24b_interest);
        let tax_24c_compromise = create_input(cx, draft.tax_24c_compromise);

        let inputs = vec![
            tax_15_total_compensation.clone(),
            tax_16a_nontaxable.clone(),
            tax_16b_not_subject.clone(),
            tax_16c_exempt.clone(),
            tax_17_regular.clone(),
            tax_18_supplementary.clone(),
            tax_20_required_withheld.clone(),
            tax_21a_previous_withheld.clone(),
            tax_21b_other_payments.clone(),
            tax_24a_surcharge.clone(),
            tax_24b_interest.clone(),
            tax_24c_compromise.clone(),
        ];

        for input in inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => {
                        this.is_validated = false;
                        this.sync_from_inputs(cx);
                    }
                    _ => {}
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
            
            tax_15_total_compensation,
            tax_16a_nontaxable,
            tax_16b_not_subject,
            tax_16c_exempt,
            tax_17_regular,
            tax_18_supplementary,
            tax_20_required_withheld,
            tax_21a_previous_withheld,
            tax_21b_other_payments,
            tax_24a_surcharge,
            tax_24b_interest,
            tax_24c_compromise,
            
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let get_val = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<f64>().unwrap_or(0.0)
        };

        self.draft.tax_15_total_compensation = get_val(&self.tax_15_total_compensation, cx);
        self.draft.tax_16a_nontaxable = get_val(&self.tax_16a_nontaxable, cx);
        self.draft.tax_16b_not_subject = get_val(&self.tax_16b_not_subject, cx);
        self.draft.tax_16c_exempt = get_val(&self.tax_16c_exempt, cx);
        self.draft.tax_17_regular = get_val(&self.tax_17_regular, cx);
        self.draft.tax_18_supplementary = get_val(&self.tax_18_supplementary, cx);
        self.draft.tax_20_required_withheld = get_val(&self.tax_20_required_withheld, cx);
        self.draft.tax_21a_previous_withheld = get_val(&self.tax_21a_previous_withheld, cx);
        self.draft.tax_21b_other_payments = get_val(&self.tax_21b_other_payments, cx);
        self.draft.tax_24a_surcharge = get_val(&self.tax_24a_surcharge, cx);
        self.draft.tax_24b_interest = get_val(&self.tax_24b_interest, cx);
        self.draft.tax_24c_compromise = get_val(&self.tax_24c_compromise, cx);

        self.draft.compute();
        
        use bir_core::forms::FormValidator;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }
}

impl FormViewTrait for Form1601CView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1601C"
    }

    fn form_subtitle(&self) -> &'static str {
        "Monthly Remittance Return of Income Taxes Withheld on Compensation"
    }

    fn form_version(&self) -> &'static str {
        "January 2018 (ENCS)"
    }

    fn current_status(&self) -> FilingStatus {
        self.draft.status.clone()
    }

    fn submitted_at(&self) -> Option<&str> {
        None
    }

    fn confirmed_at(&self) -> Option<&str> {
        None
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if let Ok(db) = self.db.lock() {
            let _ = db.save_form_draft(
                &self.draft.tin,
                "1601C",
                self.draft.taxable_year,
                Some(self.draft.month),
                &self.draft.status,
                &self.draft
            );
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message("Form saved.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Success)
                    .autohide(true),
                cx,
            );
            cx.emit(Form1601CEvent::Saved);
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Queued;
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Paid;
        cx.notify();
    }

    fn revert_to_draft(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Draft;
        cx.notify();
    }

    fn preview_pdf(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // To be implemented via typst/print module later
    }
}

impl Render for Form1601CView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(
                div()
                    .p_6()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().accent)
                    .child(self.render_header(cx))
                    .child(div().mt_6().child(self.render_status_pipeline(cx))),
            )
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
                            .child(
                                div().bg(cx.theme().background).border_1().border_color(cx.theme().border).rounded_lg().child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .p_4()
                                        .child(div().text_xl().font_weight(FontWeight::BOLD).child("Part II - Computation of Tax"))
                                        .child(self.render_input_row("15 Total Amount of Compensation", &self.tax_15_total_compensation, cx))
                                        .child(self.render_input_row("16A Less: Non-Taxable Compensation", &self.tax_16a_nontaxable, cx))
                                        .child(self.render_input_row("16B Less: Not Subject to Withholding", &self.tax_16b_not_subject, cx))
                                        .child(self.render_input_row("16C Less: Exempt Compensation", &self.tax_16c_exempt, cx))
                                        .child(self.render_input_row("17 Taxable Compensation - Regular", &self.tax_17_regular, cx))
                                        .child(self.render_input_row("18 Taxable Compensation - Supplementary", &self.tax_18_supplementary, cx))
                                        .child(self.render_computed_row("19 Total Taxable Compensation", self.draft.tax_19_total_taxable, cx))
                                        .child(self.render_input_row("20 Total Tax Required to be Withheld", &self.tax_20_required_withheld, cx))
                                )
                            )
                            .child(
                                div().bg(cx.theme().background).border_1().border_color(cx.theme().border).rounded_lg().child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .p_4()
                                        .child(div().text_xl().font_weight(FontWeight::BOLD).child("Adjustments"))
                                        .child(self.render_input_row("21A Less: Tax Withheld from Previous Months", &self.tax_21a_previous_withheld, cx))
                                        .child(self.render_input_row("21B Less: Other Payments/Credits", &self.tax_21b_other_payments, cx))
                                        .child(self.render_computed_row("22 Tax Still Due/(Overremittance)", self.draft.tax_22_still_due, cx))
                                )
                            )
                            .child(
                                div().bg(cx.theme().background).border_1().border_color(cx.theme().border).rounded_lg().child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_4()
                                        .p_4()
                                        .child(div().text_xl().font_weight(FontWeight::BOLD).child("Penalties"))
                                        .child(self.render_input_row("24A Surcharge", &self.tax_24a_surcharge, cx))
                                        .child(self.render_input_row("24B Interest", &self.tax_24b_interest, cx))
                                        .child(self.render_input_row("24C Compromise", &self.tax_24c_compromise, cx))
                                        .child(self.render_computed_row("24D Total Penalties", self.draft.tax_24d_total_penalties, cx))
                                )
                            )
                            .child(
                                div().bg(cx.theme().primary.opacity(0.1)).border_1().border_color(cx.theme().primary.opacity(0.2)).rounded_lg().child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .p_6()
                                        .child(div().text_2xl().font_weight(FontWeight::BOLD).text_color(cx.theme().primary).child("25 Total Amount Payable"))
                                        .child(div().text_2xl().font_weight(FontWeight::BLACK).text_color(cx.theme().primary).child(format!("P {:.2}", self.draft.tax_25_total_payable)))
                                )
                            )
                    )
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().accent)
                    .flex()
                    .justify_end()
                    .gap_4()
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
                            .label("Generate XML & Submit")
                            .primary()
                            .disabled(!is_draft || !self.validation_errors.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.mark_submitted(window, cx);
                            })),
                    )
            )
    }
}

impl Form1601CView {
    fn render_input_row(&self, label: &str, input: &Entity<InputState>, cx: &Context<Self>) -> impl IntoElement {
        let is_disabled = !matches!(self.draft.status, FilingStatus::Draft);
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_4()
            .child(div().w_1_2().text_sm().font_weight(FontWeight::MEDIUM).child(label.to_string()))
            .child(
                div().w_1_2().child(
                    Input::new(input)
                        .disabled(is_disabled)
                )
            )
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
            .child(div().w_1_2().text_sm().font_weight(FontWeight::BOLD).child(label.to_string()))
            .child(div().w_1_2().text_right().font_weight(FontWeight::BOLD).child(format!("{:.2}", value)))
    }
}
