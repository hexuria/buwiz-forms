//! Editor for BIR Form 2200A, Excise Tax Return for Alcohol Products
//! (January 2020). Rust owns every calculation, validation and the official submit
//! plaintext (`bir_core::forms::form_2200a`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2200a::{
    FORM_2200A_FIXED_ROW_COUNT, FORM_2200A_FIXED_ROWS, FORM_2200A_OTHER_ROWS, Form2200AAmounts,
    Form2200ADraft, Form2200APayment, Form2200APlace,
};
use bir_core::forms::queueable::{QueueableForm, period_column};
use bir_core::forms::{FilingStatus, can_queue_for_submission};
use bir_core::official_xml::official_amount;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::components::form_engine::FormViewTrait;
use crate::views::queueable_forms::{QueueableFormEvent, QueueableFormView};
use bir_core::profile::TaxpayerProfile;

impl EventEmitter<QueueableFormEvent> for Form2200AView {}

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

/// Schedule 1 amount columns: (column key, label).
const ROW_COLUMNS: &[(&str, &str)] = &[
    ("x", "Export/exempt base"),
    ("t", "Taxable base"),
    ("d", "Basic excise tax due"),
];

/// "Others" text columns: (column key, label).
const OTHER_TEXT_COLUMNS: &[(&str, &str)] = &[
    ("atc", "ATC (3 digits after XA)"),
    ("desc", "Description"),
    ("bracket", "Tax bracket / unit of measure"),
    ("rate", "Applicable tax rate"),
];

fn row_key(row: usize, column: &str) -> String {
    format!("r{row}_{column}")
}

fn other_key(row: usize, column: &str) -> String {
    format!("o{row}_{column}")
}

fn amount_cell(amounts: &Form2200AAmounts, column: &str) -> f64 {
    match column {
        "x" => amounts.export_exempt,
        "t" => amounts.taxable,
        _ => amounts.tax_due,
    }
}

fn set_amount_cell(amounts: &mut Form2200AAmounts, column: &str, value: f64) {
    match column {
        "x" => amounts.export_exempt = value,
        "t" => amounts.taxable = value,
        _ => amounts.tax_due = value,
    }
}

/// Fixed rows grouped by printed product (ATC + description), in page order.
fn row_groups() -> Vec<(usize, usize)> {
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for (index, row) in FORM_2200A_FIXED_ROWS.iter().enumerate() {
        match groups.last_mut() {
            Some((start, end))
                if FORM_2200A_FIXED_ROWS[*start].1 == row.1
                    && FORM_2200A_FIXED_ROWS[*start].2 == row.2 =>
            {
                *end = index + 1
            }
            _ => groups.push((index, index + 1)),
        }
    }
    groups
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

pub struct Form2200AView {
    draft: Form2200ADraft,
    db: Arc<Mutex<Database>>,
    scroll_handle: ScrollHandle,
    inputs: BTreeMap<String, Entity<InputState>>,
    production: PlaceSelects,
    removal: PlaceSelects,
    validation_errors: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    status_message: Option<String>,
    release_claim_confirm_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Form2200AView {
    /// Editor text for one input from the draft.
    fn initial(draft: &Form2200ADraft, key: &str) -> String {
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
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
            "dep" => money(draft.tax_deposit),
            _ => {
                let parse = |prefix: char| {
                    key.strip_prefix(prefix)
                        .and_then(|rest| rest.split_once('_'))
                        .and_then(|(row, column)| {
                            row.parse::<usize>().ok().map(|row| (row, column))
                        })
                };
                if let Some((row, column)) = parse('r') {
                    money(amount_cell(&draft.row(row), column))
                } else if let Some((row, column)) = parse('o') {
                    let other = draft.other(row);
                    match column {
                        "atc" => other.atc_digits,
                        "desc" => other.description,
                        "bracket" => other.bracket,
                        "rate" => other.rate,
                        _ => money(amount_cell(&other.amounts, column)),
                    }
                } else {
                    String::new()
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
            draft.day = if (0.0..=31.0).contains(&day) {
                day as u8
            } else {
                0
            };
        }
        if let Some(year) = number("year", "Item 1 year") {
            draft.year = if (0.0..=9999.0).contains(&year) {
                year as u16
            } else {
                0
            };
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
        for (row, (_, atc, _, bracket, _, _)) in FORM_2200A_FIXED_ROWS.iter().enumerate() {
            let mut amounts = draft.row(row);
            // The tax due of a fixed row is computed (rate × taxable).
            for (column, label) in ROW_COLUMNS.iter().take(2) {
                let label = format!("{atc} {bracket} {label}");
                if let Some(value) = number(&row_key(row, column), &label) {
                    set_amount_cell(&mut amounts, column, value);
                }
            }
            if amounts != Form2200AAmounts::default() || row < draft.schedule.len() {
                if let Some(slot) = draft.row_mut(row) {
                    *slot = amounts;
                }
            }
        }
        for row in 0..FORM_2200A_OTHER_ROWS {
            let mut other = draft.other(row);
            for (column, label) in ROW_COLUMNS {
                let label = format!("Others row {} {label}", row + 1);
                if let Some(value) = number(&other_key(row, column), &label) {
                    set_amount_cell(&mut other.amounts, column, value);
                }
            }
            other.atc_digits = self.input_text(&other_key(row, "atc"), cx);
            other.description = self.input_text(&other_key(row, "desc"), cx);
            other.bracket = self.input_text(&other_key(row, "bracket"), cx);
            other.rate = self.input_text(&other_key(row, "rate"), cx);
            if !other.is_blank() || row < draft.others.len() {
                if let Some(slot) = draft.other_mut(row) {
                    *slot = other;
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
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2200ADraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
        cx.notify();
    }

    fn place_mut(draft: &mut Form2200ADraft, item: PlaceItem) -> &mut Form2200APlace {
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
        place: &Form2200APlace,
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

    fn release_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = &self.draft;
        let result = match self.db.lock() {
            Ok(db) => db
                .release_abandoned_claimed_queueable::<Form2200ADraft>(
                    &draft.tin,
                    draft.year,
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

    fn has_error(&self, key: &str) -> bool {
        self.validation_errors
            .iter()
            .chain(self.parse_errors.iter())
            .any(|(field, _)| field == key || field.starts_with(&format!("{key}[")))
    }

    fn label(&self, text: &str, error: bool) -> Div {
        let label = div()
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

    fn field(&self, key: &str, label: &str, layout: Layout, disabled: bool) -> AnyElement {
        let input = self
            .inputs
            .get(key)
            .expect("editor input registry is complete");
        self.labelled(
            label,
            self.has_error(key),
            Input::new(input).disabled(disabled).into_any_element(),
            layout,
        )
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
                <div text_sm font_weight={FontWeight::MEDIUM}>{label.to_string()}</div>
                <div text_right font_weight={FontWeight::BOLD}>{official_amount(value)}</div>
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
        button.disabled(disabled)
    }

    fn choice_row(&self, label: &str, error: bool, buttons: Vec<Button>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(self.label(label, error))
            .children(buttons)
            .into_any_element()
    }

    fn render_period(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut months = div().flex().flex_wrap().gap_1();
        for (index, name) in MONTHS.iter().enumerate() {
            let month = index as u8 + 1;
            months = months.child(
                Self::choice(
                    ("2200a_month", index),
                    *name,
                    self.draft.month == month,
                    !editable,
                )
                .small()
                .on_click(cx.listener(move |this, _, _, cx| this.edit(cx, |d| d.month = month))),
            );
        }
        let amended = self.draft.is_amended;
        let children = vec![
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.label("Item 1 — Month", self.has_error("month")))
                .child(months)
                .into_any_element(),
            self.field("day", "Item 1 — Day", layout, !editable),
            self.field("year", "Item 1 — Year", layout, !editable),
            self.choice_row(
                "Item 2 — Amended return?",
                false,
                vec![
                    Self::choice("2200a_amended_yes", "Yes", amended, !editable).on_click(
                        cx.listener(|this, _, _, cx| this.edit(cx, |d| d.is_amended = true)),
                    ),
                    Self::choice("2200a_amended_no", "No", !amended, !editable).on_click(
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
        self.labelled(title, error, body, layout)
    }

    fn render_part_one(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let static_value = |label: &str, value: String| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.label(label, false))
                .child(div().font_weight(FontWeight::BOLD).child(value))
                .into_any_element()
        };
        let mut fields = vec![
            static_value("Item 4 — TIN", format_tin(&self.draft.tin)),
            static_value("Item 5 — RDO Code", self.draft.rdo_code.clone()),
        ];
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
        let relief_row = self.choice_row(
            "Item 12 — Availing of tax relief under a Special Law/International Tax Treaty?",
            false,
            vec![
                Self::choice("2200a_relief_yes", "Yes", relief, !editable)
                    .on_click(cx.listener(|this, _, _, cx| this.edit(cx, |d| d.tax_relief = true))),
                Self::choice("2200a_relief_no", "No", !relief, !editable).on_click(
                    cx.listener(|this, _, _, cx| this.edit(cx, |d| d.tax_relief = false)),
                ),
            ],
        );
        let mut children = vec![self.grid(layout, fields), relief_row];
        if relief {
            children.push(self.field("relief", "Item 12A — If yes, specify", layout, !editable));
        }
        self.section("Part I — Background Information", children, cx)
    }

    fn render_part_two(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mop = self.draft.manner_of_payment;
        let pick = |payment: Form2200APayment| {
            cx.listener(move |this: &mut Self, _, _, cx| {
                this.edit(cx, |d| d.manner_of_payment = payment)
            })
        };
        let mut children = vec![self.choice_row(
            "Manner of payment",
            self.has_error("manner_of_payment"),
            vec![
                Self::choice(
                    "2200a_mop_actual",
                    "13 — Payment on actual removal",
                    mop == Form2200APayment::ActualRemoval,
                    !editable,
                )
                .on_click(pick(Form2200APayment::ActualRemoval)),
                Self::choice(
                    "2200a_mop_prepay",
                    "14 — Prepayment / advance deposit",
                    mop == Form2200APayment::Prepayment,
                    !editable,
                )
                .on_click(pick(Form2200APayment::Prepayment)),
                Self::choice(
                    "2200a_mop_other",
                    "15 — Other similar schemes",
                    mop == Form2200APayment::OtherScheme,
                    // The official page enables Item 15 only after Item 14.
                    !editable
                        || !matches!(
                            mop,
                            Form2200APayment::Prepayment | Form2200APayment::OtherScheme
                        ),
                )
                .on_click(pick(Form2200APayment::OtherScheme)),
            ],
        )];
        if mop == Form2200APayment::OtherScheme {
            children.push(self.field("other", "Item 15 — Specify the scheme", layout, !editable));
        }
        self.section("Part II — Manner of Payment", children, cx)
    }

    /// One schedule line: a caption, its amount inputs and, for a fixed
    /// row, the computed tax due.
    fn amount_line(
        &self,
        caption: String,
        keys: &[String],
        computed_due: Option<f64>,
        error: bool,
        layout: Layout,
    ) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut cells: Vec<AnyElement> = keys
            .iter()
            .zip(ROW_COLUMNS)
            .map(|(key, (_, label))| {
                let input = self
                    .inputs
                    .get(key)
                    .expect("editor input registry is complete");
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .child(self.label(label, error || self.has_error(key)))
                    .child(Input::new(input).disabled(!editable))
                    .into_any_element()
            })
            .collect();
        if let Some(due) = computed_due {
            cells.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .child(self.label(ROW_COLUMNS[2].1, error))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(official_amount(due)),
                    )
                    .into_any_element(),
            );
        }
        let caption = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .child(caption);
        if layout == Layout::Phone {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(caption)
                .children(cells)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption)
                .child(div().flex().gap_3().w_full().children(cells))
                .into_any_element()
        }
    }

    fn render_schedule(&self, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let mut children: Vec<AnyElement> = vec![
            div()
                .text_sm()
                .child("Enter the tax bases for each product and period removed; the tax due is the applicable rate times the taxable base. Leave the rest blank.")
                .into_any_element(),
        ];
        for (start, end) in row_groups() {
            let (_, atc, product, _, _, _) = FORM_2200A_FIXED_ROWS[start];
            let mut card = div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().font_weight(FontWeight::BOLD).child(atc.to_string()))
                        .child(div().text_sm().child(product.to_string())),
                );
            for (row, (_, _, _, bracket, rate, _)) in FORM_2200A_FIXED_ROWS
                .iter()
                .enumerate()
                .take(end)
                .skip(start)
            {
                let caption = if bracket.is_empty() {
                    format!("Rate {rate}")
                } else {
                    format!("{bracket} · rate {rate}")
                };
                card = card.child(self.amount_line(
                    caption,
                    &[row_key(row, "x"), row_key(row, "t")],
                    Some(self.draft.row(row).tax_due),
                    self.has_error(&format!("schedule[{row}]")),
                    layout,
                ));
            }
            children.push(card.into_any_element());
        }
        for row in 0..FORM_2200A_OTHER_ROWS {
            let error = self.has_error(&format!("others[{row}]"));
            let texts: Vec<AnyElement> = OTHER_TEXT_COLUMNS
                .iter()
                .map(|(column, label)| {
                    let key = other_key(row, column);
                    let input = self
                        .inputs
                        .get(&key)
                        .expect("editor input registry is complete");
                    self.labelled(
                        label,
                        error,
                        Input::new(input).disabled(!editable).into_any_element(),
                        layout,
                    )
                })
                .collect();
            children.push(
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
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Others (specify) — row {}", row + 1)),
                    )
                    .child(self.grid(layout, texts))
                    .child(self.amount_line(
                        "Amounts".to_string(),
                        &[
                            other_key(row, "x"),
                            other_key(row, "t"),
                            other_key(row, "d"),
                        ],
                        None,
                        error,
                        layout,
                    ))
                    .into_any_element(),
            );
        }
        children.push(self.computed(
            "Total tax due (to Part III, Item 16)",
            self.draft.schedule_total,
            cx,
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
            self.field("sur", MONEY_INPUTS[3].1, layout, !editable),
            self.field("int", MONEY_INPUTS[4].1, layout, !editable),
            self.field("comp", MONEY_INPUTS[5].1, layout, !editable),
            self.computed("21D — Total penalties", d.total_penalties, cx),
            self.computed("22 — Total amount payable", d.amount_payable, cx),
            self.field("dep", MONEY_INPUTS[6].1, layout, !editable),
            self.computed("23B — Penalties", d.penalties_paid, cx),
            self.computed("23C — Total payment made today", d.total_payment, cx),
            self.computed(
                "24 — Balance to be carried over to next return",
                d.balance_to_carry_over,
                cx,
            ),
        ];
        self.section(
            "Part III — Payments and Application",
            vec![self.grid(layout, children)],
            cx,
        )
    }
}

impl QueueableFormView for Form2200AView {
    type Draft = Form2200ADraft;

    fn new(
        draft: Form2200ADraft,
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
        for row in 0..FORM_2200A_FIXED_ROW_COUNT {
            for (column, _) in ROW_COLUMNS.iter().take(2) {
                keys.push((row_key(row, column), "0.00".into()));
            }
        }
        for row in 0..FORM_2200A_OTHER_ROWS {
            for (column, _) in OTHER_TEXT_COLUMNS {
                keys.push((other_key(row, column), String::new()));
            }
            for (column, _) in ROW_COLUMNS {
                keys.push((other_key(row, column), "0.00".into()));
            }
        }
        for (key, placeholder) in keys {
            let value = Self::initial(&draft, &key);
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |this: &mut Self, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.sync_from_inputs(cx);
                    }
                },
            ));
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
        Self {
            draft,
            db,
            scroll_handle: ScrollHandle::new(),
            inputs,
            production,
            removal,
            validation_errors,
            parse_errors: Vec::new(),
            status_message: None,
            release_claim_confirm_open: false,
            _subscriptions: subscriptions,
        }
    }

    fn new_draft(profile: &TaxpayerProfile, year: u16, period: u8) -> Form2200ADraft {
        Form2200ADraft::new_from_profile(profile, year, period)
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

impl FormViewTrait for Form2200AView {
    fn form_title(&self) -> &'static str {
        "BIR Form No. 2200-A"
    }
    fn form_subtitle(&self) -> &'static str {
        "Excise Tax Return for Alcohol Products"
    }
    fn form_version(&self) -> &'static str {
        "January 2020 (ENCS)"
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
                    "2200A draft saved.".into(),
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
        if !can_queue_for_submission(Form2200ADraft::FORM_CODE) {
            self.status_message =
                Some("2200A is not enabled for in-app submission in this build.".into());
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
            self.status_message =
                Some("Fix the items listed under Needs review before submitting.".into());
            cx.notify();
            return;
        }
        let before = self.draft.clone();
        if let Err(errors) = self.draft.queue(QueueAuthSource::Gui) {
            self.validation_errors = errors;
            self.status_message =
                Some("Fix the items listed under Needs review before submitting.".into());
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
            self.status_message = Some(format!(
                "Could not queue Form 2200A. No submission was started: {error}"
            ));
            cx.notify();
            return;
        }
        self.status_message = Some(format!(
            "Queued for background submission as {}.",
            self.draft.submission_filename()
        ));
        self.notify(
            notification::NotificationType::Success,
            "Form 2200A queued.".into(),
            window,
            cx,
        );
        cx.emit(QueueableFormEvent::Saved);
        bir_core::background_cron::wake();
        cx.notify();
    }

    fn mark_paid(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message =
            Some("2200A payment status needs a verified confirmation workflow.".into());
        cx.notify();
    }

    fn revert_to_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    && let Ok(Some(current)) = db.get_queueable_draft::<Form2200ADraft>(
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

    /// The frozen 2200a-2020 sheet has no writer-cells map yet, so there is
    /// nothing faithful to fill.
    fn preview_pdf(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(QueueableFormEvent::PushNotification(
            "info".into(),
            "Print preview".into(),
            "Print preview for 2200A is not available yet: the frozen January 2020 sheet has no field map. No filing state was changed.".into(),
        ));
    }
}

impl Render for Form2200AView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(window.viewport_size().width);
        let is_draft = self.draft.lifecycle.is_editable();
        let is_queued = matches!(self.draft.lifecycle.status, FilingStatus::Queued);
        let pad = match layout {
            Layout::Phone => px(12.),
            Layout::Tablet => px(20.),
            Layout::Desktop => px(32.),
        };
        let issues: Vec<AnyElement> = self
            .parse_errors
            .iter()
            .chain(self.validation_errors.iter())
            .take(20)
            .map(|(_, message)| {
                rsx! { <div text_sm text_color={cx.theme().danger}>{message.clone()}</div> }
                    .into_any_element()
            })
            .collect();

        let toolbar =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .px(pad)
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Button::new("2200a_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2200a_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2200a_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2200a_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2200a_submit")
                                .label("Queue for submission")
                                .primary()
                                .disabled(
                                    !is_draft
                                        || !self.validation_errors.is_empty()
                                        || !self.parse_errors.is_empty(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.mark_submitted(window, cx)
                                })),
                        ),
                );

        let mut body = div()
            .w_full()
            .max_w(px(1180.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_5()
            .when_some(self.status_message.clone(), |col, message| {
                col.child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .text_sm()
                        .child(message),
                )
            })
            .when(self.release_claim_confirm_open, |col| {
                col.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("2200a_release_confirm")
                                .label("Nothing reached BIR — release")
                                .danger()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.release_claim(window, cx)
                                })),
                        )
                        .child(
                            Button::new("2200a_release_cancel")
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
            .child(self.render_schedule(layout, cx));
        body = body.child(self.render_part_three(layout, cx));
        if !issues.is_empty() {
            body = body.child(self.section("Needs review", issues, cx));
        }

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
                    }),
            )
            .child(
                div()
                    .id("2200a_scroll")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .p(pad)
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{FORM_2200A_FIXED_ROW_COUNT, Layout, format_tin, parse_amount, row_groups};
    use gpui::px;

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
        assert_eq!(format_tin("12345678800000"), "123-456-788-00000");
    }

    #[test]
    fn row_groups_cover_every_fixed_row_once() {
        let groups = row_groups();
        assert_eq!(groups.first().map(|g| g.0), Some(0));
        assert_eq!(groups.last().map(|g| g.1), Some(FORM_2200A_FIXED_ROW_COUNT));
        assert!(groups.windows(2).all(|pair| pair[0].1 == pair[1].0));
    }
}
