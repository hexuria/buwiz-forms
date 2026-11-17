//! The Calendar (port of `nsh/screens/calendar/`): every deadline for the forms the taxpayers
//! file, from the BIR tax calendar. From 600 points: the filter bar with a List and a Month
//! view. Under 600: a month grid with due dots and an Upcoming list, the taxpayers/how-often
//! filters in a sheet. The pure side lives in `lib/calendar_model.rs` (rows.ts).

use std::cell::RefCell;
use std::collections::HashSet;

use chrono::{Datelike, Local, NaiveDate};
use cui::{Align, Button, ButtonVariant, Color, ControlSize, Size, Ui};

use crate::lib::calendar_model as cm;
use crate::lib::dates::{MONTHS, MONTHS_LONG, days_until};
use crate::state::{self, AppState, ViewLayout};
use crate::theme;
use crate::types::DeadlineRow;
use crate::ui::kit;
use crate::ui::logic::{StatusChip, initials, status_chip};

const FRESH_TEXT: &str = "Deadlines show here once a taxpayer has forms for a tax year.";

// ---------------------------------------------------------------- state (filters.ts calendarState)

/// The two layouts of the wide Calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalView {
    List,
    Month,
}

/// The page's filters and what it has open; they outlive the view, so leaving the Calendar and
/// coming back finds the same taxpayers, chips, year, view and month.
#[derive(Debug, Clone)]
struct Cal {
    f: cm::Filters,
    view: CalView,
    /// The year of the month the grid shows.
    grid_year: i32,
    /// The month the grid shows, 1-12.
    grid_month: u32,
    /// The day the Month view lists, ISO, or None when none is chosen.
    day: Option<String>,
    /// The narrow layout's chosen day; None lists what is coming.
    picked: Option<String>,
    /// The narrow filter sheet.
    sheet: bool,
    /// The wide taxpayers picker.
    tp_dialog: bool,
    /// The search inside whichever picker is open.
    tp_search: String,
}

fn defaults() -> Cal {
    let now = Local::now().date_naive();
    Cal {
        f: cm::Filters {
            year: i64::from(now.year()),
            ..Default::default()
        },
        view: CalView::List,
        grid_year: now.year(),
        grid_month: now.month(),
        day: Some(now.format("%Y-%m-%d").to_string()),
        picked: None,
        sheet: false,
        tp_dialog: false,
        tp_search: String::new(),
    }
}

thread_local! {
    static CAL: RefCell<Cal> = RefCell::new(defaults());
}

/// The next session starts from the defaults (a lock, a sign-out or another taxpayer).
pub fn reset_for_session() {
    CAL.with(|c| *c.borrow_mut() = defaults());
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn now() -> chrono::DateTime<Local> {
    Local::now()
}

/// The width of the content column (parts.tsx contentWidth).
fn content_width(app: &AppState) -> f32 {
    state::content_column(ViewLayout::Page, app.main_width())
}

/// Back to every taxpayer, all frequencies, no chip and this year (calendarState.clear).
fn clear_filters(cal: &mut Cal) {
    cal.f = defaults().f;
}

// ---------------------------------------------------------------- small pieces

fn open_deadline(app: &mut AppState, d: &DeadlineRow) {
    app.open_return(crate::state::OpenReturnOptions {
        tin: d.tin.clone(),
        code: d.form_code.clone(),
        year: d.taxable_year,
        period: d.period,
        return_key: d.return_key.map(serde_json::Value::from),
        mode: None,
        snapshot_id: None,
        new_key: None,
        new_key_from: None,
    });
}

fn kind_name(k: cm::RowKind) -> &'static str {
    match k {
        cm::RowKind::Draft => "draft",
        cm::RowKind::Queued => "queued",
        cm::RowKind::Filed => "filed",
        cm::RowKind::NotStarted => "not_started",
    }
}

/// The chip a row ends in (DueRow.tsx RowChip): overdue and due soon say how many days, the
/// rest say where the return stands.
fn row_chip(d: &DeadlineRow) -> StatusChip {
    let n = days_until(&d.due_date, now());
    let open = cm::is_open_status(d);
    if open && n <= 14 {
        status_chip(if n < 0 { "overdue" } else { "due_soon" }, Some(n))
    } else {
        status_chip(kind_name(cm::kind_of(&d.status)), None)
    }
}

/// When a row is due: "Today", "3 days ago", "In 5 days".
fn when_text(n: i64) -> String {
    if n == 0 {
        "Today".to_owned()
    } else if n < 0 {
        format!("{} days ago", -n)
    } else {
        format!("In {n} days")
    }
}

fn due_month_day(d: &DeadlineRow) -> (String, String) {
    let p: Vec<&str> = d.due_date.split('-').collect();
    let m = p.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
    let day = p.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    (MONTHS[(m - 1).min(11)].to_uppercase(), day.to_string())
}

/// DueRow: date box, code, "<code> · <period>" over the taxpayer, when, status. `compact`
/// leaves out the code and "when" columns (a pane under 560 points).
fn due_row(ui: &mut Ui, app: &mut AppState, d: &DeadlineRow, compact: bool) {
    let n = days_until(&d.due_date, now());
    let open = cm::is_open_status(d);
    let timed = open && n <= 14;
    let (month, day) = due_month_day(d);
    let sub = if compact && timed && d.status == "Draft" {
        format!("{} · Draft", cm::who_line(d))
    } else {
        cm::who_line(d)
    };
    let title = format!("{} · {}", d.form_code, d.period_label);
    let kind_chip = status_chip(kind_name(cm::kind_of(&d.status)), None);
    let pressed = kit::pressable_row(ui, &cm::row_key(d), |ui| {
        // The date box: month over day, centred.
        ui.column(
            &cui::Column {
                width: Size::px(44.0),
                align: Align::Center,
                gap: 0.0,
                ..Default::default()
            },
            |ui| {
                let p = kit::frame_palette();
                ui.text_with(
                    &month,
                    &cui::Text {
                        size: theme::r#type::CAP.0,
                        font: cui::font::MEDIUM,
                        color: p.subtle_foreground,
                        ..Default::default()
                    },
                );
                ui.text_with(
                    &day,
                    &cui::Text {
                        size: theme::r#type::TITLE.0,
                        font: cui::font::MEDIUM,
                        tabular: true,
                        ..Default::default()
                    },
                );
            },
        );
        if !compact {
            ui.column(
                &cui::Column {
                    width: Size::px(68.0),
                    ..Default::default()
                },
                |ui| ui.code(&d.form_code),
            );
        }
        ui.column(
            &cui::Column {
                width: Size::FILL,
                ..Default::default()
            },
            |ui| {
                ui.text(&title);
                ui.muted(&sub);
            },
        );
        if !compact {
            ui.column(
                &cui::Column {
                    width: Size::px(96.0),
                    ..Default::default()
                },
                |ui| {
                    if timed {
                        kit::draw_chip(ui, &row_chip(d));
                    } else {
                        ui.muted(&when_text(n));
                    }
                },
            );
            ui.column(
                &cui::Column {
                    width: Size::px(88.0),
                    align: Align::End,
                    ..Default::default()
                },
                |ui| kit::draw_chip(ui, &kind_chip),
            );
        } else {
            kit::draw_chip(ui, &row_chip(d));
        }
    });
    if pressed {
        open_deadline(app, d);
    }
}

/// DayRow: the Month view's day-panel row — code, period over the taxpayer, one chip.
fn day_row(ui: &mut Ui, app: &mut AppState, d: &DeadlineRow) {
    let chip = row_chip(d);
    let pressed = kit::pressable_row(ui, &cm::row_key(d), |ui| {
        ui.column(
            &cui::Column {
                width: Size::px(52.0),
                ..Default::default()
            },
            |ui| ui.code(&d.form_code),
        );
        ui.column(
            &cui::Column {
                width: Size::FILL,
                ..Default::default()
            },
            |ui| {
                ui.text(&d.period_label);
                ui.muted(&cm::who_line(d));
            },
        );
        kit::draw_chip(ui, &chip);
    });
    if pressed {
        open_deadline(app, d);
    }
}

/// EmptyRow: muted text and an optional action button inside a ListGroup. Returns whether the
/// action was pressed.
fn empty_row(ui: &mut Ui, text: &str, action: Option<&str>) -> bool {
    let mut hit = false;
    ui.row(
        &cui::Row {
            width: Size::FILL,
            pad: theme::space(3),
            gap: theme::space(2),
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.muted(text);
            ui.spacer();
            if let Some(label) = action {
                hit = kit::small_button(ui, label, ButtonVariant::Default, None);
            }
        },
    );
    hit
}

// ---------------------------------------------------------------- taxpayers picker

/// One taxpayer as a choice: avatar, name, TIN (parts.tsx taxpayerOptions).
struct TpOpt {
    root: String,
    name: String,
    sub: String,
}

fn tp_options(app: &AppState) -> Vec<TpOpt> {
    app.taxpayers()
        .iter()
        .map(|t| TpOpt {
            root: t.tin_root.clone(),
            name: t.name.clone(),
            sub: if t.tin_display.is_empty() {
                crate::lib::format::format_tin(&t.tin_root, None)
            } else {
                t.tin_display.clone()
            },
        })
        .collect()
}

fn all_roots(opts: &[TpOpt]) -> HashSet<String> {
    opts.iter().map(|o| o.root.clone()).collect()
}

/// The taxpayers the `q` words match ("name TIN" haystack, every word).
fn tp_found<'a>(opts: &'a [TpOpt], q: &str) -> Vec<&'a TpOpt> {
    let words: Vec<String> = q
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    opts.iter()
        .filter(|o| {
            let hay = format!("{} {}", o.name, o.sub).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .collect()
}

/// A selection handed to the page: None once it holds every taxpayer (so one that appears
/// later is in it too) — the sheet's `choose`.
fn tp_choose(f: &mut cm::Filters, next: HashSet<String>, all: &HashSet<String>) {
    f.tps = if next == *all { None } else { Some(next) };
}

fn tp_current(f: &cm::Filters, all: &HashSet<String>) -> HashSet<String> {
    f.tps.clone().unwrap_or_else(|| all.clone())
}

fn tp_toggle(f: &mut cm::Filters, root: &str, all: &HashSet<String>) {
    let mut next = tp_current(f, all);
    if !next.remove(root) {
        next.insert(root.to_owned());
    }
    tp_choose(f, next, all);
}

/// One taxpayer row: avatar, name over TIN, a check when chosen. Press toggles.
fn tp_row(ui: &mut Ui, opt: &TpOpt, checked: bool) -> bool {
    let p = kit::frame_palette();
    kit::pressable_row(ui, &format!("tp-{}", opt.root), |ui| {
        ui.avatar_with(
            &opt.name,
            &cui::Avatar {
                size: 20.0,
                ..Default::default()
            },
        );
        ui.column(
            &cui::Column {
                width: Size::FILL,
                ..Default::default()
            },
            |ui| {
                ui.text(&opt.name);
                ui.muted(&opt.sub);
            },
        );
        if checked {
            ui.icon(crate::ui::icons::glyph("check"), 14.0, p.accent_blue);
        }
    })
}

/// The search field + scrollable taxpayer checklist both pickers share.
fn tp_checklist(ui: &mut Ui, opts: &[TpOpt], f: &mut cm::Filters, search: &mut String) {
    let mut q = search.clone();
    // cui-rs hands the C field `value.capacity()` bytes of buffer; the clone's
    // capacity equals its length, so typing past it truncates. Reserve headroom.
    if q.capacity() < 256 {
        q.reserve(256);
    }
    if ui
        .input_with(
            "tp-search",
            &mut q,
            &cui::Input {
                placeholder: Some("Find a taxpayer"),
                icon: Some(crate::ui::icons::glyph("search")),
                ..Default::default()
            },
        )
        .changed()
    {
        *search = q;
    }
    let all = all_roots(opts);
    let found = tp_found(opts, search.as_str());
    ui.scroll(
        &cui::Scroll {
            id: Some("tp-list"),
            height: Size::px(280.0),
            ..Default::default()
        },
        |ui| {
            for o in found.iter().copied() {
                let checked = f.tps.as_ref().map(|s| s.contains(&o.root)).unwrap_or(true);
                if tp_row(ui, o, checked) {
                    tp_toggle(f, o.root.as_str(), &all);
                }
            }
            if found.is_empty() {
                ui.muted("Nothing matches.");
            }
        },
    );
}

// ---------------------------------------------------------------- filter bar (Wide.tsx)

fn freq_index(f: cm::Freq) -> usize {
    match f {
        cm::Freq::All => 0,
        cm::Freq::Yearly => 1,
        cm::Freq::Quarterly => 2,
        cm::Freq::Monthly => 3,
    }
}

fn freq_at(i: usize) -> cm::Freq {
    match i {
        1 => cm::Freq::Yearly,
        2 => cm::Freq::Quarterly,
        3 => cm::Freq::Monthly,
        _ => cm::Freq::All,
    }
}

/// A new choice drops the month and quarter chips, which belong to Monthly and Quarterly.
fn set_freq(f: &mut cm::Filters, next: cm::Freq) {
    f.freq = next;
    f.month = 0;
    f.quarter = 0;
}

const FREQ_SEG: [(&str, Option<&str>); 4] = [
    ("All", None),
    ("Yearly", None),
    ("Quarterly", None),
    ("Monthly", None),
];
const VIEW_SEG: [(&str, Option<&str>); 2] = [("List", Some("list")), ("Month", Some("calendar"))];

fn month_chip_options() -> Vec<(&'static str, Option<&'static str>)> {
    let mut v = vec![("All months", None)];
    v.extend(MONTHS.iter().map(|m| (*m, None)));
    v
}

const QUARTER_CHIPS: [(&str, Option<&str>); 5] = [
    ("All quarters", None),
    ("Q1", None),
    ("Q2", None),
    ("Q3", None),
    ("Q4", None),
];

/// The tax year, stepped inside the years the app has (YearStepper).
fn year_stepper(ui: &mut Ui, year: &mut i64) {
    let bounds = crate::store::year_bounds();
    let p = kit::frame_palette();
    ui.box_(
        &cui::BoxOpts {
            dir: cui::Dir::Row,
            align: Align::Center,
            bg: p.card,
            border: 1.0,
            border_color: p.border_strong,
            radius: theme::radius::ROW,
            pad_x: theme::space(1),
            ..Default::default()
        },
        |ui| {
            if ui.icon_button_with(
                crate::ui::icons::glyph("chevron-left"),
                &Button {
                    size: ControlSize::Sm,
                    disabled: *year <= bounds.min,
                    tooltip: Some("Previous year"),
                    ..Default::default()
                },
            ) {
                *year -= 1;
            }
            ui.text_with(
                &year.to_string(),
                &cui::Text {
                    size: theme::r#type::CONTROL.0,
                    font: cui::font::MEDIUM,
                    tabular: true,
                    ..Default::default()
                },
            );
            if ui.icon_button_with(
                crate::ui::icons::glyph("chevron-right"),
                &Button {
                    size: ControlSize::Sm,
                    disabled: *year >= bounds.max,
                    tooltip: Some("Next year"),
                    ..Default::default()
                },
            ) {
                *year += 1;
            }
        },
    );
}

/// The taxpayers button's label: "All taxpayers", the one name, or "N taxpayers".
fn tp_button_label(app: &AppState, f: &cm::Filters) -> String {
    match &f.tps {
        None => "All taxpayers".to_owned(),
        Some(set) if set.len() == 1 => {
            let root = set.iter().next().unwrap();
            app.taxpayers()
                .iter()
                .find(|t| &t.tin_root == root)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "1 taxpayer".to_owned())
        }
        Some(set) => format!("{} taxpayers", set.len()),
    }
}

/// The wide taxpayers picker dialog (MultiCombo).
fn draw_tp_dialog(ui: &mut Ui, app: &AppState, cal: &mut Cal) {
    if !cal.tp_dialog {
        return;
    }
    let opts = tp_options(app);
    let mut search = cal.tp_search.clone();
    let mut open = true;
    ui.dialog_with(
        "Taxgayers",
        &mut open,
        &cui::Dialog {
            width: 400.0,
            ..Default::default()
        },
        |ui| {
            tp_checklist(ui, &opts, &mut cal.f, &mut search);
            ui.gap(theme::space(2));
            if kit::primary_button(ui, "Done", true) {
                cal.tp_dialog = false;
            }
        },
    );
    cal.tp_search = search;
    if !open {
        cal.tp_dialog = false;
    }
}

/// Month chips under Monthly, quarter chips under Quarterly (ChipRow).
fn draw_chip_row(ui: &mut Ui, cal: &mut Cal) {
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            ..Default::default()
        },
        |ui| match cal.f.freq {
            cm::Freq::Monthly => {
                let opts = month_chip_options();
                let cur = cal.f.month as usize;
                if let Some(i) = kit::segmented(ui, "cal-month", &opts, cur) {
                    cal.f.month = i as u32;
                }
            }
            cm::Freq::Quarterly => {
                let cur = cal.f.quarter as usize;
                if let Some(i) = kit::segmented(ui, "cal-quarter", &QUARTER_CHIPS, cur) {
                    cal.f.quarter = i as u32;
                }
                if cal.f.quarter != 0 {
                    ui.muted(&format!(
                        "Returns for {}",
                        cm::QUARTER_MONTHS[(cal.f.quarter - 1) as usize]
                    ));
                }
            }
            _ => {}
        },
    );
}

/// The filter bar: taxpayers, how often, the year (List only) and the List | Month switch.
/// FilterBar: `mt-6 w-full flex-row flex-wrap items-center gap-2`, the view switch `ml-auto`.
fn draw_filter_bar(ui: &mut Ui, app: &AppState, cal: &mut Cal) {
    let label = tp_button_label(app, &cal.f);
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            wrap: true,
            ..Default::default()
        },
        |ui| {
            if ui.button_with(
                &label,
                &Button {
                    icon: Some(crate::ui::icons::glyph("users")),
                    ..Default::default()
                },
            ) {
                cal.tp_dialog = true;
                cal.tp_search.clear();
            }
            let cur = freq_index(cal.f.freq);
            if let Some(i) = kit::segmented(ui, "cal-freq", &FREQ_SEG, cur) {
                set_freq(&mut cal.f, freq_at(i));
            }
            if cal.view == CalView::List {
                let mut year = cal.f.year;
                year_stepper(ui, &mut year);
                cal.f.year = year;
            }
            ui.spacer();
            let vcur = if cal.view == CalView::Month { 1 } else { 0 };
            if let Some(i) = kit::segmented(ui, "cal-view", &VIEW_SEG, vcur) {
                cal.view = if i == 1 {
                    CalView::Month
                } else {
                    CalView::List
                };
            }
        },
    );
}

// ---------------------------------------------------------------- the List

fn draw_list(ui: &mut Ui, app: &mut AppState, cal: &mut Cal) {
    // Every deadline the List can show for a year, in one read (Wide.tsx ListView).
    let y = cal.f.year;
    let all = state::deadlines(&format!("{y}-01-01"), &format!("{}-12-31", y + 1));
    let rows = cm::calendar_rows(&cal.f, &all);
    let groups = cm::group_rows(&cal.f, &rows);
    let count = cm::count_text(&cal.f, &rows);
    let clearable = !all.is_empty() || cm::filters_active(&cal.f, today());
    let compact = content_width(app) < 560.0;

    // ChipRow: only Monthly and Quarterly have chips, mt-2.5 above them; the count is mt-4.
    if matches!(cal.f.freq, cm::Freq::Monthly | cm::Freq::Quarterly) {
        ui.gap(10.0);
        draw_chip_row(ui, cal);
    }
    ui.gap(theme::space(4));
    ui.muted(&count);

    if rows.is_empty() {
        ui.gap(theme::space(4)); // mt-4 wrapper
        let mut clear = false;
        kit::list_group(ui, |ui| {
            clear = empty_row(
                ui,
                if clearable {
                    "No deadlines match these filters."
                } else {
                    FRESH_TEXT
                },
                if clearable {
                    Some("Clear filters")
                } else {
                    None
                },
            );
        });
        if clear {
            clear_filters(cal);
        }
        return;
    }

    for (gi, g) in groups.iter().enumerate() {
        // The group heading: mt-5 on the first, mt-8 after, mb-3 under it.
        ui.gap(theme::space(if gi == 0 { 5 } else { 8 }));
        ui.row(
            &cui::Row {
                width: Size::FILL,
                gap: theme::space(2),
                align: Align::Center,
                ..Default::default()
            },
            |ui| {
                ui.text_with(
                    &g.title,
                    &cui::Text {
                        size: theme::r#type::TITLE.0,
                        font: cui::font::MEDIUM,
                        ..Default::default()
                    },
                );
                if let Some(cap) = &g.caption {
                    ui.muted(cap);
                }
            },
        );
        ui.gap(theme::space(3));
        kit::list_group(ui, |ui| {
            for (i, &d) in g.rows.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                due_row(ui, app, d, compact);
            }
        });
    }
}

// ---------------------------------------------------------------- the Month

/// MiniChip: "<code> <initials>" in the tone of its state, 18 tall (parts.tsx). `dim` is the
/// cell's out-of-month opacity-40.
fn mini_chip(ui: &mut Ui, d: &DeadlineRow, today: NaiveDate, dim: bool) {
    let tone = cm::tone_of(d, today);
    let p = kit::frame_palette();
    let mut ink = if tone == "neutral" {
        p.muted_foreground
    } else {
        kit::tone_color(tone)
    };
    let mut fill = kit::chip_fill(tone);
    if dim {
        ink.a = (ink.a as f32 * 0.4) as u8;
        fill.a = (fill.a as f32 * 0.4) as u8;
    }
    let label = format!("{} {}", d.form_code, initials(&d.taxpayer_name));
    ui.box_(
        &cui::BoxOpts {
            height: Size::px(18.0),
            pad_x: 6.0,
            bg: fill,
            radius: 5.0,
            dir: cui::Dir::Row,
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &label,
                &cui::Text {
                    size: theme::r#type::CAP.0,
                    font: cui::font::MEDIUM,
                    color: ink,
                    nowrap: true,
                    ..Default::default()
                },
            );
        },
    );
}

/// The day's number: a filled circle on today (MonthCell/NarrowCell). `idle` is the resting
/// ink — fg2 in the wide grid, fg in the narrow one.
fn day_number(ui: &mut Ui, day: u32, is_today: bool, dim: bool, big: bool, idle: Color) {
    let p = kit::frame_palette();
    let size = if big { 24.0 } else { 22.0 };
    let mut fg = if is_today { p.primary_foreground } else { idle };
    if dim {
        fg.a = (fg.a as f32 * 0.4) as u8;
    }
    ui.box_(
        &cui::BoxOpts {
            width: Size::px(size),
            height: Size::px(size),
            radius: theme::radius::FULL,
            bg: if is_today {
                p.accent_blue
            } else {
                Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                }
            },
            dir: cui::Dir::Row,
            align: Align::Center,
            justify: cui::Justify::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &day.to_string(),
                &cui::Text {
                    size: if big {
                        theme::r#type::CONTROL.0
                    } else {
                        theme::r#type::META_STRONG.0
                    },
                    font: cui::font::MEDIUM,
                    color: fg,
                    tabular: true,
                    ..Default::default()
                },
            );
        },
    );
}

/// A 6-point dot in a tone (parts.tsx Dot); `dim` is the cell's out-of-month opacity-40.
fn dot(ui: &mut Ui, tone: &str, dim: bool) {
    let mut c = kit::tone_color(tone);
    if dim {
        c.a = (c.a as f32 * 0.4) as u8;
    }
    ui.box_(
        &cui::BoxOpts {
            width: Size::px(6.0),
            height: Size::px(6.0),
            radius: theme::radius::FULL,
            bg: c,
            ..Default::default()
        },
        |_| {},
    );
}

/// Shows a month (1-12) of a year in the grid; the chosen day follows (showMonth).
fn show_month(cal: &mut Cal, y: i32, m: u32) {
    cal.grid_year = y;
    cal.grid_month = m;
    let t = today();
    cal.day = if t.year() == y && t.month() == m {
        Some(t.format("%Y-%m-%d").to_string())
    } else {
        None
    };
    cal.picked = None;
}

fn step_month(cal: &mut Cal, by: i64) {
    // The month before (-1) or after (+1) the one shown, across year ends.
    let total = cal.grid_year * 12 + cal.grid_month as i32 - 1 + by as i32;
    let y = total.div_euclid(12);
    let m = (total.rem_euclid(12) + 1) as u32;
    show_month(cal, y, m);
}

fn show_today(cal: &mut Cal) {
    let t = today();
    show_month(cal, t.year(), t.month());
}

/// The six-week grid card shared by the wide Month view and the narrow layout. `narrow` draws
/// the dot cells (no hairlines, centred weekday header, padded card); wide draws the bordered
/// grid with MiniChips. Returns the ISO of the pressed cell.
fn draw_grid(
    ui: &mut Ui,
    cells: &[cm::Cell<'_>],
    today: NaiveDate,
    narrow: bool,
    compact: bool,
    selected: Option<&str>,
) -> Option<String> {
    let p = kit::frame_palette();
    let min_h = if narrow {
        44.0
    } else if compact {
        52.0
    } else {
        78.0
    };
    let mut pressed_iso = None;
    ui.card(
        &cui::Card {
            pad: if narrow { 4.0 } else { 0.0 },
            gap: 0.0,
            ..Default::default()
        },
        |ui| {
            // Weekday header.
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: 0.0,
                    bg: if narrow {
                        Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        }
                    } else {
                        p.background
                    },
                    ..Default::default()
                },
                |ui| {
                    for w in cm::WEEKDAYS {
                        ui.box_(
                            &cui::BoxOpts {
                                width: Size::FILL,
                                pad_x: 8.0,
                                pad_y: 6.0,
                                align: if narrow {
                                    Align::Center
                                } else {
                                    Align::Default
                                },
                                ..Default::default()
                            },
                            |ui| {
                                ui.text_with(
                                    &w[..3].to_uppercase(),
                                    &cui::Text {
                                        size: theme::r#type::CAP.0,
                                        font: cui::font::MEDIUM,
                                        color: p.subtle_foreground,
                                        ..Default::default()
                                    },
                                );
                            },
                        );
                    }
                },
            );
            if !narrow {
                ui.box_(
                    &cui::BoxOpts {
                        width: Size::FILL,
                        height: Size::px(1.0),
                        bg: p.border,
                        ..Default::default()
                    },
                    |_| {},
                );
            }
            for week in 0..6 {
                if week > 0 && !narrow {
                    ui.box_(
                        &cui::BoxOpts {
                            width: Size::FILL,
                            height: Size::px(1.0),
                            bg: p.border,
                            ..Default::default()
                        },
                        |_| {},
                    );
                }
                ui.row(
                    &cui::Row {
                        width: Size::FILL,
                        gap: 0.0,
                        align: Align::Stretch,
                        ..Default::default()
                    },
                    |ui| {
                        for col in 0..7 {
                            let c = &cells[week * 7 + col];
                            let dim = !c.in_month;
                            let is_sel = selected == Some(c.iso.as_str());
                            let id = c.iso.clone();
                            let pressed = if narrow {
                                // NarrowCell: items-center gap-0.5 py-1, day number size-6, a
                                // 6-point dot under it.
                                let idle = kit::frame_palette().foreground;
                                kit::press_cell(
                                    ui,
                                    &id,
                                    min_h,
                                    is_sel,
                                    theme::radius::ROW,
                                    4.0,
                                    4.0,
                                    2.0,
                                    |ui| {
                                        ui.column(
                                            &cui::Column {
                                                align: Align::Center,
                                                gap: 2.0,
                                                ..Default::default()
                                            },
                                            |ui| {
                                                day_number(ui, c.day, c.is_today, dim, true, idle);
                                                ui.row(
                                                    &cui::Row {
                                                        width: Size::FILL,
                                                        height: Size::px(6.0),
                                                        justify: cui::Justify::Center,
                                                        ..Default::default()
                                                    },
                                                    |ui| {
                                                        if !c.items.is_empty() {
                                                            dot(
                                                                ui,
                                                                cm::day_tone(&c.items, today),
                                                                dim,
                                                            );
                                                        }
                                                    },
                                                );
                                            },
                                        );
                                    },
                                )
                            } else {
                                // MonthCell: gap-[3px], p-1.5 (p-1 when compact).
                                let idle = kit::frame_palette().muted_foreground;
                                let pad = if compact { 4.0 } else { 6.0 };
                                kit::press_cell(ui, &id, min_h, is_sel, 0.0, pad, pad, 3.0, |ui| {
                                    ui.column(
                                        &cui::Column {
                                            gap: 3.0,
                                            ..Default::default()
                                        },
                                        |ui| {
                                            day_number(ui, c.day, c.is_today, dim, false, idle);
                                            if !compact {
                                                for &d in c.items.iter().take(2) {
                                                    mini_chip(ui, d, today, dim);
                                                }
                                                if c.items.len() > 2 {
                                                    let p = kit::frame_palette();
                                                    let mut fg = p.subtle_foreground;
                                                    if dim {
                                                        fg.a = (fg.a as f32 * 0.4) as u8;
                                                    }
                                                    ui.text_with(
                                                        &format!("+{} more", c.items.len() - 2),
                                                        &cui::Text {
                                                            size: theme::r#type::CAP.0,
                                                            color: fg,
                                                            ..Default::default()
                                                        },
                                                    );
                                                }
                                            } else if !c.items.is_empty() {
                                                let p = kit::frame_palette();
                                                let mut fg = p.muted_foreground;
                                                if dim {
                                                    fg.a = (fg.a as f32 * 0.4) as u8;
                                                }
                                                ui.text_with(
                                                    &c.items.len().to_string(),
                                                    &cui::Text {
                                                        size: theme::r#type::CAP.0,
                                                        color: fg,
                                                        ..Default::default()
                                                    },
                                                );
                                            }
                                        },
                                    );
                                })
                            };
                            if pressed {
                                pressed_iso = Some(id);
                            }
                            if col < 6 && !narrow {
                                ui.box_(
                                    &cui::BoxOpts {
                                        width: Size::px(1.0),
                                        height: Size::FILL,
                                        bg: p.border,
                                        ..Default::default()
                                    },
                                    |_| {},
                                );
                            }
                        }
                    },
                );
            }
        },
    );
    pressed_iso
}

/// The Month view's day panel: the chosen day's deadlines, or the hint.
fn draw_day_panel(
    ui: &mut Ui,
    app: &mut AppState,
    cal: &Cal,
    cells: &[cm::Cell<'_>],
    panel_w: Option<f32>,
) {
    let y = cal.grid_year;
    let m = cal.grid_month;
    let items: Vec<&DeadlineRow> = match &cal.day {
        Some(iso) => cells
            .iter()
            .find(|c| &c.iso == iso)
            .map(|c| c.items.clone())
            .unwrap_or_default(),
        None => Vec::new(),
    };
    ui.column(
        &cui::Column {
            width: panel_w.map(Size::px).unwrap_or(Size::FILL),
            gap: theme::space(2),
            ..Default::default()
        },
        |ui| {
            let title = match &cal.day {
                Some(iso) => cm::long_day(iso),
                None => format!("{} {}", MONTHS_LONG[(m - 1) as usize], y),
            };
            ui.text_with(
                &title,
                &cui::Text {
                    size: theme::r#type::EMPH.0,
                    font: cui::font::MEDIUM,
                    ..Default::default()
                },
            );
            kit::list_group(ui, |ui| match &cal.day {
                None => {
                    empty_row(ui, "Pick a day to see what is due.", None);
                }
                Some(_) if items.is_empty() => {
                    empty_row(ui, "Nothing due this day.", None);
                }
                Some(_) => {
                    for (i, &d) in items.iter().enumerate() {
                        if i > 0 {
                            kit::row_sep(ui);
                        }
                        day_row(ui, app, d);
                    }
                }
            });
        },
    );
}

/// The Month view: stepper, the six-week grid and the chosen day's panel (Wide.tsx MonthView).
fn draw_month(ui: &mut Ui, app: &mut AppState, cal: &mut Cal) {
    let y = cal.grid_year;
    let m = cal.grid_month;
    let (from, to) = cm::grid_range(y, m);
    let all = state::deadlines(&from, &to);
    let frows: Vec<DeadlineRow> = cm::by_who_and_how_often(&cal.f, &all)
        .into_iter()
        .cloned()
        .collect();
    let t = today();
    let cells = cm::grid_cells(y, m, &frows, t);
    let in_month = cm::in_month_count(y, m, &frows);
    let cw = content_width(app);
    let compact = cw < 560.0;
    let side = cw >= 760.0;
    let panel_w = if cw >= 840.0 {
        Some(320.0)
    } else {
        Some(280.0)
    };

    // ‹ Month Year › Today · "N deadlines this month" — the stepper row is mt-5.
    ui.gap(theme::space(5));
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(1),
            align: Align::Center,
            wrap: true,
            ..Default::default()
        },
        |ui| {
            if kit::icon_button(ui, "chevron-left", Some("Previous month")) {
                step_month(cal, -1);
            }
            ui.box_(
                &cui::BoxOpts {
                    width: Size::px(150.0),
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        &format!("{} {}", MONTHS_LONG[(m - 1) as usize], y),
                        &cui::Text {
                            size: theme::r#type::TITLE.0,
                            font: cui::font::MEDIUM,
                            align: cui::TextAlign::Center,
                            ..Default::default()
                        },
                    );
                },
            );
            if kit::icon_button(ui, "chevron-right", Some("Next month")) {
                step_month(cal, 1);
            }
            if kit::small_button(ui, "Today", ButtonVariant::Ghost, None) {
                show_today(cal);
            }
            ui.spacer();
            ui.muted(&format!("{} this month", cm::plural(in_month, "deadline")));
        },
    );
    ui.gap(theme::space(3));

    let mut picked_day: Option<String> = None;
    let day = cal.day.clone();
    if side {
        ui.row(
            &cui::Row {
                width: Size::FILL,
                gap: theme::space(4),
                align: Align::Start,
                ..Default::default()
            },
            |ui| {
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        picked_day = draw_grid(ui, &cells, t, false, compact, day.as_deref());
                    },
                );
                draw_day_panel(ui, app, cal, &cells, panel_w);
            },
        );
    } else {
        ui.column(
            &cui::Column {
                width: Size::FILL,
                gap: theme::space(4),
                ..Default::default()
            },
            |ui| {
                picked_day = draw_grid(ui, &cells, t, false, compact, day.as_deref());
                draw_day_panel(ui, app, cal, &cells, None);
            },
        );
    }
    if let Some(iso) = picked_day {
        cal.day = Some(iso);
    }
}

// ---------------------------------------------------------------- the wide page

fn draw_wide(ui: &mut Ui, app: &mut AppState, cal: &mut Cal) {
    draw_filter_bar(ui, app, cal);
    draw_tp_dialog(ui, app, cal);
    if app.is_fresh() {
        ui.gap(theme::space(6));
        kit::list_group(ui, |ui| {
            empty_row(ui, FRESH_TEXT, None);
        });
        return;
    }
    if cal.view == CalView::Month {
        draw_month(ui, app, cal);
    } else {
        draw_list(ui, app, cal);
    }
}

// ---------------------------------------------------------------- the narrow page

/// The two filters this layout has (taxpayers, how often): how many differ from their start.
fn narrowed_count(f: &cm::Filters) -> usize {
    (f.tps.is_some() as usize) + (f.freq != cm::Freq::All) as usize
}

/// The narrow filter sheet (Narrow.tsx FilterSheet).
fn draw_sheet(ui: &mut Ui, app: &AppState, cal: &mut Cal) {
    if !cal.sheet {
        return;
    }
    let opts = tp_options(app);
    let all = all_roots(&opts);
    let mut search = cal.tp_search.clone();
    let mut open = true;
    ui.dialog_with(
        "Filters",
        &mut open,
        &cui::Dialog {
            width: 400.0,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                "How often",
                &cui::Text {
                    size: theme::r#type::META.0,
                    font: cui::font::MEDIUM,
                    color: kit::frame_palette().muted_foreground,
                    ..Default::default()
                },
            );
            let cur = freq_index(cal.f.freq);
            if let Some(i) = kit::segmented(ui, "cal-freq-sheet", &FREQ_SEG, cur) {
                set_freq(&mut cal.f, freq_at(i));
            }
            ui.gap(theme::space(3));
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(1),
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        "Taxpayers",
                        &cui::Text {
                            size: theme::r#type::META.0,
                            font: cui::font::MEDIUM,
                            color: kit::frame_palette().muted_foreground,
                            ..Default::default()
                        },
                    );
                    ui.spacer();
                    if kit::small_button(ui, "Select all", ButtonVariant::Ghost, None) {
                        let found: HashSet<String> = tp_found(&opts, search.as_str())
                            .iter()
                            .map(|o| o.root.clone())
                            .collect();
                        let mut next = tp_current(&cal.f, &all);
                        next.extend(found);
                        tp_choose(&mut cal.f, next, &all);
                    }
                    if kit::small_button(ui, "Clear", ButtonVariant::Ghost, None) {
                        let found: HashSet<String> = tp_found(&opts, search.as_str())
                            .iter()
                            .map(|o| o.root.clone())
                            .collect();
                        let mut next = tp_current(&cal.f, &all);
                        for r in found {
                            next.remove(&r);
                        }
                        tp_choose(&mut cal.f, next, &all);
                    }
                },
            );
            tp_checklist(ui, &opts, &mut cal.f, &mut search);
            ui.gap(theme::space(3));
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    justify: cui::Justify::End,
                    gap: theme::space(2),
                    ..Default::default()
                },
                |ui| {
                    if ui.button_with(
                        "Clear filters",
                        &Button {
                            variant: ButtonVariant::Ghost,
                            size: ControlSize::Sm,
                            disabled: narrowed_count(&cal.f) == 0,
                            ..Default::default()
                        },
                    ) {
                        clear_filters(cal);
                    }
                    if kit::primary_button(ui, "Done", true) {
                        cal.sheet = false;
                    }
                },
            );
        },
    );
    cal.tp_search = search;
    if !open {
        cal.sheet = false;
    }
}

fn draw_narrow(ui: &mut Ui, app: &mut AppState, cal: &mut Cal) {
    if app.is_fresh() {
        kit::list_group(ui, |ui| {
            empty_row(ui, FRESH_TEXT, None);
        });
        return;
    }
    let y = cal.grid_year;
    let m = cal.grid_month;
    let t = today();

    // ‹ Month Year › Today
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(1),
            align: Align::Center,
            wrap: true,
            ..Default::default()
        },
        |ui| {
            if kit::icon_button(ui, "chevron-left", Some("Previous month")) {
                step_month(cal, -1);
            }
            ui.box_(
                &cui::BoxOpts {
                    width: Size::px(130.0),
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        &format!("{} {}", MONTHS_LONG[(m - 1) as usize], y),
                        &cui::Text {
                            size: theme::r#type::TITLE.0,
                            font: cui::font::MEDIUM,
                            align: cui::TextAlign::Center,
                            ..Default::default()
                        },
                    );
                },
            );
            if kit::icon_button(ui, "chevron-right", Some("Next month")) {
                step_month(cal, 1);
            }
            if kit::small_button(ui, "Today", ButtonVariant::Ghost, None) {
                show_today(cal);
            }
        },
    );
    ui.gap(theme::space(3));

    // The grid's rows (six weeks of the shown month) and the month count.
    let (from, to) = cm::grid_range(y, m);
    let all = state::deadlines(&from, &to);
    let frows: Vec<DeadlineRow> = cm::by_who_and_how_often(&cal.f, &all)
        .into_iter()
        .cloned()
        .collect();
    let cells = cm::grid_cells(y, m, &frows, t);
    let in_month = cm::in_month_count(y, m, &frows);
    let narrowed = narrowed_count(&cal.f);

    // Filters (with the narrowed count) · "N deadlines this month"
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            if ui.button_with(
                "Filters",
                &Button {
                    variant: if narrowed > 0 {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Default
                    },
                    size: ControlSize::Sm,
                    icon: Some(crate::ui::icons::glyph("users")),
                    ..Default::default()
                },
            ) {
                cal.sheet = true;
                cal.tp_search.clear();
            }
            if narrowed > 0 {
                ui.badge_with(
                    &narrowed.to_string(),
                    &cui::Badge {
                        tone: cui::Tone::Accent,
                        size: ControlSize::Sm,
                        ..Default::default()
                    },
                );
            }
            ui.spacer();
            ui.muted(&format!("{} this month", cm::plural(in_month, "deadline")));
        },
    );
    ui.gap(theme::space(3));

    // The month grid with a dot on each due day.
    let picked_iso = cal.picked.clone();
    if let Some(iso) = draw_grid(ui, &cells, t, true, true, picked_iso.as_deref()) {
        // togglePicked: a press on the chosen day lets go of it.
        cal.picked = if cal.picked.as_deref() == Some(iso.as_str()) {
            None
        } else {
            Some(iso)
        };
    }
    ui.gap(theme::space(3));

    // Upcoming (or the picked day): late first, then what falls due in the next 60 days.
    let (ufrom, uto) = cm::upcoming_range(t);
    let uall = state::deadlines(&ufrom, &uto);
    let ufrows: Vec<DeadlineRow> = cm::by_who_and_how_often(&cal.f, &uall)
        .into_iter()
        .cloned()
        .collect();
    let coming = cm::upcoming_rows(&ufrows, t);
    let listed: Vec<&DeadlineRow> = match &cal.picked {
        Some(iso) => cells
            .iter()
            .find(|c| &c.iso == iso)
            .map(|c| c.items.clone())
            .unwrap_or_default(),
        None => coming,
    };
    let empty_text = if cal.picked.is_some() {
        "Nothing due this day.".to_owned()
    } else if narrowed > 0 {
        "No deadlines match these filters.".to_owned()
    } else {
        "Nothing to do in the next 60 days.".to_owned()
    };
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &match &cal.picked {
                    Some(iso) => cm::long_day(iso),
                    None => "Upcoming".to_owned(),
                },
                &cui::Text {
                    size: theme::r#type::TITLE.0,
                    font: cui::font::MEDIUM,
                    ..Default::default()
                },
            );
            ui.spacer();
            if cal.picked.is_some()
                && kit::small_button(ui, "Show upcoming", ButtonVariant::Ghost, None)
            {
                cal.picked = None;
            }
        },
    );
    ui.gap(theme::space(3));
    let mut clear = false;
    kit::list_group(ui, |ui| {
        if listed.is_empty() {
            clear = empty_row(
                ui,
                &empty_text,
                if narrowed > 0 && cal.picked.is_none() {
                    Some("Clear filters")
                } else {
                    None
                },
            );
        } else {
            for (i, &d) in listed.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                due_row(ui, app, d, true);
            }
        }
    });
    if clear {
        clear_filters(cal);
    }
    draw_sheet(ui, app, cal);
}

pub fn draw(ui: &mut Ui, app: &mut AppState) {
    CAL.with(|c| {
        let mut cal = c.borrow_mut();
        // PageHeader: the title and the lede.
        ui.heading("Calendar", 1);
        ui.muted("Every deadline for the forms your taxpayers file, from the BIR tax calendar.");
        // FilterBar is mt-6 on the wide page; the narrow root's gap-3 starts at its first row.
        ui.gap(theme::space(if app.narrow() { 4 } else { 6 }));
        if app.narrow() {
            draw_narrow(ui, app, &mut cal);
        } else {
            draw_wide(ui, app, &mut cal);
        }
    });
}
