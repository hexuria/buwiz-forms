//! Evidence-safe editor for exact form `0619Fv2018`.
//!
//! The editor persists drafts but deliberately does not queue or submit them.
//! Due day and penalties are manual because the reviewed evidence does not
//! prove universal deadline or automatic-penalty rules.
#![allow(dead_code)]

use bir_core::db::Database;
use bir_core::forms::form_0619f::{
    Form0619FDraft, Form0619FPaymentRow, ITEM_13_ATC_CODE, ITEM_14_ATC_CODE, TAX_TYPE_CODE,
    WithholdingAgentCategory,
};
use bir_core::forms::{FilingStatus, FormValidator};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::form_engine::FormViewTrait;

pub enum Form0619FEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form0619FEvent> for Form0619FView {}

#[derive(Clone)]
struct PaymentRowInputs {
    agency: Entity<InputState>,
    number: Entity<InputState>,
    date: Entity<InputState>,
    amount: Entity<InputState>,
}

impl PaymentRowInputs {
    fn all(&self) -> [Entity<InputState>; 4] {
        [
            self.agency.clone(),
            self.number.clone(),
            self.date.clone(),
            self.amount.clone(),
        ]
    }
}

pub struct Form0619FView {
    draft: Form0619FDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    input_errors: Vec<(String, String)>,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    is_amended: bool,
    any_taxes_withheld: bool,
    is_category_government: bool,

    due_day: Entity<InputState>,
    line_of_business: Entity<InputState>,
    registered_address_2: Entity<InputState>,

    item_13_interest_final_tax_withheld: Entity<InputState>,
    item_14_other_final_tax_withheld: Entity<InputState>,
    item_16_remitted_previously: Entity<InputState>,
    item_18a_surcharge: Entity<InputState>,
    item_18b_interest: Entity<InputState>,
    item_18c_compromise: Entity<InputState>,

    tax_agent_number: Entity<InputState>,
    tax_agent_date_issue: Entity<InputState>,
    tax_agent_date_expiry: Entity<InputState>,

    payment_20: PaymentRowInputs,
    payment_21: PaymentRowInputs,
    payment_22: PaymentRowInputs,
    payment_23: PaymentRowInputs,
    payment_23_description: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form0619FView {
    pub fn new(
        draft: Form0619FDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let due_day = text_input(
            cx,
            draft
                .due_day
                .map(|day| day.to_string())
                .as_deref()
                .unwrap_or(""),
            "Manual day (1-31)",
            window,
        );
        let line_of_business = text_input(cx, &draft.line_of_business, "Line of business", window);
        let registered_address_2 = text_input(
            cx,
            &draft.registered_address_2,
            "Address line 2 (optional)",
            window,
        );

        let item_13_interest_final_tax_withheld =
            money_input(cx, draft.item_13_interest_final_tax_withheld, false, window);
        let item_14_other_final_tax_withheld =
            money_input(cx, draft.item_14_other_final_tax_withheld, false, window);
        let item_16_remitted_previously =
            money_input(cx, draft.item_16_remitted_previously, false, window);
        let item_18a_surcharge = money_input(cx, draft.item_18a_surcharge, false, window);
        let item_18b_interest = money_input(cx, draft.item_18b_interest, false, window);
        let item_18c_compromise = money_input(cx, draft.item_18c_compromise, false, window);

        let tax_agent_number = text_input(
            cx,
            &draft.tax_agent_accreditation_number,
            "Accreditation / attorney roll no.",
            window,
        );
        let tax_agent_date_issue =
            text_input(cx, &draft.tax_agent_date_of_issue, "MM/DD/YYYY", window);
        let tax_agent_date_expiry =
            text_input(cx, &draft.tax_agent_date_of_expiry, "MM/DD/YYYY", window);

        let payment_20 = payment_inputs(cx, &draft.payment_details.cash_or_bank_debit_memo, window);
        let payment_21 = payment_inputs(cx, &draft.payment_details.check, window);
        let payment_22 = payment_inputs(cx, &draft.payment_details.tax_debit_memo, window);
        let payment_23 = payment_inputs(cx, &draft.payment_details.others, window);
        let payment_23_description = text_input(
            cx,
            &draft.payment_details.others_description,
            "Describe other payment",
            window,
        );

        let mut all_inputs = vec![
            due_day.clone(),
            line_of_business.clone(),
            registered_address_2.clone(),
            item_13_interest_final_tax_withheld.clone(),
            item_14_other_final_tax_withheld.clone(),
            item_16_remitted_previously.clone(),
            item_18a_surcharge.clone(),
            item_18b_interest.clone(),
            item_18c_compromise.clone(),
            tax_agent_number.clone(),
            tax_agent_date_issue.clone(),
            tax_agent_date_expiry.clone(),
            payment_23_description.clone(),
        ];
        for row in [&payment_20, &payment_21, &payment_22, &payment_23] {
            all_inputs.extend(row.all());
        }

        let mut subscriptions = Vec::new();
        for input in all_inputs {
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if let InputEvent::Change = event {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
        }

        let validation_errors = draft.validate();
        Self {
            is_amended: draft.is_amended,
            any_taxes_withheld: draft.any_taxes_withheld,
            is_category_government: matches!(
                draft.withholding_agent_category,
                WithholdingAgentCategory::Government
            ),
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            input_errors: Vec::new(),
            validation_errors,
            status_message: None,
            due_day,
            line_of_business,
            registered_address_2,
            item_13_interest_final_tax_withheld,
            item_14_other_final_tax_withheld,
            item_16_remitted_previously,
            item_18a_surcharge,
            item_18b_interest,
            item_18c_compromise,
            tax_agent_number,
            tax_agent_date_issue,
            tax_agent_date_expiry,
            payment_20,
            payment_21,
            payment_22,
            payment_23,
            payment_23_description,
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.input_errors.clear();
        self.draft.is_amended = self.is_amended;
        self.draft.any_taxes_withheld = self.any_taxes_withheld;
        self.draft.withholding_agent_category = if self.is_category_government {
            WithholdingAgentCategory::Government
        } else {
            WithholdingAgentCategory::Private
        };

        self.draft.due_day =
            parse_optional_u8(&self.due_day, "due_day", cx, &mut self.input_errors);
        self.draft.line_of_business = input_text(&self.line_of_business, cx);
        self.draft.registered_address_2 = input_text(&self.registered_address_2, cx);

        assign_money(
            &mut self.draft.item_13_interest_final_tax_withheld,
            &self.item_13_interest_final_tax_withheld,
            "item_13_interest_final_tax_withheld",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_14_other_final_tax_withheld,
            &self.item_14_other_final_tax_withheld,
            "item_14_other_final_tax_withheld",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_16_remitted_previously,
            &self.item_16_remitted_previously,
            "item_16_remitted_previously",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_18a_surcharge,
            &self.item_18a_surcharge,
            "item_18a_surcharge",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_18b_interest,
            &self.item_18b_interest,
            "item_18b_interest",
            cx,
            &mut self.input_errors,
        );
        assign_money(
            &mut self.draft.item_18c_compromise,
            &self.item_18c_compromise,
            "item_18c_compromise",
            cx,
            &mut self.input_errors,
        );

        self.draft.tax_agent_accreditation_number = input_text(&self.tax_agent_number, cx);
        self.draft.tax_agent_date_of_issue = input_text(&self.tax_agent_date_issue, cx);
        self.draft.tax_agent_date_of_expiry = input_text(&self.tax_agent_date_expiry, cx);

        sync_payment_row(
            &mut self.draft.payment_details.cash_or_bank_debit_memo,
            &self.payment_20,
            "payment_20_cash_or_bank_debit_memo",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.check,
            &self.payment_21,
            "payment_21_check",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.tax_debit_memo,
            &self.payment_22,
            "payment_22_tax_debit_memo",
            cx,
            &mut self.input_errors,
        );
        sync_payment_row(
            &mut self.draft.payment_details.others,
            &self.payment_23,
            "payment_23_others",
            cx,
            &mut self.input_errors,
        );
        self.draft.payment_details.others_description =
            input_text(&self.payment_23_description, cx);

        self.draft.recompute();
        self.validation_errors = self.input_errors.clone();
        self.validation_errors.extend(self.draft.validate());
        cx.notify();
    }

    fn notify(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        kind: gpui_component::notification::NotificationType,
        message: impl Into<String>,
    ) {
        use gpui_component::WindowExt;
        window.push_notification(
            gpui_component::notification::Notification::new()
                .message(message.into())
                .with_type(kind)
                .autohide(true),
            cx,
        );
    }

    fn render_error_summary(&self, cx: &Context<Self>) -> AnyElement {
        if self.validation_errors.is_empty() {
            return div().into_any_element();
        }
        let mut list = div().mt_2().flex().flex_col().gap_1();
        for (field, message) in &self.validation_errors {
            list = list.child(div().text_xs().child(format!("{field}: {message}")));
        }
        div()
            .p_4()
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().warning.opacity(0.1))
            .rounded_lg()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("Draft needs review"),
            )
            .child(list)
            .into_any_element()
    }

    fn render_toggle(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Self) + 'static,
    ) -> AnyElement {
        let is_draft = self.draft.is_editable();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().text_sm().child(label))
            .child(
                div()
                    .id(id)
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(if selected {
                        cx.theme().primary.opacity(0.15)
                    } else {
                        cx.theme().background
                    })
                    .rounded_md()
                    .when(is_draft, |element| element.cursor_pointer())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.draft.is_editable() {
                            on_click(this);
                            this.sync_from_inputs(cx);
                        }
                    }))
                    .child(if selected { "Yes" } else { "No" }),
            )
            .into_any_element()
    }

    fn render_fixed_row(&self, label: &str, value: &str, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().text_sm().child(label.to_string()))
            .child(
                div()
                    .w_1_2()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted.opacity(0.5))
                    .font_weight(FontWeight::BOLD)
                    .child(value.to_string()),
            )
            .into_any_element()
    }

    fn render_input_row(&self, label: &str, input: &Entity<InputState>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().w_1_2().text_sm().child(label.to_string()))
            .child(
                div()
                    .w_1_2()
                    .child(Input::new(input).disabled(!self.draft.is_editable())),
            )
            .into_any_element()
    }

    fn render_computed_row(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted.opacity(0.5))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("₱ {value:.2}")),
            )
            .into_any_element()
    }

    fn render_payment_row(
        &self,
        item: u8,
        label: &str,
        row: &PaymentRowInputs,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{item} {label}")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.agency).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.number).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.date).disabled(!self.draft.is_editable())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.amount).disabled(!self.draft.is_editable())),
                    ),
            )
            .into_any_element()
    }
}

impl FormViewTrait for Form0619FView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 0619-F"
    }

    fn form_subtitle(&self) -> &'static str {
        "Monthly Remittance Form of Final Income Taxes Withheld"
    }

    fn form_version(&self) -> &'static str {
        "January 2018 (ENCS)"
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
        if !self.input_errors.is_empty() {
            self.status_message = Some(
                "Draft was not saved because one or more numeric/date fields are invalid."
                    .to_string(),
            );
            self.notify(
                window,
                cx,
                gpui_component::notification::NotificationType::Error,
                "Fix the highlighted 0619-F input errors before saving.",
            );
            return;
        }

        let save_result = self
            .db
            .lock()
            .map_err(|_| "Draft database lock is unavailable".to_string())
            .and_then(|db| {
                db.save_form_draft(
                    &self.draft.tin,
                    "0619F",
                    self.draft.taxable_year,
                    Some(self.draft.month),
                    &self.draft.status,
                    &self.draft,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            });

        match save_result {
            Ok(()) => {
                self.status_message = Some(if self.validation_errors.is_empty() {
                    "Draft saved.".to_string()
                } else {
                    "Draft saved locally with unresolved review items; submission remains disabled."
                        .to_string()
                });
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Success,
                    "0619-F draft saved locally.",
                );
                cx.emit(Form0619FEvent::Saved);
            }
            Err(error) => {
                self.status_message = Some(format!("Could not save draft: {error}"));
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    format!("Could not save 0619-F draft: {error}"),
                );
            }
        }
        cx.notify();
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "0619Fv2018 submission is manual/external until the due-day and transport contract are certified."
                .to_string(),
        );
        cx.emit(Form0619FEvent::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Payment status cannot be advanced automatically for a manual/external 0619-F filing."
                .to_string(),
        );
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.revert_to_draft() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => {
                self.status_message = Some(error.clone());
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    error,
                );
            }
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Freeze the latest editor values into one immutable renderer
        // envelope. Preview remains independent from filing lifecycle state.
        self.sync_from_inputs(cx);
        let render_draft = self.draft.clone();
        let envelope = bir_print::html::RenderEnvelopeV1::from(&render_draft);

        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => {
                self.status_message = Some(launch_kind.status_message().to_string());
            }
            Err(error) => {
                let message = format!(
                    "HTML print preview could not be opened: {error}. No filing state was changed."
                );
                self.status_message = Some(message.clone());
                self.notify(
                    window,
                    cx,
                    gpui_component::notification::NotificationType::Error,
                    message,
                );
            }
        }
        cx.notify();
    }
}

impl Render for Form0619FView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (due_month, due_year) = self.draft.due_month_and_year();
        let is_draft = self.draft.is_editable();
        let status_message = self.status_message.clone();
        let xml_evidence_message = self.draft.xml_evidence_warnings().join(" ");

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
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        gpui_component::button::Button::new("0619f_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form0619FEvent::BackToDashboard);
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("0619f_save")
                                    .label("Save Draft")
                                    .outline()
                                    .disabled(!is_draft)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx);
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("0619f_manual")
                                    .label("Manual / External Filing")
                                    .primary()
                                    .disabled(true),
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
                    .id("0619f_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(1000.0))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(
                                div()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(cx.theme().warning)
                                    .bg(cx.theme().warning.opacity(0.1))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Evidence boundary"),
                                    )
                                    .child(
                                        div().mt_1().text_sm().child(
                                            "Due day and all penalties are manual. The app does not queue or submit 0619-F until those rules and the transport channel are certified.",
                                        ),
                                    )
                                    .child(div().mt_2().text_xs().child(xml_evidence_message)),
                            )
                            .when_some(status_message, |element, message| {
                                element.child(
                                    div()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().muted.opacity(0.5))
                                        .child(message),
                                )
                            })
                            .child(self.render_error_summary(cx))
                            .child(
                                section_card(cx, "PART I — BACKGROUND INFORMATION")
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.draft.taxpayer_name.clone()),
                                    )
                                    .child(div().text_sm().child(format!(
                                        "TIN: {} · RDO: {}",
                                        self.draft.tin, self.draft.rdo_code
                                    )))
                                    .child(div().text_sm().child(format!(
                                        "Address: {}",
                                        self.draft.registered_address
                                    )))
                                    .child(div().text_sm().child(format!(
                                        "ZIP: {} · Contact: {} · Email: {}",
                                        self.draft.zip_code,
                                        self.draft.contact_number,
                                        self.draft.email
                                    )))
                                    .child(
                                        div()
                                            .mt_3()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(self.render_toggle(
                                                "0619f_amended",
                                                "3 Amended Form?",
                                                self.is_amended,
                                                cx,
                                                |this| this.is_amended = !this.is_amended,
                                            ))
                                            .child(self.render_toggle(
                                                "0619f_withheld",
                                                "4 Any Taxes Withheld?",
                                                self.any_taxes_withheld,
                                                cx,
                                                |this| {
                                                    this.any_taxes_withheld =
                                                        !this.any_taxes_withheld
                                                },
                                            ))
                                            .child(self.render_toggle(
                                                "0619f_government",
                                                "11 Government withholding agent?",
                                                self.is_category_government,
                                                cx,
                                                |this| {
                                                    this.is_category_government =
                                                        !this.is_category_government
                                                },
                                            ))
                                            .child(self.render_fixed_row(
                                                "1 For the Month of (MM/YYYY)",
                                                &format!(
                                                    "{:02}/{}",
                                                    self.draft.month, self.draft.taxable_year
                                                ),
                                                cx,
                                            ))
                                            .child(self.render_fixed_row(
                                                "5 Tax Type Code",
                                                TAX_TYPE_CODE,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                &format!(
                                                    "2 Due date day (month/year fixed at {due_month:02}/{due_year})"
                                                ),
                                                &self.due_day,
                                            ))
                                            .child(self.render_input_row(
                                                "Line of business (XML semantic value)",
                                                &self.line_of_business,
                                            ))
                                            .child(self.render_input_row(
                                                "9 Registered Address Line 2",
                                                &self.registered_address_2,
                                            )),
                                    ),
                            )
                            .child(
                                section_card(cx, "PART II — TAX REMITTANCE")
                                    .child(self.render_input_row(
                                        &format!(
                                            "13 {ITEM_13_ATC_CODE} — Final tax withheld on interest/deposits/trusts"
                                        ),
                                        &self.item_13_interest_final_tax_withheld,
                                    ))
                                    .child(self.render_input_row(
                                        &format!(
                                            "14 {ITEM_14_ATC_CODE} — Other final income taxes withheld"
                                        ),
                                        &self.item_14_other_final_tax_withheld,
                                    ))
                                    .child(self.render_computed_row(
                                        "15 Total (13 + 14)",
                                        self.draft.item_15_total,
                                        cx,
                                    ))
                                    .child(self.render_input_row(
                                        "16 Less: Amount Remitted from Previously Filed Form",
                                        &self.item_16_remitted_previously,
                                    ))
                                    .child(self.render_computed_row(
                                        "17 Net Amount of Remittance (15 − 16)",
                                        self.draft.item_17_net_amount_of_remittance,
                                        cx,
                                    ))
                                    .child(self.render_input_row(
                                        "18A Surcharge (manual)",
                                        &self.item_18a_surcharge,
                                    ))
                                    .child(self.render_input_row(
                                        "18B Interest (manual)",
                                        &self.item_18b_interest,
                                    ))
                                    .child(self.render_input_row(
                                        "18C Compromise (manual)",
                                        &self.item_18c_compromise,
                                    ))
                                    .child(self.render_computed_row(
                                        "18D Total Penalties (18A + 18B + 18C)",
                                        self.draft.item_18d_total_penalties,
                                        cx,
                                    ))
                                    .child(self.render_computed_row(
                                        "19 Total Amount of Remittance (17 + 18D)",
                                        self.draft.item_19_total_amount_of_remittance,
                                        cx,
                                    )),
                            )
                            .child(
                                section_card(cx, "TAX AGENT / SIGNATURE DETAILS")
                                    .child(self.render_input_row(
                                        "Tax Agent Accreditation / Attorney Roll No.",
                                        &self.tax_agent_number,
                                    ))
                                    .child(self.render_input_row(
                                        "Date of Issue",
                                        &self.tax_agent_date_issue,
                                    ))
                                    .child(self.render_input_row(
                                        "Date of Expiry",
                                        &self.tax_agent_date_expiry,
                                    )),
                            )
                            .child(
                                section_card(cx, "PART III — DETAILS OF PAYMENT")
                                    .child(
                                        div().text_xs().child(
                                            "Columns: Drawee Bank/Agency · Number · Date (MM/DD/YYYY) · Amount",
                                        ),
                                    )
                                    .child(self.render_payment_row(
                                        20,
                                        "Cash/Bank Debit Memo",
                                        &self.payment_20,
                                        cx,
                                    ))
                                    .child(self.render_payment_row(
                                        21,
                                        "Check",
                                        &self.payment_21,
                                        cx,
                                    ))
                                    .child(self.render_payment_row(
                                        22,
                                        "Tax Debit Memo",
                                        &self.payment_22,
                                        cx,
                                    ))
                                    .child(self.render_payment_row(
                                        23,
                                        "Others",
                                        &self.payment_23,
                                        cx,
                                    ))
                                    .child(self.render_input_row(
                                        "23 Others description",
                                        &self.payment_23_description,
                                    )),
                            ),
                    ),
            )
    }
}

fn section_card(cx: &Context<Form0619FView>, title: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded_lg()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child(title.to_string()),
        )
}

fn text_input(
    cx: &mut Context<Form0619FView>,
    value: &str,
    placeholder: &str,
    window: &mut Window,
) -> Entity<InputState> {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.to_string()));
    if !value.is_empty() {
        input.update(cx, |state, cx| {
            state.set_value(value.to_string(), window, cx)
        });
    }
    input
}

fn money_input(
    cx: &mut Context<Form0619FView>,
    value: f64,
    preserve_zero: bool,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "0.00", window);
    if value != 0.0 || preserve_zero {
        input.update(cx, |state, cx| {
            state.set_value(format!("{value:.2}"), window, cx)
        });
    }
    input
}

fn optional_money_input(
    cx: &mut Context<Form0619FView>,
    value: Option<f64>,
    window: &mut Window,
) -> Entity<InputState> {
    let input = text_input(cx, "", "Amount", window);
    if let Some(value) = value {
        input.update(cx, |state, cx| {
            state.set_value(format!("{value:.2}"), window, cx)
        });
    }
    input
}

fn payment_inputs(
    cx: &mut Context<Form0619FView>,
    row: &Form0619FPaymentRow,
    window: &mut Window,
) -> PaymentRowInputs {
    PaymentRowInputs {
        agency: text_input(cx, &row.drawee_bank_or_agency, "Drawee bank/agency", window),
        number: text_input(cx, &row.number, "Number", window),
        date: text_input(cx, &row.date, "MM/DD/YYYY", window),
        amount: optional_money_input(cx, row.amount, window),
    }
}

fn input_text(input: &Entity<InputState>, cx: &Context<Form0619FView>) -> String {
    input.read(cx).value().to_string()
}

fn parse_optional_u8(
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0619FView>,
    errors: &mut Vec<(String, String)>,
) -> Option<u8> {
    let raw = input_text(input, cx);
    match parse_due_day_text(&raw) {
        Ok(value) => value,
        Err(message) => {
            errors.push((field.to_string(), message.to_string()));
            None
        }
    }
}

fn parse_due_day_text(raw: &str) -> Result<Option<u8>, &'static str> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    match raw.trim().parse::<u8>() {
        Ok(value) if (1..=31).contains(&value) => Ok(Some(value)),
        _ => Err("Enter a whole number from 1 to 31"),
    }
}

fn parse_money_text(
    input: &Entity<InputState>,
    field: &str,
    blank_is_zero: bool,
    cx: &Context<Form0619FView>,
    errors: &mut Vec<(String, String)>,
) -> Option<f64> {
    let raw = input_text(input, cx);
    match parse_nonnegative_money_text(&raw, blank_is_zero) {
        Ok(value) => value,
        Err(message) => {
            errors.push((field.to_string(), message));
            None
        }
    }
}

fn parse_nonnegative_money_text(raw: &str, blank_is_zero: bool) -> Result<Option<f64>, String> {
    if raw.trim().is_empty() {
        return Ok(blank_is_zero.then_some(0.0));
    }
    match raw.trim().replace(',', "").parse::<f64>() {
        Ok(value) if value.is_finite() && value >= 0.0 => Ok(Some(value)),
        _ => Err(format!(
            "Enter a finite, non-negative numeric amount; {raw:?} is not accepted"
        )),
    }
}

fn assign_money(
    target: &mut f64,
    input: &Entity<InputState>,
    field: &str,
    cx: &Context<Form0619FView>,
    errors: &mut Vec<(String, String)>,
) {
    if let Some(value) = parse_money_text(input, field, true, cx, errors) {
        *target = value;
    }
}

fn sync_payment_row(
    target: &mut Form0619FPaymentRow,
    inputs: &PaymentRowInputs,
    field: &str,
    cx: &Context<Form0619FView>,
    errors: &mut Vec<(String, String)>,
) {
    target.drawee_bank_or_agency = input_text(&inputs.agency, cx);
    target.number = input_text(&inputs.number, cx);
    target.date = input_text(&inputs.date, cx);
    let raw = input_text(&inputs.amount, cx);
    if raw.trim().is_empty() {
        target.amount = None;
    } else if let Some(value) = parse_money_text(
        &inputs.amount,
        &format!("{field}.amount"),
        false,
        cx,
        errors,
    ) {
        target.amount = Some(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_due_day_text, parse_nonnegative_money_text};

    #[test]
    fn invalid_money_text_returns_an_error_instead_of_zero() {
        let result = parse_nonnegative_money_text("not-a-number", true);
        assert!(result.is_err());
    }

    #[test]
    fn negative_and_non_finite_money_text_are_rejected() {
        assert!(parse_nonnegative_money_text("-1", true).is_err());
        assert!(parse_nonnegative_money_text("NaN", true).is_err());
        assert!(parse_nonnegative_money_text("inf", true).is_err());
    }

    #[test]
    fn blank_money_preserves_required_and_optional_semantics() {
        assert_eq!(parse_nonnegative_money_text("", true), Ok(Some(0.0)));
        assert_eq!(parse_nonnegative_money_text("", false), Ok(None));
    }

    #[test]
    fn due_day_is_manual_and_must_be_in_calendar_day_range() {
        assert_eq!(parse_due_day_text(""), Ok(None));
        assert_eq!(parse_due_day_text("10"), Ok(Some(10)));
        assert!(parse_due_day_text("0").is_err());
        assert!(parse_due_day_text("32").is_err());
    }
}
