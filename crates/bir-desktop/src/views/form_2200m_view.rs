//! Editor for BIR Form 2200M, Excise Tax Return for Mineral Products
//! (v2018). Rust owns every calculation, validation and the official submit
//! plaintext (`bir_core::forms::form_2200m`); this view only edits source
//! values. The layout reflows for desktop, tablet and phone widths.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bir_core::db::{ABANDONED_CLAIM_RELEASE_REASON, AbandonedClaimRelease, Database};
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::form_2200m::{
    FORM_2200M_FIRST_OTHER_ROW, FORM_2200M_FIXED_ROWS, FORM_2200M_OTHER_ATC,
    FORM_2200M_SCHEDULE_ROWS, Form2200MDraft, Form2200MPayment, Form2200MPlace, Form2200MRow,
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

impl EventEmitter<QueueableFormEvent> for Form2200MView {}

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
    ("b", "B — Volume exempt (MT)"),
    ("c", "C — Locally extracted value, taxable"),
    ("d", "D — Locally extracted value, exempt"),
    ("e", "E — Tax rate"),
    ("f", "F — Tax due (locally extracted)"),
    ("g", "G — Imported value, taxable"),
    ("h", "H — Imported value, exempt"),
    ("i", "I — Imported tax rate"),
    ("j", "J — Imported tax due"),
    ("k", "K — Tax due adjustment per final value"),
    ("t", "Total tax due"),
];

fn row_key(row: usize, column: &str) -> String {
    format!("r{row}_{column}")
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

pub struct Form2200MView {
    draft: Form2200MDraft,
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
            "sur" => money(draft.surcharge),
            "int" => money(draft.interest),
            "comp" => money(draft.compromise),
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
        }
        for row in 0..FORM_2200M_SCHEDULE_ROWS {
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
            if !entry.is_blank() || row < draft.schedule.len() {
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
        draft.recompute();
        draft.lifecycle.updated_at = chrono::Utc::now().to_rfc3339();
        self.validation_errors = draft.validate();
        self.parse_errors = parse_errors;
        self.draft = draft;
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Form2200MDraft)) {
        if !self.draft.lifecycle.is_editable() {
            return;
        }
        change(&mut self.draft);
        self.draft.recompute();
        self.validation_errors = self.draft.validate();
        self.status_message = None;
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
                    ("2200m_month", index),
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
            "Item 12 — Availing of tax relief under a Special Law or International Tax Treaty?",
            self.has_error("tax_relief"),
            vec![
                Self::choice("2200m_relief_yes", "Yes", relief == Some(true), !editable).on_click(
                    cx.listener(|this, _, _, cx| this.edit(cx, |d| d.tax_relief = Some(true))),
                ),
                Self::choice("2200m_relief_no", "No", relief == Some(false), !editable).on_click(
                    cx.listener(|this, _, _, cx| this.edit(cx, |d| d.tax_relief = Some(false))),
                ),
            ],
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
        let mut children = vec![self.choice_row(
            "Manner of payment",
            self.has_error("manner_of_payment"),
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
        )];
        if mop == Form2200MPayment::OtherScheme {
            children.push(self.field("other", "Item 15 — Specify the scheme", layout, !editable));
        }
        self.section("Part II — Manner of Payment", children, cx)
    }

    fn render_row(&self, row: usize, layout: Layout, cx: &Context<Self>) -> AnyElement {
        let editable = self.draft.lifecycle.is_editable();
        let (atc, description, rate) = match FORM_2200M_FIXED_ROWS.get(row) {
            Some((atc, description, rate)) => (*atc, description.to_string(), Some(*rate)),
            None => (FORM_2200M_OTHER_ATC, "Others".to_string(), None),
        };
        let heading = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Row #{} — {atc}", row + 1)),
            )
            .child(div().text_sm().child(match rate {
                Some(rate) => format!("{description} · rate {rate}"),
                None => description,
            }));
        let fields: Vec<AnyElement> = ROW_COLUMNS
            .iter()
            .filter(|(column, _)| column_exists(row, column))
            .map(|(column, label)| {
                let key = row_key(row, column);
                let error = self.has_error(&key) || self.has_error(&format!("schedule[{row}]"));
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
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading)
            .child(self.grid(layout, fields))
            .into_any_element()
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
        for row in 0..FORM_2200M_SCHEDULE_ROWS {
            children.push(self.render_row(row, layout, cx));
        }
        children.push(self.computed(
            "Total excise tax due (sum of column F)",
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

impl QueueableFormView for Form2200MView {
    type Draft = Form2200MDraft;

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
        for row in 0..FORM_2200M_SCHEDULE_ROWS {
            for (column, _) in ROW_COLUMNS {
                if column_exists(row, column) {
                    let placeholder = if matches!(*column, "desc" | "place") {
                        ""
                    } else {
                        "blank"
                    };
                    keys.push((row_key(row, column), placeholder.into()));
                }
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
            self.status_message =
                Some("2200M is not enabled for in-app submission in this build.".into());
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
                "Could not queue Form 2200M. No submission was started: {error}"
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
        let fields = self.draft.to_bir_field_map();
        match super::form_html_preview_launcher::launch_frozen_form_preview(
            "2200m-2018",
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

impl Render for Form2200MView {
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
                .child(Button::new("2200m_back").label("← Back").on_click(
                    cx.listener(|_, _, _, cx| cx.emit(QueueableFormEvent::BackToDashboard)),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("2200m_preview")
                                .label("Print preview")
                                .outline()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.preview_pdf(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("2200m_save")
                                .label("Save draft")
                                .outline()
                                .disabled(!is_draft)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_draft(window, cx)),
                                ),
                        )
                        .when(is_queued, |row| {
                            row.child(
                                Button::new("2200m_cancel")
                                    .label("Cancel queue")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.revert_to_draft(window, cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("2200m_submit")
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

#[cfg(test)]
mod tests {
    use super::{Layout, column_exists, format_tin, parse_amount, parse_cell};
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
