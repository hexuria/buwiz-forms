//! Exact-revision local editor for BIR Form 1702-MX, January 2018 (ENCS).
//!
//! The view edits checked source fields and reparses the exact 210-key XML
//! contract. It never queues or submits this form: encrypted transport and the
//! separate two-page mandatory attachment have not been reviewed.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_1702mx::{
    Form1702MXDeductionMethod, Form1702MXDraft, Form1702MXOverpaymentDisposition,
    QUEUE_SUBMISSION_SUPPORTED,
};
use bir_core::forms::{FilingStatus, FormValidator};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use crate::components::form_engine::FormViewTrait;

pub enum Form1702MXEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1702MXEvent> for Form1702MXView {}

const PART_IV_INPUTS: &[(&str, &str)] = &[
    (
        "frm1702MX:txtPg2Pt4I34SpecialTaxRate",
        "Reviewed special/preferential rate (%)",
    ),
    (
        "frm1702MX:txtPg2Sc2It14B",
        "Schedule 2 Item 14B — Special rate (%)",
    ),
    (
        "frm1702MX:txtPg2Sc2It14C",
        "Schedule 2 Item 14C — Regular rate (%)",
    ),
    (
        "frm1702MX:txtPg2Sc3It30",
        "Schedule 3 Item 30 — Other credit description",
    ),
    (
        "frm1702MX:txtPg2Sc3It31",
        "Schedule 3 Item 31 — Other credit description",
    ),
    (
        "frm1702MX:txtPg2Pt4I31CA",
        "Schedule 3 Item 31A — Exempt column",
    ),
    (
        "frm1702MX:txtPg2Pt4I31CB",
        "Schedule 3 Item 31B — Special column",
    ),
    (
        "frm1702MX:txtPg2Pt4I31CC",
        "Schedule 3 Item 31C — Regular column",
    ),
];

const PENALTY_INPUTS: &[(&str, &str)] = &[
    ("frm1702MX:txtPg1Pt2I17", "Item 17 — Surcharge"),
    ("frm1702MX:txtPg1Pt2I18", "Item 18 — Interest"),
    ("frm1702MX:txtPg1Pt2I19", "Item 19 — Compromise"),
];

const SCHEDULE_TEXT_INPUTS: &[(&str, &str)] = &[
    (
        "frm1702MX:txtPg3Sc5It17d",
        "Schedule 5 Item 17d — Other deduction",
    ),
    (
        "frm1702MX:txtPg3Sc5It17e",
        "Schedule 5 Item 17e — Other deduction",
    ),
    (
        "frm1702MX:txtPg3Sc5It17f",
        "Schedule 5 Item 17f — Other deduction",
    ),
    (
        "frm1702MX:txtPg3Sc5It17g",
        "Schedule 5 Item 17g — Other deduction",
    ),
    (
        "frm1702MX:txtPg3Sc5It17h",
        "Schedule 5 Item 17h — Other deduction",
    ),
    (
        "frm1702MX:txtPg3Sc5It17i",
        "Schedule 5 Item 17i — Other deduction",
    ),
    (
        "frm1702MX:txtPg6Sc6I1description",
        "Schedule 6 Row 1 — Description",
    ),
    (
        "frm1702MX:txtPg6Sc6I1legal",
        "Schedule 6 Row 1 — Legal basis",
    ),
    (
        "frm1702MX:txtPg6Sc6I2description",
        "Schedule 6 Row 2 — Description",
    ),
    (
        "frm1702MX:txtPg6Sc6I2legal",
        "Schedule 6 Row 2 — Legal basis",
    ),
    (
        "frm1702MX:txtPg6Sc6I3description",
        "Schedule 6 Row 3 — Description",
    ),
    (
        "frm1702MX:txtPg6Sc6I3legal",
        "Schedule 6 Row 3 — Legal basis",
    ),
    (
        "frm1702MX:txtPg3Sc6I4description",
        "Schedule 6 Row 4 — Description",
    ),
    (
        "frm1702MX:txtPg3Sc6I4legal",
        "Schedule 6 Row 4 — Legal basis",
    ),
];

pub struct Form1702MXView {
    draft: Form1702MXDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<&'static str, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    _subscriptions: Vec<Subscription>,
}

impl Form1702MXView {
    pub fn new(
        draft: Form1702MXDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let field_map = draft.to_bir_field_map();
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (key, _) in PART_IV_INPUTS
            .iter()
            .chain(PENALTY_INPUTS.iter())
            .chain(SCHEDULE_TEXT_INPUTS.iter())
        {
            let input = cx.new(|cx| InputState::new(window, cx));
            if let Some(value) = field_map.get(*key) {
                input.update(cx, |state, cx| state.set_value(value.clone(), window, cx));
            }
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
            inputs.insert(*key, input);
        }
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            validation_errors,
            parse_errors: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        let mut fields = self.draft.to_bir_field_map();
        for (key, input) in &self.inputs {
            fields.insert((*key).to_string(), input.read(cx).value().to_string());
        }
        match Form1702MXDraft::from_bir_field_map(&fields) {
            Ok(mut parsed) => {
                parsed.id = self.draft.id;
                parsed.status = self.draft.status.clone();
                parsed.created_at = self.draft.created_at.clone();
                parsed.submitted_at = self.draft.submitted_at.clone();
                parsed.confirmed_at = self.draft.confirmed_at.clone();
                parsed.submission_filename = self.draft.submission_filename.clone();
                parsed.receipt_id = self.draft.receipt_id;
                parsed.submission_attempts = self.draft.submission_attempts;
                parsed.next_retry_at = self.draft.next_retry_at.clone();
                parsed.last_error = self.draft.last_error.clone();
                parsed.schedule_2.items = self.draft.schedule_2.items.clone();
                parsed.schedule_3.items_20_to_33[..11]
                    .clone_from_slice(&self.draft.schedule_3.items_20_to_33[..11]);
                parsed.schedule_4 = self.draft.schedule_4.clone();
                parsed.schedule_5.amounts = self.draft.schedule_5.amounts.clone();
                parsed.schedule_6.item_5_total = self.draft.schedule_6.item_5_total.clone();
                for (target, source) in parsed
                    .schedule_6
                    .rows
                    .iter_mut()
                    .zip(self.draft.schedule_6.rows.iter())
                {
                    target.amounts = source.amounts.clone();
                }
                parsed.regular_nolco = self.draft.regular_nolco.clone();
                parsed.special_nolco = self.draft.special_nolco.clone();
                parsed
                    .schedule_9
                    .rows
                    .iter_mut()
                    .zip(self.draft.schedule_9.rows.iter())
                    .for_each(|(target, source)| {
                        target.normal_income_tax = source.normal_income_tax.clone();
                        target.mcit = source.mcit.clone();
                        target.excess_mcit = source.excess_mcit.clone();
                        target.applied_previous_years = source.applied_previous_years.clone();
                        target.expired = source.expired.clone();
                        target.applied_current_year = source.applied_current_year.clone();
                        target.balance = source.balance.clone();
                    });
                parsed.schedule_10.items = self.draft.schedule_10.items.clone();
                parsed.recompute();
                self.validation_errors = parsed.validate();
                self.parse_errors.clear();
                self.draft = parsed;
            }
            Err(errors) => self.parse_errors = errors,
        }
        cx.notify();
    }

    fn set_deduction_method(&mut self, method: Form1702MXDeductionMethod, cx: &mut Context<Self>) {
        self.draft.deduction_method = method;
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn set_relief_instruction(&mut self, multiple: bool, cx: &mut Context<Self>) {
        self.draft.relief_basis.instruction_single_activity = !multiple;
        self.draft.relief_basis.instruction_multiple_activities = multiple;
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn set_overpayment_disposition(
        &mut self,
        disposition: Form1702MXOverpaymentDisposition,
        cx: &mut Context<Self>,
    ) {
        self.draft.part_ii.overpayment_disposition = Some(disposition);
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn render_input_row(&self, key: &'static str, label: &'static str) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("1702MX editor input registry is complete");
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .w_1_2()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            )
            .child(
                div()
                    .w_1_2()
                    .child(Input::new(input).disabled(!self.draft.is_editable())),
            )
            .into_any_element()
    }

    fn render_computed_row(
        &self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.into()),
            )
            .child(
                div()
                    .text_right()
                    .font_weight(FontWeight::BOLD)
                    .child(value.into()),
            )
            .into_any_element()
    }

    fn render_section(
        &self,
        title: &'static str,
        children: Vec<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(div().text_xl().font_weight(FontWeight::BOLD).child(title))
            .children(children)
            .into_any_element()
    }
}

impl FormViewTrait for Form1702MXView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702-MX"
    }

    fn form_subtitle(&self) -> &'static str {
        "Annual ITR — Mixed Income / Multiple or Special Tax Rates"
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
        if !self.parse_errors.is_empty() {
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message("Fix malformed 1702-MX fields before saving.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            );
            return;
        }
        if let Ok(db) = self.db.lock() {
            match db.save_form_draft(
                &self.draft.tin,
                "1702MX",
                self.draft.taxable_year,
                None,
                &self.draft.status,
                &self.draft,
            ) {
                Ok(_) => {
                    window.push_notification(
                        gpui_component::notification::Notification::new()
                            .message("1702-MX local draft saved.".to_string())
                            .with_type(gpui_component::notification::NotificationType::Success)
                            .autohide(true),
                        cx,
                    );
                    cx.emit(Form1702MXEvent::Saved);
                }
                Err(error) => cx.emit(Form1702MXEvent::PushNotification(
                    "error".into(),
                    "Save failed".into(),
                    error.to_string(),
                )),
            }
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Form1702MXEvent::PushNotification(
            "warning".into(),
            "Submission unavailable".into(),
            "1702MXv2018C supports local editable-save XML only. Encrypted submission and mandatory-attachment transport remain unreviewed.".into(),
        ));
    }

    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.transition_to_paid() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => cx.emit(Form1702MXEvent::PushNotification(
                "warning".into(),
                "Cannot mark paid".into(),
                error,
            )),
        }
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.revert_to_draft() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => cx.emit(Form1702MXEvent::PushNotification(
                "warning".into(),
                "Cannot revert".into(),
                error,
            )),
        }
    }

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Reparse every current editor field before freezing the immutable
        // envelope. Never preview a stale draft after a parse failure.
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            cx.emit(Form1702MXEvent::PushNotification(
                "error".into(),
                "Experimental HTML Preview Failed".into(),
                format!(
                    "{} current 1702-MX editor value(s) could not be parsed, so no preview was opened. No filing state was changed.",
                    self.parse_errors.len()
                ),
            ));
            return;
        }

        let render_draft = self.draft.clone();
        let envelope = bir_print::html::RenderEnvelopeV1::from(&render_draft);
        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => cx.emit(Form1702MXEvent::PushNotification(
                "info".into(),
                "Experimental HTML Preview".into(),
                format!(
                    "{} 1702-MX HTML parity remains experimental and the mandatory attachment remains external. No filing state was changed.",
                    launch_kind.status_message()
                ),
            )),
            Err(error) => cx.emit(Form1702MXEvent::PushNotification(
                "error".into(),
                "Experimental HTML Preview Failed".into(),
                format!(
                    "HTML print preview could not be opened: {error}. No filing state was changed."
                ),
            )),
        }
    }
}

impl Render for Form1702MXView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editable = self.draft.is_editable();
        let method = self.draft.deduction_method;
        let disposition = self.draft.part_ii.overpayment_disposition;
        let part_iv_rows = PART_IV_INPUTS
            .iter()
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();
        let penalty_rows = PENALTY_INPUTS
            .iter()
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();
        let schedule_text_rows = SCHEDULE_TEXT_INPUTS
            .iter()
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();

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
                        gpui_component::button::Button::new("1702mx_back")
                            .label("← Back")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(Form1702MXEvent::BackToDashboard)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                gpui_component::button::Button::new("1702mx_save")
                                    .label("Save Local Draft")
                                    .outline()
                                    .disabled(!editable)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_draft(window, cx)
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("1702mx_submit")
                                    .label(if QUEUE_SUBMISSION_SUPPORTED {
                                        "Queue"
                                    } else {
                                        "XML save only"
                                    })
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
                    .id("1702mx_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(960.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.render_section(
                                "Part I — Background Information",
                                vec![
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .child(self.draft.taxpayer_name.clone())
                                        .into_any_element(),
                                    div()
                                        .text_sm()
                                        .child(format!(
                                            "TIN {} · RDO {} · Year ended {:02}/{}",
                                            self.draft.tin,
                                            self.draft.rdo_code,
                                            self.draft.month,
                                            self.draft.taxable_year
                                        ))
                                        .into_any_element(),
                                    div()
                                        .text_sm()
                                        .child(format!(
                                            "{} · ATC {}",
                                            method.label(),
                                            if self.draft.atc.other_code.is_empty() {
                                                "needs review"
                                            } else {
                                                &self.draft.atc.other_code
                                            }
                                        ))
                                        .into_any_element(),
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            gpui_component::button::Button::new(
                                                "1702mx_itemized",
                                            )
                                            .label(if matches!(
                                                method,
                                                Form1702MXDeductionMethod::Itemized
                                            ) {
                                                "✓ Itemized"
                                            } else {
                                                "Itemized"
                                            })
                                            .outline()
                                            .disabled(!editable)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_deduction_method(
                                                    Form1702MXDeductionMethod::Itemized,
                                                    cx,
                                                )
                                            })),
                                        )
                                        .child(
                                            gpui_component::button::Button::new("1702mx_osd")
                                                .label(if matches!(
                                                    method,
                                                    Form1702MXDeductionMethod::OptionalStandard
                                                ) {
                                                    "✓ OSD 40%"
                                                } else {
                                                    "OSD 40%"
                                                })
                                                .outline()
                                                .disabled(!editable)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_deduction_method(
                                                        Form1702MXDeductionMethod::OptionalStandard,
                                                        cx,
                                                    )
                                                })),
                                        )
                                        .into_any_element(),
                                ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Part IV — Basis and Tax Credits",
                                part_iv_rows,
                                cx,
                            ))
                            .child(self.render_section(
                                "Part IV — Computed Summary",
                                vec![
                                    self.render_computed_row(
                                        "Schedule 2 Item 19D — Total Income Tax Due/(Overpayment)",
                                        self.draft.part_ii.item_14_total_tax_due_or_overpayment.to_string(),
                                        cx,
                                    ),
                                    self.render_computed_row(
                                        "Schedule 3 Item 32D — Total Tax Credits/Payments",
                                        self.draft.part_ii.item_15_total_tax_credits.to_string(),
                                        cx,
                                    ),
                                    self.render_computed_row(
                                        "Schedule 3 Item 33D — Net Tax Payable/(Overpayment)",
                                        self.draft.part_ii.item_16_net_tax_payable_or_overpayment.to_string(),
                                        cx,
                                    ),
                                ],
                                cx,
                            ))
                            .child(self.render_section(
                                "Part II — Penalties",
                                penalty_rows
                                    .into_iter()
                                    .chain([
                                        self.render_computed_row(
                                            "Item 20 — Total Penalties",
                                            self.draft
                                                .part_ii
                                                .item_20_total_penalties
                                                .value_or_zero()
                                                .to_string(),
                                            cx,
                                        ),
                                        self.render_computed_row(
                                            "Item 21 — Total Amount Payable/(Overpayment)",
                                            self.draft
                                                .part_ii
                                                .item_21_total_amount_payable_or_overpayment
                                                .value_or_zero()
                                                .to_string(),
                                            cx,
                                        ),
                                    ])
                                    .collect(),
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child("Overpayment disposition (one only)"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                gpui_component::button::Button::new(
                                                    "1702mx_refund",
                                                )
                                                .label(if matches!(
                                                    disposition,
                                                    Some(Form1702MXOverpaymentDisposition::Refund)
                                                ) {
                                                    "✓ Refund"
                                                } else {
                                                    "Refund"
                                                })
                                                .outline()
                                                .disabled(!editable)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_overpayment_disposition(
                                                        Form1702MXOverpaymentDisposition::Refund,
                                                        cx,
                                                    )
                                                })),
                                            )
                                            .child(
                                                gpui_component::button::Button::new("1702mx_tcc")
                                                    .label(if matches!(
                                                        disposition,
                                                        Some(Form1702MXOverpaymentDisposition::TaxCreditCertificate)
                                                    ) {
                                                        "✓ TCC"
                                                    } else {
                                                        "TCC"
                                                    })
                                                    .outline()
                                                    .disabled(!editable)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.set_overpayment_disposition(
                                                            Form1702MXOverpaymentDisposition::TaxCreditCertificate,
                                                            cx,
                                                        )
                                                    })),
                                            )
                                            .child(
                                                gpui_component::button::Button::new(
                                                    "1702mx_carry",
                                                )
                                                .label(if matches!(
                                                    disposition,
                                                    Some(Form1702MXOverpaymentDisposition::CarryOver)
                                                ) {
                                                    "✓ Carry over"
                                                } else {
                                                    "Carry over"
                                                })
                                                .outline()
                                                .disabled(!editable)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.set_overpayment_disposition(
                                                        Form1702MXOverpaymentDisposition::CarryOver,
                                                        cx,
                                                    )
                                                })),
                                            ),
                                    ),
                            )
                            .child(self.render_section(
                                "Schedules 5 and 6 — Fixed Description Rows",
                                schedule_text_rows,
                                cx,
                            ))
                            .child(self.render_section(
                                "Part IV Instruction and Separate Attachment",
                                vec![
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            gpui_component::button::Button::new(
                                                "1702mx_single_activity",
                                            )
                                            .label(if self
                                                .draft
                                                .relief_basis
                                                .instruction_single_activity
                                            {
                                                "✓ One activity/project"
                                            } else {
                                                "One activity/project"
                                            })
                                            .outline()
                                            .disabled(!editable)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_relief_instruction(false, cx)
                                            })),
                                        )
                                        .child(
                                            gpui_component::button::Button::new(
                                                "1702mx_multiple_activity",
                                            )
                                            .label(if self
                                                .draft
                                                .relief_basis
                                                .instruction_multiple_activities
                                            {
                                                "✓ Two or more activities/projects"
                                            } else {
                                                "Two or more activities/projects"
                                            })
                                            .outline()
                                            .disabled(!editable)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_relief_instruction(true, cx)
                                            })),
                                        )
                                        .into_any_element(),
                                    div()
                                        .text_sm()
                                        .child("The official mandatory attachment is a separate two-page document. Its reviewed raw fields are preserved, but this app does not yet edit, print, attach, or submit it.")
                                        .into_any_element(),
                                ],
                                cx,
                            ))
                            .when(
                                !self.parse_errors.is_empty()
                                    || !self.validation_errors.is_empty(),
                                |container| {
                                    let errors = self
                                        .parse_errors
                                        .iter()
                                        .chain(self.validation_errors.iter())
                                        .take(16)
                                        .map(|(field, message)| {
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().danger)
                                                .child(format!("{field}: {message}"))
                                                .into_any_element()
                                        })
                                        .collect::<Vec<_>>();
                                    container.child(self.render_section("Needs Review", errors, cx))
                                },
                            )
                            .child(self.render_section(
                                "Official Fixed Schedule Capacity",
                                vec![
                                    div().text_sm().child("Schedule 2: 19 rows × four tax-regime columns").into_any_element(),
                                    div().text_sm().child("Schedule 3: Items 20–33 × four columns").into_any_element(),
                                    div().text_sm().child("Schedule 5: six fixed Others rows (17d–17i)").into_any_element(),
                                    div().text_sm().child("Schedule 6: four fixed special-deduction rows").into_any_element(),
                                    div().text_sm().child("Schedules 7.1 and 8.1: four NOLCO rows each; Schedule 9: three MCIT rows").into_any_element(),
                                    div().text_sm().child("The semantic Rust draft preserves every typed schedule value in local JSON even where the reviewed editable XML stores only descriptions or derived inputs.").into_any_element(),
                                ],
                                cx,
                            )),
                    ),
            )
    }
}
