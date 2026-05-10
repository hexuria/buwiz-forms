//! BIR Form 1601C — Full in-app form view.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_1601c::Form1601CDraft;

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

    // Header Inputs
    is_amended: bool,
    any_taxes_withheld: bool,
    number_of_sheets: Entity<InputState>,
    atc: Entity<InputState>,

    // Part II Inputs
    tax_14_total_compensation: Entity<InputState>,
    tax_15_statutory_minimum_wage: Entity<InputState>,
    tax_16_holiday_pay: Entity<InputState>,
    tax_17_13th_month_pay: Entity<InputState>,
    tax_18_de_minimis: Entity<InputState>,
    tax_19_sss_gsis: Entity<InputState>,
    tax_20_other_name: Entity<InputState>,
    tax_20_other_amount: Entity<InputState>,

    tax_23_not_subject: Entity<InputState>,
    tax_25_total_taxes_withheld: Entity<InputState>,
    tax_26_adjustment: Entity<InputState>,
    tax_28_tax_remitted_previously: Entity<InputState>,
    tax_29_other_remittances_name: Entity<InputState>,
    tax_29_other_remittances_amount: Entity<InputState>,

    tax_32_surcharge: Entity<InputState>,
    tax_33_interest: Entity<InputState>,
    tax_34_compromise: Entity<InputState>,

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

        let create_input = |cx: &mut Context<Self>, val: f64, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
            if val != 0.0 {
                input.update(cx, |i, cx| i.set_value(format!("{:.2}", val), window, cx));
            }
            input
        };

        let create_text_input = |cx: &mut Context<Self>, val: &str, window: &mut Window| {
            let input = cx.new(|cx| InputState::new(window, cx));
            input.update(cx, |i, cx| i.set_value(val.to_string(), window, cx));
            input
        };

        let number_of_sheets = create_text_input(cx, &draft.number_of_sheets.to_string(), window);
        let atc = create_text_input(cx, &draft.atc, window);

        let tax_14_total_compensation = create_input(cx, draft.tax_14_total_compensation, window);
        let tax_15_statutory_minimum_wage =
            create_input(cx, draft.tax_15_statutory_minimum_wage, window);
        let tax_16_holiday_pay = create_input(cx, draft.tax_16_holiday_pay, window);
        let tax_17_13th_month_pay = create_input(cx, draft.tax_17_13th_month_pay, window);
        let tax_18_de_minimis = create_input(cx, draft.tax_18_de_minimis, window);
        let tax_19_sss_gsis = create_input(cx, draft.tax_19_sss_gsis, window);
        let tax_20_other_name = create_text_input(cx, &draft.tax_20_other_name, window);
        let tax_20_other_amount = create_input(cx, draft.tax_20_other_amount, window);

        let tax_23_not_subject = create_input(cx, draft.tax_23_not_subject, window);
        let tax_25_total_taxes_withheld =
            create_input(cx, draft.tax_25_total_taxes_withheld, window);
        let tax_26_adjustment = create_input(cx, draft.tax_26_adjustment, window);
        let tax_28_tax_remitted_previously =
            create_input(cx, draft.tax_28_tax_remitted_previously, window);
        let tax_29_other_remittances_name =
            create_text_input(cx, &draft.tax_29_other_remittances_name, window);
        let tax_29_other_remittances_amount =
            create_input(cx, draft.tax_29_other_remittances_amount, window);

        let tax_32_surcharge = create_input(cx, draft.tax_32_surcharge, window);
        let tax_33_interest = create_input(cx, draft.tax_33_interest, window);
        let tax_34_compromise = create_input(cx, draft.tax_34_compromise, window);

        let inputs = vec![
            number_of_sheets.clone(),
            atc.clone(),
            tax_14_total_compensation.clone(),
            tax_15_statutory_minimum_wage.clone(),
            tax_16_holiday_pay.clone(),
            tax_17_13th_month_pay.clone(),
            tax_18_de_minimis.clone(),
            tax_19_sss_gsis.clone(),
            tax_20_other_name.clone(),
            tax_20_other_amount.clone(),
            tax_23_not_subject.clone(),
            tax_25_total_taxes_withheld.clone(),
            tax_26_adjustment.clone(),
            tax_28_tax_remitted_previously.clone(),
            tax_29_other_remittances_name.clone(),
            tax_29_other_remittances_amount.clone(),
            tax_32_surcharge.clone(),
            tax_33_interest.clone(),
            tax_34_compromise.clone(),
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
            is_amended: draft.is_amended,
            any_taxes_withheld: draft.any_taxes_withheld,
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            validation_errors: Vec::new(),
            status_message: None,

            number_of_sheets,
            atc,

            tax_14_total_compensation,
            tax_15_statutory_minimum_wage,
            tax_16_holiday_pay,
            tax_17_13th_month_pay,
            tax_18_de_minimis,
            tax_19_sss_gsis,
            tax_20_other_name,
            tax_20_other_amount,

            tax_23_not_subject,
            tax_25_total_taxes_withheld,
            tax_26_adjustment,
            tax_28_tax_remitted_previously,
            tax_29_other_remittances_name,
            tax_29_other_remittances_amount,

            tax_32_surcharge,
            tax_33_interest,
            tax_34_compromise,

            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let get_val = |input: &Entity<InputState>, cx: &Context<Self>| {
            input.read(cx).value().parse::<f64>().unwrap_or(0.0)
        };
        let get_text =
            |input: &Entity<InputState>, cx: &Context<Self>| input.read(cx).value().to_string();

        self.draft.is_amended = self.is_amended;
        self.draft.any_taxes_withheld = self.any_taxes_withheld;
        self.draft.number_of_sheets = get_text(&self.number_of_sheets, cx)
            .parse::<u32>()
            .unwrap_or(0);
        self.draft.atc = get_text(&self.atc, cx);

        self.draft.tax_14_total_compensation = get_val(&self.tax_14_total_compensation, cx);
        self.draft.tax_15_statutory_minimum_wage = get_val(&self.tax_15_statutory_minimum_wage, cx);
        self.draft.tax_16_holiday_pay = get_val(&self.tax_16_holiday_pay, cx);
        self.draft.tax_17_13th_month_pay = get_val(&self.tax_17_13th_month_pay, cx);
        self.draft.tax_18_de_minimis = get_val(&self.tax_18_de_minimis, cx);
        self.draft.tax_19_sss_gsis = get_val(&self.tax_19_sss_gsis, cx);
        self.draft.tax_20_other_name = get_text(&self.tax_20_other_name, cx);
        self.draft.tax_20_other_amount = get_val(&self.tax_20_other_amount, cx);

        self.draft.tax_23_not_subject = get_val(&self.tax_23_not_subject, cx);
        self.draft.tax_25_total_taxes_withheld = get_val(&self.tax_25_total_taxes_withheld, cx);
        self.draft.tax_26_adjustment = get_val(&self.tax_26_adjustment, cx);
        self.draft.tax_28_tax_remitted_previously =
            get_val(&self.tax_28_tax_remitted_previously, cx);
        self.draft.tax_29_other_remittances_name =
            get_text(&self.tax_29_other_remittances_name, cx);
        self.draft.tax_29_other_remittances_amount =
            get_val(&self.tax_29_other_remittances_amount, cx);

        self.draft.tax_32_surcharge = get_val(&self.tax_32_surcharge, cx);
        self.draft.tax_33_interest = get_val(&self.tax_33_interest, cx);
        self.draft.tax_34_compromise = get_val(&self.tax_34_compromise, cx);

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
            cx.emit(Form1601CEvent::Saved);
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Queued;
        self.save_draft(window, cx);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Paid;
        self.save_draft(window, cx);
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.status = FilingStatus::Draft;
        self.save_draft(window, cx);
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
                                                    .child("Part I - Background Information"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child("Amended Return?"))
                                                    .child(
                                                        div()
                                                            .id("amended_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(if self.is_amended {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().border
                                                            })
                                                            .bg(if self.is_amended {
                                                                cx.theme().primary.opacity(0.2)
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    if matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        this.is_amended =
                                                                            !this.is_amended;
                                                                        this.sync_from_inputs(cx);
                                                                    }
                                                                },
                                                            ))
                                                            .child(if self.is_amended {
                                                                "Yes"
                                                            } else {
                                                                "No"
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child("Any Taxes Withheld?"))
                                                    .child(
                                                        div()
                                                            .id("withheld_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(
                                                                if self.any_taxes_withheld {
                                                                    cx.theme().primary
                                                                } else {
                                                                    cx.theme().border
                                                                },
                                                            )
                                                            .bg(if self.any_taxes_withheld {
                                                                cx.theme().primary.opacity(0.2)
                                                            } else {
                                                                cx.theme().background
                                                            })
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    if matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        this.any_taxes_withheld =
                                                                            !this
                                                                                .any_taxes_withheld;
                                                                        this.sync_from_inputs(cx);
                                                                    }
                                                                },
                                                            ))
                                                            .child(if self.any_taxes_withheld {
                                                                "Yes"
                                                            } else {
                                                                "No"
                                                            }),
                                                    ),
                                            )
                                            .child(self.render_input_row(
                                                "Number of Sheets Attached",
                                                &self.number_of_sheets,
                                                cx,
                                            ))
                                            .child(self.render_input_row("ATC", &self.atc, cx)),
                                    ),
                            )
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
                                                    .child("Part II - Computation of Tax"),
                                            )
                                            .child(self.render_input_row(
                                                "14 Total Amount of Compensation",
                                                &self.tax_14_total_compensation,
                                                cx,
                                            ))
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .mt_4()
                                                    .child("Less: Non-Taxable/Exempt Compensation"),
                                            )
                                            .child(self.render_input_row(
                                                "15 Statutory Minimum Wage",
                                                &self.tax_15_statutory_minimum_wage,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "16 Holiday Pay, Overtime Pay, Night Shift",
                                                &self.tax_16_holiday_pay,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "17 13th Month Pay and Other Benefits",
                                                &self.tax_17_13th_month_pay,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "18 De Minimis Benefits",
                                                &self.tax_18_de_minimis,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "19 SSS, GSIS, PHIC, HDMF Contributions",
                                                &self.tax_19_sss_gsis,
                                                cx,
                                            ))
                                            .child(self.render_input_with_text_row(
                                                "20 Other Non-Taxable Compensation",
                                                &self.tax_20_other_name,
                                                &self.tax_20_other_amount,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "21 Total Non-Taxable Compensation",
                                                self.draft.tax_21_total_non_taxable,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "22 Total Taxable Compensation (14 - 21)",
                                                self.draft.tax_22_total_taxable,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "23 Less: Taxable comp not subject to withholding",
                                                &self.tax_23_not_subject,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "24 Net Taxable Compensation (22 - 23)",
                                                self.draft.tax_24_net_taxable,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "25 Total Taxes Withheld",
                                                &self.tax_25_total_taxes_withheld,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "26 Add/Less: Adjustment from Previous Months",
                                                &self.tax_26_adjustment,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "27 Taxes Withheld for Remittance",
                                                self.draft.tax_27_taxes_withheld_for_remittance,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "28 Less: Tax Remitted in Return Previously Filed",
                                                &self.tax_28_tax_remitted_previously,
                                                cx,
                                            ))
                                            .child(self.render_input_with_text_row(
                                                "29 Other Remittances Made",
                                                &self.tax_29_other_remittances_name,
                                                &self.tax_29_other_remittances_amount,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "30 Total Tax Remittances Made",
                                                self.draft.tax_30_total_tax_remittances,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "31 Tax Still Due/(Overremittance)",
                                                self.draft.tax_31_tax_still_due,
                                                cx,
                                            )),
                                    ),
                            )
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
                                                "32 Surcharge",
                                                &self.tax_32_surcharge,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "33 Interest",
                                                &self.tax_33_interest,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "34 Compromise",
                                                &self.tax_34_compromise,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "35 Total Penalties",
                                                self.draft.tax_35_total_penalties,
                                                cx,
                                            )),
                                    ),
                            )
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
                                                    .child("36 Total Amount Payable"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "P {:.2}",
                                                        self.draft.tax_36_total_amount_payable
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
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
                    ),
            )
    }
}

impl Form1601CView {
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

    fn render_input_with_text_row(
        &self,
        label: &str,
        text_input: &Entity<InputState>,
        amount_input: &Entity<InputState>,
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
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(label.to_string()),
                    )
                    .child(Input::new(text_input).disabled(is_disabled)),
            )
            .child(
                div()
                    .w_1_2()
                    .child(Input::new(amount_input).disabled(is_disabled)),
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
