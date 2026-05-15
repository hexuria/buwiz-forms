//! BIR Form 1702MX — Full in-app form view.
//!
//! Annual Income Tax Return for Corporations with Mixed Income.
//! 3-column (Regular/Special/Total) income computation.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1702mx::Form1702MXDraft;

use crate::components::form_engine::FormViewTrait;

pub enum Form1702MXEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}
impl EventEmitter<Form1702MXEvent> for Form1702MXView {}

pub struct Form1702MXView {
    draft: Form1702MXDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    // 3-column inputs: CA (Regular), CB (Special)
    sales_ca: Entity<InputState>,
    sales_cb: Entity<InputState>,
    cost_ca: Entity<InputState>,
    cost_cb: Entity<InputState>,
    // Penalties
    surcharge: Entity<InputState>,
    interest: Entity<InputState>,
    compromise: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Form1702MXView {
    pub fn new(
        draft: Form1702MXDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subs = Vec::new();
        let ci = |cx: &mut Context<Self>, val: &str, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
            if !val.is_empty() {
                input.update(cx, |i, cx| i.set_value(val.to_string(), window, cx));
            }
            input
        };
        let sales_ca = ci(cx, &draft.txt_pg2pt4i31ca, window);
        let sales_cb = ci(cx, &draft.txt_pg2pt4i31cb, window);
        let cost_ca = ci(cx, &draft.txt_pg2pt4i32ca, window);
        let cost_cb = ci(cx, &draft.txt_pg2pt4i32cb, window);
        let surcharge = ci(cx, &draft.txt_pg1pt2i17.to_string(), window);
        let interest = ci(cx, &draft.txt_pg1pt2i18.to_string(), window);
        let compromise = ci(cx, &draft.txt_pg1pt2i19.to_string(), window);

        let inputs = vec![
            sales_ca.clone(),
            sales_cb.clone(),
            cost_ca.clone(),
            cost_cb.clone(),
            surcharge.clone(),
            interest.clone(),
            compromise.clone(),
        ];
        for input in inputs {
            subs.push(cx.subscribe_in(
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
            sales_ca,
            sales_cb,
            cost_ca,
            cost_cb,
            surcharge,
            interest,
            compromise,
            _subscriptions: subs,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let gv =
            |input: &Entity<InputState>, cx: &Context<Self>| input.read(cx).value().to_string();
        let gu = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<u32>().unwrap_or(0)
        };
        self.draft.txt_pg2pt4i31ca = gv(&self.sales_ca, cx);
        self.draft.txt_pg2pt4i31cb = gv(&self.sales_cb, cx);
        self.draft.txt_pg2pt4i32ca = gv(&self.cost_ca, cx);
        self.draft.txt_pg2pt4i32cb = gv(&self.cost_cb, cx);
        self.draft.txt_pg1pt2i17 = gu(&self.surcharge, cx);
        self.draft.txt_pg1pt2i18 = gu(&self.interest, cx);
        self.draft.txt_pg1pt2i19 = gu(&self.compromise, cx);
        self.draft.recompute();
        use bir_core::forms::FormValidator;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn render_input_row(
        &self,
        label: &str,
        input: &Entity<InputState>,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        let disabled = !matches!(self.draft.status, FilingStatus::Draft);
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
            .child(div().w_1_2().child(Input::new(input).disabled(disabled)))
    }
    fn render_computed_row(
        &self,
        label: &str,
        value: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
                    .child(if value.is_empty() {
                        "0".to_string()
                    } else {
                        value.to_string()
                    }),
            )
    }
    fn render_section(
        &self,
        title: &str,
        children: Vec<AnyElement>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mut col = div().flex().flex_col().gap_4().p_4().child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child(title.to_string()),
        );
        for child in children {
            col = col.child(child);
        }
        div()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(col)
    }
}

impl FormViewTrait for Form1702MXView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702-MX"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual ITR for Corporations (Mixed Income)"
    }
    fn form_version(&self) -> &'static str {
        "2018C (ENCS)"
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
                "1702MX",
                self.draft.taxable_year,
                None,
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
            cx.emit(Form1702MXEvent::Saved);
        }
    }
    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Form1702MXEvent::PushNotification(
            "warning".into(),
            "Preview Only".into(),
            "Form 1702MX is scaffold-only and cannot be queued for submission yet.".into(),
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
    fn preview_pdf(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}

impl Render for Form1702MXView {
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
                            .on_click(
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(Form1702MXEvent::BackToDashboard)
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("save_btn")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, w, cx| this.save_draft(w, cx))),
                            )
                            .child(
                                gpui_component::button::Button::new("submit_btn")
                                    .label("Preview Only")
                                    .primary()
                                    .disabled(true)
                                    .on_click(
                                        cx.listener(|this, _, w, cx| this.mark_submitted(w, cx)),
                                    ),
                            ),
                    ),
            )
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
                    .id("scroll")
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
                            .child(self.render_section(
                                "Taxpayer Profile",
                                vec![
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(self.draft.taxpayer_name.clone()),
                                            )
                                            .child(div().mt_1().text_sm().child(format!(
                                                "TIN: {} | RDO: {}",
                                                self.draft.tin, self.draft.rdo_code
                                            )))
                                            .child(div().mt_1().text_sm().child(format!(
                                                "Deduction: {} | Special Rate: {}%",
                                                if self.draft.rdo_pg1pt1i13method_of_deduc_optional
                                                {
                                                    "OSD (40%)"
                                                } else {
                                                    "Itemized"
                                                },
                                                self.draft.txt_pg2pt4i34special_tax_rate
                                            )))
                                            .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Part 4 — Regular Income (Col A)",
                                vec![
                                        self.render_input_row(
                                            "Item 31A: Net Sales (Regular)",
                                            &self.sales_ca,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 32A: Less: Cost of Sales",
                                            &self.cost_ca,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 33A: Gross Income (Regular)",
                                            &self.draft.txt_pg2pt4i33ca,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 35A: Deductions (Regular)",
                                            &self.draft.txt_pg2pt4i35ca,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 36A: Net Taxable (Regular)",
                                            &self.draft.txt_pg2pt4i36ca,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Part 4 — Special Income (Col B)",
                                vec![
                                        self.render_input_row(
                                            "Item 31B: Net Sales (Special)",
                                            &self.sales_cb,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 32B: Less: Cost of Sales",
                                            &self.cost_cb,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 33B: Gross Income (Special)",
                                            &self.draft.txt_pg2pt4i33cb,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 36B: Net Taxable (Special)",
                                            &self.draft.txt_pg2pt4i36cb,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Tax Computation",
                                vec![
                                        self.render_computed_row(
                                            "RCIT/MCIT on Regular",
                                            &format!("{:.0}", self.draft.txt_pg2sc2it14b),
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Special Tax",
                                            &format!("{:.0}", self.draft.txt_pg2sc2it14c),
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Column (CC)",
                                            &self.draft.txt_pg2pt4i36cc,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Add: Penalties",
                                vec![
                                        self.render_input_row(
                                            "Item 17: Surcharge",
                                            &self.surcharge,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 18: Interest",
                                            &self.interest,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 19: Compromise",
                                            &self.compromise,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 20: Total Penalties",
                                            &self.draft.txt_pg1pt2i20total_penalties.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
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
                                                    .child("Item 21: Total Amount Payable"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "₱ {}",
                                                        self.draft.txt_pg1pt2i21total_amount
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
