//! The form page: renders `form::session::FormSession` — the port of `nsh/screens/form/`.
//! The session owns all state and host calls; this file is the immediate-mode draw of it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use chrono::Datelike;
use cui::Ui;
use serde_json::{Value, json};

use crate::form::schema::{self, SchemaField, SchemaItem};
use crate::form::session::{ElectionCard, FormSession, Overlay, SessionEvent, UndoCmd};
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

/// The calendar beside a date box (NSH's CalendarButton/DateField calendar): which field it is for
/// and the month on screen. `min`/`max` are the days the calendar may offer (rows.tsx `dateRange`).
struct DatePicker {
    field: String,
    year: i32,
    month: u32,
    min: Option<chrono::NaiveDate>,
    max: Option<chrono::NaiveDate>,
}

/// Opens the picker for `field` showing the month of `current` (a typed date) or today.
fn open_date_picker(
    field: &str,
    current: &str,
    min: Option<chrono::NaiveDate>,
    max: Option<chrono::NaiveDate>,
) {
    let today = chrono::Local::now().date_naive();
    let (mut year, mut month) = (today.year(), today.month());
    if let Some(d) = crate::lib::dates::parse_typed_date(current) {
        year = d.y;
        month = d.m;
    }
    DATE_PICKER.with(|p| {
        *p.borrow_mut() = Some(DatePicker {
            field: field.to_owned(),
            year,
            month,
            min,
            max,
        });
    });
}

/// Draws the picker while it is open for `field`; returns the picked day as "YYYY-MM-DD".
fn date_picker(ui: &mut Ui, field: &str) -> Option<String> {
    let open = DATE_PICKER.with(|p| p.borrow().as_ref().is_some_and(|d| d.field == field));
    if !open {
        return None;
    }
    let mut picked: Option<String> = None;
    let mut dialog_open = true;
    let title = DATE_PICKER.with(|p| {
        p.borrow()
            .as_ref()
            .map(|d| format!("Pick a date — {}-{:02}", d.year, d.month))
            .unwrap_or_default()
    });
    ui.dialog(&title, &mut dialog_open, |ui| {
        use chrono::Datelike;
        // ‹ Month Year ›
        ui.row(&Default::default(), |ui| {
            if kit::icon_button(ui, "chevron-left", Some("Previous month")) {
                DATE_PICKER.with(|p| {
                    if let Some(d) = p.borrow_mut().as_mut() {
                        if d.month == 1 {
                            d.month = 12;
                            d.year -= 1;
                        } else {
                            d.month -= 1;
                        }
                    }
                });
            }
            let label = DATE_PICKER.with(|p| {
                p.borrow()
                    .as_ref()
                    .map(|d| {
                        const MONTHS: [&str; 12] = [
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
                        format!("{} {}", MONTHS[(d.month - 1) as usize], d.year)
                    })
                    .unwrap_or_default()
            });
            kit::section_title(ui, &label);
            if kit::icon_button(ui, "chevron-right", Some("Next month")) {
                DATE_PICKER.with(|p| {
                    if let Some(d) = p.borrow_mut().as_mut() {
                        if d.month == 12 {
                            d.month = 1;
                            d.year += 1;
                        } else {
                            d.month += 1;
                        }
                    }
                });
            }
        });
        // The month grid, one row per week. Days outside min/max stay visible but cannot be picked
        // (NSH's CalendarButton dims them).
        let (year, month, min, max) = DATE_PICKER.with(|p| {
            p.borrow()
                .as_ref()
                .map(|d| (d.year, d.month, d.min, d.max))
                .unwrap_or((0, 1, None, None))
        });
        let Some(first) = chrono::NaiveDate::from_ymd_opt(year, month, 1) else {
            return;
        };
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
        let cells = offset + days_in as i64;
        let weeks = (cells + 6) / 7;
        ui.row(&Default::default(), |ui| {
            for head in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"] {
                kit::meta_text(ui, head);
            }
        });
        for w in 0..weeks {
            ui.row(&Default::default(), |ui| {
                for i in 0..7 {
                    let idx = w * 7 + i - offset;
                    if !(0..days_in as i64).contains(&idx) {
                        kit::meta_text(ui, "");
                        continue;
                    }
                    let day = (idx + 1) as u32;
                    let date = chrono::NaiveDate::from_ymd_opt(year, month, day);
                    let Some(date) = date else {
                        continue;
                    };
                    let allowed = min.map(|m| date >= m).unwrap_or(true)
                        && max.map(|m| date <= m).unwrap_or(true);
                    let label = day.to_string();
                    if allowed {
                        if ui.button(&label) {
                            picked = Some(format!("{:04}-{:02}-{:02}", year, month, day));
                        }
                    } else {
                        let _dim = ui.disabled(true);
                        let _ = ui.button(&label);
                    }
                }
            });
        }
    });
    if picked.is_some() || !dialog_open {
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
        open_date_picker(field, current, min, max);
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
    // header line: status + flags + save state
    let mut line = vec![h.status.clone()];
    if h.sample {
        line.push("Sample".to_owned());
    }
    if h.period_set {
        line.push("period set".to_owned());
    }
    if h.amends {
        line.push("Amendment".to_owned());
    }
    kit::section_title(ui, &line.join(" · "));
    match s.save_state() {
        crate::form::session::SaveState::Saving => kit::meta_text(ui, "Saving…"),
        st => {
            let label = st.label();
            if !label.is_empty() {
                kit::meta_text(ui, &label);
            }
        }
    }
    if !h.submitted_line.is_empty() {
        kit::meta_text(ui, &h.submitted_line);
    }
    if h.amend_unsaved {
        kit::warning_text(ui, "This amendment is not saved yet.");
    }
    for n in &h.notes {
        kit::meta_text(ui, n);
    }
    for p in &h.profile_updates {
        kit::meta_text(ui, p);
    }
    for d in &h.profile_differs {
        kit::warning_text(ui, d);
    }
    for c in &h.profile_changed {
        kit::meta_text(ui, c);
    }
    for e in &h.form_errors {
        kit::danger_text(ui, e);
    }

    // action bar
    kit::card_styled(ui, 8.0, 8.0, false, false, |ui| {
        if ui.ghost_button("Back", true) {
            app.back();
        }
        if s.editable() && ui.button("Check") {
            s.check();
        }
        if s.editable()
            && (h.amend_unsaved || h.sample || s.is_dirty() || !s.is_saved())
            && ui.primary_button("Save", true)
        {
            s.save_now();
        }
        if s.editable() && ui.button("Queue for sending") {
            s.queue();
        }
        if ui.button("Print") {
            s.print();
        }
        if !h.sample && ui.ghost_button("Cancel queue", true) {
            s.cancel_queue();
        }
        let att = s.attention();
        if att > 0 && ui.danger_button(&format!("{att} to fix"), true) {
            s.check();
        }
    });

    // step pill
    kit::card_styled(ui, 6.0, 6.0, true, false, |ui| {
        if kit::icon_button(ui, "chevron-left", Some("Previous part")) {
            s.step(-1);
        }
        kit::meta_text(
            ui,
            &format!("Part {} of {}", s.current() + 1, s.row_count().max(1)),
        );
        if kit::icon_button(ui, "chevron-right", Some("Next part")) {
            s.step(1);
        }
        if s.all_parts_open() {
            if ui.ghost_button("Collapse all", true) {
                s.toggle_all_parts();
            }
        } else if ui.ghost_button("Expand all", true) {
            s.toggle_all_parts();
        }
    });

    // parts (the page's scroll area; the header, action bar and step pill stay put)
    ui.scroll(&Default::default(), |ui| {
        let part_count = s.schema_part_count();
        for pi in 0..part_count {
            let info = s.part_info(pi);
            let title = s.part_title(pi);
            kit::card_styled(ui, 8.0, 6.0, false, info.open, |ui| {
                let head = format!(
                    "{}{}{}",
                    title,
                    if info.errors > 0 {
                        format!(
                            "  ·  {} error{}",
                            info.errors,
                            if info.errors == 1 { "" } else { "s" }
                        )
                    } else {
                        String::new()
                    },
                    if info.todo > 0 {
                        format!("  ·  {} to do", info.todo)
                    } else {
                        String::new()
                    },
                );
                if ui.ghost_button(&head, true) {
                    s.toggle_part(pi);
                }
                if !info.open {
                    return;
                }
                if let Some(fold) = &info.fold {
                    let label = format!(
                        "{} ({})",
                        fold.label,
                        if fold.open { "hide" } else { "show" }
                    );
                    if ui.ghost_button(&label, true) {
                        s.toggle_profile_fold(pi);
                    }
                    if !fold.open {
                        // tucked items stay hidden
                    }
                }
                draw_part_items(ui, app, s, pi);
            });
        }
    });

    draw_overlay(ui, app, s);
    let _ = app;
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
    kit::card_styled(ui, 8.0, 6.0, false, info.step_here, |ui| {
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
                    if kit::icon_ghost_button(ui, "calendar", "Pick a date", true) {
                        open_date_picker(&key, &current, min, max);
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
    draw_text(ui, s, f);
    if s.is_shut(&f.k) {
        return;
    }
    let k = f.k.clone();
    let current = s.shown_of(&k);
    date_pick_button(ui, &k, &current, None, None);
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
    let k = f.k.clone();
    let mut text = s.shown_of(&k);
    let prev = text.clone();
    let label = if f.l.is_empty() {
        k.clone()
    } else {
        f.l.clone()
    };
    if !ui.text_input(&format!("fp-{k}"), &mut text, &label, false) {
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

    // The drawer: the open row's boxes stacked, Delete row and Done (controls/Schedule.tsx's Sheet).
    let open = open_row_get(&table_id);
    let mut dialog_open = open.is_some();
    let mut closed = false;
    if dialog_open {
        let title = open.map(|r| row_title(it, r)).unwrap_or_default();
        ui.dialog(&title, &mut dialog_open, |ui| {
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
            ui.dialog_footer(|ui| {
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
            });
        });
    }
    // Any close — Done, Delete row, the dialog's own x — is onOpenRow(null): the item is touched.
    if closed || (!dialog_open && open.is_some()) {
        s.touch(id);
        open_row_set(&table_id, None);
    }
    let _ = app;
}

// ---------------------------------------------------------------- overlays

fn draw_overlay(ui: &mut Ui, app: &mut AppState, s: &mut FormSession) {
    let overlay = s.overlay().cloned();
    let Some(ov) = overlay else {
        return;
    };
    kit::card_styled(ui, 12.0, 8.0, false, true, |ui| match &ov {
        Overlay::Removal { message } => {
            kit::section_title(ui, "Remove typed rows?");
            kit::meta_text(ui, message);
            if ui.primary_button("Remove rows", true) {
                s.confirm_removal();
            }
            if ui.ghost_button("Keep editing", true) {
                s.cancel_removal();
            }
        }
        Overlay::Existing { existing } => {
            kit::section_title(ui, &format!("{} already has this period", existing.label));
            kit::meta_text(ui, &format!("Status: {}", existing.status));
            for action in &existing.actions {
                let label = match action.as_str() {
                    "load" => "Load the saved draft",
                    "view" => "View read-only",
                    "amend" => "Amend it",
                    "keep" => "Keep this form",
                    other => other,
                };
                if ui.ghost_button(label, true) {
                    s.take_existing(action, false);
                }
            }
            if ui.ghost_button("Cancel", true) {
                s.take_existing("keep", false);
            }
        }
        Overlay::QueueConfirm { warns } => {
            kit::section_title(ui, "Queue this return?");
            for w in warns {
                kit::warning_text(ui, w);
            }
            if ui.primary_button("Queue", true) {
                s.confirm_queue();
            }
            if ui.ghost_button("Back", true) {
                s.cancel_queue_confirm();
            }
        }
        Overlay::SamplePeriod { parts, error } => {
            kit::section_title(ui, "Set the period");
            if !error.is_empty() {
                kit::danger_text(ui, error);
            }
            let mut values: HashMap<String, String> = parts
                .iter()
                .map(|p| (p.key.clone(), p.value.clone()))
                .collect();
            for p in parts {
                let mut v = values.get(&p.key).cloned().unwrap_or_default();
                let label = format!("{} period", p.part);
                if ui.text_input(&format!("period-{}", p.key), &mut v, &label, false) {
                    values.insert(p.key.clone(), v);
                }
            }
            if ui.primary_button("Save", true) {
                s.take_sample_period(values);
            }
            if ui.ghost_button("Cancel", true) {
                s.close_sample_period();
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
