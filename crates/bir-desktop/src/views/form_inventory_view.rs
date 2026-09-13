//! Inventory-driven form page for every rules bundle.
//!
//! Layout comes from `fields.json` + the ELI5 sidecar. Typed 2551Q / 1601C
//! drafts still persist on their dedicated CAS paths. Queue stays behind
//! `can_queue_for_submission`.

use crate::components::form_engine::FormViewTrait;
use crate::components::form_parts::form_accordion;
use crate::components::form_validation::ValidationPaintGate;
use bir_core::db::Database;
use bir_core::forms::form_1601c::Form1601CDraft;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::forms::inventory::{
    FormInventorySpec, GenericFormDraft, InventoryField, bir_field_map_with_inventory,
    filing_period_for_form, has_inventory, load_spec, prefill_from_profile, required_blank_errors,
    truthy, values_from_bir_map,
};
use bir_core::forms::{
    FilingPeriod, FilingStatus, FormValidator, can_queue_for_submission, find_form,
};
use bir_core::profile::TaxpayerProfile;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum FormInventoryEvent {
    BackToDashboard,
    Saved,
    Submitted,
    PushNotification(String, String, String),
}

impl EventEmitter<FormInventoryEvent> for FormInventoryView {}

pub enum InventoryBacking {
    Generic(GenericFormDraft),
    Form2551Q(Form2551QDraft),
    Form1601C(Form1601CDraft),
}

pub struct FormInventoryView {
    spec: FormInventorySpec,
    values: BTreeMap<String, String>,
    backing: InventoryBacking,
    inputs: BTreeMap<String, Entity<InputState>>,
    _subscriptions: Vec<Subscription>,
    expanded: BTreeSet<String>,
    paint_gate: ValidationPaintGate,
    validation_errors: Vec<(String, String)>,
    status_message: Option<String>,
    db: Arc<Mutex<Database>>,
    scroll: ScrollHandle,
    back_id: SharedString,
    save_id: SharedString,
    submit_id: SharedString,
    scroll_id: SharedString,
    page_id: SharedString,
}

impl FormInventoryView {
    pub fn form_code(&self) -> &str {
        &self.spec.form_code
    }

    pub fn can_open(code: &str) -> bool {
        has_inventory(code)
    }

    pub fn new(
        code: &str,
        profile: &TaxpayerProfile,
        year: u16,
        slot: u8,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let spec = load_spec(code).unwrap_or_else(|err| {
            tracing::error!(%err, code, "inventory spec missing");
            load_spec("2551Q").expect("2551Q inventory is packaged")
        });
        let period = filing_period_for_form(code, slot);
        let (backing, mut values) = load_backing(&spec, profile, year, slot, period.clone(), &db)
            .unwrap_or_else(|err| {
                tracing::error!(%err, "inventory draft load failed");
                let draft = GenericFormDraft::new(code, &profile.tin.full(), year, period);
                (InventoryBacking::Generic(draft.clone()), draft.values)
            });
        let prefill = prefill_from_profile(&spec, profile, year, &filing_period_for_form(code, slot));
        for (key, value) in prefill {
            values.entry(key).or_insert(value);
        }

        let (back_id, save_id, submit_id, scroll_id, page_id) = chrome_ids(&spec.form_code);
        let expand_all = spec.field_count < 150;
        let mut expanded = BTreeSet::new();
        for section in spec.editor_sections() {
            if expand_all || matches!(section.id.as_str(), "period" | "identity") {
                expanded.insert(section.id.clone());
            }
        }

        let mut view = Self {
            spec,
            values,
            backing,
            inputs: BTreeMap::new(),
            _subscriptions: Vec::new(),
            expanded,
            paint_gate: ValidationPaintGate::new(),
            validation_errors: Vec::new(),
            status_message: None,
            db,
            scroll: ScrollHandle::new(),
            back_id: back_id.into(),
            save_id: save_id.into(),
            submit_id: submit_id.into(),
            scroll_id: scroll_id.into(),
            page_id: page_id.into(),
        };
        let expanded_ids: Vec<String> = view.expanded.iter().cloned().collect();
        for section_id in expanded_ids {
            view.ensure_inputs_for_section(&section_id, window, cx);
        }
        view
    }

    fn ensure_inputs_for_section(
        &mut self,
        section_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = self
            .spec
            .sections
            .iter()
            .find(|section| section.id == section_id)
        else {
            return;
        };
        let keys = section.fields.clone();
        for key in keys {
            self.ensure_input(&key, window, cx);
        }
    }

    fn ensure_input(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.spec.field(key) else {
            return;
        };
        if !field.is_text_input() || self.inputs.contains_key(&field.field_key) {
            return;
        }
        let value = self
            .values
            .get(&field.field_key)
            .cloned()
            .unwrap_or_default();
        let placeholder = field.display_label();
        let field_key = field.field_key.clone();
        let input_key = field_key.clone();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.paint_gate.touch(field_key.clone());
                    let value = input.read(cx).value().to_string();
                    this.values.insert(field_key.clone(), value);
                    this.apply_backing();
                    this.refresh_visible_errors();
                    cx.notify();
                }
            },
        );
        self._subscriptions.push(subscription);
        self.inputs.insert(input_key, input);
    }

    fn apply_backing(&mut self) {
        match &mut self.backing {
            InventoryBacking::Form2551Q(draft) => {
                draft.apply_inventory_values(&self.values);
                merge_typed_values(&mut self.values, &self.spec, &draft.to_bir_field_map());
            }
            InventoryBacking::Form1601C(draft) => {
                draft.apply_inventory_values(&self.values);
                merge_typed_values(&mut self.values, &self.spec, &draft.to_bir_field_map());
            }
            InventoryBacking::Generic(draft) => {
                draft.values = self.values.clone();
                draft.inventory_editor = true;
            }
        }
    }

    fn set_radio_value(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        self.paint_gate.touch(key);
        if let Some(prefix) = radio_prefix(key)
            && (value == "true" || value == "false")
        {
            let siblings: Vec<String> = self
                .spec
                .fields
                .iter()
                .filter(|field| radio_prefix(&field.field_key) == Some(prefix.clone()))
                .map(|field| field.field_key.clone())
                .collect();
            if siblings.len() > 1 && value == "true" {
                for sibling in siblings {
                    self.values.insert(
                        sibling.clone(),
                        if sibling == key { "true" } else { "false" }.to_string(),
                    );
                }
            } else {
                self.values.insert(key.to_string(), value.to_string());
            }
        } else {
            self.values.insert(key.to_string(), value.to_string());
        }
        self.apply_backing();
        self.refresh_visible_errors();
        cx.notify();
    }

    fn refresh_visible_errors(&mut self) {
        let all = self.blocking_errors();
        self.validation_errors = all
            .into_iter()
            .filter(|(field, _)| self.paint_gate.should_paint(field))
            .collect();
    }

    fn blocking_errors(&self) -> Vec<(String, String)> {
        match &self.backing {
            InventoryBacking::Form2551Q(draft) => draft.validate(),
            InventoryBacking::Form1601C(draft) => draft.validate(),
            InventoryBacking::Generic(_) => required_blank_errors(&self.spec, &self.values),
        }
    }

    fn get_error(&self, key: &str) -> Option<&String> {
        let Some(field) = self.spec.field(key) else {
            return self
                .validation_errors
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, message)| message);
        };
        self.validation_errors
            .iter()
            .find(|(candidate, _)| error_applies_to(candidate, field))
            .map(|(_, message)| message)
    }

    fn section_has_error(&self, section_id: &str) -> bool {
        let Some(section) = self.spec.sections.iter().find(|s| s.id == section_id) else {
            return false;
        };
        section.fields.iter().any(|key| self.get_error(key).is_some())
    }

    fn status(&self) -> FilingStatus {
        match &self.backing {
            InventoryBacking::Form2551Q(d) => draft_status(&d.status),
            InventoryBacking::Form1601C(d) => draft_status(&d.status),
            InventoryBacking::Generic(d) => draft_status(&d.status),
        }
    }

    fn is_editable(&self) -> bool {
        matches!(self.status(), FilingStatus::Draft)
    }

    fn can_queue(&self) -> bool {
        can_queue_for_submission(&self.spec.form_code)
    }

    fn tin_year_period(&self) -> (String, u16, FilingPeriod) {
        match &self.backing {
            InventoryBacking::Form2551Q(d) => (
                d.tin.clone(),
                d.taxable_year,
                FilingPeriod::Quarterly(d.quarter),
            ),
            InventoryBacking::Form1601C(d) => {
                (d.tin.clone(), d.taxable_year, FilingPeriod::Monthly(d.month))
            }
            InventoryBacking::Generic(d) => (d.tin.clone(), d.taxable_year, d.period.clone()),
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_backing();
        let result = match self.db.lock() {
            Ok(db) => match &mut self.backing {
                InventoryBacking::Form2551Q(draft) => db
                    .save_2551q_draft(draft)
                    .map(|_| ())
                    .map_err(|err| err.to_string()),
                InventoryBacking::Form1601C(draft) => db
                    .save_1601c_draft(draft)
                    .map(|_| ())
                    .map_err(|err| err.to_string()),
                InventoryBacking::Generic(draft) => {
                    draft.inventory_editor = true;
                    draft.values = self.values.clone();
                    db.save_form_draft_v2(
                        &draft.tin,
                        &draft.form_code,
                        draft.taxable_year,
                        &draft.period,
                        &draft.status,
                        draft,
                    )
                    .map(|_| ())
                    .map_err(|err| err.to_string())
                }
            },
            Err(_) => Err("The form database is temporarily unavailable".to_string()),
        };
        use gpui_component::WindowExt;
        match result {
            Ok(()) => {
                self.paint_gate.mark_saved();
                self.refresh_visible_errors();
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message("Form saved.".to_string())
                        .with_type(gpui_component::notification::NotificationType::Success)
                        .autohide(true),
                    cx,
                );
                cx.emit(FormInventoryEvent::Saved);
            }
            Err(error) => {
                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .message(format!("Form was not saved: {error}"))
                        .with_type(gpui_component::notification::NotificationType::Error)
                        .autohide(false),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_backing();
        let errors = self.blocking_errors();
        self.paint_gate.mark_saved();
        self.validation_errors = errors.clone();
        if !errors.is_empty() {
            self.status_message = Some("Fix validation errors to continue.".to_string());
            use gpui_component::WindowExt;
            window.push_notification(
                gpui_component::notification::Notification::new()
                    .message(
                        "Please check the form for missing or invalid entries before submitting."
                            .to_string(),
                    )
                    .with_type(gpui_component::notification::NotificationType::Error)
                    .autohide(true),
                cx,
            );
            cx.notify();
            return;
        }
        if !self.can_queue() {
            self.status_message = Some(
                "This form is saved as a draft for manual / external filing. Queue is not enabled."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        match &mut self.backing {
            InventoryBacking::Form2551Q(draft) => {
                let before = draft.clone();
                if let Err(errors) = draft.transition_to_queued() {
                    self.validation_errors = errors;
                    self.status_message = Some("Fix validation errors before submitting".into());
                    cx.notify();
                    return;
                }
                let persist = self.db.lock().map_err(|_| "database_lock".to_string());
                let persist = persist.and_then(|db| {
                    db.save_queued_2551q_draft_and_election_with_post_commit_status(draft)
                        .map(|_| ())
                        .map_err(|err| err.to_string())
                });
                if persist.is_err() {
                    *draft = before;
                    self.status_message =
                        Some("Could not queue form. No submission was started.".into());
                    cx.notify();
                    return;
                }
            }
            InventoryBacking::Form1601C(draft) => {
                let before = draft.clone();
                if let Err(errors) = draft.transition_to_queued() {
                    self.validation_errors = errors;
                    self.status_message = Some("Fix validation errors before submitting".into());
                    cx.notify();
                    return;
                }
                let persist = self.db.lock().map_err(|_| "database_lock".to_string());
                let persist = persist.and_then(|db| {
                    db.save_queued_1601c_draft(draft)
                        .map(|_| ())
                        .map_err(|err| err.to_string())
                });
                if persist.is_err() {
                    *draft = before;
                    self.status_message =
                        Some("Could not queue form. No submission was started.".into());
                    cx.notify();
                    return;
                }
            }
            InventoryBacking::Generic(_) => {
                self.status_message = Some(
                    "This form cannot be queued for live BIR submit.".to_string(),
                );
                cx.notify();
                return;
            }
        }
        self.status_message = None;
        cx.emit(FormInventoryEvent::PushNotification(
            "info".to_string(),
            "Form Queued".to_string(),
            "Your form has been queued for background submission.".to_string(),
        ));
        cx.emit(FormInventoryEvent::Saved);
        cx.notify();
    }

    pub(crate) fn agent_scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub(crate) fn agent_2551q_draft(&self) -> Option<&Form2551QDraft> {
        match &self.backing {
            InventoryBacking::Form2551Q(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn agent_1601c_draft(&self) -> Option<&Form1601CDraft> {
        match &self.backing {
            InventoryBacking::Form1601C(d) => Some(d),
            _ => None,
        }
    }

    pub(crate) fn agent_validated(&self) -> bool {
        self.paint_gate.saved_once() || !self.validation_errors.is_empty()
    }

    pub(crate) fn agent_validation_errors(&self) -> Vec<(String, String)> {
        self.validation_errors.clone()
    }

    pub(crate) fn agent_apply_2551q_patch(
        &mut self,
        patch: crate::views::form_2551q_view::Agent2551QHostPatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(value) = patch.creditable_tax_withheld {
            self.values
                .insert("frm2551Qv2018:txt15".into(), format!("{value:.2}"));
            self.write_input("frm2551Qv2018:txt15", &format!("{value:.2}"), window, cx);
        }
        if let Some(value) = patch.other_tax_credit {
            self.values
                .insert("frm2551Qv2018:txt17".into(), format!("{value:.2}"));
            self.write_input("frm2551Qv2018:txt17", &format!("{value:.2}"), window, cx);
        }
        if let Some(value) = patch.taxable_amount_0 {
            self.values
                .insert("txtATCAmt1".into(), format!("{value:.2}"));
            self.write_input("txtATCAmt1", &format!("{value:.2}"), window, cx);
        }
        self.apply_backing();
        if patch.validate {
            self.paint_gate.mark_saved();
            self.validation_errors = self.blocking_errors();
        }
        if patch.save {
            self.save(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn agent_apply_1601c_patch(
        &mut self,
        patch: crate::views::form_1601c_view::Agent1601CHostPatch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(value) = patch.tax_14 {
            self.values
                .insert("frm1601c:txtTax14".into(), format!("{value:.2}"));
            self.write_input("frm1601c:txtTax14", &format!("{value:.2}"), window, cx);
        }
        if let Some(value) = patch.tax_25 {
            self.values
                .insert("frm1601c:txtTax25".into(), format!("{value:.2}"));
            self.write_input("frm1601c:txtTax25", &format!("{value:.2}"), window, cx);
        }
        if let Some(value) = patch.sheets {
            self.values
                .insert("frm1601c:txtSheets".into(), value.to_string());
            self.write_input("frm1601c:txtSheets", &value.to_string(), window, cx);
        }
        if let Some(value) = patch.any_taxes_withheld {
            self.set_radio_value(
                "frm1601c:TaxWithheld_1",
                if value { "true" } else { "false" },
                cx,
            );
        }
        if let Some(value) = patch.category_of_agent {
            let code = value.as_code();
            self.set_radio_value(
                if code == "G" {
                    "frm1601c:CatAgent_G"
                } else {
                    "frm1601c:CatAgent_P"
                },
                "true",
                cx,
            );
        }
        self.apply_backing();
        if patch.validate {
            self.paint_gate.mark_saved();
            self.validation_errors = self.blocking_errors();
        }
        if patch.save {
            self.save(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn agent_sync_2551q(&mut self, draft: &Form2551QDraft, cx: &mut Context<Self>) {
        if let InventoryBacking::Form2551Q(open) = &mut self.backing {
            *open = draft.clone();
            self.values = values_from_bir_map(&self.spec, &draft.to_bir_field_map());
            cx.notify();
        }
    }

    pub(crate) fn agent_sync_1601c(&mut self, draft: &Form1601CDraft, cx: &mut Context<Self>) {
        if let InventoryBacking::Form1601C(open) = &mut self.backing {
            *open = draft.clone();
            self.values = values_from_bir_map(&self.spec, &draft.to_bir_field_map());
            cx.notify();
        }
    }

    pub(crate) fn agent_preview_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_pdf(window, cx);
    }

    fn write_input(&mut self, key: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.inputs.get(key) {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx);
            });
        }
    }

    fn field_value(&self, field: &InventoryField) -> String {
        self.values
            .get(&field.field_key)
            .cloned()
            .unwrap_or_default()
    }

    fn render_field(&self, field: &InventoryField, cx: &Context<Self>) -> AnyElement {
        let error = self.get_error(&field.field_key).cloned();
        let value = self.field_value(field);
        let editable = self.is_editable() && !field.is_computed();
        let border = if error.is_some() {
            cx.theme().danger
        } else {
            cx.theme().border
        };
        let mut block = rsx! {
            <div
                id={format!("inv-field-{}", field.field_key)}
                flex
                flex_col
                gap_1
                w_full
            >
                <div
                    text_xs
                    font_weight={FontWeight::BOLD}
                    text_color={cx.theme().muted_foreground}
                >
                    {field.display_label()}
                </div>
            </div>
        };
        if field.is_computed() {
            block = block.child(rsx! {
                <div text_sm font_weight={FontWeight::BOLD} text_color={cx.theme().foreground}>
                    {if value.is_empty() {
                        "—".to_string()
                    } else {
                        value
                    }}
                </div>
            });
        } else if field.is_choice_control() {
            let options = if field.enum_values.is_empty() {
                vec!["true".to_string(), "false".to_string()]
            } else {
                field.enum_values.clone()
            };
            let mut row = div().flex().flex_wrap().gap_2();
            for option in options {
                let selected = value.eq_ignore_ascii_case(&option)
                    || (option == "true" && truthy(&value))
                    || (option == "false" && !value.is_empty() && !truthy(&value) && value == "false");
                let key = field.field_key.clone();
                let option_clone = option.clone();
                let mut chip = div()
                    .id(format!("inv-opt-{}-{option}", field.field_key))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected {
                        cx.theme().primary
                    } else {
                        border
                    })
                    .bg(if selected {
                        cx.theme().primary.opacity(0.2)
                    } else {
                        gpui::transparent_black()
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().foreground)
                            .child(option.clone()),
                    );
                if editable {
                    chip = chip.cursor_pointer().on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.set_radio_value(&key, &option_clone, cx);
                        },
                    ));
                }
                row = row.child(chip);
            }
            block = block.child(row);
        } else if let Some(input) = self.inputs.get(&field.field_key) {
            block = block.child(
                Input::new(input)
                    .disabled(!editable)
                    .appearance(false),
            );
        } else {
            block = block.child(rsx! {
                <div text_sm text_color={cx.theme().foreground}>{value}</div>
            });
        }
        if let Some(error) = error {
            block = block.child(rsx! {
                <div text_xs text_color={cx.theme().danger}>{error}</div>
            });
        }
        block.into_any_element()
    }
}

fn draft_status(status: &FilingStatus) -> FilingStatus {
    status.clone()
}

fn merge_typed_values(
    values: &mut BTreeMap<String, String>,
    spec: &FormInventorySpec,
    typed: &BTreeMap<String, String>,
) {
    for (key, value) in values_from_bir_map(spec, typed) {
        values.insert(key, value);
    }
}

fn error_applies_to(error_key: &str, field: &InventoryField) -> bool {
    if error_key == field.field_key || error_key == field.xml_key() {
        return true;
    }
    let short = field
        .field_key
        .rsplit(':')
        .next()
        .unwrap_or(field.field_key.as_str());
    if error_key.eq_ignore_ascii_case(short) {
        return true;
    }
    typed_error_aliases(error_key)
        .iter()
        .any(|alias| short.eq_ignore_ascii_case(alias) || field.field_key.contains(alias))
}

fn typed_error_aliases(error_key: &str) -> &'static [&'static str] {
    match error_key {
        "tin" => &["txtTIN1", "txtTIN2", "txtTIN3", "txtBranchCode"],
        "taxable_year" => &["txtYear"],
        "quarter" => &["qtr_"],
        "year_end_month" | "month" => &["rtnMonth", "txtMonth"],
        "taxpayer_name" => &["registeredName", "txtTaxpayerName"],
        "rdo_code" => &["txtRDOCode", "RDOCode"],
        "registered_address" => &["registeredAddress", "txtAddress"],
        "zip_code" => &["zipCode", "txtZipCode"],
        "contact_number" => &["telNo", "txtTel"],
        "email" => &["txtEmail"],
        "number_of_attached_sheets" => &["txtSheets"],
        "creditable_tax_withheld" => &["txt15"],
        "other_tax_credit" => &["txt17"],
        "item_13_election" => &["taxRate1", "taxRate2"],
        _ => &[],
    }
}

fn radio_prefix(key: &str) -> Option<String> {
    let short = key.rsplit(':').next().unwrap_or(key);
    let (stem, rest) = short.rsplit_once('_')?;
    if rest.chars().all(|c| c.is_ascii_digit()) {
        Some(format!(
            "{}:{}",
            key.rsplit_once(':').map(|(p, _)| p).unwrap_or(""),
            stem
        ))
    } else {
        None
    }
}

fn chrome_ids(code: &str) -> (String, String, String, String, String) {
    // The app wraps the form in `page_root(active_view)`. Keep this inner
    // element distinct so agent snapshots do not see duplicate page ids.
    let page = format!("form-inventory-{}", code.to_ascii_lowercase());
    let scroll = if code.eq_ignore_ascii_case("2551Q") {
        "form-2551q-scroll-area".to_string()
    } else if code.eq_ignore_ascii_case("1601C") {
        "form-1601c-scroll".to_string()
    } else {
        format!("form-{}-scroll", code.to_ascii_lowercase())
    };
    if let Some(chrome) = crate::agent::ids::FORM_CHROME
        .iter()
        .find(|chrome| chrome.code.eq_ignore_ascii_case(code))
    {
        return (
            chrome.back.to_string(),
            chrome.save.to_string(),
            chrome.submit.to_string(),
            scroll,
            page,
        );
    }
    (
        "back_btn".into(),
        "save_btn".into(),
        "submit_btn".into(),
        scroll,
        page,
    )
}

fn load_backing(
    spec: &FormInventorySpec,
    profile: &TaxpayerProfile,
    year: u16,
    slot: u8,
    period: FilingPeriod,
    db: &Arc<Mutex<Database>>,
) -> Result<(InventoryBacking, BTreeMap<String, String>), String> {
    let tin = profile.tin.full();
    let guard = db.lock().ok();
    match spec.form_code.as_str() {
        "2551Q" => {
            let quarter = slot.clamp(1, 4);
            let mut draft = if let Some(db) = guard.as_ref() {
                db.get_2551q_draft(&tin, year, quarter)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        let fresh =
                            Form2551QDraft::new_from_effective_profile(profile, year, quarter);
                        if quarter > 1 {
                            if let Some(prev) = db.get_2551q_draft(&tin, year, quarter - 1).ok().flatten()
                            {
                                return fresh.with_carried_forward(&prev);
                            }
                        }
                        fresh
                    })
            } else {
                Form2551QDraft::new_from_effective_profile(profile, year, quarter)
            };
            if matches!(draft.status, FilingStatus::Draft) {
                let _ = draft.reconcile_with_effective_profile(profile);
            }
            let values = values_from_bir_map(spec, &draft.to_bir_field_map());
            Ok((InventoryBacking::Form2551Q(draft), values))
        }
        "1601C" => {
            let month = slot.clamp(1, 12);
            let draft = if let Some(db) = guard.as_ref() {
                db.get_1601c_draft(&tin, year, month)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| Form1601CDraft::new_from_profile(profile, year, month))
            } else {
                Form1601CDraft::new_from_profile(profile, year, month)
            };
            let values = values_from_bir_map(spec, &draft.to_bir_field_map());
            Ok((InventoryBacking::Form1601C(draft), values))
        }
        other => {
            if let Some(db) = guard.as_ref()
                && let Ok(Some(existing)) =
                    db.get_form_draft_v2::<GenericFormDraft>(&tin, other, year, &period)
                && existing.inventory_editor
            {
                return Ok((InventoryBacking::Generic(existing.clone()), existing.values));
            }
            let mut draft = GenericFormDraft::new(other, &tin, year, period);
            if let Some(typed) = seed_typed_map(other, profile, year, slot, guard.as_ref()) {
                draft.values = values_from_bir_map(spec, &typed);
            }
            Ok((InventoryBacking::Generic(draft.clone()), draft.values))
        }
    }
}

fn seed_typed_map(
    code: &str,
    profile: &TaxpayerProfile,
    year: u16,
    slot: u8,
    db: Option<&std::sync::MutexGuard<'_, Database>>,
) -> Option<BTreeMap<String, String>> {
    let tin = profile.tin.full();
    match code {
        "1701Q" => {
            let quarter = slot.clamp(1, 4);
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_1701q::Form1701QDraft>(
                        &tin,
                        "1701Q",
                        year,
                        Some(quarter),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_1701q::Form1701QDraft::new_from_profile(
                        profile, year, quarter,
                    )
                });
            Some(draft.to_bir_field_map())
        }
        "0619E" => {
            let month = slot.clamp(1, 12);
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_0619e::Form0619EDraft>(
                        &tin,
                        "0619E",
                        year,
                        Some(month),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_0619e::Form0619EDraft::new_from_profile(
                        profile, year, month,
                    )
                });
            Some(draft.to_bir_field_map())
        }
        "0619F" => {
            let month = slot.clamp(1, 12);
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_0619f::Form0619FDraft>(
                        &tin,
                        "0619F",
                        year,
                        Some(month),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_0619f::Form0619FDraft::new_from_profile(
                        profile, year, month,
                    )
                });
            Some(draft.to_bir_field_map())
        }
        "0605" => {
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_0605::Form0605Draft>(
                        &tin, "0605", year, Some(slot),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_0605::Form0605Draft::new_from_profile(profile, year, slot)
                });
            Some(draft.to_bir_field_map())
        }
        "2550Q" => {
            let quarter = slot.clamp(1, 4);
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_2550q::Form2550QDraft>(
                        &tin,
                        "2550Q",
                        year,
                        Some(quarter),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_2550q::Form2550QDraft::new_from_profile(
                        profile, year, quarter,
                    )
                });
            Some(draft.to_bir_field_map())
        }
        "1701" => {
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_1701::Form1701Draft>(
                        &tin, "1701", year, Some(slot),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_1701::Form1701Draft::new_from_profile(profile, year, slot)
                });
            Some(draft.to_bir_field_map())
        }
        "1702RT" => {
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_1702rt::Form1702RTDraft>(
                        &tin, "1702RT", year, Some(slot),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_1702rt::Form1702RTDraft::new_from_profile(
                        profile, year, slot,
                    )
                });
            Some(draft.to_bir_field_map())
        }
        "1702MX" => {
            let draft = db
                .and_then(|db| {
                    db.get_form_draft::<bir_core::forms::form_1702mx::Form1702MXDraft>(
                        &tin, "1702MX", year, Some(slot),
                    )
                    .ok()
                    .flatten()
                })
                .unwrap_or_else(|| {
                    bir_core::forms::form_1702mx::Form1702MXDraft::new_from_profile(profile, year)
                });
            Some(draft.to_bir_field_map())
        }
        _ => None,
    }
}

impl FormViewTrait for FormInventoryView {
    fn form_title(&self) -> &'static str {
        // Titles are static registry strings; fall back to a stable label.
        find_form(&self.spec.form_code)
            .map(|def| def.title)
            .unwrap_or("BIR form")
    }

    fn form_subtitle(&self) -> &'static str {
        "Inventory editor"
    }

    fn form_version(&self) -> &'static str {
        find_form(&self.spec.form_code)
            .map(|_| "eBIRForms inventory")
            .unwrap_or("eBIRForms inventory")
    }

    fn current_status(&self) -> FilingStatus {
        self.status()
    }

    fn submitted_at(&self) -> Option<&str> {
        match &self.backing {
            InventoryBacking::Form2551Q(d) => d.submitted_at.as_deref(),
            InventoryBacking::Form1601C(d) => d.submitted_at.as_deref(),
            InventoryBacking::Generic(_) => None,
        }
    }

    fn confirmed_at(&self) -> Option<&str> {
        match &self.backing {
            InventoryBacking::Form2551Q(d) => d.confirmed_at.as_deref(),
            InventoryBacking::Form1601C(d) => d.confirmed_at.as_deref(),
            InventoryBacking::Generic(_) => None,
        }
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.submit(window, cx);
    }

    fn mark_paid(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn revert_to_draft(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let slug = self.spec.frozen_bundle.clone();
        let title = format!("{} — Print Preview", self.spec.form_code);
        let typed = match &self.backing {
            InventoryBacking::Form2551Q(d) => d.to_bir_field_map(),
            InventoryBacking::Form1601C(d) => d.to_bir_field_map(),
            InventoryBacking::Generic(_) => BTreeMap::new(),
        };
        let fields = bir_field_map_with_inventory(&self.spec, &self.values, &typed);
        match crate::views::form_html_preview_launcher::launch_frozen_form_preview(
            &slug, &fields, &title, cx,
        ) {
            Ok(_) => {
                self.status_message = Some("Print preview opened.".into());
            }
            Err(error) => {
                self.status_message = Some(format!("HTML print preview could not be opened: {error}"));
            }
        }
        cx.notify();
    }
}

impl Render for FormInventoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pipeline = <Self as FormViewTrait>::render_status_pipeline(self, cx);
        let header = <Self as FormViewTrait>::render_header(self, cx);
        let can_queue = self.can_queue();
        let editable = self.is_editable();
        let code = self.spec.form_code.clone();
        let title = find_form(&self.spec.form_code)
            .map(|def| def.title.to_string())
            .unwrap_or_else(|| format!("BIR Form {code}"));

        let mut sections = div().flex().flex_col().gap_4().p_8();
        let section_list: Vec<_> = self.spec.editor_sections().cloned().collect();
        for section in section_list {
            let expanded = self.expanded.contains(&section.id);
            let has_error = self.section_has_error(&section.id);
            let section_id = section.id.clone();
            let mut body = div().flex().flex_col().gap_4();
            if expanded {
                for key in &section.fields {
                    if let Some(field) = self.spec.field(key) {
                        body = body.child(self.render_field(field, cx));
                    }
                }
            }
            sections = sections.child(form_accordion(
                &format!("inv-sec-{}", section.id),
                &section.title,
                expanded,
                !has_error,
                has_error,
                cx.listener(move |this, _, window, cx| {
                    if this.expanded.contains(&section_id) {
                        this.expanded.remove(&section_id);
                    } else {
                        this.expanded.insert(section_id.clone());
                        this.ensure_inputs_for_section(&section_id, window, cx);
                    }
                    cx.notify();
                }),
                body.into_any_element(),
                cx,
            ));
        }

        let submit_label = if can_queue {
            "Submit"
        } else {
            "Manual / external filing"
        };

        rsx! {
            <div
                id={self.page_id.clone()}
                size_full
                flex
                flex_col
                min_h_0
                bg={cx.theme().background}
            >
                <div
                    flex
                    items_center
                    justify_between
                    px_8
                    py_4
                    bg={cx.theme().background}
                    border_b_1
                    border_color={cx.theme().border}
                >
                    {gpui_component::button::Button::new(self.back_id.clone())
                        .label("← Back")
                        .on_click(cx.listener(|_this, _, _, cx| {
                            cx.emit(FormInventoryEvent::BackToDashboard);
                        }))}
                    <div flex items_center gap_3>
                        {if editable {
                            gpui_component::button::Button::new(self.save_id.clone())
                                .label("Save")
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save(window, cx);
                                }))
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }}
                        {if editable {
                            gpui_component::button::Button::new(self.submit_id.clone())
                                .label(submit_label)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit(window, cx);
                                }))
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }}
                    </div>
                </div>
                <div
                    flex
                    items_center
                    justify_center
                    px_8
                    py_4
                    bg={cx.theme().background}
                    border_b_1
                    border_color={cx.theme().border}
                >
                    {pipeline}
                </div>
                <div px_8 pt_4 bg={cx.theme().background}>
                    {header}
                    <div text_sm text_color={cx.theme().muted_foreground} mt_2>
                        {title}
                    </div>
                    {if let Some(message) = &self.status_message {
                        rsx! {
                            <div text_sm text_color={cx.theme().warning} mt_2>
                                {message.clone()}
                            </div>
                        }
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    }}
                    {if !self.validation_errors.is_empty() {
                        let list = self
                            .validation_errors
                            .iter()
                            .take(8)
                            .map(|(_, message)| message.clone())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        rsx! {
                            <div
                                id="form-inventory-validation"
                                text_sm
                                text_color={cx.theme().danger}
                                mt_2
                            >
                                {list}
                            </div>
                        }
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    }}
                </div>
                <div
                    id={self.scroll_id.clone()}
                    flex_1
                    min_h_0
                    overflow_y_scroll
                    track_scroll={&self.scroll}
                    bg={cx.theme().background}
                >
                    {sections}
                </div>
            </div>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::radio_prefix;
    use bir_core::forms::inventory::load_spec;

    #[test]
    fn editor_omits_print_xml_section() {
        let spec = load_spec("2551Q").expect("2551Q");
        assert!(spec.editor_sections().all(|s| s.id != "print_xml"));
        assert!(
            spec.print_xml_keys()
                .iter()
                .any(|k| k.contains("txtPg2TIN1"))
        );
    }

    #[test]
    fn radio_prefix_groups_true_false_pairs() {
        assert_eq!(
            radio_prefix("frm2551Qv2018:forThe_1").as_deref(),
            Some("frm2551Qv2018:forThe")
        );
        assert_eq!(
            radio_prefix("frm2551Qv2018:forThe_2").as_deref(),
            Some("frm2551Qv2018:forThe")
        );
    }

    #[test]
    fn typed_validation_keys_match_inventory_fields() {
        let spec = load_spec("2551Q").expect("2551Q");
        let tin = spec.field("frm2551Qv2018:txtTIN1").expect("tin1");
        assert!(super::error_applies_to("tin", tin));
        let year = spec.field("frm2551Qv2018:txtYear").expect("year");
        assert!(super::error_applies_to("taxable_year", year));
        assert!(!super::error_applies_to("tin", year));
    }
}
