use crate::components::form_engine::FormViewTrait;
use crate::components::form_parts::{TaxpayerInfoProps, taxpayer_info_section};
use bir_core::forms::FilingStatus;
use bir_core::forms::Form1701QDraft;
use gpui::*;
use gpui_component::*;

pub enum Form1701QEvent {
    BackToDashboard,
}

pub struct Form1701QView {
    pub draft: Form1701QDraft,
    pub show_filing_period: bool,
    pub show_background_info: bool,
    pub show_tax_computation: bool,
}

impl EventEmitter<Form1701QEvent> for Form1701QView {}

impl Form1701QView {
    pub fn new(draft: Form1701QDraft) -> Self {
        Self {
            draft,
            show_filing_period: true,
            show_background_info: true,
            show_tax_computation: true,
        }
    }
}

impl FormViewTrait for Form1701QView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1701Q"
    }

    fn form_subtitle(&self) -> &'static str {
        "Quarterly Income Tax Return for Individuals, Estates and Trusts"
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

    fn save_draft(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn mark_submitted(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn mark_paid(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn revert_to_draft(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn preview_pdf(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn print_confirmation(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}

impl Render for Form1701QView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_block = <Self as FormViewTrait>::render_header(self, cx);
        let status_pipeline = <Self as FormViewTrait>::render_status_pipeline(self, cx);

        let background_info_content = taxpayer_info_section(
            TaxpayerInfoProps {
                tin: &self.draft.tin,
                tin_err: None,
                rdo: &self.draft.rdo_code,
                rdo_err: None,
                name: &self.draft.taxpayer_name,
                name_err: None,
                address: &self.draft.registered_address,
                address_err: None,
                zip: &self.draft.zip_code,
                zip_err: None,
                contact: &self.draft.contact_number,
                contact_err: None,
                email: &self.draft.email,
                email_err: None,
            },
            cx,
        );

        let filing_period_content = div()
            .flex()
            .gap_4()
            .child(crate::components::form_parts::readonly_field(
                "Taxable Year",
                &self.draft.taxable_year,
                None,
                cx,
            ))
            .child(crate::components::form_parts::readonly_field(
                "Quarter",
                &self.draft.quarter,
                None,
                cx,
            ));

        let tax_computation_content = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(crate::components::form_parts::computation_row_readonly(
                "Total Tax Due",
                self.draft.total_tax_due,
                false,
                cx,
            ))
            .child(
                div()
                    .pt_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(crate::components::form_parts::computation_row_readonly(
                        "Total Amount Payable / (Overpayment)",
                        self.draft.total_amount_payable,
                        true,
                        cx,
                    )),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(title_block)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_8()
                    .py_4()
                    .bg(cx.theme().secondary)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(status_pipeline),
            )
            .child(
                div()
                    .id("form_1701q_scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_8()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(900.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_8()
                            .child(crate::components::form_parts::form_accordion(
                                "acc_filing_period",
                                "FILING PERIOD",
                                self.show_filing_period,
                                true,
                                false,
                                cx.listener(|this: &mut Self, _, _, cx| {
                                    this.show_filing_period = !this.show_filing_period;
                                    cx.notify();
                                }),
                                filing_period_content.into_any_element(),
                                cx,
                            ))
                            .child(crate::components::form_parts::form_accordion(
                                "acc_background_info",
                                "PART I — BACKGROUND INFORMATION",
                                self.show_background_info,
                                true,
                                false,
                                cx.listener(|this: &mut Self, _, _, cx| {
                                    this.show_background_info = !this.show_background_info;
                                    cx.notify();
                                }),
                                background_info_content.into_any_element(),
                                cx,
                            ))
                            .child(crate::components::form_parts::form_accordion(
                                "acc_tax_computation",
                                "PART II — COMPUTATION OF TAX",
                                self.show_tax_computation,
                                true,
                                false,
                                cx.listener(|this: &mut Self, _, _, cx| {
                                    this.show_tax_computation = !this.show_tax_computation;
                                    cx.notify();
                                }),
                                tax_computation_content.into_any_element(),
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_8()
                    .py_4()
                    .bg(cx.theme().background)
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui_component::button::Button::new("back_btn")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form1701QEvent::BackToDashboard);
                            })),
                    ),
            )
    }
}
