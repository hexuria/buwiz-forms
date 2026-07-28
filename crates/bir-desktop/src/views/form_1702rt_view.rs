//! Exact-revision editor for BIR Form 1702-RT, January 2018 (ENCS).
//!
//! Rust owns every calculation and the checked XML contract. This view edits
//! source fields, then reparses and recomputes the semantic draft; malformed
//! whole-peso input is surfaced instead of becoming zero.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::form_1702rt::{
    Form1702RTDeductionMethod, Form1702RTDraft, Form1702RTOverpaymentDisposition,
    QUEUE_SUBMISSION_SUPPORTED,
};
use bir_core::forms::{FilingStatus, FormValidator};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::form_engine::FormViewTrait;

pub enum Form1702RTEvent {
    BackToDashboard,
    Saved,
    Submitted,
    Confirmed,
    PushNotification(String, String, String),
}

impl EventEmitter<Form1702RTEvent> for Form1702RTView {}

const CORE_INPUTS: &[(&str, &str)] = &[
    (
        "frm1702RT:txtPg2Pt4I27Sales",
        "Item 27 — Sales/Receipts/Revenues/Fees",
    ),
    (
        "frm1702RT:txtPg2Pt4I28LessSales",
        "Item 28 — Sales Returns, Allowances and Discounts",
    ),
    (
        "frm1702RT:txtPg2Pt4I30LessCost",
        "Item 30 — Cost of Sales/Services",
    ),
    (
        "frm1702RT:txtPg2Pt4I32AddOtherTaxable",
        "Item 32 — Other Taxable Income",
    ),
    (
        "frm1702RT:Pg2Pt4I40IncomeTaxRate",
        "Item 40 — Applicable Income Tax Rate (%)",
    ),
    ("frm1702RT:txtPg1Pt2I17Surcharge", "Item 17 — Surcharge"),
    ("frm1702RT:txtPg1Pt2I18Interest", "Item 18 — Interest"),
    ("frm1702RT:txtPg1Pt2I19Compromise", "Item 19 — Compromise"),
];

const CREDIT_INPUTS: &[(&str, &str)] = &[
    (
        "frm1702RT:txtPg2Pt4I44ExcessCredits",
        "Item 44 — Prior Year's Excess Credits",
    ),
    (
        "frm1702RT:txtPg2Pt4I45IncomeTaxPaymentUnderMCIT",
        "Item 45 — Previous Quarter MCIT Payments",
    ),
    (
        "frm1702RT:txtPg2Pt4I46IncomeTaxUnderRegular",
        "Item 46 — Previous Quarter Regular Payments",
    ),
    (
        "frm1702RT:txtPg2Pt4I48CreditableTaxWithheldFromPrevious",
        "Item 48 — Previous Quarter Creditable Tax Withheld",
    ),
    (
        "frm1702RT:txtPg2Pt4I49CreditableTaxWithheldFor4thQuarter",
        "Item 49 — Fourth Quarter Creditable Tax Withheld",
    ),
    (
        "frm1702RT:txtPg2Pt4I50ForeignTaxCredits",
        "Item 50 — Foreign Tax Credits",
    ),
    (
        "frm1702RT:txtPg2Pt4I51TaxPaidInReturn",
        "Item 51 — Tax Paid on Previously Filed Return",
    ),
    (
        "frm1702RT:txtPg2Pt452SpecialTaxCredits",
        "Item 52 — Special Tax Credits",
    ),
    (
        "frm1702RT:txtPg2Pt4I53C1",
        "Item 53 — Other Credit Description",
    ),
    ("frm1702RT:txtPg2Pt4I53C2", "Item 53 — Other Credit Amount"),
    (
        "frm1702RT:txtPg2Pt4I54C1",
        "Item 54 — Other Credit Description",
    ),
    ("frm1702RT:txtPg2Pt4I54C2", "Item 54 — Other Credit Amount"),
];

pub struct Form1702RTView {
    draft: Form1702RTDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<&'static str, Entity<InputState>>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    _subscriptions: Vec<Subscription>,
}

impl Form1702RTView {
    pub fn new(
        draft: Form1702RTDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let field_map = draft.to_bir_field_map();
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        for (key, _) in CORE_INPUTS.iter().chain(CREDIT_INPUTS.iter()) {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
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
        match Form1702RTDraft::from_bir_field_map(&fields) {
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
                parsed.recompute();
                self.validation_errors = parsed.validate();
                self.parse_errors.clear();
                self.draft = parsed;
            }
            Err(errors) => self.parse_errors = errors,
        }
        cx.notify();
    }

    fn set_deduction_method(&mut self, method: Form1702RTDeductionMethod, cx: &mut Context<Self>) {
        self.draft.deduction_method = method;
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn set_overpayment_disposition(
        &mut self,
        disposition: Form1702RTOverpaymentDisposition,
        cx: &mut Context<Self>,
    ) {
        self.draft.part_ii.overpayment_disposition = Some(disposition);
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    fn render_input_row(&self, key: &'static str, label: &'static str) -> AnyElement {
        let disabled = !self.draft.is_editable();
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let root = rsx! {
            <div flex items_center justify_between gap_4>
                <div w_1_2 text_sm font_weight={FontWeight::MEDIUM}>{label}</div>
                <div w_1_2>{Input::new(input).disabled(disabled)}</div>
            </div>
        };
        root.into_any_element()
    }

    fn render_computed_row(
        &self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let root = rsx! {
            <div flex items_center justify_between gap_4 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div text_sm font_weight={FontWeight::MEDIUM}>{label.into()}</div>
                <div text_right font_weight={FontWeight::BOLD}>{value.into()}</div>
            </div>
        };
        root.into_any_element()
    }

    fn render_section(
        &self,
        title: &'static str,
        children: Vec<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let root = rsx! {
            <div flex flex_col gap_4 p_5 bg={cx.theme().background} border_1 border_color={cx.theme().border} rounded_lg>
                <div text_xl font_weight={FontWeight::BOLD}>{title}</div>
                {...children}
            </div>
        };
        root.into_any_element()
    }
}

impl FormViewTrait for Form1702RTView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 1702-RT"
    }
    fn form_subtitle(&self) -> &'static str {
        "Annual ITR — Corporation Subject Only to Regular Income Tax Rate"
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
                    .message("Fix malformed 1702-RT fields before saving.".to_string())
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            );
            return;
        }
        if let Ok(db) = self.db.lock() {
            match db.save_form_draft(
                &self.draft.tin,
                "1702RT",
                self.draft.taxable_year,
                None,
                &self.draft.status,
                &self.draft,
            ) {
                Ok(_) => {
                    window.push_notification(
                        gpui_component::notification::Notification::new()
                            .message("1702-RT draft saved.".to_string())
                            .with_type(gpui_component::notification::NotificationType::Success)
                            .autohide(true),
                        cx,
                    );
                    cx.emit(Form1702RTEvent::Saved);
                }
                Err(error) => cx.emit(Form1702RTEvent::PushNotification(
                    "error".into(),
                    "Save failed".into(),
                    error.to_string(),
                )),
            }
        }
    }

    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Form1702RTEvent::PushNotification(
            "warning".into(),
            "Submission unavailable".into(),
            "1702RTv2018C has checked editable-save XML, but no reviewed electronic-submission contract.".into(),
        ));
    }

    fn mark_paid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.transition_to_paid() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => cx.emit(Form1702RTEvent::PushNotification(
                "warning".into(),
                "Cannot mark paid".into(),
                error,
            )),
        }
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.draft.revert_to_draft() {
            Ok(()) => self.save_draft(window, cx),
            Err(error) => cx.emit(Form1702RTEvent::PushNotification(
                "warning".into(),
                "Cannot revert".into(),
                error,
            )),
        }
    }

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Reparse every current editor field before freezing the immutable
        // envelope. Never fall back to the previously parsed draft on error.
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            cx.emit(Form1702RTEvent::PushNotification(
                "error".into(),
                "Experimental HTML Preview Failed".into(),
                format!(
                    "{} current 1702-RT editor value(s) could not be parsed, so no preview was opened. No filing state was changed.",
                    self.parse_errors.len()
                ),
            ));
            return;
        }

        let render_draft = self.draft.clone();
        let envelope = bir_print::html::RenderEnvelopeV1::from(&render_draft);
        match super::form_html_preview_launcher::launch_html_form_preview(&envelope, cx) {
            Ok(launch_kind) => cx.emit(Form1702RTEvent::PushNotification(
                "info".into(),
                "Experimental HTML Preview".into(),
                format!(
                    "{} 1702-RT HTML parity remains experimental. No filing state was changed.",
                    launch_kind.status_message()
                ),
            )),
            Err(error) => cx.emit(Form1702RTEvent::PushNotification(
                "error".into(),
                "Experimental HTML Preview Failed".into(),
                format!(
                    "HTML print preview could not be opened: {error}. No filing state was changed."
                ),
            )),
        }
    }
}

impl Render for Form1702RTView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_draft = self.draft.is_editable();
        let p4 = &self.draft.part_iv;
        let p2 = &self.draft.part_ii;
        let method = self.draft.deduction_method;
        let disposition = p2.overpayment_disposition;

        let core_rows = CORE_INPUTS
            .iter()
            .take(5)
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();
        let penalty_rows = CORE_INPUTS
            .iter()
            .skip(5)
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();
        let credit_rows = CREDIT_INPUTS
            .iter()
            .map(|(key, label)| self.render_input_row(key, label))
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(rsx! {
                <div flex items_center justify_between px_8 py_4 border_b_1 border_color={cx.theme().border}>
                    {gpui_component::button::Button::new("1702rt_back")
                        .label("← Back")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(Form1702RTEvent::BackToDashboard)))}
                    <div flex items_center gap_3>
                        {gpui_component::button::Button::new("1702rt_save")
                            .label("Save Draft")
                            .outline()
                            .disabled(!is_draft)
                            .on_click(cx.listener(|this, _, window, cx| this.save_draft(window, cx)))}
                        {gpui_component::button::Button::new("1702rt_submit")
                            .label(if QUEUE_SUBMISSION_SUPPORTED { "Queue" } else { "XML save only" })
                            .primary()
                            .disabled(true)}
                    </div>
                </div>
            })
            .child(rsx! {
                <div p_6 border_b_1 border_color={cx.theme().border} bg={cx.theme().accent}>
                    {self.render_header(cx)}
                    <div mt_6>{self.render_status_pipeline(cx)}</div>
                </div>
            })
            .child(
                div()
                    .id("1702rt_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p_8()
                    .child(
                        div()
                            .max_w(px(900.))
                            .mx_auto()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(self.render_section(
                                "Part I — Background Information",
                                vec![
                                    div().text_lg().font_weight(FontWeight::BOLD).child(self.draft.taxpayer_name.clone()).into_any_element(),
                                    div().text_sm().child(format!("TIN {} · RDO {} · Year ended {:02}/{}", self.draft.tin, self.draft.rdo_code, self.draft.month, self.draft.taxable_year)).into_any_element(),
                                    div().text_sm().child(format!("{} · ATC {}", method.label(), if self.draft.atc.other_code.is_empty() { "needs review" } else { &self.draft.atc.other_code })).into_any_element(),
                                    div().flex().gap_2()
                                        .child(gpui_component::button::Button::new("1702rt_itemized").label("Itemized").outline().disabled(!is_draft).on_click(cx.listener(|this, _, _, cx| this.set_deduction_method(Form1702RTDeductionMethod::Itemized, cx))))
                                        .child(gpui_component::button::Button::new("1702rt_osd").label("OSD 40%").outline().disabled(!is_draft).on_click(cx.listener(|this, _, _, cx| this.set_deduction_method(Form1702RTDeductionMethod::OptionalStandard, cx))))
                                        .into_any_element(),
                                ],
                                cx,
                            ))
                            .child(self.render_section("Part IV — Computation of Tax", core_rows, cx))
                            .child(self.render_section(
                                "Part IV — Computed Amounts",
                                vec![
                                    self.render_computed_row("Item 29 — Net Sales", p4.item_29_net_sales.to_string(), cx),
                                    self.render_computed_row("Item 31 — Gross Income from Operations", p4.item_31_gross_income_from_operations.to_string(), cx),
                                    self.render_computed_row("Item 33 — Total Taxable Income", p4.item_33_total_taxable_income.to_string(), cx),
                                    self.render_computed_row("Item 37 — Total Itemized Deductions", p4.item_37_total_itemized_deductions.to_string(), cx),
                                    self.render_computed_row("Item 38 — Optional Standard Deduction", p4.item_38_optional_standard_deduction.to_string(), cx),
                                    self.render_computed_row("Item 39 — Net Taxable Income/(Loss)", p4.item_39_net_taxable_income_or_loss.to_string(), cx),
                                    self.render_computed_row("Item 41 — Normal Income Tax Due", p4.item_41_normal_income_tax_due.to_string(), cx),
                                    self.render_computed_row("Item 42 — MCIT Due (2% of Item 33)", p4.item_42_mcit_due.to_string(), cx),
                                    self.render_computed_row("Item 43 — Tax Due", p4.item_43_tax_due.to_string(), cx),
                                ],
                                cx,
                            ))
                            .child(self.render_section("Part IV — Tax Credits/Payments", credit_rows, cx))
                            .child(self.render_section(
                                "Part II — Total Tax Payable",
                                penalty_rows
                                    .into_iter()
                                    .chain([
                                        self.render_computed_row("Item 20 — Total Penalties", p2.item_20_total_penalties.to_string(), cx),
                                        self.render_computed_row("Item 21 — Total Amount Payable/(Overpayment)", p2.item_21_total_amount_payable_or_overpayment.to_string(), cx),
                                    ])
                                    .collect(),
                                cx,
                            ))
                            .child(rsx! {
                                <div flex flex_col gap_2>
                                    <div text_sm font_weight={FontWeight::BOLD}>{"Overpayment disposition (one only)"}</div>
                                    <div flex gap_2>
                                        {gpui_component::button::Button::new("1702rt_refund").label(if matches!(disposition, Some(Form1702RTOverpaymentDisposition::Refund)) { "✓ Refund" } else { "Refund" }).outline().disabled(!is_draft).on_click(cx.listener(|this, _, _, cx| this.set_overpayment_disposition(Form1702RTOverpaymentDisposition::Refund, cx)))}
                                        {gpui_component::button::Button::new("1702rt_tcc").label(if matches!(disposition, Some(Form1702RTOverpaymentDisposition::TaxCreditCertificate)) { "✓ TCC" } else { "TCC" }).outline().disabled(!is_draft).on_click(cx.listener(|this, _, _, cx| this.set_overpayment_disposition(Form1702RTOverpaymentDisposition::TaxCreditCertificate, cx)))}
                                        {gpui_component::button::Button::new("1702rt_carry").label(if matches!(disposition, Some(Form1702RTOverpaymentDisposition::CarryOver)) { "✓ Carry over" } else { "Carry over" }).outline().disabled(!is_draft).on_click(cx.listener(|this, _, _, cx| this.set_overpayment_disposition(Form1702RTOverpaymentDisposition::CarryOver, cx)))}
                                    </div>
                                </div>
                            })
                            .when(!self.parse_errors.is_empty() || !self.validation_errors.is_empty(), |container| {
                                let errors = self.parse_errors.iter().chain(self.validation_errors.iter()).take(12).map(|(field, message)| {
                                    rsx! { <div text_sm text_color={cx.theme().danger}>{format!("{field}: {message}")}</div> }
                                }).collect::<Vec<_>>();
                                container.child(self.render_section("Needs Review", errors.into_iter().map(IntoElement::into_any_element).collect(), cx))
                            })
                            .child(self.render_section(
                                "Official Fixed Schedule Capacity",
                                vec![
                                    div().text_sm().child("Schedule I: Items 1–16, 17a–17c, and six fixed Others rows (17d–17i)").into_any_element(),
                                    div().text_sm().child("Schedule II: four special-deduction rows").into_any_element(),
                                    div().text_sm().child("Schedule IIIA: four NOLCO rows (Items 4–7)").into_any_element(),
                                    div().text_sm().child("Schedule IV: three MCIT rows").into_any_element(),
                                    div().text_sm().child("All schedule values are preserved by the semantic model and exact XML round-trip; expanded row editing follows in this same editor.").into_any_element(),
                                ],
                                cx,
                            )),
                    ),
            )
    }
}
