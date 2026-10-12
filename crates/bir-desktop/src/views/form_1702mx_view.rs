//! Editor for BIR Form 1702-MX (January 2018), Annual Income Tax Return for
//! corporations with mixed income subject to multiple tax rates or special
//! rates. Rust owns every calculation, validation and the official submit
//! plaintext (`bir_core::forms::form_1702mx_official`); this view only edits
//! source values. The layout reflows for desktop, tablet and phone widths.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_1702mx::{
    Form1702MXDeductionMethod, Form1702MXDraft, Form1702MXFilingBasis,
    Form1702MXOverpaymentDisposition, Form1702MXRegimeAmounts, PercentInput, WholePeso,
    WholePesoInput,
};
use bir_core::forms::form_1702mx_official::{FORM_1702MX_ATC_OPTIONS, official_display};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingPeriod, FilingStatus, can_queue_for_submission};
use bir_core::profile::TaxpayerProfile;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;
use gpui_rsx::rsx;

use super::event_form_kit as kit;
use crate::components::combobox::ComboboxState;
use crate::components::form_engine::FormViewTrait;
use crate::views::form_controls::{
    self, MultiChange, field_error, messages_for, multi_select_field, select_field,
    with_field_error,
};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};

impl EventEmitter<QueueableFormEvent> for Form1702MXView {}

/// Width classes the page lays out for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    Phone,
    Tablet,
    Desktop,
}

impl Layout {
    fn for_width(width: Pixels) -> Self {
        if width < px(700.) {
            Self::Phone
        } else if width < px(1100.) {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }
}

/// Month the year ends: (stored month number, shown name).
const MONTH_OPTIONS: [(&str, &str); 12] = [
    ("1", "January"),
    ("2", "February"),
    ("3", "March"),
    ("4", "April"),
    ("5", "May"),
    ("6", "June"),
    ("7", "July"),
    ("8", "August"),
    ("9", "September"),
    ("10", "October"),
    ("11", "November"),
    ("12", "December"),
];

/// Value of the Item 5 "IC 055 — MCIT" slot in the ATC multi-select.
const MCIT_VALUE: &str = "IC055";
/// The official form has two Item 5 ATC slots: IC 055 and one other code.
const ATC_MAX: usize = 2;
const PROFILE_NOTE: &str = "These come from your taxpayer profile. Edit them in Profile Settings.";

/// Item 5 choices: IC 055 first, then the 13 official alternatives.
fn atc_options() -> Vec<(&'static str, &'static str)> {
    let mut options = vec![(MCIT_VALUE, "IC 055 — MCIT")];
    options.extend_from_slice(FORM_1702MX_ATC_OPTIONS);
    options
}

/// Validation keys (the model's) that belong to one text input. Messages
/// that name a whole group of inputs are shown by the group instead.
fn model_error_keys(key: &str) -> &'static [&'static str] {
    match key {
        "name" => &["taxpayer_name"],
        "address" => &["registered_address"],
        "phone" => &["contact_number"],
        "email" => &["email"],
        "incorp" => &["incorporation_date"],
        "rate34" => &["relief_basis.special_tax_rate"],
        "rate14b" => &["schedule_2.item_14_special_rate"],
        "rate14c" => &["schedule_2.item_14_regular_rate"],
        "s2.17.2" => &["schedule_2.item_18"],
        _ => &[],
    }
}
const COLUMNS: [&str; 3] = ["A (exempt)", "B (special rate)", "C (regular rate)"];

/// Schedule 5 row labels (Items 1-16, 17a-17i).
const SCHEDULE_5_ITEMS: [&str; 25] = [
    "1 Advertising and promotions",
    "2 Amortizations",
    "3 Bad debts",
    "4 Charitable contributions",
    "5 Commissions",
    "6 Communication, light and water",
    "7 Depletion",
    "8 Depreciation",
    "9 Director's fees",
    "10 Fringe benefits",
    "11 Fuel and oil",
    "12 Insurance",
    "13 Interest",
    "14 Janitorial and messengerial services",
    "15 Losses",
    "16 Management and consultancy fee",
    "17a Miscellaneous",
    "17b Office supplies",
    "17c Other services",
    "17d Others",
    "17e Others",
    "17f Others",
    "17g Others",
    "17h Others",
    "17i Others",
];

/// Free-text inputs: (key, label).
const TEXT_INPUTS: &[(&str, &str)] = &[
    ("year", "Item 2 — Year ended: year (YYYY)"),
    ("name", "Item 8 — Registered Name"),
    ("address", "Item 9 — Registered Address"),
    ("zip", "Item 9A — ZIP Code"),
    ("phone", "Item 11 — Contact Number"),
    ("email", "Item 12 — Email Address"),
    ("incorp", "Item 10 — Date of Incorporation (MM/DD/YYYY)"),
    ("rate34", "Schedule 1 Item 4B — Special tax rate (%)"),
    ("rate14b", "Schedule 2 Item 14B — Special tax rate (%)"),
    ("rate14c", "Schedule 2 Item 14C — Regular tax rate (%)"),
    ("d30", "Schedule 3 Item 30 — Others (specify)"),
    ("d31", "Schedule 3 Item 31 — Others (specify)"),
    ("attachments", "Item 22 — Number of attachments"),
];

/// Accepts `1,234`, `(1,234)`, blank (zero). Whole pesos.
fn parse_peso(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    let negative = trimmed.starts_with('(') && trimmed.ends_with(')') || trimmed.starts_with('-');
    let cleaned: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if cleaned.is_empty() {
        return Some(0);
    }
    let parsed: f64 = cleaned.parse().ok()?;
    let rounded = (parsed + 0.5).floor() as i64;
    Some(if negative { -rounded } else { rounded })
}

/// A percent such as `25`, `7.5` or `25.00` to hundredths.
fn parse_percent(value: &str) -> Option<Option<i32>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(None);
    }
    let parsed: f64 = trimmed.parse().ok()?;
    if !parsed.is_finite() {
        return None;
    }
    Some(Some((parsed * 100.0).round() as i32))
}

/// Schedule 1 Item 4B as shown: one decimal like the page, but a stored
/// second decimal is kept so reopening never changes the value.
fn rate34_text(hundredths: i32) -> String {
    let value = f64::from(hundredths) / 100.0;
    if hundredths % 10 == 0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

fn peso_text(input: &WholePesoInput) -> String {
    match input.amount {
        Some(WholePeso(0)) | None => String::new(),
        Some(WholePeso(value)) => official_display(value),
    }
}

fn percent_text(rate: &PercentInput) -> String {
    rate.hundredths
        .filter(|h| *h != 0)
        .map(|h| format!("{:.2}", f64::from(h) / 100.0))
        .unwrap_or_default()
}

fn column_mut(row: &mut Form1702MXRegimeAmounts, column: usize) -> &mut WholePesoInput {
    match column {
        0 => &mut row.exempt,
        1 => &mut row.special,
        _ => &mut row.regular,
    }
}

fn column_ref(row: &Form1702MXRegimeAmounts, column: usize) -> &WholePesoInput {
    match column {
        0 => &row.exempt,
        1 => &row.special,
        2 => &row.regular,
        _ => &row.total,
    }
}

/// Every whole-peso input, keyed `<group>.<row>.<column>`.
fn amount_keys() -> Vec<String> {
    let mut keys = Vec::new();
    let abc = |keys: &mut Vec<String>, group: &str, rows: &[usize]| {
        for row in rows {
            for column in 0..3 {
                keys.push(format!("{group}.{row}.{column}"));
            }
        }
    };
    abc(&mut keys, "s2", &[0, 1, 3, 5]);
    keys.push("s2.15.1".into());
    keys.push("s2.17.2".into());
    abc(&mut keys, "s3", &[0, 2, 4, 5, 6, 7, 8, 9, 10, 11]);
    keys.push("s3.1.2".into());
    for key in ["s4.0.0", "s4.0.1", "s4.1.0", "s4.1.1", "s4.1.2"] {
        keys.push(key.into());
    }
    abc(&mut keys, "s5", &(0..25).collect::<Vec<_>>());
    abc(&mut keys, "s6", &[0, 1, 2, 3]);
    for group in ["n7", "n8"] {
        for row in 0..3 {
            for column in 0..4 {
                keys.push(format!("{group}.{row}.{column}"));
            }
        }
    }
    for row in 0..3 {
        for column in [0, 1, 3, 4, 5] {
            keys.push(format!("s9.{row}.{column}"));
        }
    }
    abc(&mut keys, "s10", &[0, 1, 2, 4, 5, 6, 7]);
    for key in ["p2.17", "p2.18", "p2.19"] {
        keys.push(key.into());
    }
    keys
}

fn amount_slot<'a>(draft: &'a mut Form1702MXDraft, key: &str) -> Option<&'a mut WholePesoInput> {
    let parts: Vec<&str> = key.split('.').collect();
    let num = |i: usize| parts.get(i).and_then(|p| p.parse::<usize>().ok());
    Some(match parts.first()? {
        &"s2" => column_mut(draft.schedule_2.items.get_mut(num(1)?)?, num(2)?),
        &"s3" => column_mut(draft.schedule_3.items_20_to_33.get_mut(num(1)?)?, num(2)?),
        &"s4" => column_mut(draft.schedule_4.items.get_mut(num(1)?)?, num(2)?),
        &"s5" => column_mut(draft.schedule_5.amounts.get_mut(num(1)?)?, num(2)?),
        &"s6" => column_mut(
            &mut draft.schedule_6.rows.get_mut(num(1)?)?.amounts,
            num(2)?,
        ),
        &"m6" => column_mut(
            &mut draft.schedule_6.more_rows.get_mut(num(1)?)?.amounts,
            num(2)?,
        ),
        &"s10" => column_mut(draft.schedule_10.items.get_mut(num(1)?)?, num(2)?),
        &"n7" | &"n8" => {
            let table = if parts[0] == "n7" {
                &mut draft.schedule_7_1
            } else {
                &mut draft.schedule_8_1
            };
            let row = table.rows.get_mut(num(1)?)?;
            match num(2)? {
                0 => &mut row.amount,
                1 => &mut row.applied_previous_years,
                2 => &mut row.expired,
                _ => &mut row.applied_current_year,
            }
        }
        &"s9" => {
            let row = draft.schedule_9.rows.get_mut(num(1)?)?;
            match num(2)? {
                0 => &mut row.normal_income_tax,
                1 => &mut row.mcit,
                3 => &mut row.applied_previous_years,
                4 => &mut row.expired,
                _ => &mut row.applied_current_year,
            }
        }
        &"p2" => match num(1)? {
            17 => &mut draft.part_ii.item_17_surcharge,
            18 => &mut draft.part_ii.item_18_interest,
            _ => &mut draft.part_ii.item_19_compromise,
        },
        _ => return None,
    })
}

/// Free text bound to the draft: relief rows, descriptions and years.
fn text_slot<'a>(draft: &'a mut Form1702MXDraft, key: &str) -> Option<&'a mut String> {
    let parts: Vec<&str> = key.split('.').collect();
    let num = |i: usize| parts.get(i).and_then(|p| p.parse::<usize>().ok());
    Some(match key {
        "name" => &mut draft.taxpayer_name,
        "address" => &mut draft.registered_address,
        "zip" => &mut draft.zip_code,
        "phone" => &mut draft.contact_number,
        "email" => &mut draft.email,
        "incorp" => &mut draft.incorporation_date,
        "d30" => &mut draft.schedule_3.item_30_description,
        "d31" => &mut draft.schedule_3.item_31_description,
        "attachments" => &mut draft.number_of_attachments,
        _ => match *parts.first()? {
            "rel" => {
                let rd = &mut draft.relief_details;
                let column = num(2)?;
                match num(1)? {
                    1 => rd.investment_promotion_agency.get_mut(column)?,
                    2 => rd.legal_basis.get_mut(column)?,
                    3 => rd.registered_activity.get_mut(column)?,
                    5 => rd.effectivity_from.get_mut(column)?,
                    _ => rd.effectivity_until.get_mut(column)?,
                }
            }
            "d5" => draft
                .schedule_5
                .other_descriptions_17d_to_17i
                .get_mut(num(1)?)?,
            "d6" => &mut draft.schedule_6.rows.get_mut(num(1)?)?.description,
            "l6" => &mut draft.schedule_6.rows.get_mut(num(1)?)?.legal_basis,
            "dm6" => &mut draft.schedule_6.more_rows.get_mut(num(1)?)?.description,
            "lm6" => &mut draft.schedule_6.more_rows.get_mut(num(1)?)?.legal_basis,
            "y7" => &mut draft.schedule_7_1.rows.get_mut(num(1)?)?.year_incurred,
            "y8" => &mut draft.schedule_8_1.rows.get_mut(num(1)?)?.year_incurred,
            "y9" => &mut draft.schedule_9.rows.get_mut(num(1)?)?.year,
            "d10" => draft.schedule_10.descriptions.get_mut(num(1)?)?,
            _ => return None,
        },
    })
}

fn bound_text_keys() -> Vec<String> {
    let mut keys = Vec::new();
    for item in [1, 2, 3, 5, 6] {
        for column in 0..3 {
            keys.push(format!("rel.{item}.{column}"));
        }
    }
    for index in 0..6 {
        keys.push(format!("d5.{index}"));
    }
    for index in 0..4 {
        keys.push(format!("d6.{index}"));
        keys.push(format!("l6.{index}"));
    }
    for index in 0..3 {
        keys.push(format!("y7.{index}"));
        keys.push(format!("y8.{index}"));
        keys.push(format!("y9.{index}"));
    }
    for index in [1, 2, 4, 5, 6, 7] {
        keys.push(format!("d10.{index}"));
    }
    keys
}

/// Schedule 6 Item 4 "(Add more...)" amount keys for the rows the draft holds.
fn more_amount_keys(draft: &Form1702MXDraft) -> Vec<String> {
    (0..draft.schedule_6.more_rows.len())
        .flat_map(|row| (0..3).map(move |column| format!("m6.{row}.{column}")))
        .collect()
}

/// Schedule 6 Item 4 "(Add more...)" description / legal basis keys.
fn more_text_keys(draft: &Form1702MXDraft) -> Vec<String> {
    (0..draft.schedule_6.more_rows.len())
        .flat_map(|row| [format!("dm6.{row}"), format!("lm6.{row}")])
        .collect()
}

const CONTENT_MAX_W: Pixels = px(1180.);

pub struct Form1702MXView {
    draft: Form1702MXDraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    month_select: Entity<ComboboxState>,
    atc_select: Entity<ComboboxState>,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    /// On-screen label of each input, recorded as it renders, so a parse
    /// message can name its field.
    field_labels: RefCell<BTreeMap<String, String>>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    header: FormHeaderState,
    _subscriptions: Vec<Subscription>,
}

impl Form1702MXView {
    fn initial(draft: &Form1702MXDraft, key: &str) -> String {
        let mut scratch = draft.clone();
        if let Some(slot) = amount_slot(&mut scratch, key) {
            return peso_text(slot);
        }
        match key {
            "year" => draft.taxable_year.to_string(),
            "rate34" => draft
                .relief_basis
                .special_tax_rate
                .hundredths
                .filter(|h| *h != 0)
                .map(rate34_text)
                .unwrap_or_default(),
            "rate14b" => percent_text(&draft.schedule_2.item_14_special_rate),
            "rate14c" => percent_text(&draft.schedule_2.item_14_regular_rate),
            _ => text_slot(&mut scratch, key)
                .map(|text| text.trim().to_string())
                .unwrap_or_default(),
        }
    }

    fn all_keys() -> Vec<String> {
        let mut keys: Vec<String> = TEXT_INPUTS.iter().map(|(k, _)| k.to_string()).collect();
        keys.extend(bound_text_keys());
        keys.extend(amount_keys());
        keys
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Read every editor value into the draft, then recompute and validate.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let mut parse_errors = Vec::new();
        let mut draft = self.draft.clone();
        for key in amount_keys().into_iter().chain(more_amount_keys(&draft)) {
            let text = self.input_text(&key, cx);
            match parse_peso(&text) {
                Some(value) => {
                    if let Some(slot) = amount_slot(&mut draft, &key) {
                        slot.set(WholePeso(value));
                    }
                }
                None => {
                    let label = self.field_label(&key);
                    parse_errors.push((
                        key.clone(),
                        format!("{label}: \"{text}\" is not an amount."),
                    ))
                }
            }
        }
        for key in [
            "name",
            "address",
            "zip",
            "phone",
            "email",
            "incorp",
            "d30",
            "d31",
            "attachments",
        ]
        .into_iter()
        .map(String::from)
        .chain(bound_text_keys())
        .chain(more_text_keys(&draft))
        {
            let text = self.input_text(&key, cx);
            if let Some(slot) = text_slot(&mut draft, &key) {
                *slot = text;
            }
        }
        draft.registered_name_lines[0] = draft.taxpayer_name.clone();
        draft.registered_address_lines[0] = draft.registered_address.clone();
        // The year is chosen by the library tile and is read-only; keep the
        // draft's existing taxable_year.
        for (key, label) in [
            ("rate34", "Schedule 1 Item 4B"),
            ("rate14b", "Schedule 2 Item 14B"),
            ("rate14c", "Schedule 2 Item 14C"),
        ] {
            let text = self.input_text(key, cx);
            match parse_percent(&text) {
                Some(value) => {
                    let rate = PercentInput {
                        hundredths: value,
                        raw: text.trim().to_string(),
                    };
                    match key {
                        "rate34" => draft.relief_basis.special_tax_rate = rate,
                        "rate14b" => draft.schedule_2.item_14_special_rate = rate,
                        _ => draft.schedule_2.item_14_regular_rate = rate,
                    }
                }
                None => {
                    parse_errors.push((key.into(), format!("{label}: \"{text}\" is not a rate.")))
                }
            }
        }
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn new_input(
        draft: &Form1702MXDraft,
        key: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let value = Self::initial(draft, key);
        let placeholder = placeholder.to_string();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |state, cx| state.set_value(value, window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.validated = false;
                    this.sync_from_inputs(cx);
                }
            },
        );
        (input, subscription)
    }

    /// Schedule 6 Item 4 Add Row / Remove: read the page, change the rows,
    /// create editors for new rows, then show every row again from the
    /// draft so nothing shifts into the wrong row.
    fn edit_rows(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Form1702MXDraft),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        self.sync_from_inputs(cx);
        change(&mut self.draft);
        self.draft.recompute();
        let keys: Vec<(String, &str)> = more_amount_keys(&self.draft)
            .into_iter()
            .map(|key| (key, "0"))
            .chain(more_text_keys(&self.draft).into_iter().map(|key| (key, "")))
            .collect();
        for (key, placeholder) in keys {
            if self.inputs.contains_key(&key) {
                continue;
            }
            let (input, subscription) = Self::new_input(&self.draft, &key, placeholder, window, cx);
            self.inputs.insert(key, input);
            self._subscriptions.push(subscription);
        }
        let updates: Vec<(Entity<InputState>, String)> = self
            .inputs
            .iter()
            .map(|(key, input)| (input.clone(), Self::initial(&self.draft, key)))
            .collect();
        for (input, value) in updates {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.sync_from_inputs(cx);
        self.status_message = None;
    }

    /// Add Row: the first click opens the modal with Item 4 as row 4.1
    /// (`checkNullRow_OnClick_modalTablePg3Sc6`) and adds row 4.2; later
    /// clicks add one blank row. The official modal has no row limit.
    fn add_more_row(draft: &mut Form1702MXDraft) {
        let s6 = &mut draft.schedule_6;
        if s6.more_rows.is_empty() {
            s6.more_rows.push(s6.rows[3].clone());
        }
        s6.more_rows.push(Default::default());
    }

    /// Remove (`deleteRow`, rows renumber). A single row left goes back to
    /// Item 4, as `Save_modalTablePg3Sc6` does with one row.
    fn remove_more_row(draft: &mut Form1702MXDraft, index: usize) {
        let s6 = &mut draft.schedule_6;
        if index >= s6.more_rows.len() {
            return;
        }
        s6.more_rows.remove(index);
        if s6.more_rows.len() == 1 {
            s6.rows[3] = s6.more_rows.remove(0);
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form1702MXDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.validated = false;
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    /// Item 5 ATC from the multi-select: IC 055 plus at most one other code
    /// (when two other codes arrive, the newest replaces the older).
    fn set_atcs(&mut self, picked: Vec<String>, cx: &mut Context<Self>) {
        let mcit = picked.iter().any(|value| value == MCIT_VALUE);
        let other = picked.into_iter().rfind(|value| value != MCIT_VALUE);
        self.edit(cx, |d| {
            d.atc.mcit_selected = mcit;
            d.atc.other_selected = other.is_some();
            if let Some(code) = other {
                d.atc.other_code = code;
            }
        });
    }

    /// The Item 5 values the draft holds, in slot order.
    fn picked_atcs(draft: &Form1702MXDraft) -> Vec<String> {
        let mut picked = Vec::new();
        if draft.atc.mcit_selected {
            picked.push(MCIT_VALUE.to_string());
        }
        if draft.atc.other_selected {
            picked.push(draft.atc.other_code.clone());
        }
        picked
    }

    fn atc_change() -> MultiChange<Self> {
        Rc::new(
            |this: &mut Self, picked: Vec<String>, _: &mut Window, cx: &mut Context<Self>| {
                this.set_atcs(picked, cx)
            },
        )
    }

    /// Validation messages recorded for any of the model's `keys`.
    fn message_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(&self.validation_errors, keys)
    }

    /// Put the messages for `keys` directly under an already-built row.
    fn with_errors(&self, row: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(row, self.message_for(keys))
    }

    /// The "Please provide complete data on ..." messages that name `marker`
    /// (for example `Schedule 5 Item`), so each schedule shows only its own.
    fn description_message(&self, marker: &str) -> Option<String> {
        let own: Vec<(String, String)> = self
            .validation_errors
            .iter()
            .filter(|(key, message)| key == "descriptions" && message.contains(marker))
            .cloned()
            .collect();
        messages_for(&own, &["descriptions"])
    }

    /// Put the description messages that name `marker` under `row`.
    fn described(&self, row: AnyElement, marker: &str) -> AnyElement {
        with_field_error(row, self.description_message(marker))
    }

    /// Error lines for the messages that exist, in order.
    fn error_lines(messages: impl IntoIterator<Item = Option<String>>) -> Vec<AnyElement> {
        messages
            .into_iter()
            .flatten()
            .map(|message| field_error(message).into_any_element())
            .collect()
    }

    /// The Schedule 1 (Part IV) messages that name column `letter`.
    fn relief_message(&self, letter: char) -> Option<String> {
        let own: Vec<(String, String)> = self
            .validation_errors
            .iter()
            .filter(|(key, message)| {
                key == "relief_details"
                    && (1..=6).any(|item| message.contains(&format!("Item {item}{letter}")))
            })
            .cloned()
            .collect();
        messages_for(&own, &["relief_details"])
    }

    /// The message under one text or amount input: its parse error and the
    /// model messages that belong to it alone.
    fn input_message(&self, key: &str) -> Option<String> {
        let parse = messages_for(&self.parse_errors, &[key]);
        let model = self.message_for(model_error_keys(key));
        match (parse, model) {
            (Some(parse), Some(model)) => Some(format!("{parse} {model}")),
            (parse, model) => parse.or(model),
        }
    }

    /// Messages for values that come from the taxpayer profile and have no
    /// input here (the RDO code), with the note on where to fix them.
    fn render_profile_errors(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let message = self.message_for(&["rdo_code"])?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(field_error(format!("RDO Code: {message}")))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(PROFILE_NOTE),
                )
                .into_any_element(),
        )
    }

    fn notify(
        &self,
        kind: notification::NotificationType,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(
            notification::Notification::new()
                .message(message)
                .with_type(kind)
                .autohide(true),
            cx,
        );
    }

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form1702MXDraft>(
                    &draft.tin,
                    draft.taxable_year,
                    period_column(&draft.filing_period()),
                    ABANDONED_CLAIM_RELEASE_REASON,
                )
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.release_claim_confirm_open = false;
        match result {
            Ok(AbandonedClaimRelease::Released { draft, .. }) => {
                self.draft = draft;
                self.status_message = None;
                cx.emit(QueueableFormEvent::Saved);
            }
            Ok(AbandonedClaimRelease::AlreadyClear { draft, .. }) => {
                if let Some(draft) = draft {
                    self.draft = draft;
                }
            }
            Err(error) => {
                self.notify(
                    notification::NotificationType::Error,
                    format!("Could not release the claim: {error}"),
                    window,
                    cx,
                );
            }
        }
        self.validation_errors = self.draft.validate();
        cx.notify();
    }

    // ── Rendering helpers ──

    fn section(&self, title: &str, children: Vec<AnyElement>, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_col gap_4 p_5 bg={cx.theme().background} border_1 border_color={cx.theme().border} rounded_lg>
                <div text_lg font_weight={FontWeight::BOLD}>{title.to_string()}</div>
                {...children}
            </div>
        }
        .into_any_element()
    }

    /// A labelled input. On phones the label sits above the field.
    /// The label recorded for `key`, or a plain fallback before first render.
    fn field_label(&self, key: &str) -> String {
        self.field_labels
            .borrow()
            .get(key)
            .cloned()
            .unwrap_or_else(|| "Amount".to_string())
    }

    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        self.field_labels
            .borrow_mut()
            .entry(key.to_string())
            .or_insert_with(|| label.to_string());
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let editable = self.draft.lifecycle.is_editable();
        let body = div().child(Input::new(input).disabled(disabled || !editable));
        let label = div()
            .min_w_0()
            .max_w_full()
            .whitespace_normal()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let row = match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(body.w_full()),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.5)))
                .child(body.w(relative(0.5))),
        };
        with_field_error(row.into_any_element(), self.input_message(key))
    }

    fn text(&self, key: &str, layout: Layout) -> AnyElement {
        let label = TEXT_INPUTS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, label)| *label)
            .unwrap_or("");
        self.field(key, label, layout, false)
    }

    /// Two columns on desktop, one otherwise.
    fn grid(&self, layout: Layout, children: Vec<AnyElement>) -> AnyElement {
        if layout == Layout::Desktop {
            let mut rows = div().flex().flex_col().gap_3().w_full();
            let mut iter = children.into_iter();
            while let Some(left) = iter.next() {
                let mut row = div()
                    .flex()
                    .gap_6()
                    .w_full()
                    .child(div().flex_1().child(left));
                row = match iter.next() {
                    Some(right) => row.child(div().flex_1().child(right)),
                    None => row.child(div().flex_1()),
                };
                rows = rows.child(row);
            }
            rows.into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .children(children)
                .into_any_element()
        }
    }

    fn computed(&self, label: &str, value: i64, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{official_display(value)}</div>
            </div>
        }
        .into_any_element()
    }

    fn choice(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        selected: bool,
        disabled: bool,
    ) -> Button {
        let label: SharedString = label.into();
        let button = Button::new(id).label(if selected {
            format!("✓ {label}")
        } else {
            label.to_string()
        });
        let button = if selected {
            button.primary()
        } else {
            button.outline()
        };
        // Let long chips shrink to the panel instead of running past its edge.
        button.disabled(disabled).min_w_0().max_w_full()
    }

    /// Label then bold value; stacks on phone.
    fn read_only_row(layout: Layout, label: &str, value: &str) -> AnyElement {
        let label = div()
            .min_w_0()
            .max_w_full()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(label.to_string());
        let value = div()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .child(value.to_string());
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(value)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(label)
                .child(value)
                .into_any_element()
        }
    }

    fn choice_row(layout: Layout, title: &str, buttons: Vec<Button>) -> AnyElement {
        if layout == Layout::Phone {
            return div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(
                    div()
                        .min_w_0()
                        .max_w_full()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(title.to_string()),
                )
                .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
                .into_any_element();
        }
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .max_w_full()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(title.to_string()),
            )
            .children(buttons)
            .into_any_element()
    }

    /// One schedule row: label, its A-C inputs and its computed total.
    fn abc_row(
        &self,
        group: &str,
        row: usize,
        label: &str,
        total: i64,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let fields: Vec<AnyElement> = (0..3)
            .map(|column| {
                self.field_labels.borrow_mut().insert(
                    format!("{group}.{row}.{column}"),
                    format!("{label} — {}", COLUMNS[column]),
                );
                self.field(
                    &format!("{group}.{row}.{column}"),
                    COLUMNS[column],
                    layout,
                    false,
                )
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(div().font_weight(FontWeight::BOLD).child(label.to_string()))
            .child(self.grid(layout, fields))
            .child(self.computed("D (total)", total, cx))
            .into_any_element()
    }

    /// A computed row across A-D.
    fn computed_row(
        &self,
        label: &str,
        row: &Form1702MXRegimeAmounts,
        cx: &Context<Self>,
    ) -> AnyElement {
        let values: Vec<String> = (0..4)
            .map(|c| official_display(column_ref(row, c).value_or_zero().0))
            .collect();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .p_2()
            .bg(cx.theme().muted.opacity(0.5))
            .rounded_md()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .min_w_0()
                    .max_w_full()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(format!(
                        "A {} · B {} · C {} · D {}",
                        values[0], values[1], values[2], values[3]
                    )),
            )
            .into_any_element()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fiscal = d.filing_basis == Form1702MXFilingBasis::Fiscal;
        // Editable only for a Fiscal or Short period return; otherwise the
        // select shows December read-only.
        let month_text = d.month.to_string();
        let month_select = select_field(
            "Item 2 — Year ended: month",
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=12).contains(&d.month).then_some(month_text.as_str()),
            !editable || (!fiscal && !d.is_short_period),
            self.message_for(&["month", "taxable_year"]).as_deref(),
            cx,
        );
        let (amended, short) = (d.is_amended, d.is_short_period);
        let children = vec![
            Self::choice_row(
                layout,
                "Item 1 — For (Calendar or Fiscal year)",
                vec![
                    Self::choice("1702mx_calendar", "Calendar", !fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.filing_basis = Form1702MXFilingBasis::Calendar)
                        }),
                    ),
                    Self::choice("1702mx_fiscal", "Fiscal", fiscal, !editable).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| {
                                d.filing_basis = Form1702MXFilingBasis::Fiscal;
                                if d.month == 12 {
                                    d.month = 6;
                                }
                            })
                        }),
                    ),
                ],
            ),
            month_select,
            Self::read_only_row(
                layout,
                "Item 2 — Year ended: year (YYYY)",
                &d.taxable_year.to_string(),
            ),
            Self::choice_row(
                layout,
                "Item 3 — Amended return?",
                vec![
                    Self::choice("1702mx_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("1702mx_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
            ),
            Self::choice_row(
                layout,
                "Item 4 — Short period return?",
                vec![
                    Self::choice("1702mx_short_yes", "Yes", short, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_short_period = true)),
                    ),
                    Self::choice("1702mx_short_no", "No", !short, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_short_period = false)),
                    ),
                ],
            ),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn render_part_one(&self, layout: Layout, cx: &mut Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let fixed = |label: &str, value: String| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.to_string()),
                )
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element()
        };
        let fields = vec![
            fixed("Item 6 — TIN", format_tin(&d.tin)),
            fixed("Item 7 — RDO Code", d.rdo_code.clone()),
            self.text("name", layout),
            self.text("address", layout),
            self.text("zip", layout),
            self.text("incorp", layout),
            self.text("phone", layout),
            self.text("email", layout),
        ];
        // Two official slots: IC 055 and one other code.
        let atc_select = multi_select_field(
            "1702mx_atc",
            "Item 5 — ATC",
            &self.atc_select,
            &atc_options(),
            &Self::picked_atcs(d),
            ATC_MAX,
            !editable,
            self.message_for(&["atc"]).as_deref(),
            Self::atc_change(),
            cx,
        );
        let method = d.deduction_method;
        let mut rows = vec![atc_select, self.grid(layout, fields)];
        rows.extend(self.render_profile_errors(cx));
        rows.push(self.with_errors(
            Self::choice_row(
                layout,
                "Item 13 — Method of deduction",
                vec![
                    Self::choice(
                        "1702mx_itemized",
                        "Itemized",
                        method == Form1702MXDeductionMethod::Itemized,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.deduction_method = Form1702MXDeductionMethod::Itemized
                        })
                    })),
                    Self::choice(
                        "1702mx_osd",
                        "Optional Standard Deduction (40%)",
                        method == Form1702MXDeductionMethod::OptionalStandard,
                        !editable,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(cx, |d| {
                            d.deduction_method = Form1702MXDeductionMethod::OptionalStandard
                        })
                    })),
                ],
            ),
            &["deduction_method"],
        ));
        self.section("Part I — Background Information", rows, cx)
    }

    fn render_relief(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let multiple = self.draft.relief_basis.instruction_multiple_activities;
        let mut rows = vec![self.with_errors(
            Self::choice_row(
                layout,
                "Instruction",
                vec![
                Self::choice(
                    "1702mx_inst_a",
                    "A — one activity per column",
                    !multiple,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| {
                        d.relief_basis.instruction_multiple_activities = false
                    })
                })),
                Self::choice(
                    "1702mx_inst_b",
                    "B — more activities (attachments)",
                    multiple,
                    !editable,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(cx, |d| {
                        d.relief_basis.instruction_multiple_activities = true
                    })
                })),
            ],
            ),
            &["relief_basis"],
        )];
        for (column, title) in COLUMNS.iter().enumerate() {
            let mut fields = vec![
                self.field(
                    &format!("rel.1.{column}"),
                    "Item 1 — Investment promotion agency",
                    layout,
                    false,
                ),
                self.field(
                    &format!("rel.2.{column}"),
                    "Item 2 — Legal basis",
                    layout,
                    false,
                ),
                self.field(
                    &format!("rel.3.{column}"),
                    "Item 3 — Registered activity",
                    layout,
                    false,
                ),
                self.field(
                    &format!("rel.5.{column}"),
                    "Item 5 — Effectivity from (MM/DD/YYYY)",
                    layout,
                    false,
                ),
                self.field(
                    &format!("rel.6.{column}"),
                    "Item 6 — Effectivity until (MM/DD/YYYY)",
                    layout,
                    false,
                ),
            ];
            if column == 1 {
                fields.insert(3, self.text("rate34", layout));
            }
            rows.push(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Column {title}"))
                    .into_any_element(),
            );
            rows.push(with_field_error(
                self.grid(layout, fields),
                self.relief_message(['A', 'B', 'C'][column]),
            ));
        }
        self.section("Part IV Schedule 1 — Tax relief availment", rows, cx)
    }

    fn render_schedule_2(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s2 = &self.draft.schedule_2.items;
        let total = |i: usize| s2[i].total.value_or_zero().0;
        let mut rows = vec![
            self.abc_row(
                "s2",
                0,
                "1 Sales / revenues / receipts / fees",
                total(0),
                layout,
                cx,
            ),
            self.abc_row(
                "s2",
                1,
                "2 Less: cost of sales / services",
                total(1),
                layout,
                cx,
            ),
            self.computed_row("3 Gross income from operation", &s2[2], cx),
            self.abc_row(
                "s2",
                3,
                "4 Other taxable income (not subject to final tax)",
                total(3),
                layout,
                cx,
            ),
            self.computed_row("5 Total gross income", &s2[4], cx),
            self.abc_row("s2", 5, "6 Other income", total(5), layout, cx),
            self.computed_row("7 Total taxable income", &s2[6], cx),
            self.computed_row(
                "8 Ordinary allowable itemized deductions (Schedule 5)",
                &s2[7],
                cx,
            ),
            self.computed_row(
                "9 Special allowable itemized deductions (Schedule 6)",
                &s2[8],
                cx,
            ),
            self.computed_row("10 NOLCO (Schedules 7 / 8)", &s2[9], cx),
            self.computed_row("11 Total itemized deductions", &s2[10], cx),
            self.computed_row("12 Optional standard deduction", &s2[11], cx),
            self.computed_row("13 Net taxable income", &s2[12], cx),
        ];
        // Items 14-19 in official order: inputs sit between the computed rows.
        rows.push(self.grid(
            layout,
            vec![self.text("rate14b", layout), self.text("rate14c", layout)],
        ));
        rows.push(self.computed_row("15 Income tax due", &s2[14], cx));
        rows.push(self.field(
            "s2.15.1",
            "16B — Less: share of other government agencies",
            layout,
            false,
        ));
        rows.push(self.computed_row("17 Net income tax due", &s2[16], cx));
        rows.push(self.field(
            "s2.17.2",
            "18C — MCIT (2% of gross income)",
            layout,
            !self.draft.atc.mcit_selected,
        ));
        rows.push(self.computed_row("19 Tax due", &s2[18], cx));
        self.section("Schedule 2 — Computation of Tax", rows, cx)
    }

    fn render_schedule_3(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s3 = &self.draft.schedule_3.items_20_to_33;
        let labels = [
            (0usize, "20 Prior year's excess credits"),
            (2, "22 Creditable tax withheld from previous quarters"),
            (
                4,
                "24 Creditable tax withheld per BIR Form 2307 (4th quarter)",
            ),
            (5, "25 Foreign tax credits"),
            (6, "26 Tax paid in previous quarters"),
            (7, "27 Tax paid in return previously filed (amended)"),
            (8, "28 Special tax credits"),
            (9, "29 Other tax credits (Schedule 4)"),
            (10, "30 Others"),
            (11, "31 Others"),
        ];
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, label) in labels {
            if index == 10 {
                rows.push(self.described(self.text("d30", layout), "Schedule 3 Item 30"));
            }
            if index == 11 {
                rows.push(self.described(self.text("d31", layout), "Schedule 3 Item 31"));
            }
            rows.push(self.abc_row(
                "s3",
                index,
                label,
                s3[index].total.value_or_zero().0,
                layout,
                cx,
            ));
            if index == 7 {
                // Item 27 applies only to an amended return.
                rows.extend(Self::error_lines([
                    self.message_for(&["schedule_3.item_27"])
                ]));
            }
        }
        rows.insert(
            1,
            self.field(
                "s3.1.2",
                "21C — Income tax payment under MCIT from previous quarters",
                layout,
                false,
            ),
        );
        rows.push(self.computed_row("23 Excess MCIT applied (Schedule 9)", &s3[3], cx));
        rows.push(self.computed_row("32 Total tax credits / payments", &s3[12], cx));
        rows.push(self.computed_row("33 Net tax payable / (overpayment)", &s3[13], cx));
        self.section("Schedule 3 — Tax Credits / Payments", rows, cx)
    }

    fn render_schedule_4(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s4 = &self.draft.schedule_4.items;
        let rows = vec![
            self.with_errors(
                self.grid(
                    layout,
                    vec![
                        self.field(
                            "s4.0.0",
                            "1A — Regular income tax otherwise due (exempt)",
                            layout,
                            false,
                        ),
                        self.field(
                            "s4.0.1",
                            "1B — Regular income tax otherwise due (special)",
                            layout,
                            false,
                        ),
                        self.field(
                            "s4.1.0",
                            "2A — Special allowable itemized deductions × rate",
                            layout,
                            false,
                        ),
                        self.field(
                            "s4.1.1",
                            "2B — Special allowable itemized deductions × rate",
                            layout,
                            false,
                        ),
                        self.field(
                            "s4.1.2",
                            "2C — Special allowable itemized deductions × rate",
                            layout,
                            false,
                        ),
                    ],
                ),
                &["schedule_4"],
            ),
            self.computed_row("3 Total", &s4[2], cx),
            self.computed_row("5 Tax relief", &s4[4], cx),
            self.computed_row("7 Total tax relief availment", &s4[6], cx),
        ];
        self.section("Schedule 4 — Tax Relief Availment", rows, cx)
    }

    fn render_schedule_5(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s5 = &self.draft.schedule_5.amounts;
        let mut rows = Vec::new();
        for (index, label) in SCHEDULE_5_ITEMS.iter().enumerate() {
            if index >= 19 {
                rows.push(self.described(
                    self.field(
                        &format!("d5.{}", index - 19),
                        &format!("Item {} — description", &label[..3]),
                        layout,
                        false,
                    ),
                    &format!("Schedule 5 Item {}", &label[..3]),
                ));
            }
            rows.push(self.abc_row(
                "s5",
                index,
                label,
                s5[index].total.value_or_zero().0,
                layout,
                cx,
            ));
        }
        rows.push(self.computed_row(
            "18 Total ordinary allowable itemized deductions",
            &s5[25],
            cx,
        ));
        self.section(
            "Schedule 5 — Ordinary Allowable Itemized Deductions",
            rows,
            cx,
        )
    }

    fn render_schedule_6(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s6 = &self.draft.schedule_6;
        let editable = self.draft.lifecycle.is_editable();
        let mut rows = Vec::new();
        for (index, entry) in s6.rows.iter().enumerate() {
            if index == 3 && s6.is_folded() {
                // Item 4 reads OTHERS and the modal subtotals.
                rows.push(Self::read_only_row(
                    layout,
                    "Item 4 — description / legal basis",
                    "OTHERS / OTHERS",
                ));
                rows.push(self.computed_row("Item 4", &entry.amounts, cx));
                continue;
            }
            rows.push(self.described(
                self.grid(
                    layout,
                    vec![
                        self.field(
                            &format!("d6.{index}"),
                            &format!("Item {} — description", index + 1),
                            layout,
                            false,
                        ),
                        self.field(
                            &format!("l6.{index}"),
                            &format!("Item {} — legal basis", index + 1),
                            layout,
                            false,
                        ),
                    ],
                ),
                &format!("Schedule 6 Item {}", index + 1),
            ));
            rows.push(self.abc_row(
                "s6",
                index,
                &format!("Item {}", index + 1),
                entry.amounts.total.value_or_zero().0,
                layout,
                cx,
            ));
        }
        rows.push(
            div()
                .font_weight(FontWeight::BOLD)
                .child("Item 4 — (Add more...)")
                .into_any_element(),
        );
        for index in 0..s6.more_rows.len() {
            let remove = Button::new(("1702mx_s6_remove", index))
                .disabled(!editable)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_rows(window, cx, |d| Self::remove_more_row(d, index))
                }));
            let caption = format!("4.{}", index + 1);
            rows.push(kit::row_heading(&caption, remove));
            rows.push(self.grid(
                layout,
                vec![
                    self.field(&format!("dm6.{index}"), "Description", layout, false),
                    self.field(&format!("lm6.{index}"), "Legal basis", layout, false),
                ],
            ));
            rows.push(self.abc_row(
                "m6",
                index,
                &caption,
                s6.more_rows[index].amounts.total.value_or_zero().0,
                layout,
                cx,
            ));
        }
        let item_4 = &s6.rows[3];
        let link_open = s6.is_folded()
            || !item_4.description.trim().is_empty()
                && !item_4.legal_basis.trim().is_empty()
                && item_4.amounts.total.value_or_zero().0 != 0;
        rows.push(
            kit::add_row_button("1702mx_s6_add", !editable || !link_open)
                .on_click(
                    cx.listener(|this, _, window, cx| {
                        this.edit_rows(window, cx, Self::add_more_row)
                    }),
                )
                .into_any_element(),
        );
        rows.extend(Self::error_lines([
            self.message_for(&["schedule_6.more_rows"])
        ]));
        if s6.is_folded() {
            rows.push(self.computed_row("Item 4 subtotal", &s6.modal_subtotal(), cx));
        }
        rows.push(self.computed_row(
            "5 Total special allowable itemized deductions",
            &self.draft.schedule_6.item_5_total,
            cx,
        ));
        self.section(
            "Schedule 6 — Special Allowable Itemized Deductions",
            rows,
            cx,
        )
    }

    fn render_nolco(
        &self,
        group: &str,
        title: &str,
        layout: Layout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let table = if group == "n7" {
            &self.draft.schedule_7_1
        } else {
            &self.draft.schedule_8_1
        };
        let year_group = if group == "n7" { "y7" } else { "y8" };
        // How the validation messages name this table.
        let (table_key, schedule) = if group == "n7" {
            ("schedule_7_1", "Schedule 7")
        } else {
            ("schedule_8_1", "Schedule 8.1")
        };
        let mut rows = Vec::new();
        for row in 0..3 {
            let fields = vec![
                self.field(
                    &format!("{year_group}.{row}"),
                    "Year incurred",
                    layout,
                    false,
                ),
                self.field(&format!("{group}.{row}.0"), "A — Amount", layout, false),
                self.field(
                    &format!("{group}.{row}.1"),
                    "B — Applied previous years",
                    layout,
                    false,
                ),
                self.field(&format!("{group}.{row}.2"), "C — Expired", layout, false),
                self.field(
                    &format!("{group}.{row}.3"),
                    "D — Applied this year",
                    layout,
                    false,
                ),
            ];
            rows.push(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Item {}", row + 4))
                    .into_any_element(),
            );
            rows.push(self.described(
                self.grid(layout, fields),
                &format!("{schedule} Item {}", row + 4),
            ));
            rows.push(self.computed(
                "E — Unapplied",
                table.rows[row].unapplied.value_or_zero().0,
                cx,
            ));
            let balance_key = format!("{table_key}.rows[{row}]");
            rows.extend(Self::error_lines([
                self.message_for(&[balance_key.as_str()])
            ]));
        }
        rows.push(self.computed(
            "Item 7 — This year's net operating loss",
            table.rows[3].amount.value_or_zero().0,
            cx,
        ));
        rows.push(self.computed(
            "Item 8 — Total NOLCO applied this year",
            table.item_8_total_applied_current_year.value_or_zero().0,
            cx,
        ));
        self.section(title, rows, cx)
    }

    fn render_schedule_9(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let disabled = !self.draft.atc.mcit_selected;
        let mut rows = Vec::new();
        for (row, entry) in self.draft.schedule_9.rows.iter().enumerate() {
            let fields = vec![
                self.field(&format!("y9.{row}"), "Year", layout, disabled),
                self.field(
                    &format!("s9.{row}.0"),
                    "A — Normal income tax",
                    layout,
                    disabled,
                ),
                self.field(&format!("s9.{row}.1"), "B — MCIT", layout, disabled),
                self.field(
                    &format!("s9.{row}.3"),
                    "D — Applied previous years",
                    layout,
                    disabled,
                ),
                self.field(&format!("s9.{row}.4"), "E — Expired", layout, disabled),
                self.field(
                    &format!("s9.{row}.5"),
                    "F — Applied this year",
                    layout,
                    disabled,
                ),
            ];
            rows.push(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Item {}", row + 1))
                    .into_any_element(),
            );
            rows.push(self.described(
                self.grid(layout, fields),
                &format!("Schedule 9 Item {}", row + 1),
            ));
            rows.push(self.computed("C — Excess MCIT", entry.excess_mcit.value_or_zero().0, cx));
            rows.push(self.computed("G — Balance", entry.balance.value_or_zero().0, cx));
            let balance_key = format!("schedule_9.rows[{row}]");
            rows.extend(Self::error_lines([
                self.message_for(&[balance_key.as_str()])
            ]));
        }
        rows.push(
            self.computed(
                "4 Total excess MCIT applied",
                self.draft
                    .schedule_9
                    .item_4_total_applied_current_year
                    .value_or_zero()
                    .0,
                cx,
            ),
        );
        self.section(
            "Schedule 9 — Excess MCIT over Normal Income Tax (IC 055)",
            rows,
            cx,
        )
    }

    fn render_schedule_10(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let s10 = &self.draft.schedule_10.items;
        let mut rows = Vec::new();
        for (index, label) in [
            (0usize, "1 Net income / (loss) per books (may be negative)"),
            (1, "2 Add: non-deductible expenses / taxable other income"),
            (2, "3 Add: others"),
            (
                4,
                "5 Less: non-taxable income / income subjected to final tax",
            ),
            (5, "6 Less: others"),
            (6, "7 Less: special deductions"),
            (7, "8 Less: others"),
        ] {
            if index != 0 {
                rows.push(self.described(
                    self.field(
                        &format!("d10.{index}"),
                        &format!("Item {} — description", index + 1),
                        layout,
                        false,
                    ),
                    &format!("Schedule 10 Item {}", index + 1),
                ));
            }
            rows.push(self.abc_row(
                "s10",
                index,
                label,
                s10[index].total.value_or_zero().0,
                layout,
                cx,
            ));
        }
        rows.push(self.computed_row("4 Total", &s10[3], cx));
        rows.push(self.computed_row("9 Total", &s10[8], cx));
        rows.push(self.computed_row(
            "10 Net taxable income (must equal Schedule 2 Item 13)",
            &s10[9],
            cx,
        ));
        rows.extend(Self::error_lines([self.message_for(&["schedule_10"])]));
        self.section(
            "Schedule 10 — Reconciliation of Net Income per Books",
            rows,
            cx,
        )
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let p2 = &self.draft.part_ii;
        let mut children = vec![
            self.computed(
                "14 Total income tax due",
                p2.item_14_total_tax_due_or_overpayment.0,
                cx,
            ),
            self.computed(
                "15 Less: total tax credits / payments",
                p2.item_15_total_tax_credits.0,
                cx,
            ),
            self.computed(
                "16 Net tax payable / (overpayment)",
                p2.item_16_net_tax_payable_or_overpayment.0,
                cx,
            ),
            self.field("p2.17", "17 Surcharge", layout, false),
            self.field("p2.18", "18 Interest", layout, false),
            self.field("p2.19", "19 Compromise", layout, false),
            self.computed(
                "20 Total penalties",
                p2.item_20_total_penalties.value_or_zero().0,
                cx,
            ),
            self.computed(
                "21 Total amount payable / (overpayment)",
                p2.item_21_total_amount_payable_or_overpayment
                    .value_or_zero()
                    .0,
                cx,
            ),
        ];
        if p2.item_16_net_tax_payable_or_overpayment.0 < 0 {
            let over = p2.overpayment_disposition;
            let pick =
                |id: &'static str, label: &'static str, value: Form1702MXOverpaymentDisposition| {
                    Self::choice(id, label, over == Some(value), !editable).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.edit(cx, |d| d.part_ii.overpayment_disposition = Some(value))
                        },
                    ))
                };
            children.push(self.with_errors(
                Self::choice_row(
                    layout,
                    "If overpayment, mark one:",
                    vec![
                        pick(
                            "1702mx_refund",
                            "To be refunded",
                            Form1702MXOverpaymentDisposition::Refund,
                        ),
                        pick(
                            "1702mx_tcc",
                            "To be issued a Tax Credit Certificate",
                            Form1702MXOverpaymentDisposition::TaxCreditCertificate,
                        ),
                        pick(
                            "1702mx_carry",
                            "To be carried over",
                            Form1702MXOverpaymentDisposition::CarryOver,
                        ),
                    ],
                ),
                &["part_ii.overpayment_disposition"],
            ));
        }
        children.extend(Self::error_lines([self.message_for(&["part_ii"])]));
        self.section(
            "Part II — Total Tax Payable",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form1702MXView {
    type Draft = Form1702MXDraft;

    fn new(
        draft: Form1702MXDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut money: Vec<String> = amount_keys();
        money.extend(more_amount_keys(&draft));
        let keys = Self::all_keys()
            .into_iter()
            .chain(more_amount_keys(&draft))
            .chain(more_text_keys(&draft));
        for key in keys {
            let placeholder = if money.contains(&key) { "0" } else { "" };
            let (input, subscription) = Self::new_input(&draft, &key, placeholder, window, cx);
            subscriptions.push(subscription);
            inputs.insert(key, input);
        }
        let month_select = form_controls::new_select_state(&MONTH_OPTIONS, window, cx);
        subscriptions.push(form_controls::on_select(
            &month_select,
            &MONTH_OPTIONS,
            window,
            cx,
            |this: &mut Self, value, _, cx| {
                if let Ok(month) = value.parse::<u8>() {
                    this.edit(cx, |d| d.month = month);
                }
            },
        ));
        let atc_choices = atc_options();
        let atc_select = form_controls::new_select_state(&atc_choices, window, cx);
        subscriptions.push(form_controls::on_multi_select(
            &atc_select,
            &atc_choices,
            ATC_MAX,
            window,
            cx,
            |this: &Self| Self::picked_atcs(&this.draft),
            Self::atc_change(),
        ));
        let validation_errors = draft.validate();
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            month_select,
            atc_select,
            validation_errors,
            parse_errors: Vec::new(),
            field_labels: RefCell::default(),
            status_message: None,
            release_claim_confirm_open: false,
            validated: false,
            header: FormHeaderState::new(),
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, _period: u8) -> Form1702MXDraft {
        let mut draft = Form1702MXDraft::new_from_profile(profile, year);
        draft.recompute();
        draft
    }

    /// Annual: earlier builds stored the row with a NULL period column, so
    /// fall back to the period-key lookup (the first generic save adopts it).
    fn load_draft(
        db: &Database,
        profile: &TaxpayerProfile,
        year: u16,
        period: u8,
    ) -> Form1702MXDraft {
        let tin = profile.tin.full();
        db.get_queueable_draft::<Form1702MXDraft>(&tin, year, 0)
            .ok()
            .flatten()
            .or_else(|| {
                db.get_form_draft_v2::<Form1702MXDraft>(&tin, "1702MX", year, &FilingPeriod::Annual)
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| Self::new_draft(profile, year, period))
    }
}

fn format_tin(tin: &str) -> String {
    let digits: String = tin.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 9 {
        return tin.to_string();
    }
    let branch = digits.get(9..).unwrap_or("");
    format!(
        "{}-{}-{}-{:0>5}",
        &digits[0..3],
        &digits[3..6],
        &digits[6..9],
        branch
    )
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
        self.draft.lifecycle.status.clone()
    }
    fn submitted_at(&self) -> Option<&str> {
        self.draft.lifecycle.submitted_at.as_deref()
    }
    fn confirmed_at(&self) -> Option<&str> {
        self.draft.lifecycle.confirmed_at.as_deref()
    }

    fn save_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        // Save what parses; unreadable amounts keep their text in the box
        // and keep Queue blocked.
        let unreadable = !self.parse_errors.is_empty();
        let result = match self.db.lock() {
            Ok(db) => db
                .save_queueable_draft(&self.draft)
                .map(|id| {
                    // save-009: saved anyway; the alert goes in the status box.
                    let amended_required = db.amended_return_required(&self.draft).unwrap_or(false);
                    (id, amended_required)
                })
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok((id, amended_required)) => {
                self.draft.id = Some(id);
                let (kind, message) = if unreadable {
                    (
                        notification::NotificationType::Warning,
                        "1702-MX draft saved. Entries that are not amounts were not saved; fix them inline.",
                    )
                } else {
                    (
                        notification::NotificationType::Success,
                        "1702-MX draft saved.",
                    )
                };
                // The save never refuses; the warning stays in the status box.
                let mut status: Vec<&str> = Vec::new();
                if unreadable {
                    status.push(message);
                }
                if amended_required {
                    status.push(bir_core::forms::queueable::AMENDED_RETURN_ALERT);
                }
                self.status_message = (!status.is_empty()).then(|| status.join(" "));
                self.notify(kind, message.into(), window, cx);
                cx.emit(QueueableFormEvent::Saved);
                cx.notify();
            }
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Save failed".into(),
                error,
            )),
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !can_queue_for_submission(<Form1702MXDraft as QueueableForm>::FORM_CODE) {
            self.status_message =
                Some("1702-MX is not enabled for in-app submission in this build.".into());
            cx.notify();
            return;
        }
        if !self.draft.lifecycle.is_editable() {
            self.status_message =
                Some("This return is already queued or filed and cannot be queued again.".into());
            cx.notify();
            return;
        }
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() || !self.validation_errors.is_empty() {
            self.status_message = Some("Fix the items marked below before submitting.".into());
            cx.notify();
            return;
        }
        let before = self.draft.clone();
        if let Err(errors) = self.draft.queue(QueueAuthSource::Gui) {
            self.validation_errors = errors;
            self.status_message = Some("Fix the items marked below before submitting.".into());
            cx.notify();
            return;
        }
        let saved = match self.db.lock() {
            Ok(db) => db
                .save_queued_queueable(&self.draft)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = saved {
            self.draft = before;
            self.status_message = Some(bir_core::forms::queueable::queue_failure_message(
                "Form 1702-MX",
                &error,
            ));
            cx.notify();
            return;
        }
        // D5C: the toast and the stepper are the only confirmation.
        self.status_message = None;
        self.validated = false;
        self.notify(
            notification::NotificationType::Success,
            "Form 1702-MX queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("1702-MX payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    /// D7A + D8A: Queued reverts plainly (a claimed submission keeps the
    /// release confirm); Submitted/Confirmed keeps a snapshot of what BIR
    /// received and marks the return amended.
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let status = self.draft.lifecycle.status.clone();
        if !can_user_revert(&status) {
            return;
        }
        if matches!(status, FilingStatus::Queued) && !self.draft.lifecycle.is_unclaimed() {
            self.cancel_queue(window, cx);
            return;
        }
        match revert_for_amendment(&self.db, &self.draft) {
            Ok(reverted) => {
                self.draft = reverted.draft;
                self.validated = false;
                self.validation_errors = self.draft.validate();
                self.status_message = reverted
                    .snapshot_id
                    .map(|_| AMENDED_REVERT_MESSAGE.to_string());
                if reverted.snapshot_id.is_some() {
                    self.header.load_sent_snapshot(&self.db, &self.draft);
                }
                self.notify(
                    notification::NotificationType::Success,
                    "Form 1702-MX reverted to Draft.".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => self.notify(
                notification::NotificationType::Error,
                format!("Could not revert to Draft: {error}"),
                window,
                cx,
            ),
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                "Fix the highlighted entries first. No filing state was changed.".into(),
            ));
            return;
        }
        let fields = self.draft.to_bir_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "1702mx-2018c",
            &fields,
            "1702-MX — Print Preview",
            cx,
        ) {
            Ok(kind) => cx.emit(QueueableFormEvent::PushNotification(
                "info".into(),
                "Print preview".into(),
                format!("{} No filing state was changed.", kind.status_message()),
            )),
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                format!("{error}. No filing state was changed."),
            )),
        }
    }
}

impl Render for Form1702MXView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let month_text = self.draft.month.to_string();
        form_controls::sync_select(
            &self.month_select,
            &MONTH_OPTIONS,
            (1..=12)
                .contains(&self.draft.month)
                .then_some(month_text.as_str()),
            window,
            cx,
        );
        form_controls::sync_multi_select(
            &self.atc_select,
            &atc_options(),
            &Self::picked_atcs(&self.draft),
            window,
            cx,
        );
        let error_count = self.parse_errors.len() + self.validation_errors.len();

        let phone = layout == Layout::Phone;
        self.header.ensure_sent_snapshot(&self.db, &self.draft);
        let props = self.header_props();
        let header_buttons = render_header_buttons(self, &props, HEADER_ACTIONS, phone, cx);
        let status_box = render_status_box(self, &props, HEADER_ACTIONS, cx);
        // Phone: no stepper, so the box sits at the top of the body.
        let (status_box, phone_status_box) = if phone {
            (None, status_box)
        } else {
            (status_box, None)
        };

        let toolbar = div()
            .flex()
            .items_center()
            .justify_between()
            .when(phone, |d| d.gap_1().py_2())
            .when(!phone, |d| d.gap_2().py_3())
            .px(pad)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("1702mx_back")
                    .label("← Back")
                    .when(phone, |b| b.small())
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                    ),
            )
            .child(header_buttons);

        let mut body = div()
            .w_full()
            .max_w(CONTENT_MAX_W)
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .children(phone_status_box)
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("1702mx_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("1702mx_release_cancel")
                                .label("Keep queued")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.release_claim_confirm_open = false;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(self.render_period(layout, cx))
            .child(self.render_part_one(layout, cx))
            .child(self.render_part_two(layout, cx))
            .child(self.render_relief(layout, cx))
            .child(self.render_schedule_2(layout, cx))
            .child(self.render_schedule_3(layout, cx))
            .child(self.render_schedule_4(layout, cx))
            .child(self.render_schedule_5(layout, cx))
            .child(self.render_schedule_6(layout, cx))
            .child(self.render_nolco("n7", "Schedules 7 / 7.1 — NOLCO (regular rate)", layout, cx))
            .child(self.render_nolco("n8", "Schedules 8 / 8.1 — NOLCO (special rate)", layout, cx))
            .child(self.render_schedule_9(layout, cx))
            .child(self.render_schedule_10(layout, cx))
            .child(self.section(
                "Attachments",
                vec![self.grid(layout, vec![self.text("attachments", layout)])],
                cx,
            ));

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(cx.theme().background)
            .child(toolbar)
            .child(
                div()
                    .px(pad)
                    .py_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(self.render_header(cx))
                    .when(layout != Layout::Phone, |d| {
                        d.child(div().mt_4().child(self.render_status_pipeline(cx)))
                    })
                    .children(status_box.map(|b| div().mt_3().child(b))),
            )
            .child(
                div()
                    .id("1702mx_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form1702MXView {
    /// The pre-header queue cancel, kept for a claimed submission.
    fn cancel_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.draft.lifecycle.status, FilingStatus::Queued) {
            self.status_message =
                Some("This return cannot be reverted after submission has started.".into());
            cx.notify();
            return;
        }
        if !self.draft.lifecycle.is_unclaimed() {
            self.release_claim_confirm_open = true;
            self.status_message = Some(
                "Submission was claimed. Confirm nothing reached BIR to return it to an editable Draft. This does not file.".into(),
            );
            cx.notify();
            return;
        }
        let queued = self.draft.clone();
        let canceled = match self.db.lock() {
            Ok(db) => db
                .cancel_queued_queueable(&queued)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match canceled {
            Ok(draft) => {
                self.draft = draft;
                self.status_message = None;
                self.validation_errors = self.draft.validate();
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702MXDraft>(
                        &queued.tin,
                        queued.taxable_year,
                        period_column(&queued.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Warning,
                    format!("The queued return was not canceled: {error}"),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn blocking_errors(&self) -> usize {
        self.parse_errors.len() + self.validation_errors.len()
    }

    /// What the shared header shows.
    fn header_props(&self) -> FormHeaderProps<'static> {
        FormHeaderProps {
            slug: "1702mx",
            status: self.draft.lifecycle.status.clone(),
            editor_safe: true,
            validated: self.validated,
            errors: self
                .parse_errors
                .iter()
                .chain(self.validation_errors.iter())
                .map(|(_, message)| message.replace('\n', " "))
                .collect(),
            content_max_w: CONTENT_MAX_W,
            success_text: DEFAULT_SUCCESS_MESSAGE,
            message: self.status_message.clone(),
            separate_validate: separate_validate_button(&self.db),
        }
    }

    /// Validate: re-read the entries, run the official checks and mark the
    /// current entries validated.
    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
        self.validation_errors = self.draft.validate();
        self.validated = true;
        cx.notify();
    }

    /// Dev Mode "Simulate BIR confirmation" (debug builds only).
    #[cfg(debug_assertions)]
    fn header_simulate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filename = self.draft.lifecycle.submission_filename.clone();
        match crate::dev_mode::simulate_confirmation(&self.db, filename.as_deref()) {
            Ok(()) => {
                if let Ok(db) = self.db.lock()
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form1702MXDraft>(
                        &self.draft.tin,
                        self.draft.taxable_year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "1702-MX marked Confirmed (simulated, nothing was sent to BIR).".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => self.notify(
                notification::NotificationType::Error,
                format!("Simulated confirmation failed: {error}"),
                window,
                cx,
            ),
        }
        cx.notify();
    }
}

const HEADER_ACTIONS: FormHeaderActions<Form1702MXView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form1702MXView::blocking_errors,
    on_validate: Form1702MXView::header_validate,
    on_submit: Form1702MXView::mark_submitted,
    on_save: Form1702MXView::save_draft,
    on_print: Form1702MXView::preview_pdf,
    on_revert: Form1702MXView::revert_to_draft,
    #[cfg(debug_assertions)]
    on_simulate: Some(Form1702MXView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{
        ATC_MAX, Layout, MCIT_VALUE, MONTH_OPTIONS, amount_keys, atc_options, bound_text_keys,
        format_tin, model_error_keys, parse_percent, parse_peso,
    };
    use gpui::px;

    #[test]
    fn atc_choices_and_months_match_the_official_slots() {
        let options = atc_options();
        assert_eq!(options.len(), 14);
        assert_eq!(options[0].0, MCIT_VALUE);
        assert_eq!(ATC_MAX, 2);
        assert_eq!(MONTH_OPTIONS.len(), 12);
        assert_eq!(model_error_keys("email"), ["email"]);
        assert!(model_error_keys("s2.0.0").is_empty());
    }

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn entries_parse_like_the_official_page() {
        assert_eq!(parse_peso("1,234"), Some(1234));
        assert_eq!(parse_peso("(1,234)"), Some(-1234));
        assert_eq!(parse_peso("12.5"), Some(13));
        assert_eq!(parse_peso(""), Some(0));
        assert_eq!(parse_percent("7.5"), Some(Some(750)));
        assert_eq!(parse_percent("x"), None);
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
        let keys = amount_keys();
        let unique: std::collections::BTreeSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len());
        assert!(!bound_text_keys().is_empty());
    }
}
