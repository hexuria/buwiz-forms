//! BIR Form 1702RT — Full in-app form view.
//!
//! Annual Income Tax Return for Corporations Subject to Regular Tax.
//! RCIT (25%) vs MCIT (2%) comparison with OSD/Itemized deductions.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1702rt::Form1702RTDraft;

use crate::components::form_engine::FormViewTrait;

pub enum Form1702RTEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}
impl EventEmitter<Form1702RTEvent> for Form1702RTView {}

pub struct Form1702RTView {
    draft: Form1702RTDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    // Key inputs
    sales: Entity<InputState>,
    less_sales: Entity<InputState>,
    cost_of_sales: Entity<InputState>,
    other_taxable: Entity<InputState>,
    surcharge: Entity<InputState>,
    interest: Entity<InputState>,
    compromise: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Form1702RTView {
    pub fn new(
        draft: Form1702RTDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subs = Vec::new();
        let ci = |cx: &mut Context<Self>, val: &str, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
            if !val.is_empty() && val != "0" {
                input.update(cx, |i, cx| i.set_value(val.to_string(), window, cx));
            }
            input
        };
        let sales = ci(cx, &draft.txt_pg2pt4i27sales, window);
        let less_sales = ci(cx, &draft.txt_pg2pt4i28less_sales, window);
        let cost_of_sales = ci(cx, &draft.txt_pg2pt4i30less_cost, window);
        let other_taxable = ci(cx, &draft.txt_pg2pt4i32add_other_taxable, window);
        let surcharge = ci(cx, &draft.txt_pg1pt2i17surcharge, window);
        let interest = ci(cx, &draft.txt_pg1pt2i18interest, window);
        let compromise = ci(cx, &draft.txt_pg1pt2i19compromise, window);

        let inputs = vec![
            sales.clone(),
            less_sales.clone(),
            cost_of_sales.clone(),
            other_taxable.clone(),
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
            sales,
            less_sales,
            cost_of_sales,
            other_taxable,
            surcharge,
            interest,
            compromise,
            _subscriptions: subs,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let gv =
            |input: &Entity<InputState>, cx: &Context<Self>| input.read(cx).value().to_string();
        self.draft.txt_pg2pt4i27sales = gv(&self.sales, cx);
        self.draft.txt_pg2pt4i28less_sales = gv(&self.less_sales, cx);
        self.draft.txt_pg2pt4i30less_cost = gv(&self.cost_of_sales, cx);
        self.draft.txt_pg2pt4i32add_other_taxable = gv(&self.other_taxable, cx);
        self.draft.txt_pg1pt2i17surcharge = gv(&self.surcharge, cx);
        self.draft.txt_pg1pt2i18interest = gv(&self.interest, cx);
        self.draft.txt_pg1pt2i19compromise = gv(&self.compromise, cx);
        self.draft.recompute();
        use bir_core::forms::FormValidator;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn pm(s: &str) -> f64 {
        s.replace(',', "").parse::<f64>().unwrap_or(0.0)
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
                    .child(value.to_string()),
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

impl FormViewTrait for Form1702RTView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702-RT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual ITR for Corporations (Regular Tax)"
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
                "1702RT",
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
            cx.emit(Form1702RTEvent::Saved);
        }
    }
    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.validation_errors.is_empty() {
            cx.notify();
            return;
        }
        self.draft.status = FilingStatus::Queued;
        self.save_draft(window, cx);
        bir_core::background_cron::wake();
        cx.emit(Form1702RTEvent::PushNotification(
            "info".into(),
            "Form Queued".into(),
            "Queued for submission.".into(),
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

impl Render for Form1702RTView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);
        let rcit = self.draft.txt_pg2pt4i41income_tax_due as f64;
        let mcit = Self::pm(&self.draft.txt_pg2pt4i42minimum_corporate);
        let is_mcit = mcit > rcit;

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
                                    cx.emit(Form1702RTEvent::BackToDashboard)
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
                                    .label("Generate XML & Submit")
                                    .primary()
                                    .disabled(!is_draft)
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
                                                "Deduction: {} | Tax Rate: {}%",
                                                if self.draft.rdo_pg1pt1i13optional_standard {
                                                    "OSD (40%)"
                                                } else {
                                                    "Itemized"
                                                },
                                                if self.draft.pg2pt4i40income_tax_rate > 0 {
                                                    self.draft.pg2pt4i40income_tax_rate
                                                } else {
                                                    25
                                                }
                                            )))
                                            .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Part 4 — Income Computation",
                                vec![
                                        self.render_input_row(
                                            "Item 27: Sales/Revenues/Receipts",
                                            &self.sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 28: Less: Sales Returns",
                                            &self.less_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 29: Net Sales",
                                            &self.draft.txt_pg2pt4i29net_sales.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 30: Less: Cost of Sales",
                                            &self.cost_of_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 31: Gross Income",
                                            &self.draft.txt_pg2pt4i31gross_income,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 32: Add: Other Taxable Income",
                                            &self.other_taxable,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 33: Total Gross Income",
                                            &self.draft.txt_pg2pt4i33total_gross.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Deductions & Net Taxable",
                                vec![
                                        self.render_computed_row(
                                            "Item 38: OSD (40%)",
                                            &self.draft.txt_pg2pt4i38optional_standard.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 39: Net Taxable Income",
                                            &self.draft.txt_pg2pt4i39net_taxable.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "RCIT vs MCIT Comparison",
                                vec![
                                        self.render_computed_row(
                                            "Item 41: RCIT (Regular CIT)",
                                            &self.draft.txt_pg2pt4i41income_tax_due.to_string(),
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 42: MCIT (2% of Gross)",
                                            &self.draft.txt_pg2pt4i42minimum_corporate,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            &format!(
                                                "Item 43: Tax Due ({})",
                                                if is_mcit {
                                                    "MCIT applies"
                                                } else {
                                                    "RCIT applies"
                                                }
                                            ),
                                            &self.draft.txt_pg2pt4i43total_income_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Tax Credits & Net Tax",
                                vec![
                                        self.render_computed_row(
                                            "Item 55: Total Tax Credits",
                                            &self.draft.txt_pg2pt4i55total_tax_credits,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 56: Net Tax",
                                            &self.draft.txt_pg2pt4i56net_tax,
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
                                            &self.draft.txt_pg1pt2i20total_penalties,
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
