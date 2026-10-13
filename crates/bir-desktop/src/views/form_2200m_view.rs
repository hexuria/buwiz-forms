//! Editor for BIR Form 2200M, Excise Tax Return for Mineral Products
//! (v2018). Rust owns every calculation, validation and the official submit
//! plaintext (`bir_core::forms::form_2200m`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2200m::{
    FORM_2200M_FIRST_OTHER_ROW, FORM_2200M_FIXED_ROWS, FORM_2200M_MAX_ROWS, FORM_2200M_OTHER_ATC,
    FORM_2200M_SCHEDULE_ROWS, Form2200MDraft, Form2200MPayment, Form2200MPlace, Form2200MRow,
};
use bir_core::forms::queueable::{QueueableForm, can_user_revert, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::*;
use gpui_rsx::rsx;

use super::event_form_kit as kit;
use crate::components::form_engine::FormViewTrait;
use crate::components::penalty_section::{
    self, PenaltyPartyView, PenaltySection, PenaltySectionEvent,
};
use crate::views::form_controls::{messages_for, with_field_error};
use crate::views::form_header::{
    AMENDED_REVERT_MESSAGE, DEFAULT_SUCCESS_MESSAGE, FormHeaderActions, FormHeaderProps,
    FormHeaderState, render_header_buttons, render_status_box, revert_for_amendment,
    separate_validate_button,
};
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::penalties::PenaltyComponent;
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form2200MView {}

/// Where Items 4 and 5 (TIN, RDO) are fixed: they have no input on this page.
const PROFILE_NOTE: &str = "These come from your taxpayer profile. Edit them in Profile Settings.";

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

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Text inputs: (key, label, placeholder).
const TEXT_INPUTS: &[(&str, &str, &str)] = &[
    ("day", "Item 1 — Day", "DD"),
    ("year", "Item 1 — Year", "YYYY"),
    ("sheets", "Item 3 — No. of sheets attached", "0"),
    ("name", "Item 6 — Taxpayer's Name", ""),
    ("address", "Item 7 — Registered Address", ""),
    ("zip", "Item 7A — Zip Code", ""),
    ("phone", "Item 8 — Contact Number", "digits only"),
    ("email", "Item 9 — Email Address", ""),
    ("relief", "Item 12A — If yes, specify", ""),
    ("other", "Item 15 — Other similar scheme (specify)", ""),
];

/// Part III money inputs: (key, label).
const MONEY_INPUTS: &[(&str, &str)] = &[
    ("bal", "17A — Balance carried over from previous return"),
    ("cred", "17B — Creditable excise tax, if applicable"),
    (
        "prev",
        "19 — Payment on return previously filed (amended only)",
    ),
    ("sur", "21A — Surcharge"),
    ("int", "21B — Interest"),
    ("comp", "21C — Compromise"),
    ("dep", "23A — Tax payment / deposit"),
];

/// Schedule 1 cells: (column key, label). `desc`, `e` and `i` exist on
/// rows 9–10 only.
const ROW_COLUMNS: &[(&str, &str)] = &[
    ("desc", "Description"),
    ("place", "Place of removal"),
    ("a", "A — Volume taxable (MT)"),
    ("b", "B — Volume export (MT)"),
    ("c", "C — Locally extracted value, taxable"),
    ("d", "D — Locally extracted value, export"),
    ("e", "E — Tax rate"),
    ("f", "F — Tax due (locally extracted)"),
    ("g", "G — Imported value, taxable"),
    ("h", "H — Imported value, exempt"),
    ("i", "I — Imported tax rate"),
    ("j", "J — Imported tax due"),
    ("k", "K — Tax due adjustment per final value"),
    ("t", "Total tax due (L) = (F+J+K)"),
];

fn row_key(row: usize, column: &str) -> String {
    format!("r{row}_{column}")
}

fn row_text_column(column: &str) -> bool {
    matches!(column, "desc" | "place")
}

fn column_exists(row: usize, column: &str) -> bool {
    row >= FORM_2200M_FIRST_OTHER_ROW || !matches!(column, "desc" | "e" | "i")
}

fn row_cell(row: &Form2200MRow, column: &str) -> Option<f64> {
    match column {
        "a" => row.volume_taxable,
        "b" => row.volume_exempt,
        "c" => row.local_value_taxable,
        "d" => row.local_value_exempt,
        "e" => row.local_rate,
        "f" => row.local_tax_due,
        "g" => row.imported_taxable,
        "h" => row.imported_exempt,
        "i" => row.imported_rate,
        "j" => row.imported_tax_due,
        "k" => row.adjustment,
        "t" => row.total_tax_due,
        _ => None,
    }
}

fn set_row_cell(row: &mut Form2200MRow, column: &str, value: Option<f64>) {
    let cell = match column {
        "a" => &mut row.volume_taxable,
        "b" => &mut row.volume_exempt,
        "c" => &mut row.local_value_taxable,
        "d" => &mut row.local_value_exempt,
        "e" => &mut row.local_rate,
        "f" => &mut row.local_tax_due,
        "g" => &mut row.imported_taxable,
        "h" => &mut row.imported_exempt,
        "i" => &mut row.imported_rate,
        "j" => &mut row.imported_tax_due,
        "k" => &mut row.adjustment,
        "t" => &mut row.total_tax_due,
        _ => return,
    };
    *cell = value;
}

/// Accepts `1,234.56`, `1234.5`, blank (zero).
fn parse_amount(value: &str) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Some(0.0);
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A Schedule 1 cell: blank stays blank (`None`).
fn parse_cell(value: &str) -> Option<Option<f64>> {
    if value.trim().is_empty() {
        Some(None)
    } else {
        parse_amount(value).map(Some)
    }
}

/// One region, province or city in the official dropdowns.
#[derive(Clone)]
struct GeoOption {
    code: String,
    name: String,
}

impl SelectItem for GeoOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.code
    }

    fn matches(&self, query: &str) -> bool {
        self.name
            .to_ascii_lowercase()
            .contains(&query.trim().to_ascii_lowercase())
    }
}

type GeoSelect = Entity<SelectState<SearchableVec<GeoOption>>>;

fn region_options() -> SearchableVec<GeoOption> {
    SearchableVec::new(
        bir_core::reference::get_all_regions()
            .into_iter()
            .map(|r| GeoOption {
                code: r.code,
                name: r.name,
            })
            .collect::<Vec<_>>(),
    )
}

fn province_options(region: &str) -> SearchableVec<GeoOption> {
    SearchableVec::new(
        bir_core::reference::get_provinces_for_region(region)
            .into_iter()
            .map(|p| GeoOption {
                code: p.code,
                name: p.name,
            })
            .collect::<Vec<_>>(),
    )
}

fn city_options(region: &str, province: &str) -> SearchableVec<GeoOption> {
    SearchableVec::new(
        bir_core::reference::get_cities_for_province(province)
            .into_iter()
            .filter(|c| c.region_code == region)
            .map(|c| GeoOption {
                code: c.code,
                name: c.name,
            })
            .collect::<Vec<_>>(),
    )
}

/// Region, province and city dropdowns for Item 10 or 11.
struct PlaceSelects {
    region: GeoSelect,
    province: GeoSelect,
    city: GeoSelect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaceItem {
    Production,
    Removal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeoLevel {
    Region,
    Province,
    City,
}

const CONTENT_MAX_W: Pixels = px(1180.);

pub struct Form2200MView {
    draft: Form2200MDraft,
    /// Schedule 1 rows on screen: the page's 10 plus every row added with
    /// "Add row" (`addRowSched1` has no limit), never fewer than the draft
    /// holds.
    rows: usize,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    production: PlaceSelects,
    removal: PlaceSelects,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    /// Shared header state (⋯ menu, status box, sent snapshot).
    header: FormHeaderState,
    /// Validate ran at the current entries; every edit clears it.
    validated: bool,
    /// The shared penalty section (computed values, overrides, note).
    penalty: Entity<PenaltySection>,
    _subscriptions: Vec<Subscription>,
}

impl Form2200MView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2200MDraft, key: &str) -> String {
        let money = |value: f64| {
            if value == 0.0 {
                String::new()
            } else {
                official_amount(value)
            }
        };
        match key {
            "day" => format!("{:02}", draft.day),
            "year" => draft.year.to_string(),
            "sheets" => draft.number_of_attached_sheets.to_string(),
            "name" => draft.taxpayer_name.clone(),
            "address" => draft.registered_address.clone(),
            "zip" => draft.zip_code.clone(),
            "phone" => draft.contact_number.clone(),
            "email" => draft.email.clone(),
            "relief" => draft.tax_relief_specify.clone(),
            "other" => draft.other_scheme_description.clone(),
            "bal" => money(draft.balance_carried_over),
            "cred" => money(draft.creditable_excise_tax),
            "prev" => money(draft.previous_payment),
            "sur" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Surcharge),
            "int" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Interest),
            "comp" => penalty_section::override_text(&draft.penalty, PenaltyComponent::Compromise),
            "dep" => money(draft.tax_deposit),
            _ => {
                let Some((row, column)) = key
                    .strip_prefix('r')
                    .and_then(|rest| rest.split_once('_'))
                    .and_then(|(row, column)| row.parse::<usize>().ok().map(|row| (row, column)))
                else {
                    return String::new();
                };
                let entry = draft.row(row);
                match column {
                    "desc" => entry.description,
                    "place" => entry.place_of_removal,
                    _ => row_cell(&entry, column)
                        .map(official_amount)
                        .unwrap_or_default(),
                }
            }
        }
    }

    fn input_text(&self, key: &str, cx: &App) -> String {
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Read every editor value into the draft, then recompute and validate.
    /// A malformed number is reported instead of becoming zero.
    fn sync_from_inputs(&mut self, cx: &mut Context<Self>) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let mut parse_errors = Vec::new();
        let mut draft = self.draft.clone();
        {
            let mut number = |key: &str, label: &str| -> Option<f64> {
                let text = self.input_text(key, cx);
                let parsed = parse_amount(&text);
                if parsed.is_none() {
                    parse_errors.push((
                        key.to_string(),
                        format!("{label}: \"{text}\" is not a number."),
                    ));
                }
                parsed
            };
            if let Some(day) = number("day", "Item 1 day") {
                // An out-of-range day is kept (capped to the field's range)
                // so validate() raises the official valid-date alert.
                draft.day = day.clamp(0.0, f64::from(u8::MAX)) as u8;
            }
            if let Some(sheets) = number("sheets", "Item 3") {
                draft.number_of_attached_sheets = if (0.0..=9999.0).contains(&sheets) {
                    sheets as u16
                } else {
                    9999
                };
            }
            for (key, label) in MONEY_INPUTS {
                if let Some(value) = number(key, label) {
                    match *key {
                        "bal" => draft.balance_carried_over = value,
                        "cred" => draft.creditable_excise_tax = value,
                        "prev" => draft.previous_payment = value,
                        "sur" => draft.surcharge = value,
                        "int" => draft.interest = value,
                        "comp" => draft.compromise = value,
                        _ => draft.tax_deposit = value,
                    }
                }
            }
        }
        for row in 0..self.rows {
            let mut entry = draft.row(row);
            for (column, label) in ROW_COLUMNS {
                if !column_exists(row, column) {
                    continue;
                }
                let key = row_key(row, column);
                let text = self.input_text(&key, cx);
                match *column {
                    "desc" => entry.description = text,
                    "place" => entry.place_of_removal = text,
                    _ => match parse_cell(&text) {
                        Some(value) => set_row_cell(&mut entry, column, value),
                        None => parse_errors.push((
                            key.clone(),
                            format!("Row #{} {label}: \"{text}\" is not a number.", row + 1),
                        )),
                    },
                }
            }
            // An added row stays on the page (and is checked whole) until
            // it is removed.
            if !entry.is_blank() || row < draft.schedule.len() || row >= FORM_2200M_SCHEDULE_ROWS {
                if let Some(slot) = draft.row_mut(row) {
                    *slot = entry;
                }
            }
        }
        draft.taxpayer_name = self.input_text("name", cx);
        draft.registered_address = self.input_text("address", cx);
        draft.zip_code = self.input_text("zip", cx).trim().to_string();
        draft.contact_number = self.input_text("phone", cx).trim().to_string();
        draft.email = self.input_text("email", cx).trim().to_string();
        draft.tax_relief_specify = self.input_text("relief", cx);
        draft.other_scheme_description = self.input_text("other", cx);
        // An empty penalty box means "use computed"; any amount overrides it.
        parse_errors.extend(penalty_section::apply_overrides(
            &self.penalty,
            &Form2200MDraft::penalty_items(),
            &mut draft,
            cx,
        ));
        draft.recompute();
        draft.lifecycle.updated_at = self.draft.lifecycle.updated_at.clone();
        let changed = draft != self.draft;
        if changed {
            draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        }
        if changed || parse_errors != self.parse_errors {
            self.validated = false;
        }
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    /// Input keys and placeholders of one Schedule 1 row.
    fn row_inputs(row: usize) -> Vec<(String, String)> {
        ROW_COLUMNS
            .iter()
            .filter(|(column, _)| column_exists(row, column))
            .map(|(column, _)| {
                let placeholder = if row_text_column(column) { "" } else { "blank" };
                (row_key(row, column), placeholder.to_string())
            })
            .collect()
    }

    fn new_input(
        draft: &Form2200MDraft,
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
                    this.sync_from_inputs(cx);
                }
            },
        );
        (input, subscription)
    }

    /// Add row / Remove (`addRowSched1` / `delRowSched1`): read the page,
    /// change the rows, create editors for new rows, then show every row
    /// again from the draft so nothing shifts into the wrong row.
    fn edit_rows(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Self),
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        self.sync_from_inputs(cx);
        change(self);
        for row in 0..self.rows {
            for (key, placeholder) in Self::row_inputs(row) {
                if self.inputs.contains_key(&key) {
                    continue;
                }
                let (input, subscription) =
                    Self::new_input(&self.draft, &key, &placeholder, window, cx);
                self.inputs.insert(key, input);
                self._subscriptions.push(subscription);
            }
        }
        self.draft.recompute();
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
        // Adding or removing a row is an edit even when the rebuilt draft
        // compares equal to the already-changed one.
        self.validated = false;
    }

    fn add_row(&mut self) {
        if self.rows < FORM_2200M_MAX_ROWS {
            self.rows += 1;
            if let Some(row) = self.draft.row_mut(self.rows - 1) {
                // A fresh added row starts blank, like `rowTemplate`.
                *row = Default::default();
            }
        }
    }

    /// Only added rows can be removed; the page's own rows stay.
    fn remove_row(&mut self, row: usize) {
        if row < FORM_2200M_SCHEDULE_ROWS || row >= self.rows {
            return;
        }
        if row < self.draft.schedule.len() {
            self.draft.schedule.remove(row);
        }
        self.rows -= 1;
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2200MDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        self.validated = false;
        cx.notify();
    }

    fn place_mut(draft: &mut Form2200MDraft, item: PlaceItem) -> &mut Form2200MPlace {
        match item {
            PlaceItem::Production => &mut draft.place_of_production,
            PlaceItem::Removal => &mut draft.place_of_removal,
        }
    }

    fn selects(&self, item: PlaceItem) -> &PlaceSelects {
        match item {
            PlaceItem::Production => &self.production,
            PlaceItem::Removal => &self.removal,
        }
    }

    /// `getProvince` / `getCity`: a new region clears province and city, a
    /// new province clears the city, and the dependent lists reload.
    fn pick_place(
        &mut self,
        item: PlaceItem,
        level: GeoLevel,
        code: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        let place = Self::place_mut(&mut self.draft, item);
        match level {
            GeoLevel::Region => {
                place.region = code;
                place.province.clear();
                place.city.clear();
            }
            GeoLevel::Province => {
                place.province = code;
                place.city.clear();
            }
            GeoLevel::City => place.city = code,
        }
        let place = place.clone();
        let selects = self.selects(item);
        let (province, city) = (selects.province.clone(), selects.city.clone());
        if level == GeoLevel::Region {
            province.update(cx, |state, cx| {
                state.set_items(province_options(&place.region), window, cx);
                state.set_selected_index(None, window, cx);
            });
        }
        if level != GeoLevel::City {
            city.update(cx, |state, cx| {
                state.set_items(city_options(&place.region, &place.province), window, cx);
                state.set_selected_index(None, window, cx);
            });
        }
        self.edit(cx, |_| {});
    }

    fn new_place_selects(
        place: &Form2200MPlace,
        item: PlaceItem,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> PlaceSelects {
        let make = |options: SearchableVec<GeoOption>,
                    selected: &str,
                    window: &mut Window,
                    cx: &mut Context<Self>| {
            let selected = selected.to_string();
            cx.new(|cx| {
                let mut state = SelectState::new(options, None, window, cx).searchable(true);
                if !selected.is_empty() {
                    state.set_selected_value(&selected, window, cx);
                }
                state
            })
        };
        let region = make(region_options(), &place.region, window, cx);
        let province = make(province_options(&place.region), &place.province, window, cx);
        let city = make(
            city_options(&place.region, &place.province),
            &place.city,
            window,
            cx,
        );
        for (select, level) in [
            (&region, GeoLevel::Region),
            (&province, GeoLevel::Province),
            (&city, GeoLevel::City),
        ] {
            subscriptions.push(cx.subscribe_in(
                select,
                window,
                move |this: &mut Self,
                      _,
                      event: &SelectEvent<SearchableVec<GeoOption>>,
                      window,
                      cx| {
                    let SelectEvent::Confirm(value) = event;
                    this.pick_place(item, level, value.clone().unwrap_or_default(), window, cx);
                },
            ));
        }
        PlaceSelects {
            region,
            province,
            city,
        }
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

    /// Cancel a Queued return (claimed: ask to release the claim first).
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200MDraft>(
                        &queued.tin,
                        queued.year,
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

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form2200MDraft>(
                    &draft.tin,
                    draft.year,
                    period_column(&draft.filing_period()),
                    ABANDONED_CLAIM_RELEASE_REASON,
                )
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.release_claim_confirm_open = false;
        self.validated = false;
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

    fn has_error(&self, key: &str) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| field == key || field.starts_with(&format!("{key}[")))
    }

    fn label(&self, text: &str, error: bool) -> Div {
        let label = div()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(text.to_string());
        if error {
            label.text_color(gpui::red())
        } else {
            label
        }
    }

    /// A labelled control. On phones the label sits above it.
    fn labelled(&self, label: &str, error: bool, body: AnyElement, layout: Layout) -> AnyElement {
        let label = self.label(label, error);
        match layout {
            Layout::Phone => div()
                .flex()
                .flex_col()
                .gap_1()
                .w_full()
                .child(label)
                .child(div().w_full().child(body)),
            _ => div()
                .flex()
                .items_center()
                .gap_4()
                .w_full()
                .child(label.w(relative(0.45)))
                .child(div().w(relative(0.55)).child(body)),
        }
        .into_any_element()
    }

    /// Validator keys (`Form2200MDraft::validate`) behind input `key`.
    fn model_keys(key: &str) -> &'static [&'static str] {
        match key {
            "day" => &["day"],
            "year" => &["year"],
            "sheets" => &["number_of_attached_sheets"],
            "name" => &["taxpayer_name"],
            "address" => &["registered_address"],
            "zip" => &["zip_code"],
            "phone" => &["contact_number"],
            "email" => &["email"],
            "relief" => &["tax_relief_specify"],
            "other" => &["other_scheme_description"],
            "bal" => &["balance_carried_over"],
            "cred" => &["creditable_excise_tax"],
            "prev" => &["previous_payment"],
            "sur" => &["surcharge"],
            "int" => &["interest"],
            "comp" => &["compromise"],
            "dep" => &["tax_deposit"],
            _ => &[],
        }
    }

    /// Messages for any of `keys`, from malformed input and from validation.
    fn errors_for(&self, keys: &[&str]) -> Option<String> {
        messages_for(
            self.parse_errors
                .iter()
                .chain(self.validation_errors.iter()),
            keys,
        )
    }

    /// Put the messages for `keys` directly under an already-built control.
    fn with_errors(&self, content: AnyElement, keys: &[&str]) -> AnyElement {
        with_field_error(content, self.errors_for(keys))
    }

    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        let mut keys = vec![key];
        keys.extend_from_slice(Self::model_keys(key));
        let message = self.errors_for(&keys);
        let row = self.labelled(
            label,
            message.is_some(),
            Input::new(input).disabled(disabled).into_any_element(),
            layout,
        );
        with_field_error(row, message)
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

    fn computed(&self, label: &str, value: f64, cx: &Context<Self>) -> AnyElement {
        rsx! {
            <div flex flex_wrap items_center justify_between gap_2 p_2 bg={cx.theme().muted.opacity(0.5)} rounded_md>
                <div min_w_0 flex_1 text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div flex_none text_right font_weight={FontWeight::BOLD}>{official_amount(value)}</div>
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
        button.disabled(disabled).min_w_0().max_w_full()
    }

    /// Phone stacks the label above a full-width wrapping chip row; other
    /// layouts keep one wrapping row with the label taking the free space.
    fn choice_row(
        &self,
        label: &str,
        error: bool,
        layout: Layout,
        buttons: Vec<Button>,
    ) -> AnyElement {
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .w_full()
                .child(self.label(label, error).max_w_full())
                .child(div().flex().flex_wrap().gap_2().w_full().children(buttons))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(self.label(label, error).flex_1().max_w_full())
                .children(buttons)
                .into_any_element()
        }
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let read_only_row = |label: &str, value: String, key: &str| {
            let shown = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.label(label, self.errors_for(&[key]).is_some()))
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element();
            self.with_errors(shown, &[key])
        };
        let month_name = MONTHS
            .get((self.draft.month as usize).wrapping_sub(1))
            .copied()
            .unwrap_or("-");
        let amended = self.draft.is_amended;
        let children = vec![
            read_only_row("Item 1 — Month", month_name.to_string(), "month"),
            self.field("day", "Item 1 — Day", layout, !editable),
            read_only_row("Item 1 — Year", self.draft.year.to_string(), "year"),
            self.choice_row(
                "Item 2 — Amended return?",
                false,
                layout,
                vec![
                    Self::choice("2200m_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("2200m_amended_no", "No", !amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = false)),
                    ),
                ],
            ),
            self.field(
                "sheets",
                "Item 3 — No. of sheets attached",
                layout,
                !editable,
            ),
        ];
        self.section("Return period", vec![self.grid(layout, children)], cx)
    }

    fn place_block(&self, item: PlaceItem, title: &str, layout: Layout) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let selects = self.selects(item);
        let key = match item {
            PlaceItem::Production => "place_of_production",
            PlaceItem::Removal => "place_of_removal",
        };
        let error = self.has_error(key);
        let message = self.errors_for(&[key]);
        let select = |state: &GeoSelect, placeholder: &'static str| {
            Select::new(state)
                .placeholder(placeholder)
                .search_placeholder("Search")
                .disabled(!editable)
                .into_any_element()
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .child(select(&selects.region, "(Select Region)"))
            .child(select(&selects.province, "(Select Province)"))
            .child(select(&selects.city, "(Select City)"))
            .into_any_element();
        with_field_error(self.labelled(title, error, body, layout), message)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let static_value = |label: &str, value: String, key: &str| {
            let shown = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.label(label, self.errors_for(&[key]).is_some()))
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element();
            self.with_errors(shown, &[key])
        };
        let mut fields = vec![
            static_value("Item 4 — TIN", format_tin(&self.draft.tin), "tin"),
            static_value("Item 5 — RDO Code", self.draft.rdo_code.clone(), "rdo_code"),
        ];
        if self.errors_for(&["tin", "rdo_code"]).is_some() {
            fields.push(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(PROFILE_NOTE)
                    .into_any_element(),
            );
        }
        for (key, label, _) in TEXT_INPUTS.iter().skip(3).take(5) {
            fields.push(self.field(key, label, layout, !editable));
        }
        fields.push(self.place_block(
            PlaceItem::Production,
            "Item 10 — Place of Production",
            layout,
        ));
        fields.push(self.place_block(PlaceItem::Removal, "Item 11 — Place of Removal", layout));
        let relief = self.draft.tax_relief;
        let relief_row = self.with_errors(
            self.choice_row(
                "Item 12 — Availing of tax relief under a Special Law or International Tax Treaty?",
                self.has_error("tax_relief"),
                layout,
                vec![
                    Self::choice("2200m_relief_yes", "Yes", relief == Some(true), !editable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_relief = Some(true))
                        })),
                    Self::choice("2200m_relief_no", "No", relief == Some(false), !editable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit(cx, |d| d.tax_relief = Some(false))
                        })),
                ],
            ),
            &["tax_relief"],
        );
        let mut children = vec![self.grid(layout, fields), relief_row];
        if relief == Some(true) {
            children.push(self.field("relief", "Item 12A — If yes, specify", layout, !editable));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mop = self.draft.manner_of_payment;
        let pick = |payment: Form2200MPayment| {
            cx.listener(move |this: &mut Self, _, _, cx| {
                this.edit(cx, |d| d.manner_of_payment = payment)
            })
        };
        let manner = self.choice_row(
            "Manner of payment",
            self.has_error("manner_of_payment"),
            layout,
            vec![
                Self::choice(
                    "2200m_mop_actual",
                    "13 — Payment on actual removal",
                    mop == Form2200MPayment::ActualRemoval,
                    !editable,
                )
                .on_click(pick(Form2200MPayment::ActualRemoval)),
                Self::choice(
                    "2200m_mop_prepay",
                    "14 — Prepayment / advance deposit",
                    mop == Form2200MPayment::Prepayment,
                    !editable,
                )
                .on_click(pick(Form2200MPayment::Prepayment)),
                // The official page enables Item 15 only after Item 14.
                Self::choice(
                    "2200m_mop_other",
                    "15 — Other similar schemes",
                    mop == Form2200MPayment::OtherScheme,
                    !editable
                        || !matches!(
                            mop,
                            Form2200MPayment::Prepayment | Form2200MPayment::OtherScheme
                        ),
                )
                .on_click(pick(Form2200MPayment::OtherScheme)),
            ],
        );
        let mut children = vec![self.with_errors(manner, &["manner_of_payment"])];
        if mop == Form2200MPayment::OtherScheme {
            children.push(self.field(
                "other",
                "Other Similar Schemes (specify)",
                layout,
                !editable,
            ));
        }
        self.section("Part II — Manner of Payment", children, cx)
    }

    fn render_row(&self, row: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let (atc, description, rate) = match FORM_2200M_FIXED_ROWS.get(row) {
            Some((atc, description, rate)) => (*atc, description.to_string(), Some(*rate)),
            None => (
                FORM_2200M_OTHER_ATC,
                "Other Metallic Minerals (specify)".to_string(),
                None,
            ),
        };
        let caption = format!("Row #{} — {atc}", row + 1);
        let title = if row >= FORM_2200M_SCHEDULE_ROWS {
            let remove = Button::new(("2200m_row_remove", row))
                .disabled(!editable)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_rows(window, cx, |v| v.remove_row(row))
                }));
            kit::row_heading(&caption, remove)
        } else {
            div()
                .font_weight(FontWeight::BOLD)
                .child(caption)
                .into_any_element()
        };
        let heading = div()
            .flex()
            .flex_col()
            .child(title)
            .child(div().min_w_0().text_sm().child(description))
            .when_some(rate, |d, rate| {
                d.child(div().min_w_0().text_sm().child(format!("Rate: {rate}")))
            });
        let fields: Vec<AnyElement> = ROW_COLUMNS
            .iter()
            .filter(|(column, _)| column_exists(row, column))
            .map(|(column, label)| {
                let key = row_key(row, column);
                let cell_key = match *column {
                    "desc" => format!("schedule[{row}].description"),
                    "place" => format!("schedule[{row}].place_of_removal"),
                    _ => String::new(),
                };
                let message = self.errors_for(&[key.as_str(), cell_key.as_str()]);
                let input = self
                    .inputs
                    .get(&key)
                    .expect("editor input registry is complete");
                let cell = self.labelled(
                    label,
                    message.is_some(),
                    Input::new(input).disabled(!editable).into_any_element(),
                    layout,
                );
                with_field_error(cell, message)
            })
            .collect();
        let row_error = format!("schedule[{row}]");
        let card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading)
            .child(self.grid(layout, fields))
            .into_any_element();
        self.with_errors(card, &[row_error.as_str()])
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let mut children: Vec<AnyElement> = vec![
            div()
                .text_sm()
                .child(
                    "Leave unused rows blank. A used row needs its place of removal and at least one more column; rows 9–10 also need a description.",
                )
                .into_any_element(),
        ];
        for row in 0..self.rows {
            children.push(self.render_row(row, layout, cx));
        }
        let editable = self.draft.lifecycle.is_editable();
        children.push(
            kit::add_row_button(
                "2200m_row_add",
                !editable || self.rows >= FORM_2200M_MAX_ROWS,
            )
            .on_click(cx.listener(|this, _, window, cx| this.edit_rows(window, cx, Self::add_row)))
            .into_any_element(),
        );
        children.push(self.with_errors(
            self.computed(
                "TOTAL EXCISE TAX DUE (Part III, Item 16)",
                self.draft.schedule_total,
                cx,
            ),
            &["schedule"],
        ));
        self.section(
            "Part V — Schedule 1: Removals and Excise Tax Due",
            children,
            cx,
        )
    }

    fn render_part_three(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let d = &self.draft;
        let children = vec![
            self.computed(
                "16 — Excise tax due (from Schedule 1)",
                d.excise_tax_due,
                cx,
            ),
            self.field("bal", MONEY_INPUTS[0].1, layout, !editable),
            self.field("cred", MONEY_INPUTS[1].1, layout, !editable),
            self.computed("17C — Total (17A + 17B)", d.total_credits, cx),
            self.computed("18 — Net tax due / (overpayment)", d.net_tax_due, cx),
            self.field(
                "prev",
                MONEY_INPUTS[2].1,
                layout,
                !editable || !d.is_amended,
            ),
            self.computed("20 — Tax still due / (overpayment)", d.tax_still_due, cx),
            penalty_section::render(
                &self.penalty,
                &PenaltyPartyView::all(&self.draft),
                &self
                    .validation_errors
                    .iter()
                    .chain(&self.parse_errors)
                    .cloned()
                    .collect::<Vec<_>>(),
                cx,
            ),
            self.computed("21D — Total penalties", d.total_penalties, cx),
            self.computed("22 — Amount Payable/(Overpayment)", d.amount_payable, cx),
            self.field("dep", MONEY_INPUTS[6].1, layout, !editable),
            self.computed("23B — Penalties", d.penalties_paid, cx),
            self.computed("23C — Total payment made today", d.total_payment, cx),
            self.with_errors(
                self.computed(
                    "24 — Balance to be carried over to next return",
                    d.balance_to_carry_over,
                    cx,
                ),
                &["balance_to_carry_over"],
            ),
        ];
        self.section(
            "Part III — Payments and Application",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2200MView {
    type Draft = Form2200MDraft;

    const PRINT_SLUG: &'static str = "2200m-2018";

    fn print_fields(draft: &Form2200MDraft) -> std::collections::BTreeMap<String, String> {
        draft.to_print_field_map()
    }

    fn agent_draft(&self) -> &Form2200MDraft {
        &self.draft
    }

    fn header_actions() -> crate::views::form_header::FormHeaderActions<Self> {
        HEADER_ACTIONS
    }

    fn new(
        draft: Form2200MDraft,
        db: Arc<Mutex<Database>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut subscriptions = Vec::new();
        let mut keys: Vec<(String, String)> = TEXT_INPUTS
            .iter()
            .map(|(key, _, placeholder)| (key.to_string(), placeholder.to_string()))
            .collect();
        for (key, _) in MONEY_INPUTS {
            keys.push((key.to_string(), "0.00".into()));
        }
        let rows = draft.row_count();
        for row in 0..rows {
            keys.extend(Self::row_inputs(row));
        }
        for (key, placeholder) in keys {
            let (input, subscription) = Self::new_input(&draft, &key, &placeholder, window, cx);
            subscriptions.push(subscription);
            inputs.insert(key, input);
        }
        let production = Self::new_place_selects(
            &draft.place_of_production,
            PlaceItem::Production,
            window,
            cx,
            &mut subscriptions,
        );
        let removal = Self::new_place_selects(
            &draft.place_of_removal,
            PlaceItem::Removal,
            window,
            cx,
            &mut subscriptions,
        );
        let validation_errors = draft.validate();
        // The penalty boxes are overrides (empty means "use computed").
        let penalty_views = PenaltyPartyView::all(&draft);
        let penalty_inputs = vec![[
            inputs["sur"].clone(),
            inputs["int"].clone(),
            inputs["comp"].clone(),
        ]];
        let penalty = cx.new(|cx| PenaltySection::wrap(&penalty_views, penalty_inputs, window, cx));
        subscriptions.push(cx.subscribe_in(
            &penalty,
            window,
            |this: &mut Self, _, _: &PenaltySectionEvent, _, cx| {
                this.validated = false;
                this.sync_from_inputs(cx);
            },
        ));
        Self {
            draft,
            rows,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            production,
            removal,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            header: FormHeaderState::new(),
            validated: false,
            penalty,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2200MDraft {
        Form2200MDraft::new_from_profile(profile, year, period)
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

impl FormViewTrait for Form2200MView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2200-M"
    }
    fn form_subtitle(&self) -> &'static str {
        "Excise Tax Return for Mineral Products"
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
        if !self.parse_errors.is_empty() {
            self.notify(
                notification::NotificationType::Error,
                "Fix the highlighted numbers before saving.".into(),
                window,
                cx,
            );
            return;
        }
        let result = match self.db.lock() {
            Ok(db) => db
                .save_queueable_draft(&self.draft)
                .map_err(|e| e.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(id) => {
                self.draft.id = Some(id);
                self.notify(
                    notification::NotificationType::Success,
                    "2200M draft saved.".into(),
                    window,
                    cx,
                );
                cx.emit(QueueableFormEvent::Saved);
            }
            Err(error) => cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Save failed".into(),
                error,
            )),
        }
    }

    fn mark_submitted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !can_queue_for_submission(Form2200MDraft::FORM_CODE) {
            self.status_message = Some(queue_unavailable());
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
                "Form 2200M",
                &error,
            ));
            cx.notify();
            return;
        }
        self.status_message = None;
        self.notify(
            notification::NotificationType::Success,
            "Form 2200M queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2200M payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    /// D7A + D8A: a claimed Queued return keeps the release-claim
    /// confirmation; everything else goes through
    /// `Database::revert_queueable_for_amendment`.
    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !can_user_revert(&self.draft.lifecycle.status) {
            return;
        }
        if matches!(self.draft.lifecycle.status, FilingStatus::Queued)
            && !self.draft.lifecycle.is_unclaimed()
        {
            self.cancel_queue(window, cx);
            return;
        }
        match revert_for_amendment(&self.db, &self.draft) {
            Ok(reverted) => {
                self.draft = reverted.draft;
                self.validated = false;
                self.parse_errors.clear();
                self.validation_errors = self.draft.validate();
                self.status_message = reverted
                    .snapshot_id
                    .map(|_| AMENDED_REVERT_MESSAGE.to_string());
                if reverted.snapshot_id.is_some() {
                    self.header.load_sent_snapshot(&self.db, &self.draft);
                }
                self.notify(
                    notification::NotificationType::Success,
                    "Form 2200-M reverted to Draft.".into(),
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
        // Reload the editors from the reverted draft.
        let keys: Vec<String> = self.inputs.keys().cloned().collect();
        for key in keys {
            let value = Self::initial(&self.draft, &key);
            if let Some(input) = self.inputs.get(&key) {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
        cx.notify();
    }

    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.sync_from_inputs(cx);
        if !self.parse_errors.is_empty() {
            cx.emit(QueueableFormEvent::PushNotification(
                "error".into(),
                "Print preview failed".into(),
                "Fix the highlighted numbers first. No filing state was changed.".into(),
            ));
            return;
        }
        let fields = Self::print_fields(&self.draft);
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            Self::PRINT_SLUG,
            &fields,
            "2200M — Print Preview",
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

/// Why Queue for submission is off: the registry leaves 2200-M unqueueable
/// (the official PROD SFTP map has no 2200Mv2018 folder).
fn queue_unavailable() -> String {
    super::form_header::queue_unavailable_message("2200-M")
}

impl Render for Form2200MView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };

        let phone = layout == Layout::Phone;
        self.header.ensure_sent_snapshot(&self.db, &self.draft);
        let props = self.header_props();
        let header_buttons = render_header_buttons(self, &props, HEADER_ACTIONS, phone, cx);
        let status_box = render_status_box(self, &props, HEADER_ACTIONS, cx);

        let toolbar =
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px(pad)
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Button::new("2200m_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(header_buttons);

        let mut body = div()
            .w_full()
            .max_w(CONTENT_MAX_W)
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("2200m_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2200m_release_cancel")
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
            .child(self.render_part_three(layout, cx))
            .child(self.render_schedule(layout, cx));
        // The printed 2200M sheet says "has been made" (not "have").
        body = body.children(
            crate::views::event_form_kit::declaration_and_payment_with_text(
                layout == Layout::Phone,
                "I/We declare under the penalties of perjury that this return, and all its attachments, has been made in good faith, verified by me/us, and to the best of my/our knowledge and belief, is true and correct, pursuant to the provisions of the National Internal Revenue Code, as amended, and the regulations issued under authority thereof. Further, I/we give my/our consent to the processing of my/our information as contemplated under the *Data Privacy Act of 2012 (R.A. No. 10173) for legitimate and lawful purposes.",
                "Part IV – Details of Payment",
                crate::views::event_form_kit::PAYMENT_ROWS_2200,
                crate::views::event_form_kit::PAYMENT_COLUMNS_2200,
                cx,
            ),
        );
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
                    .when(!phone, |d| {
                        d.child(div().mt_4().child(self.render_status_pipeline(cx)))
                    })
                    // Phone has no stepper; the box sits under the titles.
                    .children(status_box.map(|b| div().mt_3().child(b))),
            )
            .child(
                div()
                    .id("2200m_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

impl Form2200MView {
    /// What the shared header shows. 2200-M can't be queued (issue #87):
    /// the header hides Submit for it, and a Draft with no open items shows
    /// why in the status box.
    fn header_props(&self) -> FormHeaderProps<'static> {
        let blocking_errors = self.blocking_errors();
        let unavailable = matches!(self.draft.lifecycle.status, FilingStatus::Draft)
            && blocking_errors == 0
            && !can_queue_for_submission(Form2200MDraft::FORM_CODE);
        FormHeaderProps {
            slug: "2200m",
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
            message: self
                .status_message
                .clone()
                .or_else(|| unavailable.then(queue_unavailable)),
            separate_validate: separate_validate_button(&self.db),
        }
    }

    fn blocking_errors(&self) -> usize {
        self.parse_errors.len() + self.validation_errors.len()
    }

    /// The header's Validate: read the editors, run the official checks
    /// and record that they ran at the current entries.
    fn header_validate(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = None;
        self.sync_from_inputs(cx);
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200MDraft>(
                        &self.draft.tin,
                        self.draft.year,
                        period_column(&self.draft.filing_period()),
                    )
                {
                    self.draft = current;
                }
                self.notify(
                    notification::NotificationType::Success,
                    "2200-M marked Confirmed (simulated, nothing was sent to BIR).".into(),
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

const HEADER_ACTIONS: FormHeaderActions<Form2200MView> = FormHeaderActions {
    state: |this| &mut this.header,
    state_ref: |this| &this.header,
    blocking_errors: Form2200MView::blocking_errors,
    on_validate: Form2200MView::header_validate,
    on_submit: |this, window, cx| this.mark_submitted(window, cx),
    on_save: |this, window, cx| this.save_draft(window, cx),
    on_print: |this, window, cx| this.preview_pdf(window, cx),
    on_revert: |this, window, cx| this.revert_to_draft(window, cx),
    #[cfg(debug_assertions)]
    on_simulate: Some(Form2200MView::header_simulate),
    #[cfg(not(debug_assertions))]
    on_simulate: None,
};

#[cfg(test)]
mod tests {
    use super::{Layout, column_exists, format_tin, parse_amount, parse_cell};
    use gpui::px;

    #[test]
    fn header_uses_shared_buttons_and_status_box() {
        let src = include_str!("form_2200m_view.rs");
        assert!(src.contains(concat!("render_header_", "buttons(")));
        assert!(src.contains(concat!("render_status_", "box(")));
        assert!(src.contains(concat!("revert_for_", "amendment(")));
        assert!(src.contains("unavailable.then(queue_unavailable)"));
        assert!(!src.contains(concat!("Queued for background ", "submission")));
        assert!(!src.contains(concat!("\"Queue for ", "submission\"")));
    }

    #[test]
    fn layout_breakpoints() {
        assert!(Layout::for_width(px(390.)) == Layout::Phone);
        assert!(Layout::for_width(px(820.)) == Layout::Tablet);
        assert!(Layout::for_width(px(1280.)) == Layout::Desktop);
    }

    #[test]
    fn amounts_accept_official_formatting() {
        assert_eq!(parse_amount("1,234.50"), Some(1234.5));
        assert_eq!(parse_amount(""), Some(0.0));
        assert_eq!(parse_amount("12a"), None);
        assert_eq!(parse_cell(" "), Some(None));
        assert_eq!(parse_cell("0"), Some(Some(0.0)));
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }

    #[test]
    fn rates_and_descriptions_only_on_the_other_rows() {
        assert!(!column_exists(0, "desc"));
        assert!(!column_exists(7, "e"));
        assert!(column_exists(8, "desc"));
        assert!(column_exists(9, "i"));
        assert!(column_exists(0, "f"));
    }
}
