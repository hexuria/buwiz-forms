//! BIR Form 0619E — Full in-app form view.
//!
//! Monthly Remittance Form for Creditable Income Taxes Withheld (Expanded).
//! Follows the 1601C view pattern with editable inputs for user-entered fields
//! and auto-computed derived fields via `recompute()`.
#![allow(dead_code)]

use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::form_0619e::Form0619EDraft;

use crate::components::form_engine::FormViewTrait;

pub enum Form0619EEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form0619EEvent> for Form0619EView {}

pub struct Form0619EView {
    draft: Form0619EDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,

    is_validated: bool,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,

    // Header inputs
    is_amended: bool,
    any_taxes_withheld: bool,
    is_category_government: bool,
    atc: Entity<InputState>,

    // Tax computation inputs (user-entered)
    tax_14_total_withheld: Entity<InputState>,
    tax_15_adjustment: Entity<InputState>,

    // Penalty inputs (user-entered)
    tax_17a_surcharge: Entity<InputState>,
    tax_17b_interest: Entity<InputState>,
    tax_17c_compromise: Entity<InputState>,

    _subscriptions: Vec<Subscription>,
}

impl Form0619EView {
    pub fn new(
        draft: Form0619EDraft,
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

        let atc = create_text_input(cx, &draft.txt_atc, window);
        let tax_14_total_withheld = create_input(cx, draft.txt_tax14, window);
        let tax_15_adjustment = create_input(cx, draft.txt_tax15, window);
        let tax_17a_surcharge = create_input(cx, draft.txt_tax17a, window);
        let tax_17b_interest = create_input(cx, draft.txt_tax17b, window);
        let tax_17c_compromise = create_input(cx, draft.txt_tax17c, window);

        let inputs = vec![
            atc.clone(),
            tax_14_total_withheld.clone(),
            tax_15_adjustment.clone(),
            tax_17a_surcharge.clone(),
            tax_17b_interest.clone(),
            tax_17c_compromise.clone(),
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
            any_taxes_withheld: draft.opt_withheld_y,
            is_category_government: draft.opt_category_g,
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            is_validated: false,
            validation_errors: Vec::new(),
            status_message: None,

            atc,
            tax_14_total_withheld,
            tax_15_adjustment,
            tax_17a_surcharge,
            tax_17b_interest,
            tax_17c_compromise,

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
        self.draft.opt_amend_y = self.is_amended;
        self.draft.opt_amend_n = !self.is_amended;
        self.draft.opt_withheld_y = self.any_taxes_withheld;
        self.draft.opt_withheld_n = !self.any_taxes_withheld;
        self.draft.opt_category_g = self.is_category_government;
        self.draft.opt_category_p = !self.is_category_government;
        self.draft.txt_atc = get_text(&self.atc, cx);

        self.draft.txt_tax14 = get_val(&self.tax_14_total_withheld, cx);
        self.draft.txt_tax15 = get_val(&self.tax_15_adjustment, cx);
        self.draft.txt_tax17a = get_val(&self.tax_17a_surcharge, cx);
        self.draft.txt_tax17b = get_val(&self.tax_17b_interest, cx);
        self.draft.txt_tax17c = get_val(&self.tax_17c_compromise, cx);

        self.draft.recompute();

        use bir_core::forms::FormValidator;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }
}

impl FormViewTrait for Form0619EView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 0619E"
    }

    fn form_subtitle(&self) -> &'static str {
        "Monthly Remittance Form for Creditable Income Taxes Withheld (Expanded)"
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
        if let Ok(db) = self.db.lock() {
            let _ = db.save_form_draft(
                &self.draft.tin,
                "0619E",
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
            cx.emit(Form0619EEvent::Saved);
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "Preview only: validation, XML submission, persistence, and print layout are not certified."
                .to_string(),
        );
        cx.emit(Form0619EEvent::PushNotification(
            "warning".to_string(),
            "Preview Only".to_string(),
            "Form 0619E is scaffold-only and cannot be queued for submission yet.".to_string(),
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

impl Render for Form0619EView {
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
                                cx.emit(Form0619EEvent::BackToDashboard);
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
                            // Part I - Background Information
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
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_4()
                                                    .items_center()
                                                    .child(div().child("Category:"))
                                                    .child(
                                                        div()
                                                            .id("cat_gov_btn")
                                                            .p_2()
                                                            .border_1()
                                                            .border_color(
                                                                if self.is_category_government {
                                                                    cx.theme().primary
                                                                } else {
                                                                    cx.theme().border
                                                                },
                                                            )
                                                            .bg(
                                                                if self.is_category_government {
                                                                    cx.theme()
                                                                        .primary
                                                                        .opacity(0.2)
                                                                } else {
                                                                    cx.theme().background
                                                                },
                                                            )
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    if matches!(
                                                                        this.draft.status,
                                                                        FilingStatus::Draft
                                                                    ) {
                                                                        this.is_category_government =
                                                                            !this
                                                                                .is_category_government;
                                                                        this.sync_from_inputs(cx);
                                                                    }
                                                                },
                                                            ))
                                                            .child(
                                                                if self.is_category_government {
                                                                    "Government"
                                                                } else {
                                                                    "Private"
                                                                },
                                                            ),
                                                    ),
                                            )
                                            .child(self.render_input_row("ATC Code", &self.atc, cx)),
                                    ),
                            )
                            // Part II - Tax Computation
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
                                                "14 Total Amount of Taxes Withheld for the Month",
                                                &self.tax_14_total_withheld,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "15 Less: Adjustment for Over-remittance from Prior Month(s)",
                                                &self.tax_15_adjustment,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "16 Tax Still Due (Item 14 − Item 15)",
                                                self.draft.txt_tax16,
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
                                                "17A Surcharge",
                                                &self.tax_17a_surcharge,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "17B Interest",
                                                &self.tax_17b_interest,
                                                cx,
                                            ))
                                            .child(self.render_input_row(
                                                "17C Compromise",
                                                &self.tax_17c_compromise,
                                                cx,
                                            ))
                                            .child(self.render_computed_row(
                                                "17D Total Penalties (17A + 17B + 17C)",
                                                self.draft.txt_tax17d,
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
                                                    .child("18 Total Amount Due (16 + 17D)"),
                                            )
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::BLACK)
                                                    .text_color(cx.theme().primary)
                                                    .child(format!(
                                                        "₱ {:.2}",
                                                        self.draft.txt_tax18
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Form0619EView {
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
