//! BIR Form 1701 — Full in-app form view.
//!
//! Annual Income Tax Return for Individuals Earning Income PURELY from
//! Business/Profession. Supports graduated tax table and 8% flat rate.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1701::Form1701Draft;

use crate::components::form_engine::FormViewTrait;

pub enum Form1701Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1701Event> for Form1701View {}

pub struct Form1701View {
    draft: Form1701Draft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    is_validated: bool,
    validation_errors: Vec<(String, String)>,

    // Schedule B income sources (Item 1G — total taxpayer revenue)
    income_source1: Entity<InputState>,
    income_source2: Entity<InputState>,
    income_source3: Entity<InputState>,
    // Cost of sales (Items 5G, 6G)
    cost_of_sales1: Entity<InputState>,
    cost_of_sales2: Entity<InputState>,
    // Itemized deductions (Items 10-14, only used when not OSD)
    deduction_10c: Entity<InputState>,
    deduction_11c: Entity<InputState>,
    deduction_12c: Entity<InputState>,
    deduction_13c: Entity<InputState>,
    deduction_14c: Entity<InputState>,
    // Tax credits (Item 23A)
    tax_credits_a: Entity<InputState>,
    // Penalties
    surcharge_a: Entity<InputState>,
    interest_a: Entity<InputState>,
    compromise_a: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form1701View {
    pub fn new(
        draft: Form1701Draft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subs = Vec::new();

        let ci = |cx: &mut Context<Self>, val: f64, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let income_source1 = ci(cx, draft.txt_pg1m_i1gschd_b, window);
        let income_source2 = ci(cx, draft.txt_pg1m_i2gschd_b, window);
        let income_source3 = ci(cx, draft.txt_pg1m_i3gschd_b, window);
        let cost_of_sales1 = ci(cx, draft.txt_pg1m_i5gschd_b, window);
        let cost_of_sales2 = ci(cx, draft.txt_pg1m_i6gschd_b, window);
        let deduction_10c = ci(cx, draft.txt_pg1m_i10cschd_b, window);
        let deduction_11c = ci(cx, draft.txt_pg1m_i11cschd_b, window);
        let deduction_12c = ci(cx, draft.txt_pg1m_i12cschd_b, window);
        let deduction_13c = ci(cx, draft.txt_pg1m_i13cschd_b, window);
        let deduction_14c = ci(cx, draft.txt_pg1m_i14cschd_b, window);
        let tax_credits_a = ci(cx, draft.txt_pg1i23a, window);
        let surcharge_a = ci(cx, draft.txt_pg1i25a, window);
        let interest_a = ci(cx, draft.txt_pg1i26a, window);
        let compromise_a = ci(cx, draft.txt_pg1i27a, window);

        let inputs: Vec<Entity<InputState>> = vec![
            income_source1.clone(),
            income_source2.clone(),
            income_source3.clone(),
            cost_of_sales1.clone(),
            cost_of_sales2.clone(),
            deduction_10c.clone(),
            deduction_11c.clone(),
            deduction_12c.clone(),
            deduction_13c.clone(),
            deduction_14c.clone(),
            tax_credits_a.clone(),
            surcharge_a.clone(),
            interest_a.clone(),
            compromise_a.clone(),
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
            income_source1,
            income_source2,
            income_source3,
            cost_of_sales1,
            cost_of_sales2,
            deduction_10c,
            deduction_11c,
            deduction_12c,
            deduction_13c,
            deduction_14c,
            tax_credits_a,
            surcharge_a,
            interest_a,
            compromise_a,
            _subscriptions: subs,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let gv = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<f64>().unwrap_or(0.0)
        };
        self.draft.txt_pg1m_i1gschd_b = gv(&self.income_source1, cx);
        self.draft.txt_pg1m_i2gschd_b = gv(&self.income_source2, cx);
        self.draft.txt_pg1m_i3gschd_b = gv(&self.income_source3, cx);
        self.draft.txt_pg1m_i5gschd_b = gv(&self.cost_of_sales1, cx);
        self.draft.txt_pg1m_i6gschd_b = gv(&self.cost_of_sales2, cx);
        self.draft.txt_pg1m_i10cschd_b = gv(&self.deduction_10c, cx);
        self.draft.txt_pg1m_i11cschd_b = gv(&self.deduction_11c, cx);
        self.draft.txt_pg1m_i12cschd_b = gv(&self.deduction_12c, cx);
        self.draft.txt_pg1m_i13cschd_b = gv(&self.deduction_13c, cx);
        self.draft.txt_pg1m_i14cschd_b = gv(&self.deduction_14c, cx);
        self.draft.txt_pg1i23a = gv(&self.tax_credits_a, cx);
        self.draft.txt_pg1i25a = gv(&self.surcharge_a, cx);
        self.draft.txt_pg1i26a = gv(&self.interest_a, cx);
        self.draft.txt_pg1i27a = gv(&self.compromise_a, cx);

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

    fn tax_rate_label(&self) -> &str {
        if self.draft.rdo_pg1i21tax_rate_p {
            "8% Flat Rate"
        } else {
            "Graduated Rate (TRAIN)"
        }
    }

    fn deduction_method_label(&self) -> &str {
        if self.draft.rdo_pg1i21amethod_deduction_o {
            "OSD (40%)"
        } else {
            "Itemized"
        }
    }
}

impl FormViewTrait for Form1701View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual Income Tax Return for Individuals"
    }
    fn form_version(&self) -> &'static str {
        "2018 (ENCS)"
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
                "1701",
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
            cx.emit(Form1701Event::Saved);
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Form1701Event::PushNotification(
            "warning".into(),
            "Preview Only".into(),
            "Form 1701 is scaffold-only and cannot be queued for submission yet.".into(),
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

impl Render for Form1701View {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = matches!(self.draft.status, FilingStatus::Draft);
        let is_osd = self.draft.rdo_pg1i21amethod_deduction_o;

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
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(Form1701Event::BackToDashboard)),
                            ),
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
            // Header
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
                            // Profile
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
                                                "Tax Rate: {} | Deductions: {}",
                                                self.tax_rate_label(),
                                                self.deduction_method_label()
                                            )))
                                            .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Income sources (Schedule B)
                            .child(self.render_section(
                                "Schedule B — Income Sources",
                                vec![
                                        self.render_input_row(
                                            "Income Source 1 (Gross Revenue)",
                                            &self.income_source1,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Income Source 2",
                                            &self.income_source2,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Income Source 3",
                                            &self.income_source3,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 4: Total Gross Sales/Revenue",
                                            self.draft.txt_pg1m_i4cschd_b,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Cost of Sales
                            .child(self.render_section(
                                "Cost of Sales / Services",
                                vec![
                                        self.render_input_row(
                                            "Cost of Sales 1",
                                            &self.cost_of_sales1,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Cost of Sales 2",
                                            &self.cost_of_sales2,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 7: Total Cost of Sales",
                                            self.draft.txt_pg1m_i7cschd_b,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 8: Gross Income",
                                            self.draft.txt_pg1m_i8cschd_b,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Deductions
                            .child(self.render_section(
                                &format!("Deductions ({})", self.deduction_method_label()),
                                {
                                    let mut rows: Vec<AnyElement> = Vec::new();
                                    if is_osd {
                                        rows.push(
                                            self.render_computed_row(
                                                "Item 9: OSD (40% of Gross Income)",
                                                self.draft.txt_pg1m_i9cschd_b,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                    } else {
                                        rows.push(
                                            self.render_input_row(
                                                "Item 10: Salaries & Wages",
                                                &self.deduction_10c,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                        rows.push(
                                            self.render_input_row(
                                                "Item 11: Rent",
                                                &self.deduction_11c,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                        rows.push(
                                            self.render_input_row(
                                                "Item 12: Interest",
                                                &self.deduction_12c,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                        rows.push(
                                            self.render_input_row(
                                                "Item 13: Depreciation",
                                                &self.deduction_13c,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                        rows.push(
                                            self.render_input_row(
                                                "Item 14: Other Deductions",
                                                &self.deduction_14c,
                                                cx,
                                            )
                                            .into_any_element(),
                                        );
                                    }
                                    rows.push(
                                        self.render_computed_row(
                                            "Item 15: Total Deductions",
                                            self.draft.txt_pg1m_i15cschd_b,
                                            cx,
                                        )
                                        .into_any_element(),
                                    );
                                    rows.push(
                                        self.render_computed_row(
                                            "Item 16: Net Taxable Income",
                                            self.draft.txt_pg1m_i16cschd_b,
                                            cx,
                                        )
                                        .into_any_element(),
                                    );
                                    rows
                                },
                                cx,
                            ))
                            // Tax computation
                            .child(self.render_section(
                                "Tax Computation (Page 1 Summary)",
                                vec![
                                        self.render_computed_row(
                                            "Item 22A: Tax Due (Taxpayer)",
                                            self.draft.txt_pg1i22atax_due,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 23A: Tax Credits/Payments",
                                            &self.tax_credits_a,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 24A: Tax Payable",
                                            self.draft.txt_pg1i24atax_payable,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Penalties
                            .child(self.render_section(
                                "Add: Penalties",
                                vec![
                                        self.render_input_row(
                                            "Item 25A: Surcharge",
                                            &self.surcharge_a,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 26A: Interest",
                                            &self.interest_a,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Item 27A: Compromise",
                                            &self.compromise_a,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Item 28A: Total Penalties",
                                            self.draft.txt_pg1i28a,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Aggregate
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
                                                    .child("Item 32: Aggregate Amount Payable"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "₱ {:.2}",
                                                        self.draft.txt_pg1i32aggregate_amt_pyble
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
