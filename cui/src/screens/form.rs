//! The form page: renders `form::session::FormSession` — the port of `nsh/screens/form/`.
//! The session owns all state and host calls; this file is the immediate-mode draw of it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use chrono::Datelike;
use cui::Ui;
use serde_json::json;

use crate::form::schema::{self, SchemaField, SchemaItem};
use crate::form::session::{ElectionCard, FormSession, Overlay, SamplePeriodPart, SessionEvent};
use crate::lib::mask;
use crate::state::{self, AppState, FormTarget, ViewName};
use crate::store;
use crate::ui::kit::{self, UiExt};

thread_local! {
    static SESSION: RefCell<Option<FormSession>> = const { RefCell::new(None) };
    static SESSION_KEY: RefCell<String> = const { RefCell::new(String::new()) };
    /// session toast sid -> app toast id (for DismissToast).
    static TOAST_MAP: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    /// Whether the form view is on screen this frame (the leave hook flushes only then).
    static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The schedule row open in its drawer, by table id (`fp-table-<first key>`), as rows.tsx's `openRow` signal.
    static OPEN_ROW: RefCell<HashMap<String, Option<usize>>> = RefCell::new(HashMap::new());
    /// The date picker that is open, if any: the field it is for and the month it shows.
    static DATE_PICKER: RefCell<Option<DatePicker>> = const { RefCell::new(None) };
}

/// The calendar beside a date box (NSH's CalendarButton/DateField calendar — a real
/// nsh-base `DatePicker` popup): which field it is for, the month on screen, the grid's
/// view (days / months / years, the header toggles), the day the field holds and the
/// days the calendar may offer (rows.tsx `dateRange`).
struct DatePicker {
    field: String,
    year: i32,
    month: u32,
    /// 0 = day grid, 1 = month grid, 2 = year grid.
    view: u8,
    /// The day the field holds, highlighted in the day grid.
    value: Option<chrono::NaiveDate>,
    min: Option<chrono::NaiveDate>,
    max: Option<chrono::NaiveDate>,
    /// Frames to ignore cell picks after open or a view change: the press that
    /// opened/switched the grid must not land on a cell that appears under it.
    settle: u8,
}

/// Opens the picker for `field` showing the month of `current` (a typed date) or today.
fn open_date_picker(
    ui: &Ui,
    field: &str,
    current: &str,
    min: Option<chrono::NaiveDate>,
    max: Option<chrono::NaiveDate>,
) {
    let today = chrono::Local::now().date_naive();
    let (mut year, mut month, mut value) = (today.year(), today.month(), None);
    if let Some(d) = crate::lib::dates::parse_typed_date(current)
        && let Some(date) = chrono::NaiveDate::from_ymd_opt(d.y, d.m, d.d)
    {
        year = d.y;
        month = d.m;
        value = Some(date);
    }
    DATE_PICKER.with(|p| {
        *p.borrow_mut() = Some(DatePicker {
            field: field.to_owned(),
            year,
            month,
            view: 0,
            value,
            min,
            max,
            settle: 3,
        });
    });
    ui.popover_open(&format!("fp-cal-{field}"));
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTH_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/// A square calendar cell: fixed width, label centered, picked day on the primary fill.
fn cal_cell(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    subtle: bool,
    enabled: bool,
    width: f32,
) -> bool {
    let p = kit::frame_palette();
    let mut b = cui::Button {
        variant: if selected {
            cui::ButtonVariant::Primary
        } else {
            cui::ButtonVariant::Ghost
        },
        size: cui::ControlSize::Md,
        width: cui::Size::px(width),
        disabled: !enabled,
        ..Default::default()
    };
    if subtle && !selected {
        b.color = p.subtle_foreground;
    }
    ui.button_with(label, &b)
}

/// Draws the picker as a calendar popover anchored to its trigger while it is open for
/// `field`; returns the picked day as "YYYY-MM-DD".
fn date_picker(ui: &mut Ui, field: &str) -> Option<String> {
    let pid = format!("fp-cal-{field}");
    let opts = cui_sys::cui_popover_opts {
        placement: cui_sys::CUI_PLACE_BOTTOM_START,
        width: 292.0,
        pad: 12.0,
        gap: 6.0,
        ..Default::default()
    };
    let mut picked: Option<String> = None;
    let open = ui.popover_begin(&pid, &opts);
    if open {
        // The calendar's own cells are native buttons — modal-safe inside the popover.
        let _m = kit::modal_off();
        if DATE_PICKER.with(|p| p.borrow().as_ref().is_some_and(|d| d.field == field)) {
            // ‹ header › — chevrons page the grid; the month and year labels toggle the
            // month and year grids (nsh-base DatePicker's monthToggle/yearToggle views).
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    justify: cui::Justify::Between,
                    ..Default::default()
                },
                |ui| {
                    if kit::icon_button(ui, "chevron-left", Some("Previous")) {
                        DATE_PICKER.with(|p| {
                            if let Some(d) = p.borrow_mut().as_mut() {
                                match d.view {
                                    2 => d.year -= 12,
                                    _ => {
                                        if d.month == 1 {
                                            d.month = 12;
                                            d.year -= 1;
                                        } else {
                                            d.month -= 1;
                                        }
                                    }
                                }
                                d.view = 0;
                                d.settle = 3;
                            }
                        });
                    }
                    let (month_label, year_label) = DATE_PICKER.with(|p| {
                        p.borrow()
                            .as_ref()
                            .map(|d| {
                                (
                                    MONTH_NAMES[(d.month - 1) as usize].to_owned(),
                                    if d.view == 2 {
                                        format!("{} – {}", d.year - 5, d.year + 6)
                                    } else {
                                        d.year.to_string()
                                    },
                                )
                            })
                            .unwrap_or_default()
                    });
                    ui.row(
                        &cui::Row {
                            gap: crate::theme::space(1),
                            ..Default::default()
                        },
                        |ui| {
                            if kit::ghost_button(ui, &month_label, true) {
                                DATE_PICKER.with(|p| {
                                    if let Some(d) = p.borrow_mut().as_mut() {
                                        d.view = if d.view == 1 { 0 } else { 1 };
                                        d.settle = 3;
                                    }
                                });
                            }
                            if kit::ghost_button(ui, &year_label, true) {
                                DATE_PICKER.with(|p| {
                                    if let Some(d) = p.borrow_mut().as_mut() {
                                        d.view = if d.view == 2 { 0 } else { 2 };
                                        d.settle = 3;
                                    }
                                });
                            }
                        },
                    );
                    if kit::icon_button(ui, "chevron-right", Some("Next")) {
                        DATE_PICKER.with(|p| {
                            if let Some(d) = p.borrow_mut().as_mut() {
                                match d.view {
                                    2 => d.year += 12,
                                    _ => {
                                        if d.month == 12 {
                                            d.month = 1;
                                            d.year += 1;
                                        } else {
                                            d.month += 1;
                                        }
                                    }
                                }
                                d.view = 0;
                                d.settle = 3;
                            }
                        });
                    }
                },
            );

            let (year, month, view, value, min, max, settle) = DATE_PICKER.with(|p| {
                p.borrow()
                    .as_ref()
                    .map(|d| (d.year, d.month, d.view, d.value, d.min, d.max, d.settle))
                    .unwrap_or((0, 1, 0, None, None, None, 0))
            });
            let armed = settle == 0;
            let allowed = |date: chrono::NaiveDate| {
                min.map(|m| date >= m).unwrap_or(true) && max.map(|m| date <= m).unwrap_or(true)
            };
            let grid = |ui: &mut Ui, f: &mut dyn FnMut(&mut Ui)| {
                ui.column(
                    &cui::Column {
                        width: cui::Size::FILL,
                        align: cui::Align::Center,
                        gap: crate::theme::space(1),
                        ..Default::default()
                    },
                    f,
                );
            };
            match view {
                // The month grid: a pick lands on that month, back to its days.
                1 => {
                    grid(ui, &mut |ui| {
                        for w in 0..3u32 {
                            ui.row(
                                &cui::Row {
                                    justify: cui::Justify::Center,
                                    gap: crate::theme::space(1),
                                    ..Default::default()
                                },
                                |ui| {
                                    for i in 0..4u32 {
                                        let m = w * 4 + i + 1;
                                        if m > 12 {
                                            ui.box_(
                                                &cui::BoxOpts {
                                                    width: cui::Size::px(64.0),
                                                    height: cui::Size::px(32.0),
                                                    ..Default::default()
                                                },
                                                |_| {},
                                            );
                                            continue;
                                        }
                                        let selected = m == month;
                                        if armed
                                            && cal_cell(
                                                ui,
                                                MONTH_SHORT[(m - 1) as usize],
                                                selected,
                                                false,
                                                true,
                                                64.0,
                                            )
                                        {
                                            DATE_PICKER.with(|p| {
                                                if let Some(d) = p.borrow_mut().as_mut() {
                                                    d.month = m;
                                                    d.view = 0;
                                                    d.settle = 3;
                                                }
                                            });
                                        }
                                    }
                                },
                            );
                        }
                    });
                }
                // The year grid: the twelve years around the shown one.
                2 => {
                    grid(ui, &mut |ui| {
                        for w in 0..3i32 {
                            ui.row(
                                &cui::Row {
                                    justify: cui::Justify::Center,
                                    gap: crate::theme::space(1),
                                    ..Default::default()
                                },
                                |ui| {
                                    for i in 0..4i32 {
                                        let y = year - 5 + w * 4 + i;
                                        let selected =
                                            y == year || Some(y) == value.map(|d| d.year());
                                        if armed
                                            && cal_cell(
                                                ui,
                                                &y.to_string(),
                                                selected,
                                                false,
                                                true,
                                                64.0,
                                            )
                                        {
                                            DATE_PICKER.with(|p| {
                                                if let Some(d) = p.borrow_mut().as_mut() {
                                                    d.year = y;
                                                    d.view = 0;
                                                    d.settle = 3;
                                                }
                                            });
                                        }
                                    }
                                },
                            );
                        }
                    });
                }
                // The day grid: weekday heads, then the weeks — adjacent-month days show
                // dimmed and still pick (NSH's `data-outside` cells).
                _ => {
                    ui.row(
                        &cui::Row {
                            width: cui::Size::FILL,
                            justify: cui::Justify::Center,
                            gap: crate::theme::space(1),
                            ..Default::default()
                        },
                        |ui| {
                            for head in WEEKDAYS {
                                ui.box_(
                                    &cui::BoxOpts {
                                        width: cui::Size::px(36.0),
                                        align: cui::Align::Center,
                                        justify: cui::Justify::Center,
                                        ..Default::default()
                                    },
                                    |ui| kit::meta_text(ui, head),
                                );
                            }
                        },
                    );
                    if let Some(first) = chrono::NaiveDate::from_ymd_opt(year, month, 1) {
                        let days_in = {
                            let (ny, nm) = if month == 12 {
                                (year + 1, 1)
                            } else {
                                (year, month + 1)
                            };
                            chrono::NaiveDate::from_ymd_opt(ny, nm, 1)
                                .map(|n| (n - chrono::Duration::days(1)).day())
                                .unwrap_or(31)
                        };
                        let offset = first.weekday().num_days_from_sunday() as i64;
                        let weeks = ((offset + days_in as i64) + 6) / 7;
                        for w in 0..weeks {
                            ui.row(
                                &cui::Row {
                                    width: cui::Size::FILL,
                                    justify: cui::Justify::Center,
                                    gap: crate::theme::space(1),
                                    ..Default::default()
                                },
                                |ui| {
                                    for i in 0..7i64 {
                                        let idx = w * 7 + i - offset;
                                        let date = first + chrono::Duration::days(idx);
                                        let in_month = date.year() == year && date.month() == month;
                                        let enabled = allowed(date);
                                        let selected = value == Some(date);
                                        if armed
                                            && cal_cell(
                                                ui,
                                                &date.day().to_string(),
                                                selected,
                                                !in_month,
                                                enabled,
                                                36.0,
                                            )
                                        {
                                            picked = Some(format!(
                                                "{:04}-{:02}-{:02}",
                                                date.year(),
                                                date.month(),
                                                date.day()
                                            ));
                                        }
                                    }
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    // Count down the post-switch guard once the grid has been drawn.
    DATE_PICKER.with(|p| {
        if let Some(d) = p.borrow_mut().as_mut()
            && d.field == field
            && d.settle > 0
        {
            d.settle -= 1;
        }
    });
    ui.popover_end();
    if picked.is_some() {
        DATE_PICKER.with(|p| {
            if p.borrow().as_ref().is_some_and(|d| d.field == field) {
                *p.borrow_mut() = None;
            }
        });
        ui.popover_close(&pid);
    } else if !open && DATE_PICKER.with(|p| p.borrow().as_ref().is_some_and(|d| d.field == field)) {
        // Dismissed by an outside click or Escape: drop the picker's month state.
        DATE_PICKER.with(|p| {
            if p.borrow().as_ref().is_some_and(|d| d.field == field) {
                *p.borrow_mut() = None;
            }
        });
    }
    picked
}

/// The "pick a date" button beside a date box; opens the picker for `field`.
fn date_pick_button(
    ui: &mut Ui,
    field: &str,
    current: &str,
    min: Option<chrono::NaiveDate>,
    max: Option<chrono::NaiveDate>,
) {
    let _g = ui.push_id(format!("pick-{field}"));
    if kit::icon_button(ui, "calendar", Some("Pick a date")) {
        open_date_picker(ui, field, current, min, max);
    }
}

/// What the split boxes hold now, as the calendar's own day ("" until the month and the year read
/// as one) — rows.tsx `splitIso`.
fn split_iso(s: &FormSession, dp: &schema::DateParts) -> String {
    let m: i64 = s
        .value_of(&dp.month.k)
        .chars()
        .take(2)
        .collect::<String>()
        .trim()
        .parse()
        .unwrap_or(0);
    let y: i64 = s.value_of(&dp.year.k).trim().parse().unwrap_or(0);
    let d_raw = dp
        .day
        .as_ref()
        .map(|d| s.value_of(&d.k))
        .unwrap_or_default();
    let d: i64 = d_raw.trim().parse().unwrap_or(0);
    if !(1..=12).contains(&m) || y <= 1900 {
        return String::new();
    }
    let d = if (1..=31).contains(&d) { d } else { 1 };
    format!("{y}-{:02}-{:02}", m, d)
}

/// The days the calendar beside a split date may offer, given the parts that are shut (locked or
/// computed) — rows.tsx `dateRange`: a shut year limits it to that year, a shut year and month to
/// that month. A shut month alone limits nothing.
fn split_date_range(
    s: &FormSession,
    dp: &schema::DateParts,
) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
    let shut = |f: &SchemaField| f.c == Some(true) || s.is_shut(&f.k);
    let typed = s.value_of(&dp.year.k);
    // A year printed as 20YY (1702Q Year Ended) holds its last two digits.
    let year: i64 =
        if !typed.is_empty() && typed.len() <= 2 && typed.chars().all(|c| c.is_ascii_digit()) {
            2000 + typed.parse::<i64>().unwrap_or(0)
        } else {
            typed.trim().parse().unwrap_or(0)
        };
    if !shut(&dp.year) || !(1900..=9999).contains(&year) {
        return (None, None);
    }
    let m: i64 = s
        .value_of(&dp.month.k)
        .chars()
        .take(2)
        .collect::<String>()
        .trim()
        .parse()
        .unwrap_or(0);
    let (y, mo) = (year as i32, m as u32);
    if shut(&dp.month) && (1..=12).contains(&m) {
        let first = chrono::NaiveDate::from_ymd_opt(y, mo, 1);
        let (ny, nm) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
        let last =
            chrono::NaiveDate::from_ymd_opt(ny, nm, 1).map(|n| n - chrono::Duration::days(1));
        return (first, last);
    }
    (
        chrono::NaiveDate::from_ymd_opt(y, 1, 1),
        chrono::NaiveDate::from_ymd_opt(y, 12, 31),
    )
}

fn target_key(t: &FormTarget) -> String {
    format!(
        "{}|{}|{}|{}|{}|{:?}|{:?}",
        t.tin, t.form_code, t.year, t.period, t.mode, t.key, t.snapshot_id
    )
}

fn params_for(t: &FormTarget) -> state::ViewParams {
    let mut m = serde_json::Map::new();
    m.insert(
        "target".to_owned(),
        json!({
            "tin": t.tin, "formCode": t.form_code, "year": t.year, "period": t.period,
            "key": t.key, "mode": t.mode, "snapshotId": t.snapshot_id,
            "newKey": t.new_key, "newKeyFrom": t.new_key_from,
        }),
    );
    m
}

fn with_session<R>(f: impl FnOnce(&mut FormSession) -> R) -> Option<R> {
    SESSION.with(|c| c.borrow_mut().as_mut().map(f))
}

/// Drain the session's asks of the page around it.
fn drain_events(app: &mut AppState) {
    let events = with_session(|s| s.take_events()).unwrap_or_default();
    for ev in events {
        match ev {
            SessionEvent::Toast {
                sid,
                message,
                icon,
                action,
            } => {
                let app_id = app.toast(
                    message,
                    icon.as_deref(),
                    action.map(|(label, cmd)| {
                        (
                            label,
                            Rc::new(RefCell::new(move || {
                                with_session(|s| s.run_undo(cmd));
                            })) as crate::state::ToastActionFn,
                        )
                    }),
                );
                TOAST_MAP.with(|m| {
                    m.borrow_mut().insert(sid, app_id);
                });
            }
            SessionEvent::DismissToast { sid } => {
                let app_id = TOAST_MAP.with(|m| m.borrow_mut().remove(&sid));
                if let Some(id) = app_id {
                    app.dismiss_toast(id);
                }
            }
            SessionEvent::AfterWrite(name) => store::bump(Some(&name)),
            SessionEvent::Saved(t) => {
                app.bump_recent_return(
                    &t.tin,
                    &t.form_code,
                    t.year,
                    t.period,
                    t.key.map(|k| json!(k)),
                );
            }
            SessionEvent::Retarget(t) => {
                SESSION_KEY.with(|k| *k.borrow_mut() = target_key(&t));
                app.go(ViewName::Form, params_for(&t));
            }
            SessionEvent::QueueChanged(_) => store::bump(Some("screen_form_queue")),
        }
    }
}

/// Whether the form view drew this frame.
pub fn set_active(on: bool) {
    ACTIVE.with(|a| a.set(on));
}

/// The leave hook: what is typed is flushed and, where autosave may, saved (dispose's finish).
pub fn flush_on_leave() {
    if !ACTIVE.with(|a| a.get()) {
        return;
    }
    with_session(|s| {
        s.flush_now();
    });
}

/// The Return view.
pub fn draw(ui: &mut Ui, app: &mut AppState) {
    set_active(true);
    let Some(t) = state::form_target(app.params()) else {
        app.go(ViewName::Home, Default::default());
        return;
    };
    let key = target_key(&t);
    SESSION.with(|c| {
        let mut g = c.borrow_mut();
        let stale = g
            .as_ref()
            .map(|s| !s.alive() || SESSION_KEY.with(|k| k.borrow().as_str() != key))
            .unwrap_or(true);
        if stale {
            let mut s = FormSession::new(t.clone());
            s.open_route(t);
            *g = Some(s);
            SESSION_KEY.with(|k| *k.borrow_mut() = key);
            TOAST_MAP.with(|m| m.borrow_mut().clear());
            REVEAL_SEEN.with(|c| c.set(0));
            REVEAL_DONE.with(|c| c.set(0));
        }
    });
    with_session(|s| {
        s.poll();
    });
    drain_events(app);
    with_session(|s| s.notes_drawn());

    let state_now = with_session(|s| {
        if s.loading() {
            "loading".to_owned()
        } else if !s.error().is_empty() {
            format!("error:{}", s.error())
        } else {
            "open".to_owned()
        }
    })
    .unwrap_or_else(|| "loading".to_owned());
    if state_now == "loading" {
        ui.centered("Opening the return…");
        return;
    }
    if let Some(msg) = state_now.strip_prefix("error:") {
        ui.danger_text(msg);
        if ui.ghost_button("Back", true) {
            app.go(ViewName::Home, Default::default());
        }
        return;
    }
    SESSION.with(|c| {
        if let Some(s) = c.borrow_mut().as_mut() {
            draw_page(ui, app, s);
        }
    });
}

// ---------------------------------------------------------------- the page

fn draw_page(ui: &mut Ui, app: &mut AppState, s: &mut FormSession) {
    let h = s.header_info();
    let m = form_metrics(app);
    let loaded = s.schema.is_some() && !h.loading && h.error.is_empty() && s.error().is_empty();
    // Floating layers (sections popover, date picker, schedule drawer) are modal for CUI's
    // own widgets only — custom hit tests below must not fire under them. `was` keeps the
    // guard up on the frame a layer is dismissed, so the closing click cannot leak through.
    let pop_open = ui.popover_is_open("fp-sections");
    let drawer_open = OPEN_ROW.with(|m| m.borrow().values().any(|v| v.is_some()));
    let date_open = DATE_PICKER.with(|p| p.borrow().is_some());
    let layer_open = pop_open || drawer_open || date_open;
    POPOVER_OPEN_FLAG.with(|c| c.set(layer_open));
    let layer_was = LAYER_WAS_OPEN.with(|c| c.replace(layer_open));
    kit::set_modal(loaded && (layer_open || layer_was));
    if m.wide && loaded {
        ui.row(
            &cui::Row {
                width: cui::Size::FILL,
                height: cui::Size::FILL,
                ..Default::default()
            },
            |ui| {
                draw_navigator(ui, s, m.edge, app.prefs.compact_form_nav);
                draw_form_pane(ui, app, s, &m, true);
            },
        );
    } else {
        draw_form_pane(ui, app, s, &m, false);
    }
    draw_overlay(ui, app, s);
}

/// The page's measures (FormPage.tsx): 32pt side padding (16 under 640), a column of at most
/// 1120 less that padding centred in the pane; the "On this form" navigator from a 760-point
/// column, the form column 236 points narrower than the page column beside it.
struct FormMetrics {
    narrow: bool,
    pad_x: f32,
    column: f32,
    edge: f32,
    form_w: f32,
    wide: bool,
}

fn form_metrics(app: &AppState) -> FormMetrics {
    let w = app.main_width();
    let pad_x = if w < state::BREAK_NARROW_CONTENT {
        16.0
    } else {
        32.0
    };
    let column = (w.min(1120.0) - pad_x * 2.0).max(200.0);
    let edge = pad_x.max(((w - column) / 2.0).floor());
    FormMetrics {
        narrow: w < state::BREAK_PHONE,
        pad_x,
        column,
        edge,
        form_w: (column - 236.0).max(200.0),
        wide: column >= 760.0,
    }
}

/// The scroll column plus the floating step pill at its foot (the form's own column when the
/// navigator shows, the whole pane otherwise).
fn draw_form_pane(
    ui: &mut Ui,
    app: &mut AppState,
    s: &mut FormSession,
    m: &FormMetrics,
    nav: bool,
) {
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            height: cui::Size::FILL,
            ..Default::default()
        },
        |ui| {
            ui.scroll(
                &cui::Scroll {
                    id: Some("form-scroll"),
                    ..Default::default()
                },
                |ui| {
                    if nav {
                        // pt-7 pr-7.5 pb-28 pl-5.5 around a column of formW.
                        ui.column(
                            &cui::Column {
                                id: Some("fp-scroll-content"),
                                width: cui::Size::px(m.form_w),
                                pad_left: 22.0,
                                pad_right: 30.0,
                                pad_top: 28.0,
                                pad_bottom: 120.0,
                                gap: crate::theme::space(4),
                                ..Default::default()
                            },
                            |ui| draw_form_body(ui, app, s),
                        );
                    } else {
                        // px-4/8 pt-5/7 pb-32 around a centred column of at most `column`.
                        ui.column(
                            &cui::Column {
                                id: Some("fp-scroll-content"),
                                width: cui::Size::FILL,
                                align: cui::Align::Center,
                                pad_left: m.edge,
                                pad_right: m.edge,
                                ..Default::default()
                            },
                            |ui| {
                                ui.column(
                                    &cui::Column {
                                        width: cui::Size::px(m.column),
                                        pad_x: m.pad_x,
                                        pad_top: if m.pad_x == 16.0 { 20.0 } else { 28.0 },
                                        pad_bottom: 120.0,
                                        gap: crate::theme::space(4),
                                        ..Default::default()
                                    },
                                    |ui| draw_form_body(ui, app, s),
                                );
                            },
                        );
                    }
                },
            );
            consume_reveal(ui, s);
            if s.schema.is_some() {
                draw_pill(ui, s, nav, m.narrow, m.form_w);
            }
        },
    );
}

thread_local! {
    /// The reveal seq seen last frame and the one already scrolled to (FormSession.reveal is
    /// never cleared, so the draw tracks consumption itself).
    static REVEAL_SEEN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static REVEAL_DONE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// The existing-return offer's "don't ask again for drafts" box (NSH Dialogs' `fast`).
    static OFFER_FAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Last frame's floating-layer state, for the chrome drawn before the page (app.rs reads
    /// it through `sections_popover_open` to raise the modal guard for the whole frame) and
    /// for the dismissal frame's click swallow.
    static LAYER_WAS_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static POPOVER_OPEN_FLAG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the sections popover was open at the end of the last drawn form frame.
pub fn sections_popover_open() -> bool {
    POPOVER_OPEN_FLAG.with(std::cell::Cell::get)
}

/// FormSession.reveal -> scroll the step/jump target into view (FormPage.tsx `reveal`).
/// CUI records last frame's rects, so a fresh reveal waits one settled frame before reading
/// them — folding parts changes the layout the target sits in.
fn consume_reveal(ui: &mut Ui, s: &mut FormSession) {
    let Some(rev) = s.reveal.clone() else {
        return;
    };
    let done = REVEAL_DONE.with(std::cell::Cell::get);
    if rev.seq <= done {
        return;
    }
    let seen = REVEAL_SEEN.with(std::cell::Cell::get);
    if rev.seq != seen {
        REVEAL_SEEN.with(|c| c.set(rev.seq));
        ui.request_redraw();
        return;
    }
    REVEAL_DONE.with(|c| c.set(rev.seq));
    let view_id = ui.make_id("form-scroll");
    let content_id = ui.make_id("fp-scroll-content");
    let view = ui.get_rect(view_id);
    let content = ui.get_rect(content_id);
    if view.w <= 0.0 || content.w <= 0.0 {
        return;
    }
    // The tagged content column's top is the scroll origin shifted by the offset, so the
    // current scroll offset is view.y - content.y (the scroll's own pad is 0).
    let offset = view.y - content.y;
    let mut tag = format!("fp-partcard-{}", rev.part);
    if let Some(row) = &rev.row {
        let item_tag = format!("fp-item-{row}");
        let item_id = ui.make_id(&item_tag);
        let r = ui.get_rect(item_id);
        if r.w > 0.0 || r.h > 0.0 {
            tag = item_tag;
        }
    }
    let tag_id = ui.make_id(&tag);
    let r = ui.get_rect(tag_id);
    if r.w <= 0.0 && r.h <= 0.0 {
        return;
    }
    // cui__dp_reveal's rule: scroll the least that shows the target, clamped at the top.
    let mut ty = offset;
    if r.y < view.y {
        ty -= view.y - r.y;
    } else if r.y + r.h > view.y + view.h {
        ty += r.y + r.h - (view.y + view.h);
    }
    ui.scroll_to("form-scroll", ty.max(0.0));
}

/// Header, notes and the part cards — everything inside the page's one scroll.
fn draw_form_body(ui: &mut Ui, app: &mut AppState, s: &mut FormSession) {
    draw_header(ui, s);
    draw_notes(ui, s);
    let part_count = s.schema_part_count();
    for pi in 0..part_count {
        let info = s.part_info(pi);
        let card_tag = format!("fp-partcard-{pi}");
        kit::card_styled_tagged(ui, Some(&card_tag), 0.0, 0.0, false, info.open, |ui| {
            // ListRow: chevron, the semibold part name, "N items" (rows.tsx part header).
            let name = s.part_title(pi);
            let items = s.part_item_count(pi);
            let items_label = format!("{} item{}", items, if items == 1 { "" } else { "s" });
            let pressed = kit::pressable_row(ui, &format!("fp-part-{pi}-toggle"), |ui| {
                kit::icon(
                    ui,
                    if info.open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    16.0,
                );
                ui.text_with(
                    &name,
                    &cui::Text {
                        font: cui::font::MEDIUM,
                        size: 13.0,
                        nowrap: true,
                        width: cui::Size::FILL,
                        ..Default::default()
                    },
                );
                kit::meta_text(ui, &items_label);
            });
            if pressed {
                s.toggle_part(pi);
            }
            if !info.open {
                return;
            }
            ui.column(
                &cui::Column {
                    width: cui::Size::FILL,
                    pad: crate::theme::space(3),
                    gap: crate::theme::space(4),
                    ..Default::default()
                },
                |ui| {
                    if let Some(fold) = &info.fold {
                        let label = format!(
                            "{} ({})",
                            fold.label,
                            if fold.open { "hide" } else { "show" }
                        );
                        if ui.ghost_button(&label, true) {
                            s.toggle_profile_fold(pi);
                        }
                    }
                    draw_part_items(ui, app, s, pi);
                },
            );
        });
    }
}

/// A small NSH Chip: freq/pages/fields/status notes under the form title.
fn form_chip(ui: &mut Ui, label: &str, tone: &str) {
    let t = match tone {
        "accent" => cui::Tone::Accent,
        "success" => cui::Tone::Success,
        "warning" => cui::Tone::Warning,
        "danger" => cui::Tone::Danger,
        "info" => cui::Tone::Info,
        _ => cui::Tone::Neutral,
    };
    ui.badge_with(
        label,
        &cui::Badge {
            tone: t,
            size: cui::ControlSize::Sm,
            ..Default::default()
        },
    );
}

/// FormPage.tsx `Header`: "BIR Form <code> · Revision", the title, the chip row, then the
/// action buttons and the save-state line at the right.
fn draw_header(ui: &mut Ui, s: &mut FormSession) {
    let h = s.header_info();
    let Some(f) = s.schema.as_ref() else {
        return;
    };
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: crate::theme::space(2),
            align: cui::Align::Center,
            wrap: true,
            ..Default::default()
        },
        |ui| {
            ui.code(&format!("BIR Form {}", f.c));
            kit::meta_text(ui, &format!("Revision {}", f.rev));
        },
    );
    ui.heading(&f.t, 1);
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: crate::theme::space(2),
            wrap: true,
            ..Default::default()
        },
        |ui| {
            form_chip(ui, &f.freq, "neutral");
            let pages = f.pages as i64;
            form_chip(
                ui,
                &format!("{} page{}", pages, if pages == 1 { "" } else { "s" }),
                "neutral",
            );
            let n = f.n as i64;
            form_chip(ui, &format!("{} fields", n), "neutral");
            if f.q < 0.8 {
                form_chip(ui, "Draft layout", "warning");
            }
            if h.status != "Draft" {
                let label = if h.editable {
                    h.status.clone()
                } else {
                    format!("{} · read-only", h.status)
                };
                form_chip(
                    ui,
                    &label,
                    if h.status == "Queued" {
                        "accent"
                    } else {
                        "success"
                    },
                );
            }
            if h.amends {
                form_chip(ui, "Amended return", "warning");
            }
            if h.filed_original_view.is_some() {
                form_chip(ui, "Filed original · read-only", "success");
            }
            if h.sample {
                form_chip(ui, "Sample", "accent");
            }
            if h.loading {
                form_chip(ui, "Opening…", "accent");
            }
        },
    );
    // The action buttons and the save line, right-aligned (items-end in NSH).
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            gap: crate::theme::space(2),
            wrap: true,
            justify: cui::Justify::End,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            if kit::small_button(
                ui,
                "Print preview",
                cui::ButtonVariant::Secondary,
                Some("printer"),
            ) {
                s.print();
            }
            if h.filed_original
                && kit::small_button(
                    ui,
                    "View the filed original",
                    cui::ButtonVariant::Secondary,
                    Some("history"),
                )
            {
                s.open_filed_original();
            }
            if h.filed_original_view.is_some()
                && kit::small_button(
                    ui,
                    "Back to the amendment",
                    cui::ButtonVariant::Secondary,
                    None,
                )
            {
                s.back_to_amendment();
            }
            if h.editable && h.filed_original_view.is_none() {
                if ui.button_with(
                    "Check",
                    &cui::Button {
                        size: cui::ControlSize::Sm,
                        loading: s.checking(),
                        ..Default::default()
                    },
                ) {
                    s.check();
                }
                if kit::small_button(ui, "Queue", cui::ButtonVariant::Secondary, Some("send")) {
                    s.queue();
                }
            }
            if h.status == "Queued"
                && !h.sample
                && kit::small_button(ui, "Cancel queue", cui::ButtonVariant::Secondary, None)
            {
                s.cancel_queue();
            }
            if h.editable
                && h.sample
                && !h.period_set
                && kit::primary_button(ui, "Save as return…", true)
            {
                s.save_now();
            }
            if h.editable && h.amend_unsaved && kit::primary_button(ui, "Save amendment", true) {
                s.save_now();
            }
        },
    );
    // SaveLine: Saving… / the state's own message, danger on failure, success once saved.
    match s.save_state() {
        crate::form::session::SaveState::Saving => kit::meta_text(ui, "Saving…"),
        st => {
            let label = st.label();
            if !label.is_empty() {
                let failed = matches!(st, crate::form::session::SaveState::Failed(_));
                let saved = matches!(st, crate::form::session::SaveState::Saved);
                if failed {
                    kit::danger_text(ui, &label);
                } else if saved {
                    kit::success_text(ui, &label);
                } else {
                    kit::meta_text(ui, &label);
                }
            }
        }
    }
}

/// FormPage.tsx `Notes`: the sample note, profile-change lines, the use-profile-values offer,
/// amendment and filed strips, the profile-changed list and the form's own errors.
fn draw_notes(ui: &mut Ui, s: &mut FormSession) {
    let h = s.header_info();
    if h.sample {
        kit::meta_text(ui, schema::SAMPLE_NOTE);
    }
    if !h.profile_updates.is_empty() {
        kit::meta_text(ui, &schema::profile_updates_note(&h.profile_updates));
    }
    for n in &h.notes {
        kit::meta_text(ui, n);
    }
    if !h.profile_differs.is_empty() {
        ui.row(
            &cui::Row {
                width: cui::Size::FILL,
                gap: crate::theme::space(2),
                wrap: true,
                align: cui::Align::Center,
                ..Default::default()
            },
            |ui| {
                kit::meta_text(ui, &schema::profile_differs_note(&h.profile_differs));
                if ui.ghost_button("Use profile values", true) {
                    s.use_profile_values();
                }
            },
        );
    }
    if h.amends {
        kit::meta_text(
            ui,
            &format!("This return amends a filed one. {}", schema::FILED_KEPT),
        );
    }
    if let Some(view) = &h.filed_original_view {
        let when = view
            .filed_on
            .as_deref()
            .map(|d| format!(" on {}", crate::lib::dates::short_iso_date(d)))
            .unwrap_or_default();
        kit::meta_text(
            ui,
            &format!(
                "This is the version filed{when}, kept in this return's history; the current return is its amendment. Nothing here can be changed."
            ),
        );
    }
    if !h.profile_changed.is_empty() {
        kit::meta_text(ui, schema::PROFILE_CHANGED_NOTE);
        for c in &h.profile_changed {
            kit::meta_text(ui, &format!("· {c}"));
        }
    }
    if !h.submitted_line.is_empty() {
        kit::meta_text(ui, &h.submitted_line);
    }
    for e in &h.form_errors {
        kit::danger_text(ui, e);
    }
}

/// The wide "On this form" navigator: header with the part count and the expand-all toggle,
/// the needs-attention shortcut, then one pressable row per part.
fn draw_navigator(ui: &mut Ui, s: &mut FormSession, edge: f32, compact: bool) {
    let part_count = s.schema_part_count();
    let opens: Vec<bool> = (0..part_count).map(|i| s.part_info(i).open).collect();
    let open_one = opens
        .iter()
        .position(|o| *o)
        .filter(|_| opens.iter().filter(|o| **o).count() == 1);
    let needing = (0..part_count)
        .filter(|&i| s.part_info(i).errors > 0)
        .count();
    ui.column(
        &cui::Column {
            width: cui::Size::px(edge + 214.0),
            height: cui::Size::FILL,
            pad_left: edge,
            pad_top: 28.0,
            pad_bottom: 16.0,
            gap: crate::theme::space(1),
            ..Default::default()
        },
        |ui| {
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    gap: crate::theme::space(1),
                    align: cui::Align::Center,
                    pad_x: crate::theme::space(2),
                    pad_bottom: crate::theme::space(1),
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        "On this form",
                        &cui::Text {
                            font: cui::font::MEDIUM,
                            size: 12.0,
                            color: crate::ui::kit::frame_palette().subtle_foreground,
                            ..Default::default()
                        },
                    );
                    ui.box_(
                        &cui::BoxOpts {
                            width: cui::Size::FILL,
                            ..Default::default()
                        },
                        |_| {},
                    );
                    let count = match open_one {
                        Some(i) => format!("{} of {}", i + 1, part_count),
                        None => format!("{} parts", part_count),
                    };
                    kit::meta_text(ui, &count);
                    let icon = if s.all_parts_open() {
                        "chevrons-down-up"
                    } else {
                        "chevrons-up-down"
                    };
                    if kit::icon_button(ui, icon, Some("Expand or collapse all parts")) {
                        s.toggle_all_parts();
                    }
                },
            );
            if needing > 0 {
                let label = format!(
                    "{} part{} need{} attention · next ↓",
                    needing,
                    if needing > 1 { "s" } else { "" },
                    if needing > 1 { "" } else { "s" },
                );
                if kit::pressable_row(ui, "fp-nav-attention", |ui| {
                    ui.text_with(
                        &label,
                        &cui::Text {
                            size: 12.0,
                            color: crate::ui::kit::frame_palette().danger,
                            ..Default::default()
                        },
                    );
                }) {
                    if let Some(next) = s.next_needing_part() {
                        s.jump_to(next, None);
                    }
                }
            }
            ui.scroll(
                &cui::Scroll {
                    id: Some("fp-nav-list"),
                    ..Default::default()
                },
                |ui| {
                    for pi in 0..part_count {
                        let info = s.part_info(pi);
                        let name = s.part_title(pi);
                        let selected = open_one == Some(pi);
                        let err = info.errors;
                        let items = s.part_item_count(pi);
                        // NSH navRows: compact shows one 28pt line of the shortened name;
                        // otherwise the full name wraps (rows at least 32 tall).
                        let shown = if compact {
                            schema::compact_part_name(&name)
                        } else {
                            name.clone()
                        };
                        if kit::pressable_row(ui, &format!("fp-nav-{pi}"), |ui| {
                            ui.text_with(
                                &shown,
                                &cui::Text {
                                    size: 13.0,
                                    nowrap: compact,
                                    width: if compact {
                                        cui::Size::FILL
                                    } else {
                                        cui::Size::FIT
                                    },
                                    color: if selected {
                                        crate::ui::kit::frame_palette().foreground
                                    } else {
                                        crate::ui::kit::frame_palette().muted_foreground
                                    },
                                    ..Default::default()
                                },
                            );
                            if err > 0 {
                                kit::tone_badge(ui, &err.to_string(), "danger");
                            }
                            kit::tone_badge(ui, &items.to_string(), "neutral");
                        }) {
                            s.jump_to(pi, None);
                        }
                    }
                },
            );
        },
    );
}

/// The pill's two lines: the small cap (to-do progress or "tag · i of n") and the emph line
/// (the current to-do's item or the section's short name) — FormPage.tsx `small`/`big`.
fn pill_texts(s: &FormSession) -> (String, String) {
    let parts = s.schema.as_ref().map(|f| f.parts.as_slice()).unwrap_or(&[]);
    if parts.is_empty() {
        return (String::new(), String::new());
    }
    let cur = s.current().min(parts.len() - 1);
    let ids: Vec<String> = s.todo_now().iter().map(|r| r.id.clone()).collect();
    let (tag, _short) = schema::section_names(&parts[cur], cur);
    let small = if ids.is_empty() {
        format!("{} · {} of {}", tag, cur + 1, parts.len())
    } else {
        let at = s.step_at().and_then(|a| ids.iter().position(|i| i == a));
        match at {
            Some(i) => {
                let pi: usize = ids[i]
                    .split(':')
                    .next()
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);
                let (ptag, _) =
                    schema::section_names(&parts[pi.min(parts.len() - 1)], pi.min(parts.len() - 1));
                format!("{} of {} to do · {}", i + 1, ids.len(), ptag)
            }
            None => format!("{} to do · {}", ids.len(), tag),
        }
    };
    let big = match s.step_at() {
        Some(at) if ids.iter().any(|i| i == at) => s
            .item(at)
            .map(|it| {
                if it.no.is_empty() {
                    it.l.clone()
                } else {
                    format!("Item {} · {}", it.no, it.l)
                }
            })
            .unwrap_or_default(),
        _ => schema::section_names(&parts[cur], cur).1,
    };
    (small, big)
}

/// The step tooltip: "Check" on the check target, else to-do or section direction labels.
fn step_label(s: &FormSession, dir: i32) -> &'static str {
    match s.step_target(dir) {
        Some(crate::form::session::StepTarget::Check) => "Check",
        _ => {
            if s.todo_now().is_empty() {
                if dir < 0 {
                    "Previous section"
                } else {
                    "Next section"
                }
            } else if dir < 0 {
                "Previous to do"
            } else {
                "Next to do"
            }
        }
    }
}

/// The floating guide at the foot of the form column: ‹ prev, the sections list, the two-line
/// label with the attention count, › next (a check when the step ends on Check).
fn draw_pill(ui: &mut Ui, s: &mut FormSession, nav: bool, narrow: bool, form_w: f32) {
    let _ = narrow;
    let (small, big) = pill_texts(s);
    let attention = s.attention();
    let can_prev = s.step_target(-1).is_some();
    let can_next = s.step_target(1).is_some();
    let next_check = matches!(
        s.step_target(1),
        Some(crate::form::session::StepTarget::Check)
    );
    // The strip centres the pill over the form column (left 22, width formW) when the navigator
    // shows, over the pane with 16pt side padding otherwise.
    ui.row(
        &cui::Row {
            width: cui::Size::FILL,
            height: cui::Size::px(72.0),
            align: cui::Align::Center,
            justify: cui::Justify::Center,
            ..Default::default()
        },
        |ui| {
            if nav {
                ui.box_(
                    &cui::BoxOpts {
                        width: cui::Size::px(22.0 + form_w),
                        height: cui::Size::FILL,
                        align: cui::Align::Center,
                        justify: cui::Justify::Center,
                        ..Default::default()
                    },
                    |ui| {
                        draw_pill_bar(
                            ui, s, &small, &big, attention, can_prev, can_next, next_check,
                        )
                    },
                );
            } else {
                ui.box_(
                    &cui::BoxOpts {
                        width: cui::Size::FILL,
                        height: cui::Size::FILL,
                        pad_x: 16.0,
                        align: cui::Align::Center,
                        justify: cui::Justify::Center,
                        ..Default::default()
                    },
                    |ui| {
                        draw_pill_bar(
                            ui, s, &small, &big, attention, can_prev, can_next, next_check,
                        )
                    },
                );
            }
        },
    );
}

/// The pill itself: rounded-22 raised bar, ‹ list main › (FormPage.tsx `Pill`).
fn draw_pill_bar(
    ui: &mut Ui,
    s: &mut FormSession,
    small: &str,
    big: &str,
    attention: usize,
    can_prev: bool,
    can_next: bool,
    next_check: bool,
) {
    let p = crate::ui::kit::frame_palette();
    let pop_open = ui.popover_is_open("fp-sections");
    ui.box_(
        &cui::BoxOpts {
            width: cui::Size::FILL,
            max_width: 488.0,
            height: cui::Size::px(56.0),
            pad: 6.0,
            gap: 6.0,
            bg: p.raised,
            border: 1.0,
            border_color: p.border_strong,
            radius: 22.0,
            elevation: cui::Elevation::Floating,
            dir: cui::Dir::Row,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            if ui.icon_button_with(
                crate::ui::icons::glyph("chevron-left"),
                &cui::Button {
                    size: cui::ControlSize::Sm,
                    disabled: !can_prev,
                    tooltip: Some(step_label(s, -1)),
                    ..Default::default()
                },
            ) {
                s.step(-1);
            }
            // The list button toggles the sections popover; the popover anchors to it, so
            // popover_begin must come right after it is drawn.
            let list_pressed = ui.icon_button_with(
                crate::ui::icons::glyph("list"),
                &cui::Button {
                    size: cui::ControlSize::Sm,
                    selected: pop_open,
                    tooltip: Some("All sections"),
                    ..Default::default()
                },
            );
            if list_pressed {
                ui.popover_toggle("fp-sections");
            }
            // The two-line label, click opens the same list.
            let label_open = kit::pressable_row(ui, "fp-pill-open", |ui| {
                ui.column(
                    &cui::Column {
                        width: cui::Size::FILL,
                        gap: 0.0,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text_with(
                            small,
                            &cui::Text {
                                size: 11.0,
                                color: p.muted_foreground,
                                nowrap: true,
                                ..Default::default()
                            },
                        );
                        ui.text_with(
                            big,
                            &cui::Text {
                                font: cui::font::MEDIUM,
                                size: 14.0,
                                nowrap: true,
                                ..Default::default()
                            },
                        );
                    },
                );
            });
            if label_open {
                ui.popover_open("fp-sections");
            }
            if attention > 0 {
                kit::tone_badge(ui, &attention.to_string(), "danger");
            }
            if ui.icon_button_with(
                crate::ui::icons::glyph(if next_check {
                    "circle-check"
                } else {
                    "chevron-right"
                }),
                &cui::Button {
                    size: cui::ControlSize::Sm,
                    disabled: !can_next,
                    tooltip: Some(step_label(s, 1)),
                    ..Default::default()
                },
            ) {
                s.step(1);
            }
            // The popover anchors to the last widget drawn — the next icon at the pill's
            // right edge. TOP_END puts the list's right edge on the icon's right edge; at
            // 476pt (pill width - 12) it lands centred on the pill, like NSH's section list.
            // anchor_off lifts it the rest of the way: the icon's top sits 13pt under the
            // pill's, so 15 puts the list's bottom 2pt above the pill instead of inside it.
            // popover_begin must stay the last thing in the bar so the anchor is stable.
            let opts = cui_sys::cui_popover_opts {
                _pad: 0,
                placement: cui_sys::CUI_PLACE_TOP_END,
                width: 476.0,
                pad: 8.0,
                gap: 6.0,
                anchor_off: 15.0,
            };
            if ui.popover_begin("fp-sections", &opts) {
                // The page's press_hit widgets stay suppressed under the popover, but the
                // popover's own rows must still take clicks.
                let _m = kit::modal_off();
                draw_sections_list(ui, s);
            }
            ui.popover_end();
        },
    );
}

/// The sections list inside the pill's popover (FormPage.tsx `SectionsList`): the "On this
/// form" head with the expand-all toggle, then one row per part — accent bar on the current
/// open part, a danger "!" or an empty ring, the short name and tag, and the fix/todo count.
fn draw_sections_list(ui: &mut Ui, s: &mut FormSession) {
    let part_count = s.schema_part_count();
    let cur = s.current().min(part_count.saturating_sub(1));
    let needing = (0..part_count)
        .filter(|&i| s.part_info(i).errors > 0)
        .count();
    let title = if needing > 0 {
        format!(
            "{} section{} · {} need{} attention",
            part_count,
            if part_count > 1 { "s" } else { "" },
            needing,
            if needing > 1 { "" } else { "s" },
        )
    } else {
        format!(
            "{} section{}",
            part_count,
            if part_count > 1 { "s" } else { "" }
        )
    };
    ui.column(
        &cui::Column {
            width: cui::Size::px(460.0),
            gap: crate::theme::space(1),
            ..Default::default()
        },
        |ui| {
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    gap: crate::theme::space(2),
                    align: cui::Align::Start,
                    pad_left: crate::theme::space(2),
                    pad_right: crate::theme::space(1),
                    pad_bottom: crate::theme::space(2),
                    ..Default::default()
                },
                |ui| {
                    ui.column(
                        &cui::Column {
                            width: cui::Size::FILL,
                            ..Default::default()
                        },
                        |ui| {
                            kit::meta_text(ui, "On this form");
                            ui.text_with(
                                &title,
                                &cui::Text {
                                    font: cui::font::MEDIUM,
                                    size: 16.0,
                                    ..Default::default()
                                },
                            );
                        },
                    );
                    let icon = if s.all_parts_open() {
                        "chevrons-down-up"
                    } else {
                        "chevrons-up-down"
                    };
                    if kit::icon_button(ui, icon, Some("Expand or collapse all parts")) {
                        s.toggle_all_parts();
                    }
                },
            );
            // The popup caps its height (~420); long part lists scroll inside it.
            let list_h = (64.0 + part_count as f32 * 52.0).min(340.0);
            ui.scroll(
                &cui::Scroll {
                    id: Some("fp-sections-list"),
                    height: cui::Size::px(list_h),
                    ..Default::default()
                },
                |ui| {
                    for pi in 0..part_count {
                        let info = s.part_info(pi);
                        let (tag, short) = s
                            .schema
                            .as_ref()
                            .and_then(|f| f.parts.get(pi))
                            .map(|p| schema::section_names(p, pi))
                            .unwrap_or_default();
                        let _ = tag;
                        let lit = cur == pi && info.open;
                        let right = if info.errors > 0 {
                            format!("Fix · {}", info.errors)
                        } else if info.todo > 0 {
                            format!("{} to do", info.todo)
                        } else {
                            let n = s.part_item_count(pi);
                            format!("{} item{}", n, if n == 1 { "" } else { "s" })
                        };
                        if kit::pressable_row(ui, &format!("fp-section-{pi}"), |ui| {
                            let p = crate::ui::kit::frame_palette();
                            ui.box_(
                                &cui::BoxOpts {
                                    width: cui::Size::px(3.0),
                                    height: cui::Size::px(28.0),
                                    bg: if lit { p.accent_blue } else { p.raised },
                                    radius: 2.0,
                                    ..Default::default()
                                },
                                |_| {},
                            );
                            // The status ring: danger "!" when the part has errors, else an
                            // empty bordered circle.
                            ui.box_(
                                &cui::BoxOpts {
                                    width: cui::Size::px(18.0),
                                    height: cui::Size::px(18.0),
                                    bg: if info.errors > 0 { p.danger } else { p.raised },
                                    border: if info.errors > 0 { 0.0 } else { 2.0 },
                                    border_color: p.border_strong,
                                    radius: 9999.0,
                                    align: cui::Align::Center,
                                    justify: cui::Justify::Center,
                                    ..Default::default()
                                },
                                |ui| {
                                    if info.errors > 0 {
                                        ui.text_with(
                                            "!",
                                            &cui::Text {
                                                font: cui::font::MEDIUM,
                                                size: 11.0,
                                                color: p.primary_foreground,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                },
                            );
                            ui.column(
                                &cui::Column {
                                    width: cui::Size::FILL,
                                    ..Default::default()
                                },
                                |ui| {
                                    ui.text_with(
                                        &short,
                                        &cui::Text {
                                            font: cui::font::MEDIUM,
                                            size: 14.0,
                                            nowrap: true,
                                            ..Default::default()
                                        },
                                    );
                                    kit::meta_text(ui, &tag);
                                },
                            );
                            ui.text_with(
                                &right,
                                &cui::Text {
                                    font: cui::font::MEDIUM,
                                    size: 12.0,
                                    color: if info.errors > 0 {
                                        crate::ui::kit::frame_palette().danger
                                    } else {
                                        crate::ui::kit::frame_palette().muted_foreground
                                    },
                                    ..Default::default()
                                },
                            );
                        }) {
                            ui.popover_close("fp-sections");
                            s.jump_to(pi, None);
                        }
                    }
                },
            );
        },
    );
}

fn draw_part_items(ui: &mut Ui, app: &mut AppState, s: &mut FormSession, pi: usize) {
    let count = s.part_item_count(pi);
    for ii in 0..count {
        let id = format!("{pi}:{ii}");
        let Some(item) = s.item(&id).cloned() else {
            continue;
        };
        draw_item(ui, app, s, &id, &item);
    }
}

fn draw_item(ui: &mut Ui, app: &mut AppState, s: &mut FormSession, id: &str, it: &SchemaItem) {
    // Every item can carry the same "Edit"/"Done"/"Use it" buttons: scope them per item.
    let _g = ui.push_id(id.to_owned());
    let info = s.row_info(id);
    let card_tag = format!("fp-item-{id}");
    kit::card_styled_tagged(ui, Some(&card_tag), 8.0, 6.0, false, info.step_here, |ui| {
        if !it.l.is_empty() {
            kit::section_title(ui, &it.l);
        }
        if !info.message.is_empty() {
            kit::danger_text(ui, &info.message);
        }
        for n in &info.notes {
            kit::meta_text(ui, n);
        }
        for n in &info.needs {
            kit::warning_text(ui, n);
        }
        if !info.kept.is_empty() {
            kit::meta_text(ui, &format!("Kept from {}", info.kept));
        }
        if !info.from_profile.is_empty() {
            kit::meta_text(ui, &info.from_profile);
        }
        if info.can_edit && ui.ghost_button(if info.editing { "Done" } else { "Edit" }, true) {
            if info.editing {
                s.end_row_edit(id);
            } else {
                s.start_profile_edit(id);
            }
        }
        if let Some((on, note)) = &info.to_profile {
            let mut v = *on;
            if kit::checkbox(ui, "Save to profile too", &mut v) {
                s.set_to_profile(id, v);
            }
            kit::meta_text(ui, note);
        }
        if let Some(pen) = info.penalty {
            if pen == "edit" && ui.ghost_button("Edit penalties", true) {
                s.open_penalties(id);
            }
            if pen == "computed" && ui.ghost_button("Use computed amounts", true) {
                s.use_penalty_computed(id);
            }
        }
        if info.stoppable {
            let label = if info.is_stop {
                "Resume here"
            } else {
                "Stop here"
            };
            if ui.ghost_button(label, true) {
                s.set_stop(id, !info.is_stop);
            }
        }
        if !info.from.is_empty() {
            kit::meta_text(ui, &info.from);
            if ui.ghost_button("Use it", true) {
                s.use_from(id);
            }
        }
        for (token, label) in &info.checkable {
            if ui.ghost_button(label, true) {
                s.mark_checked(token);
            }
        }

        if it.rows.is_some() {
            draw_schedule(ui, app, s, id, it);
        } else {
            // An item the host marks one_date draws its month, day and year boxes as ONE date box
            // (rows.tsx); the three keys are skipped and the "<month>#date" key edits them together.
            let one = schema::one_date(it);
            let one_keys: Vec<String> = one
                .as_ref()
                .map(|o| {
                    [
                        Some(o.month.k.clone()),
                        o.day.as_ref().map(|d| d.k.clone()),
                        Some(o.year.k.clone()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                })
                .unwrap_or_default();
            // A date printed in split boxes gets a calendar button beside them — not when the item
            // has a date box of its own (that box carries its calendar).
            let split = if one.is_none() && !it.f.iter().any(|f| f.t == "date") {
                schema::date_parts(&it.f)
            } else {
                None
            };
            for f in &it.f {
                if f.t == "hidden" || f.t == "radio" {
                    continue;
                }
                // The one-date box stands where the month box was; day and year stay skipped.
                if let Some(o) = &one
                    && f.k == o.month.k
                {
                    let sf = SchemaField {
                        k: o.one_key.clone(),
                        l: o.month.l.clone(),
                        t: "date".to_owned(),
                        c: None,
                        r: None,
                        m: None,
                        o: None,
                        a: None,
                        w: None,
                        grp: None,
                        rg: None,
                        pool: None,
                    };
                    draw_field(ui, s, &sf, id);
                    continue;
                }
                if one_keys.contains(&f.k) {
                    continue;
                }
                draw_field(ui, s, f, id);
            }
            // The calendar beside the split boxes (rows.tsx `splitOpen`/`splitIso`/`splitRange`).
            if let Some(dp) = &split {
                let parts = [Some(&dp.month), dp.day.as_ref(), Some(&dp.year)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                let open =
                    s.editable() && !parts.iter().all(|f| f.c == Some(true) || s.is_shut(&f.k));
                if open {
                    let key = format!("split-date:{id}");
                    let _g = ui.push_id(key.clone());
                    let (min, max) = split_date_range(s, dp);
                    let current = split_iso(s, dp);
                    // NSH's CalendarButton trigger: a small icon-only calendar button (fields.tsx
                    // CalendarPicker), not a labelled ghost button.
                    if kit::icon_button(ui, "calendar", Some("Pick a date")) {
                        open_date_picker(ui, &key, &current, min, max);
                    }
                    if let Some(iso) = date_picker(ui, &key) {
                        s.pick_date(dp, &iso);
                    }
                }
            }
            // radio groups: one row of tick boxes per grp
            let mut groups: Vec<(String, Vec<&SchemaField>)> = Vec::new();
            for f in it.f.iter().filter(|f| f.t == "radio") {
                let g = f.grp.clone().unwrap_or_else(|| f.k.clone());
                if let Some((_, v)) = groups.iter_mut().find(|(k, _)| *k == g) {
                    v.push(f);
                } else {
                    groups.push((g, vec![f]));
                }
            }
            for (_, fs) in groups {
                for f in fs {
                    draw_radio(ui, s, f);
                }
            }
        }

        for ask in &info.asks {
            kit::section_title(ui, &ask.label);
            if !ask.note.is_empty() {
                kit::meta_text(ui, &ask.note);
            }
            if ask.control == "choice" {
                for c in &ask.choices {
                    let picked = ask.pick.as_ref() == Some(&c.value);
                    let label = format!("{}{}", if picked { "● " } else { "○ " }, c.label);
                    if ui.ghost_button(&label, true) {
                        s.pick_ask(&ask.fact, c.value.clone());
                    }
                }
            } else {
                let mut text = s.ask_text_of(&ask.fact);
                if ui.text_input(&format!("ask-{}", ask.fact), &mut text, &ask.label, false) {
                    s.type_ask(&ask.fact, &text);
                }
            }
            if ui.primary_button("Save", true) {
                s.save_ask(&ask.fact);
            }
            if !info.ask_saved.is_empty() {
                kit::meta_text(ui, &info.ask_saved);
            }
        }
        if let Some(card) = &info.election {
            match card {
                ElectionCard::Ask { question, pick } => {
                    kit::section_title(ui, question);
                    for (v, label) in [
                        ("graduated", "Graduated rates"),
                        ("eight_percent", "8% flat rate"),
                        ("not_applicable", "Not applicable"),
                    ] {
                        let on = pick.as_deref() == Some(v);
                        if ui.ghost_button(
                            &format!("{} {}", if on { "●" } else { "○" }, label),
                            true,
                        ) {
                            s.pick_election(v);
                        }
                    }
                    if ui.primary_button("Save", true) {
                        s.record_election();
                    }
                }
                ElectionCard::Blocked {
                    title,
                    body,
                    action,
                } => {
                    if !title.is_empty() {
                        kit::warning_text(ui, title);
                    }
                    if !body.is_empty() {
                        kit::meta_text(ui, body);
                    }
                    if let Some(a) = action {
                        let label = if *a == "fp-election-follow" {
                            "Update from tax profile"
                        } else {
                            "Try again"
                        };
                        if ui.ghost_button(label, true) {
                            s.election_action(a);
                        }
                    }
                }
            }
        }
        if !info.election_saved.is_empty() {
            kit::meta_text(ui, &info.election_saved);
        }
    });
    let _ = app;
}

// ---------------------------------------------------------------- fields

fn draw_field(ui: &mut Ui, s: &mut FormSession, f: &SchemaField, _item: &str) {
    let k = &f.k;
    let shut = s.is_shut(k);
    if shut {
        // locked/computed: shown text or the reason it is blank
        let text = s.shown_of(k);
        if text.is_empty() {
            let reason = s.lock_reason(k);
            if !reason.is_empty() {
                kit::meta_text(ui, &format!("{}: {}", f.l, reason));
            }
        } else {
            kit::meta_text(ui, &format!("{}: {}", f.l, text));
        }
        return;
    }
    let kind = f.t.as_str();
    match kind {
        "sel" => draw_select(ui, s, f),
        "chk" => {
            let mut v = s.value_of(k) == "true" || s.value_of(k) == "1";
            if kit::checkbox(ui, &f.l, &mut v) {
                let edits =
                    HashMap::from([(k.clone(), if v { "true" } else { "false" }.to_owned())]);
                s.choose(edits);
            }
        }
        "date" => draw_date(ui, s, f),
        "text" | "money" | "pct" | "int" | "tin" | _ => draw_text(ui, s, f),
    }
    let _ = kind;
}

/// A date box (controls/FieldControl.tsx `DateBox`): typed text that reads as a date, plus the
/// calendar a pick fills in — `pick` writes "MM/DD/YYYY" through the same edit path as typing.
fn draw_date(ui: &mut Ui, s: &mut FormSession, f: &SchemaField) {
    let k = f.k.clone();
    // NSH's DateField: the typed box and its calendar button share one row, the button at
    // the box's right edge — not a stray icon under the field.
    ui.row(
        &cui::Row {
            gap: 6.0,
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            draw_text_w(ui, s, f, Some(170.0));
            if !s.is_shut(&k) {
                let current = s.shown_of(&k);
                date_pick_button(ui, &k, &current, None, None);
            }
        },
    );
    if s.is_shut(&f.k) {
        return;
    }
    // popover_open ran inside the button's "pick-" id scope, so the picker must begin in the
    // same scope or cui_make_id gives it a different popup id and it never opens.
    let _g = ui.push_id(format!("pick-{k}"));
    if let Some(iso) = date_picker(ui, &k) {
        // "YYYY-MM-DD" -> "MM/DD/YYYY", the shape the typed-date parser reads.
        let parts: Vec<&str> = iso.split('-').collect();
        if parts.len() == 3 {
            let (y, m, d) = (parts[0], parts[1], parts[2]);
            let text = format!("{m}/{d}/{y}");
            s.focus(&k);
            s.set_shown(&k, &text);
            s.edit(&k, &text);
            s.blur(&k);
        }
    }
}

fn draw_text(ui: &mut Ui, s: &mut FormSession, f: &SchemaField) {
    draw_text_w(ui, s, f, None);
}

/// `draw_text` with an optional fixed width — the date box shares its row with the calendar
/// button, so it takes a compact width instead of filling the item.
fn draw_text_w(ui: &mut Ui, s: &mut FormSession, f: &SchemaField, width: Option<f32>) {
    let k = f.k.clone();
    let mut text = s.shown_of(&k);
    let prev = text.clone();
    let label = if f.l.is_empty() {
        k.clone()
    } else {
        f.l.clone()
    };
    let changed = match width {
        Some(w) => {
            if text.capacity() < 256 {
                text.reserve(256);
            }
            ui.input_with(
                &format!("fp-{k}"),
                &mut text,
                &cui::Input {
                    placeholder: Some(label.as_str()),
                    width: cui::Size::px(w),
                    ..Default::default()
                },
            )
            .changed()
        }
        None => ui.text_input(&format!("fp-{k}"), &mut text, &label, false),
    };
    if !changed {
        return;
    }
    if text == prev {
        return;
    }
    s.focus(&k);
    // mask while typing
    let kind = s.field_kind(&k);
    let max = s.field_max(&k);
    if let Some(mask) = mask::mask_for(&kind, true, max) {
        let m = mask::reconcile(&prev, &text, &mask, None, None, true);
        if m.rejected {
            s.set_shown(&k, &prev);
            return;
        }
        s.set_shown(&k, &m.text);
        s.edit(&k, &m.text);
    } else {
        s.set_shown(&k, &text);
        s.edit(&k, &text);
    }
    s.blur(&k);
}

fn draw_select(ui: &mut Ui, s: &mut FormSession, f: &SchemaField) {
    let k = f.k.clone();
    let (shown, options) = s.sel_shown(&k);
    let labels: Vec<String> = options.iter().map(|o| o.label.clone()).collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let cur = options.iter().position(|o| o.value == shown).unwrap_or(0) as i32;
    let mut idx = cur;
    let label = if f.l.is_empty() {
        k.clone()
    } else {
        f.l.clone()
    };
    if ui.select(&format!("fp-{k}"), &refs, &mut idx) && (idx as usize) < options.len() {
        let v = options[idx as usize].value.clone();
        if v != shown {
            let edits = HashMap::from([(k.clone(), v)]);
            s.choose(edits);
        }
    }
    let _ = label;
}

fn draw_radio(ui: &mut Ui, s: &mut FormSession, f: &SchemaField) {
    let k = f.k.clone();
    let mut v = s.value_of(&k) == "true" || s.value_of(&k) == "1";
    let label = if f.l.is_empty() {
        k.clone()
    } else {
        f.l.clone()
    };
    if kit::checkbox(ui, &label, &mut v) {
        s.pick_radio(&k);
    }
}

// ---------------------------------------------------------------- schedules
//
// rows.tsx `scheduleProps` + controls/Schedule.tsx + controls/model.ts: a schedule is NOT a sideways
// grid. One compact line per row that holds something (its number and the first meaningful cells); a
// press opens that row in a drawer (CUI's modal dialog — NSH's Sheet) with the row's boxes stacked,
// Delete row and Done. "Add row" opens the first empty row. A table whose every drawn column is
// computed is a totals table: its rows are not rows to open and stay visible under the list.

/// The schedule's drawn columns (hidden ones left out), with their index in `cols`.
fn schedule_columns<'a>(it: &'a SchemaItem) -> Vec<(usize, &'a crate::form::schema::SchemaColumn)> {
    it.cols
        .iter()
        .enumerate()
        .filter(|(_, c)| c.t != "hidden")
        .collect()
}

/// The drawer/list id of a schedule: `fp-table-<first key>` (rows.tsx's testID).
fn schedule_table_id(it: &SchemaItem, item_id: &str) -> String {
    format!(
        "fp-table-{}",
        it.keys
            .first()
            .and_then(|r| r.first())
            .cloned()
            .unwrap_or_else(|| item_id.to_owned())
    )
}

/// A row's printed number and preprinted caption (model.ts `rowParts`).
fn row_parts(it: &SchemaItem, r: usize) -> (i64, String) {
    let n = it.start.unwrap_or(1.0) as i64 + r as i64;
    let caption = it
        .row_labels
        .as_ref()
        .and_then(|l| l.rows.get(r))
        .cloned()
        .unwrap_or_default();
    (n, caption)
}

/// The drawer's title of a row: "Row 3", with its caption after a dot where the sheet prints one.
fn row_title(it: &SchemaItem, r: usize) -> String {
    let (n, caption) = row_parts(it, r);
    if caption.is_empty() {
        format!("Row {n}")
    } else {
        format!("Row {n} · {caption}")
    }
}

/// What a row is called in the list: its number, or its preprinted caption when the sheet prints one
/// and rows are not numbered (rows.tsx `rowName`).
fn row_name(it: &SchemaItem, r: usize) -> String {
    let numbered = it.row_labels.is_none() || it.numbered != Some(false);
    if numbered {
        row_parts(it, r).0.to_string()
    } else {
        let caption = it
            .row_labels
            .as_ref()
            .and_then(|l| l.rows.get(r))
            .cloned()
            .unwrap_or_default();
        if caption.is_empty() {
            row_parts(it, r).0.to_string()
        } else {
            caption
        }
    }
}

/// One schedule cell's text for a row's one-line summary (rows.tsx `summary`): "" when the cell is
/// empty, a sel's option label's first segment, a ticked box's column words, else the value.
fn cell_summary(s: &FormSession, k: &str, col: &crate::form::schema::SchemaColumn) -> String {
    let v = s.value_of(k);
    if schema::empty_cell(&v, &col.t) {
        return String::new();
    }
    if col.t == "sel" {
        let (_shown, options) = s.sel_shown(k);
        if let Some(o) = options.iter().find(|o| o.value == v) {
            return o.label.split(" — ").next().unwrap_or(&o.label).to_owned();
        }
        return v;
    }
    if v == "true" {
        return col.l.clone();
    }
    v
}

/// A row's one-line summary for the list ("RENT · 12,000.00"), "" when the row is empty.
fn row_summary(
    s: &FormSession,
    it: &SchemaItem,
    columns: &[(usize, &crate::form::schema::SchemaColumn)],
    r: usize,
) -> String {
    let Some(row) = it.keys.get(r) else {
        return String::new();
    };
    columns
        .iter()
        .filter_map(|(c, col)| row.get(*c).map(|k| cell_summary(s, k, col)))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A row holds something: any drawn column's cell is not empty (rows.tsx `filled`).
fn row_filled(
    s: &FormSession,
    it: &SchemaItem,
    columns: &[(usize, &crate::form::schema::SchemaColumn)],
    r: usize,
) -> bool {
    let Some(row) = it.keys.get(r) else {
        return false;
    };
    columns.iter().any(|(c, col)| {
        row.get(*c)
            .map(|k| !schema::empty_cell(&s.value_of(k), &col.t))
            .unwrap_or(false)
    })
}

/// The rows the list shows: every row holding something, plus the first empty one (rows.tsx
/// `shownRows`). "Add row" opens that first empty one.
fn shown_rows(filled: &[bool]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut empty = false;
    for (r, &f) in filled.iter().enumerate() {
        if f {
            out.push(r);
        } else if !empty {
            empty = true;
            out.push(r);
        }
    }
    out
}

fn open_row_get(table_id: &str) -> Option<usize> {
    OPEN_ROW.with(|m| m.borrow().get(table_id).copied().flatten())
}

fn open_row_set(table_id: &str, row: Option<usize>) {
    OPEN_ROW.with(|m| {
        m.borrow_mut().insert(table_id.to_owned(), row);
    });
}

/// The read-only text of an open row's cell for the totals card (controls/model.ts `readText`,
/// simplified to the session's shown text: the host already formats money, dates and choices).
fn cell_read_text(s: &FormSession, k: &str) -> String {
    let shown = s.shown_of(k);
    if shown.is_empty() {
        s.value_of(k)
    } else {
        shown
    }
}

fn draw_schedule(ui: &mut Ui, app: &mut AppState, s: &mut FormSession, id: &str, it: &SchemaItem) {
    let rows = it.keys.len();
    let columns = schedule_columns(it);
    let table_id = schedule_table_id(it, id);
    // One table per item: "Add row"/row buttons repeat, so scope them per table.
    let _g = ui.push_id(table_id.clone());
    let locked = !s.editable();

    // Precompute the per-frame row data (immutable borrows of the session end before drawing).
    let filled: Vec<bool> = (0..rows).map(|r| row_filled(s, it, &columns, r)).collect();
    let summaries: Vec<String> = (0..rows).map(|r| row_summary(s, it, &columns, r)).collect();
    let shown = shown_rows(&filled);
    // A table whose every drawn column is computed: its rows are totals, not something to type into.
    let total_table = !columns.is_empty() && columns.iter().all(|(_, c)| c.c == Some(true));
    // The first shown row that holds nothing is the one "Add row" opens.
    let empty_row = if total_table {
        None
    } else {
        shown.iter().copied().find(|&r| summaries[r].is_empty())
    };
    // The lines are the shown rows that hold something (and are not the empty "Add row" target).
    let lines: Vec<usize> = if total_table {
        Vec::new()
    } else {
        shown
            .iter()
            .copied()
            .filter(|&r| Some(r) != empty_row)
            .collect()
    };
    let totals: Vec<usize> = if total_table {
        (0..rows).collect()
    } else {
        Vec::new()
    };

    if lines.is_empty() && totals.is_empty() {
        kit::meta_text(ui, "No rows yet.");
    }
    for &r in &lines {
        let (n, caption) = row_parts(it, r);
        let mut label = format!("{n}  {}", summaries[r]);
        if !caption.is_empty() {
            label = format!("{n} · {caption} — {}", summaries[r]);
        }
        if ui.button_with(
            &label,
            &cui::Button {
                variant: cui::ButtonVariant::Secondary,
                icon_right: Some(crate::ui::icons::glyph("chevron-right")),
                ..Default::default()
            },
        ) {
            open_row_set(&table_id, Some(r));
        }
    }
    if !locked && let Some(r) = empty_row {
        if kit::secondary_button(ui, "+ Add row", true) {
            open_row_set(&table_id, Some(r));
        }
    }
    for &r in &totals {
        kit::card_styled(ui, 6.0, 4.0, true, false, |ui| {
            kit::meta_text(ui, &format!("{} (totals)", row_name(it, r)));
            for (c, col) in &columns {
                let Some(k) = it.keys.get(r).and_then(|row| row.get(*c)) else {
                    continue;
                };
                kit::meta_text(ui, &format!("{}: {}", col.l, cell_read_text(s, k)));
            }
        });
    }

    // The drawer: the open row's boxes stacked, Delete row and Done (controls/Schedule.tsx's
    // Sheet — a right drawer whose body scrolls, which a plain dialog cannot do).
    let open = open_row_get(&table_id);
    let mut dialog_open = open.is_some();
    let mut closed = false;
    if dialog_open {
        let title = open.map(|r| row_title(it, r)).unwrap_or_default();
        ui.sheet(
            &title,
            &mut dialog_open,
            &cui::Sheet {
                size: 400.0,
                ..Default::default()
            },
            |ui| {
                // The drawer's fields are the layer's own widgets — exempt them from the page guard.
                let _m = kit::modal_off();
                if let Some(r) = open {
                    for (c, col) in &columns {
                        let Some(k) = it.keys.get(r).and_then(|row| row.get(*c)) else {
                            continue;
                        };
                        let tick = col.t == "chk" || col.t == "radio";
                        if !tick {
                            kit::meta_text(ui, &col.l);
                        }
                        let f = SchemaField {
                            k: k.clone(),
                            l: col.l.clone(),
                            t: if col.t == "hidden" {
                                "text".to_owned()
                            } else {
                                col.t.clone()
                            },
                            c: col.c,
                            r: None,
                            m: None,
                            o: col.o.clone(),
                            a: None,
                            w: col.w,
                            grp: None,
                            rg: None,
                            pool: None,
                        };
                        match col.t.as_str() {
                            "sel" => draw_select(ui, s, &f),
                            "chk" => {
                                let mut v = matches!(s.value_of(k).as_str(), "true" | "1");
                                if kit::checkbox(ui, &col.l, &mut v) {
                                    let edits = HashMap::from([(
                                        k.clone(),
                                        if v { "true" } else { "false" }.to_owned(),
                                    )]);
                                    s.choose(edits);
                                }
                            }
                            "radio" => draw_radio(ui, s, &f),
                            _ => draw_field(ui, s, &f, id),
                        }
                    }
                }
                // dialog_footer does not draw in this CUI build — a plain row carries the actions.
                ui.row(
                    &cui::Row {
                        width: cui::Size::FILL,
                        justify: cui::Justify::End,
                        gap: crate::theme::space(2),
                        pad_top: crate::theme::space(2),
                        ..Default::default()
                    },
                    |ui| {
                        // Delete row clears the row's typed cells and closes the drawer (the session's choose
                        // may hold it for the removal confirm first, as the UI's edits do).
                        let can_delete = !locked && open.is_some_and(|r| !summaries[r].is_empty());
                        if kit::danger_button(ui, "Delete row", can_delete)
                            && let Some(r) = open
                        {
                            let edits: HashMap<String, String> = it
                                .keys
                                .get(r)
                                .map(|row| {
                                    row.iter()
                                        .enumerate()
                                        .filter(|(c, k)| {
                                            it.cols
                                                .get(*c)
                                                .map(|col| col.t != "hidden")
                                                .unwrap_or(false)
                                                && !s.is_shut(k)
                                        })
                                        .map(|(_, k)| (k.clone(), String::new()))
                                        .collect()
                                })
                                .unwrap_or_default();
                            if !edits.is_empty() {
                                s.choose(edits);
                            }
                            closed = true;
                        }
                        if kit::primary_button(ui, "Done", true) {
                            closed = true;
                        }
                    },
                );
            },
        );
    }
    // Any close — Done, Delete row, the dialog's own x — is onOpenRow(null): the item is touched.
    if closed || (!dialog_open && open.is_some()) {
        s.touch(id);
        open_row_set(&table_id, None);
    }
    let _ = app;
}

// ---------------------------------------------------------------- overlays

/// The "don't ask again for drafts" box of the existing-return offer (Dialog's `fast` signal).
fn offer_fast_lane() -> bool {
    OFFER_FAST.with(|c| c.get())
}

fn draw_overlay(ui: &mut Ui, app: &mut AppState, s: &mut FormSession) {
    let overlay = s.overlay().cloned();
    let Some(ov) = overlay else {
        OFFER_FAST.with(|c| c.set(false));
        return;
    };
    kit::card_styled(ui, 12.0, 8.0, false, true, |ui| match &ov {
        Overlay::Removal { message } => {
            kit::section_title(ui, message);
            if ui.primary_button("Continue", true) {
                s.confirm_removal();
            }
            if ui.ghost_button("Cancel", true) {
                s.cancel_removal();
            }
        }
        Overlay::Existing { existing } => {
            let offer = schema::existing_offer_for(existing);
            kit::section_title(ui, &offer.title);
            if !offer.subtitle.is_empty() {
                kit::meta_text(ui, &offer.subtitle);
            }
            for c in &offer.choices {
                let label = format!("{} — {}", c.label, c.hint);
                let hit = match c.variant {
                    "primary" => ui.primary_button(&label, true),
                    "danger" => kit::danger_button(ui, &label, true),
                    _ => ui.ghost_button(&label, true),
                };
                if hit {
                    s.take_existing(c.id, false);
                }
            }
            if offer.fast_lane {
                let mut on = offer_fast_lane();
                if ui.checkbox_with(
                    "Don't ask again for drafts (this session)",
                    &mut on,
                    &cui::Check::default(),
                ) {
                    OFFER_FAST.with(|c| c.set(on));
                    s.set_fast_lane(on);
                }
            }
        }
        Overlay::QueueConfirm { warns } => {
            kit::section_title(ui, "Queue this return?");
            for w in warns {
                kit::warning_text(ui, w);
            }
            if ui.primary_button("Continue", true) {
                s.confirm_queue();
            }
            if ui.ghost_button("Cancel", true) {
                s.cancel_queue_confirm();
            }
        }
        Overlay::SamplePeriod { parts, error } => {
            kit::section_title(ui, "Which period is this return for?");
            kit::meta_text(ui, "Save makes this sample your return for that period.");
            let mut values: HashMap<String, String> = parts
                .iter()
                .map(|p| (p.key.clone(), p.value.clone()))
                .collect();
            for p in parts.iter().filter(|p| p.part != "quarter") {
                let mut v = values.get(&p.key).cloned().unwrap_or_default();
                let label = schema::period_part_label(&p.part);
                if ui.text_input(&format!("period-{}", p.key), &mut v, label, false) {
                    values.insert(p.key.clone(), v);
                }
            }
            let quarters: Vec<&SamplePeriodPart> =
                parts.iter().filter(|p| p.part == "quarter").collect();
            if !quarters.is_empty() {
                kit::meta_text(ui, schema::period_part_label("quarter"));
                ui.row(
                    &cui::Row {
                        width: cui::Size::FILL,
                        gap: crate::theme::space(2),
                        wrap: true,
                        ..Default::default()
                    },
                    |ui| {
                        for p in &quarters {
                            let on = values.get(&p.key).map(String::as_str) == Some("true");
                            let label = format!("Q{}", p.quarter.unwrap_or(0));
                            let hit = if on {
                                ui.primary_button(&label, true)
                            } else {
                                kit::small_button(ui, &label, cui::ButtonVariant::Secondary, None)
                            };
                            if hit {
                                for q in &quarters {
                                    values.insert(
                                        q.key.clone(),
                                        if q.key == p.key { "true" } else { "false" }.to_owned(),
                                    );
                                }
                            }
                        }
                    },
                );
            }
            if !error.is_empty() {
                kit::danger_text(ui, error);
            }
            if ui.ghost_button("Cancel", true) {
                s.close_sample_period();
            }
            if ui.primary_button("Save", true) {
                s.take_sample_period(values);
            }
        }
    });
    let _ = app;
}

/// Leave hooks and retarget handling live in state.rs; the session resets when the route's target changes.
pub fn reset_for_session() {
    SESSION.with(|c| *c.borrow_mut() = None);
    SESSION_KEY.with(|k| *k.borrow_mut() = String::new());
}

/// Whether the open form's pill can step ‹ › (formUi.canStep): a schema is up and a target exists.
pub fn can_step() -> bool {
    with_session(|s| s.step_target(1).is_some() || s.step_target(-1).is_some()).unwrap_or(false)
}

/// The palette's ‹ › actions (formUi.step).
pub fn step(dir: i32) {
    with_session(|s| s.step(dir));
}
