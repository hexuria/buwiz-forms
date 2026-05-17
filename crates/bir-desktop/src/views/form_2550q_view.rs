//! BIR Form 2550Q — Full in-app form view.
//!
//! Quarterly Value-Added Tax Return. Complex form with sales, output VAT,
//! input VAT, deductions, net VAT payable, and penalties sections.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_2550q::Form2550QDraft;

use crate::components::form_engine::FormViewTrait;

pub enum Form2550QV2Event {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form2550QV2Event> for Form2550QV2View {}

pub struct Form2550QV2View {
    draft: Form2550QDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    // Sales inputs
    vatable_sales: Entity<InputState>,
    zero_rated_sales: Entity<InputState>,
    exempt_sales: Entity<InputState>,
    vat_exempt_imports: Entity<InputState>,

    // Output VAT adjustments
    add_output_vat: Entity<InputState>,
    less_output_vat: Entity<InputState>,

    // Purchase inputs
    domestic_purchase: Entity<InputState>,
    domestic_input_tax: Entity<InputState>,
    import_purchase: Entity<InputState>,
    import_input_tax: Entity<InputState>,
    services_purchase: Entity<InputState>,
    service_input_tax: Entity<InputState>,

    // Carry-forward & other input tax
    input_tax_carried: Entity<InputState>,
    transitional_input_tax: Entity<InputState>,
    presumptive_input_tax: Entity<InputState>,

    // Deductions
    adj_deductions: Entity<InputState>,

    // Tax credits
    creditable_vat: Entity<InputState>,
    adv_vat_payment: Entity<InputState>,

    // Penalties
    surcharge: Entity<InputState>,
    interest: Entity<InputState>,
    compromise: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form2550QV2View {
    pub fn new(
        draft: Form2550QDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        let ci = |cx: &mut Context<Self>, val: f64, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let vatable_sales = ci(cx, draft.vatable_sales, window);
        let zero_rated_sales = ci(cx, draft.zero_rated_sales, window);
        let exempt_sales = ci(cx, draft.exempt_sales, window);
        let vat_exempt_imports = ci(cx, draft.vat_exempt_imports, window);
        let add_output_vat = ci(cx, draft.add_output_vat, window);
        let less_output_vat = ci(cx, draft.less_output_vat, window);
        let domestic_purchase = ci(cx, draft.domestic_purchase, window);
        let domestic_input_tax = ci(cx, draft.domestic_input_tax, window);
        let import_purchase = ci(cx, draft.import_purchase, window);
        let import_input_tax = ci(cx, draft.import_input_tax, window);
        let services_purchase = ci(cx, draft.services_purchase, window);
        let service_input_tax = ci(cx, draft.service_input_tax, window);
        let input_tax_carried = ci(cx, draft.input_tax_carried, window);
        let transitional_input_tax = ci(cx, draft.transitional_input_tax, window);
        let presumptive_input_tax = ci(cx, draft.presumptive_input_tax, window);
        let adj_deductions = ci(cx, draft.adj_deductions, window);
        let creditable_vat = ci(cx, draft.creditable_vat, window);
        let adv_vat_payment = ci(cx, draft.adv_vat_payment, window);
        let surcharge = ci(cx, draft.surcharge, window);
        let interest = ci(cx, draft.interest, window);
        let compromise = ci(cx, draft.compromise, window);

        let inputs: Vec<Entity<InputState>> = vec![
            vatable_sales.clone(),
            zero_rated_sales.clone(),
            exempt_sales.clone(),
            vat_exempt_imports.clone(),
            add_output_vat.clone(),
            less_output_vat.clone(),
            domestic_purchase.clone(),
            domestic_input_tax.clone(),
            import_purchase.clone(),
            import_input_tax.clone(),
            services_purchase.clone(),
            service_input_tax.clone(),
            input_tax_carried.clone(),
            transitional_input_tax.clone(),
            presumptive_input_tax.clone(),
            adj_deductions.clone(),
            creditable_vat.clone(),
            adv_vat_payment.clone(),
            surcharge.clone(),
            interest.clone(),
            compromise.clone(),
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
            vatable_sales,
            zero_rated_sales,
            exempt_sales,
            vat_exempt_imports,
            add_output_vat,
            less_output_vat,
            domestic_purchase,
            domestic_input_tax,
            import_purchase,
            import_input_tax,
            services_purchase,
            service_input_tax,
            input_tax_carried,
            transitional_input_tax,
            presumptive_input_tax,
            adj_deductions,
            creditable_vat,
            adv_vat_payment,
            surcharge,
            interest,
            compromise,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let gv = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<f64>().unwrap_or(0.0)
        };
        self.draft.vatable_sales = gv(&self.vatable_sales, cx);
        self.draft.zero_rated_sales = gv(&self.zero_rated_sales, cx);
        self.draft.exempt_sales = gv(&self.exempt_sales, cx);
        self.draft.vat_exempt_imports = gv(&self.vat_exempt_imports, cx);
        self.draft.add_output_vat = gv(&self.add_output_vat, cx);
        self.draft.less_output_vat = gv(&self.less_output_vat, cx);
        self.draft.domestic_purchase = gv(&self.domestic_purchase, cx);
        self.draft.domestic_input_tax = gv(&self.domestic_input_tax, cx);
        self.draft.import_purchase = gv(&self.import_purchase, cx);
        self.draft.import_input_tax = gv(&self.import_input_tax, cx);
        self.draft.services_purchase = gv(&self.services_purchase, cx);
        self.draft.service_input_tax = gv(&self.service_input_tax, cx);
        self.draft.input_tax_carried = gv(&self.input_tax_carried, cx);
        self.draft.transitional_input_tax = gv(&self.transitional_input_tax, cx);
        self.draft.presumptive_input_tax = gv(&self.presumptive_input_tax, cx);
        self.draft.adj_deductions = gv(&self.adj_deductions, cx);
        self.draft.creditable_vat = gv(&self.creditable_vat, cx);
        self.draft.adv_vat_payment = gv(&self.adv_vat_payment, cx);
        self.draft.surcharge = gv(&self.surcharge, cx);
        self.draft.interest = gv(&self.interest, cx);
        self.draft.compromise = gv(&self.compromise, cx);

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
}

impl FormViewTrait for Form2550QV2View {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2550Q"
    }
    fn form_subtitle(&self) -> &'static str {
        "Quarterly Value-Added Tax Return"
    }
    fn form_version(&self) -> &'static str {
        "January 2024 (ENCS)"
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
                "2550Q",
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
            cx.emit(Form2550QV2Event::Saved);
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        use bir_core::forms::FormValidator;
        let errors = self.draft.validate();
        if !errors.is_empty() {
            self.is_validated = true;
            self.validation_errors = errors;
            cx.emit(Form2550QV2Event::PushNotification(
                "warning".into(),
                "Validation Failed".into(),
                "Please fix the errors before submitting.".into(),
            ));
            cx.notify();
            return;
        }

        match self.draft.transition_to_queued() {
            Ok(()) => {
                self.save_draft(window, cx);
                cx.emit(Form2550QV2Event::Submitted);
            }
            Err(errs) => {
                self.is_validated = true;
                self.validation_errors = errs;
                cx.emit(Form2550QV2Event::PushNotification(
                    "warning".into(),
                    "Cannot Submit".into(),
                    "Form validation failed. Check the form for errors.".into(),
                ));
                cx.notify();
            }
        }
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

impl Render for Form2550QV2View {
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
                                cx.emit(Form2550QV2Event::BackToDashboard)
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
                                    .on_click(cx.listener(|this, _, w, cx| this.save_draft(w, cx))),
                            )
                            .child(
                                gpui_component::button::Button::new("submit_btn")
                                    .label("Submit for Filing")
                                    .primary()
                                    .disabled(!is_draft)
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
                                                "Address: {}",
                                                self.draft.registered_address
                                            )))
                                            .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Sales
                            .child(self.render_section(
                                "Part I — Sales / Receipts",
                                vec![
                                        self.render_input_row(
                                            "Vatable Sales",
                                            &self.vatable_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Zero-Rated Sales",
                                            &self.zero_rated_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Exempt Sales",
                                            &self.exempt_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "VAT-Exempt Imports",
                                            &self.vat_exempt_imports,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Sales / Receipts",
                                            self.draft.total_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Output VAT
                            .child(self.render_section(
                                "Part II — Output VAT",
                                vec![
                                        self.render_computed_row(
                                            "Output VAT on Vatable Sales (12%)",
                                            self.draft.output_vat_sales,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Add: Other Output VAT",
                                            &self.add_output_vat,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Less: Allowable Output VAT Deductions",
                                            &self.less_output_vat,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Output Tax Due",
                                            self.draft.output_tax_due,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Input VAT
                            .child(self.render_section(
                                "Part III — Purchases / Input VAT",
                                vec![
                                        self.render_input_row(
                                            "Domestic Purchases (Amount)",
                                            &self.domestic_purchase,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Domestic Input Tax",
                                            &self.domestic_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Importation (Amount)",
                                            &self.import_purchase,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Import Input Tax",
                                            &self.import_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Services (Amount)",
                                            &self.services_purchase,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Service Input Tax",
                                            &self.service_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Current Input Tax",
                                            self.draft.total_cur_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Input Tax Carried Over from Prior Quarter",
                                            &self.input_tax_carried,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Transitional Input Tax",
                                            &self.transitional_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Presumptive Input Tax",
                                            &self.presumptive_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Available Input Tax",
                                            self.draft.total_avail_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Deductions
                            .child(self.render_section(
                                "Part IV — Deductions from Input Tax",
                                vec![
                                        self.render_input_row(
                                            "Deductions / Adjustments",
                                            &self.adj_deductions,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Deductions",
                                            self.draft.total_deductions,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Allowable Input Tax",
                                            self.draft.total_allow_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Net VAT
                            .child(self.render_section(
                                "Part V — Net VAT Payable",
                                vec![
                                        self.render_computed_row(
                                            "Net VAT Payable",
                                            self.draft.net_vat_payable,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Excess Input Tax (carry-forward)",
                                            self.draft.excess_input_tax,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Tax Credits
                            .child(self.render_section(
                                "Part VI — Tax Credits",
                                vec![
                                        self.render_input_row(
                                            "Creditable VAT",
                                            &self.creditable_vat,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_input_row(
                                            "Advance VAT Payment",
                                            &self.adv_vat_payment,
                                            cx,
                                        )
                                        .into_any_element(),
                                        self.render_computed_row(
                                            "Total Tax Credits",
                                            self.draft.total_tax_credits,
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
                                        self.render_input_row("Surcharge", &self.surcharge, cx)
                                            .into_any_element(),
                                        self.render_input_row("Interest", &self.interest, cx)
                                            .into_any_element(),
                                        self.render_input_row("Compromise", &self.compromise, cx)
                                            .into_any_element(),
                                        self.render_computed_row(
                                            "Total Penalties",
                                            self.draft.penalties,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ],
                                cx,
                            ))
                            // Total Payable
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
                                                    .child("Total Amount Payable"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "₱ {:.2}",
                                                        self.draft.total_payable
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}
