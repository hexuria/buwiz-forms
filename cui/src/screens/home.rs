//! Home (port of `nsh/screens/home/`): the greeting, five status tiles, the next 30 days, the
//! deadlines they pick, each taxpayer at a glance, the as-needed forms, recently filed; on a fresh
//! install, the welcome with three numbered steps.

use chrono::{Datelike, Timelike};
use cui::{Align, Ui};
use serde_json::json;

use crate::lib::dashboard::{self, Dashboard, HomeView};
use crate::lib::dates::{MONTHS, MONTHS_LONG, days_until, short_date};
use crate::state::ViewLayout;
use crate::state::{
    AppState, OpenReturnOptions, ViewName, as_needed, cancel_queue, content_column, deadlines,
    returns_filed, saved_returns, today_iso,
};
use crate::types::{AsNeededForm, AsNeededReturn, DeadlineRow, ReturnRow};
use crate::ui::kit::UiExt;

/// Home's pick state (home/pick.ts): survives the view on purpose — Back shows the list you left.
thread_local! {
    static HOME_PICK: std::cell::RefCell<HomeView> = std::cell::RefCell::new(HomeView::default());
}

fn pick() -> HomeView {
    HOME_PICK.with(|p| p.borrow().clone())
}

fn set_pick(p: HomeView) {
    HOME_PICK.with(|cell| *cell.borrow_mut() = p);
}

fn toggle_tile(key: &str) {
    let mut p = pick();
    if p.pick.as_deref() == Some(key) {
        p.pick = None;
    } else {
        p.pick = Some(key.to_owned());
        p.day = None;
    }
    set_pick(p);
}

fn toggle_day(iso: &str) {
    let mut p = pick();
    if p.pick.as_deref() == Some("day") && p.day.as_deref() == Some(iso) {
        p.pick = None;
    } else {
        p.pick = Some("day".to_owned());
        p.day = Some(iso.to_owned());
    }
    set_pick(p);
}

fn clear_pick() {
    set_pick(HomeView::default());
}

/// Reset on session change (home/pick.ts's onSessionChange).
pub fn reset_for_session() {
    clear_pick();
}

fn greeting(now: &chrono::DateTime<chrono::Local>) -> &'static str {
    let h = now.hour();
    if h < 12 {
        "Good morning"
    } else if h < 18 {
        "Good afternoon"
    } else {
        "Good evening"
    }
}

fn long_today(now: &chrono::DateTime<chrono::Local>) -> String {
    const WEEKDAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    format!(
        "{}, {} {}, {}",
        WEEKDAYS[now.weekday().num_days_from_sunday() as usize],
        MONTHS_LONG[(now.month() - 1) as usize],
        now.day(),
        now.year()
    )
}

/// The page's width before its first layout.
fn first_width(app: &AppState) -> f32 {
    if app.narrow() {
        app.window_width - 32.0
    } else {
        content_column(ViewLayout::Page, app.main_width())
    }
}

/// Tiles per row: five from 640 wide, three from 420, two under it.
fn tile_columns(width: f32) -> usize {
    if width >= 640.0 {
        5
    } else if width >= 420.0 {
        3
    } else {
        2
    }
}

const TILE_GAP: f32 = 8.0;

fn load(app: &AppState) -> Dashboard {
    dashboard::load_dashboard(
        &|from, to| deadlines(from, to),
        &|| saved_returns(),
        &|| returns_filed(),
        &|| app.taxpayers(),
        chrono::Local::now(),
    )
}

// ---------------------------------------------------------------- welcome

const STEPS: [(&str, &str, &str); 3] = [
    (
        "user",
        "Add a taxpayer",
        "TIN, registered name and head-office address",
    ),
    (
        "calendar",
        "Add a tax year",
        "Any year up to next year, per branch",
    ),
    (
        "file-text",
        "Choose its forms",
        "All 43 BIR forms, or start from a pack",
    ),
];

// ---------------------------------------------------------------- dashboard

/// A row of the as-needed list: a form's own row, or one of its returns that holds no trigger date yet.
struct AsNeededEntry {
    key: String,
    form: AsNeededForm,
    undated: Option<AsNeededReturn>,
}

fn as_needed_entries(forms: Vec<AsNeededForm>) -> Vec<AsNeededEntry> {
    let mut out = Vec::new();
    for form in forms {
        let id = format!("as-needed-{}-{}", form.tin, form.form_code);
        out.push(AsNeededEntry {
            key: id.clone(),
            form: form.clone(),
            undated: None,
        });
        for r in &form.undated {
            out.push(AsNeededEntry {
                key: format!("{id}-{}", r.draft_id),
                form: form.clone(),
                undated: Some(r.clone()),
            });
        }
    }
    out
}

/// The tile's press: Overdue and Due in 7 days narrow the list below; the others open Filings.
fn press_tile(app: &mut AppState, spec_key: &str, today: &str) {
    match spec_key {
        "overdue" | "week" => toggle_tile(spec_key),
        "drafts" => {
            let mut p = serde_json::Map::new();
            p.insert("statuses".into(), json!(["Draft"]));
            app.go(ViewName::Filings, p);
        }
        "waiting" => {
            let mut p = serde_json::Map::new();
            p.insert("statuses".into(), json!(["Queued"]));
            app.go(ViewName::Filings, p);
        }
        _ => {
            let mut p = serde_json::Map::new();
            p.insert("statuses".into(), json!(["Submitted", "Confirmed", "Paid"]));
            p.insert("updatedMonth".into(), json!(today[..7].to_owned()));
            app.go(ViewName::Filings, p);
        }
    }
}

/// Who a row belongs to and how a row names its owner (home/rows.ts, in lib::filings_model).
fn who_line(branch_count: i64, taxpayer_name: &str, branch_name: &str) -> String {
    crate::lib::filings_model::home_who_line(branch_count, taxpayer_name, branch_name)
}

fn status_kind(status: &str) -> &'static str {
    crate::lib::filings_model::status_kind(status)
}

fn status_label(status: &str) -> &'static str {
    match status_kind(status) {
        "draft" => "Draft",
        "queued" => "Queued",
        "filed" => "Filed",
        _ => "Not started",
    }
}

fn deadline_key(d: &DeadlineRow) -> String {
    crate::lib::filings_model::deadline_key(d)
}

// row actions (home/actions.ts)

fn open_return_opts(
    tin: &str,
    code: &str,
    year: i64,
    period: i64,
    return_key: Option<i64>,
) -> OpenReturnOptions {
    OpenReturnOptions {
        tin: tin.to_owned(),
        code: code.to_owned(),
        year,
        period,
        return_key: return_key.map(serde_json::Value::from),
        mode: None,
        snapshot_id: None,
        new_key: None,
        new_key_from: None,
    }
}

fn open_deadline(app: &mut AppState, d: &DeadlineRow) {
    app.open_return(open_return_opts(
        &d.tin,
        &d.form_code,
        d.taxable_year,
        d.period,
        d.return_key,
    ));
}

fn open_listed(app: &mut AppState, r: &ReturnRow) {
    app.open_return(open_return_opts(
        &r.tin,
        &r.form_code,
        r.taxable_year,
        r.period,
        r.return_key,
    ));
}

fn use_as_template(app: &mut AppState, r: &ReturnRow) {
    match crate::store::write("create_template_from_return", vec![json!(r.id)]) {
        Ok(_) => {
            app.toast(
                format!("{} template saved from {}.", r.form_code, r.period_label),
                Some("layout-template"),
                None,
            );
        }
        Err(e) => {
            app.toast(
                crate::lib::errors::error_text(&e),
                Some("triangle-alert"),
                None,
            );
        }
    }
}

fn select_as_needed(app: &mut AppState, f: &AsNeededForm) {
    app.select_taxpayer(&f.tin_root, true, Some(f.branch_code.clone()));
    app.select_year(f.taxable_year);
}

fn start_as_needed(app: &mut AppState, f: &AsNeededForm) {
    select_as_needed(app, f);
    app.start_new_return(Some(&f.form_code));
}

fn open_undated(app: &mut AppState, f: &AsNeededForm, r: &AsNeededReturn) {
    select_as_needed(app, f);
    app.open_return(open_return_opts(
        &f.tin,
        &f.form_code,
        f.taxable_year,
        r.period,
        r.return_key,
    ));
}

// ---------------------------------------------------------------- drawing

// ---------------------------------------------------------------- drawing

use cui::{BoxOpts, ButtonVariant, Color, MouseButton, Size, Tone};

use crate::ui::kit;
use crate::ui::logic::status_chip;

fn tone_color(ui: &mut Ui, tone: &str) -> Color {
    let p = kit::frame_palette();
    match tone {
        "danger" => p.danger,
        "warning" => p.warning,
        "accent" => p.accent_blue,
        "success" => p.success,
        "fg2" => p.muted_foreground,
        _ => p.foreground,
    }
}

fn chip_for(status: &str) -> crate::ui::logic::StatusChip {
    status_chip(status, None)
}

/// A small step circle for the welcome list (ui/status.tsx's StepCircle).
fn step_circle(ui: &mut Ui, n: usize) {
    let p = kit::frame_palette();
    ui.box_(
        &BoxOpts {
            width: Size::px(22.0),
            height: Size::px(22.0),
            radius: crate::theme::radius::FULL,
            bg: p.accent_blue,
            align: Align::Center,
            justify: cui::Justify::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &(n + 1).to_string(),
                &cui::Text {
                    size: crate::theme::r#type::CAP.0,
                    color: p.primary_foreground,
                    ..Default::default()
                },
            );
        },
    );
}

// ---------------------------------------------------------------- tiles

/// One of the five counts (parts.tsx's StatTile): icon and label, the number, a caption.
fn draw_stat_tile(ui: &mut Ui, spec: &dashboard::TileSpec, width: f32, picked: bool) -> bool {
    let _g = ui.push_id(format!("tile-{}", spec.key));
    ui.card(
        &cui::Card {
            width: Size::px(width),
            pad: crate::theme::space(3),
            gap: crate::theme::space(1),
            hoverable: true,
            selected: picked,
            ..Default::default()
        },
        |ui| {
            ui.row(
                &cui::Row {
                    gap: 6.0,
                    ..Default::default()
                },
                |ui| {
                    let color = tone_color(ui, spec.tone);
                    ui.icon(crate::ui::icons::glyph(spec.icon), 14.0, color);
                    ui.text_with(
                        &spec.label,
                        &cui::Text {
                            size: crate::theme::r#type::META.0,
                            color: kit::frame_palette().muted_foreground,
                            ..Default::default()
                        },
                    );
                    ui.spacer();
                    if picked {
                        let c = kit::frame_palette().subtle_foreground;
                        ui.icon(crate::ui::icons::glyph("x"), 14.0, c);
                    }
                },
            );
            ui.text_with(
                &spec.n.to_string(),
                &cui::Text {
                    size: crate::theme::r#type::PAGE.0,
                    tabular: true,
                    ..Default::default()
                },
            );
            ui.text_with(
                &spec.sub,
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: kit::frame_palette().subtle_foreground,
                    ..Default::default()
                },
            );
        },
    )
}

// ---------------------------------------------------------------- strip

/// One dot of the strip: accent for a draft, warning within a week, quiet after (parts.tsx's Dot).
fn strip_dot(ui: &mut Ui, d: &DeadlineRow) {
    let c = if d.status == "Draft" {
        kit::frame_palette().accent_blue
    } else if days_until(&d.due_date, chrono::Local::now()) <= 7 {
        kit::frame_palette().warning
    } else {
        kit::frame_palette().subtle_foreground
    };
    ui.box_(
        &BoxOpts {
            width: Size::px(8.0),
            height: Size::px(8.0),
            radius: crate::theme::radius::FULL,
            bg: c,
            ..Default::default()
        },
        |_| {},
    );
}

/// The next 30 days (parts.tsx's DeadlineStrip): a bordered strip, a column per day with a dot
/// per open deadline and the date under it; the first and last day are named below.
fn draw_strip(ui: &mut Ui, data: &Dashboard, view: &HomeView, width: f32) -> Option<String> {
    let every_day = width >= 560.0;
    let mut picked_day: Option<String> = None;
    let mouse = ui.mouse_pos();
    let clicked = ui.mouse_clicked(MouseButton::Left);
    let pal = kit::frame_palette();
    let cell_ids: Vec<String> = data
        .strip
        .iter()
        .map(|d| format!("strip-{}", d.iso))
        .collect();

    ui.box_(
        &BoxOpts {
            width: Size::FILL,
            height: Size::px(92.0),
            border: 1.0,
            border_color: pal.border,
            radius: crate::theme::radius::CARD,
            pad_top: 10.0,
            pad_x: 12.0,
            pad_bottom: 8.0,
            dir: cui::Dir::Row,
            align: Align::Stretch,
            ..Default::default()
        },
        |ui| {
            for (i, day) in data.strip.iter().enumerate() {
                let picked = view.pick.as_deref() == Some("day")
                    && view.day.as_deref() == Some(day.iso.as_str());
                let hovered = kit::press_hovered(&cell_ids[i], mouse.x, mouse.y);
                let bg = if picked {
                    pal.selected
                } else if hovered {
                    pal.hover
                } else {
                    Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    }
                };
                ui.column(
                    &cui::Column {
                        id: Some(&cell_ids[i]),
                        width: Size::FILL,
                        height: Size::FILL,
                        align: Align::Center,
                        justify: cui::Justify::End,
                        gap: 3.0,
                        bg,
                        radius: crate::theme::radius::CTL,
                        ..Default::default()
                    },
                    |ui| {
                        // dots, at most five, then the date (col-reverse in NSH: date at the bottom)
                        for d in day.rows.iter().take(5) {
                            strip_dot(ui, d);
                        }
                        if every_day || i % 7 == 0 {
                            ui.text_with(
                                &day.date.day().to_string(),
                                &cui::Text {
                                    size: crate::theme::r#type::CAP.0,
                                    tabular: true,
                                    color: if i == 0 {
                                        pal.foreground
                                    } else {
                                        pal.subtle_foreground
                                    },
                                    ..Default::default()
                                },
                            );
                        } else {
                            ui.box_(
                                &BoxOpts {
                                    width: Size::px(1.0),
                                    height: Size::px(12.0),
                                    ..Default::default()
                                },
                                |_| {},
                            );
                        }
                    },
                );
            }
        },
    );
    for (i, id) in cell_ids.iter().enumerate() {
        let wid = ui.make_id(id);
        let rect = ui.get_rect(wid);
        kit::press_remember(id, rect);
        if clicked && kit::press_hit(rect, mouse.x, mouse.y) {
            picked_day = Some(data.strip[i].iso.clone());
        }
    }

    // "Today, Mon D" … "Mon D" under the strip.
    let first = &data.strip[0].date;
    let last = &data.strip[data.strip.len() - 1].date;
    ui.row(
        &cui::Row {
            width: Size::FILL,
            justify: cui::Justify::Between,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &format!(
                    "Today, {} {}",
                    MONTHS[(first.month() - 1) as usize],
                    first.day()
                ),
                &cui::Text {
                    size: crate::theme::r#type::CAP.0,
                    color: pal.subtle_foreground,
                    ..Default::default()
                },
            );
            ui.text_with(
                &format!("{} {}", MONTHS[(last.month() - 1) as usize], last.day()),
                &cui::Text {
                    size: crate::theme::r#type::CAP.0,
                    color: pal.subtle_foreground,
                    ..Default::default()
                },
            );
        },
    );
    picked_day
}

// ---------------------------------------------------------------- rows

/// When a deadline falls (parts.tsx's DueWhen): an overdue or due-soon chip for an open one,
/// else "Today", "N days ago" or "In N days".
fn draw_due_when(ui: &mut Ui, d: &DeadlineRow) {
    let days = days_until(&d.due_date, chrono::Local::now());
    let open = d.status == "Not started" || d.status == "Draft";
    if open && days < 0 {
        kit::draw_chip(ui, &status_chip("overdue", Some(days)));
    } else if open && days <= 14 {
        kit::draw_chip(ui, &status_chip("due_soon", Some(days)));
    } else {
        let text = if days == 0 {
            "Today".to_owned()
        } else if days < 0 {
            format!("{} days ago", -days)
        } else {
            format!("In {days} days")
        };
        ui.text_with(
            &text,
            &cui::Text {
                size: crate::theme::r#type::META.0,
                color: kit::frame_palette().muted_foreground,
                ..Default::default()
            },
        );
    }
}

fn due_date_box(ui: &mut Ui, d: &DeadlineRow) {
    let parts: Vec<&str> = d.due_date.split('-').collect();
    let (m, day) = (
        parts
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
        parts
            .get(2)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
    );
    let pal = kit::frame_palette();
    ui.column(
        &cui::Column {
            width: Size::px(44.0),
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &MONTHS[(m - 1) as usize].to_uppercase(),
                &cui::Text {
                    size: crate::theme::r#type::CAP.0,
                    color: pal.subtle_foreground,
                    ..Default::default()
                },
            );
            ui.text_with(
                &day.to_string(),
                &cui::Text {
                    size: crate::theme::r#type::TITLE.0,
                    tabular: true,
                    ..Default::default()
                },
            );
        },
    );
}

/// A deadline row (parts.tsx's DueRow): date box, code, "<code> · <period>" over the taxpayer,
/// when, status. The whole row opens the return.
fn draw_due_row(ui: &mut Ui, d: &DeadlineRow, compact: bool) -> bool {
    let key = deadline_key(d);
    let _g = ui.push_id(key.clone());
    kit::pressable_row(ui, &format!("due-{key}"), |ui| {
        due_date_box(ui, d);
        if !compact {
            ui.box_(
                &BoxOpts {
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
                ui.text(&format!("{} · {}", d.form_code, d.period_label));
                ui.muted(&who_line(d.branch_count, &d.taxpayer_name, &d.branch_name));
            },
        );
        if !compact {
            ui.box_(
                &BoxOpts {
                    width: Size::px(96.0),
                    ..Default::default()
                },
                |ui| draw_due_when(ui, d),
            );
        }
        ui.box_(
            &BoxOpts {
                width: Size::px(88.0),
                align: Align::End,
                ..Default::default()
            },
            |ui| kit::draw_chip(ui, &chip_for(status_kind(&d.status))),
        );
    })
}

/// What a filed return offers (parts.tsx's FiledActions): Cancel queue while queued, its status
/// or Use as template, then Open.
fn draw_filed_actions(ui: &mut Ui, app: &mut AppState, r: &ReturnRow) -> bool {
    let filed = matches!(status_kind(&r.status), "filed");
    let has_template = crate::store::templates(&r.tin)
        .iter()
        .any(|t| t.form_code == r.form_code);
    let mut opened = false;
    if r.status == "Queued" && kit::small_button(ui, "Cancel queue", ButtonVariant::Ghost, None) {
        cancel_queue(
            app,
            &r.tin,
            &r.form_code,
            r.taxable_year,
            (r.period != 0).then_some(r.period),
        );
    }
    if has_template || !filed {
        kit::draw_chip(ui, &chip_for(status_kind(&r.status)));
    } else if kit::small_button(
        ui,
        "Use as template",
        ButtonVariant::Default,
        Some(crate::ui::icons::glyph("layout-template")),
    ) {
        use_as_template(app, r);
    }
    if kit::small_button(ui, "Open", ButtonVariant::Ghost, None) {
        open_listed(app, r);
        opened = true;
    }
    opened
}

/// A filed (or saved) return (parts.tsx's FiledRow). Compact (under 640): the code and the date
/// go, and the actions sit under the name.
fn draw_filed_row(ui: &mut Ui, app: &mut AppState, r: &ReturnRow, compact: bool) {
    let _g = ui.push_id(format!("row-{}", r.id));
    if compact {
        ui.column(
            &cui::Column {
                width: Size::FILL,
                gap: 6.0,
                ..Default::default()
            },
            |ui| {
                ui.row(
                    &cui::Row {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        ui.column(
                            &cui::Column {
                                width: Size::FILL,
                                ..Default::default()
                            },
                            |ui| {
                                ui.text(&format!("{} · {}", r.form_code, r.period_label));
                                ui.muted(&format!(
                                    "{} · {}",
                                    who_line(r.branch_count, &r.taxpayer_name, &r.branch_name),
                                    short_date(&r.updated_at)
                                ));
                            },
                        );
                    },
                );
                ui.row(
                    &cui::Row {
                        width: Size::FILL,
                        justify: cui::Justify::End,
                        gap: 4.0,
                        ..Default::default()
                    },
                    |ui| {
                        draw_filed_actions(ui, app, r);
                    },
                );
            },
        );
    } else {
        ui.row(
            &cui::Row {
                width: Size::FILL,
                gap: crate::theme::space(3),
                ..Default::default()
            },
            |ui| {
                ui.box_(
                    &BoxOpts {
                        width: Size::px(68.0),
                        ..Default::default()
                    },
                    |ui| ui.code(&r.form_code),
                );
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text(&format!("{} · {}", r.form_code, r.period_label));
                        ui.muted(&who_line(r.branch_count, &r.taxpayer_name, &r.branch_name));
                    },
                );
                ui.box_(
                    &BoxOpts {
                        width: Size::px(104.0),
                        ..Default::default()
                    },
                    |ui| {
                        ui.text_with(
                            &short_date(&r.updated_at),
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: kit::frame_palette().muted_foreground,
                                ..Default::default()
                            },
                        );
                    },
                );
                ui.row(
                    &cui::Row {
                        gap: 4.0,
                        ..Default::default()
                    },
                    |ui| {
                        draw_filed_actions(ui, app, r);
                    },
                );
            },
        );
    }
}

/// One taxpayer (parts.tsx's TaxpayerRow): avatar, filed this year (a bar when wide), the next
/// open deadline, and the button that opens it.
fn draw_taxpayer_row(ui: &mut Ui, app: &mut AppState, row: &dashboard::TaxpayerRow, compact: bool) {
    let _g = ui.push_id(format!("tp-{}", row.taxpayer.tin_root));
    let tp = &row.taxpayer;
    let next = row.next.as_ref();
    let next_days = next.map(|n| days_until(&n.due_date, chrono::Local::now()));
    let pct = if row.due > 0 {
        row.done as f32 / row.due as f32
    } else {
        0.0
    };
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: crate::theme::space(3),
            ..Default::default()
        },
        |ui| {
            ui.avatar_with(
                &tp.name,
                &cui::Avatar {
                    size: 28.0,
                    ..Default::default()
                },
            );
            if compact {
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text(&tp.name);
                        ui.text_with(
                            &dashboard::progress_text(row),
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: kit::frame_palette().subtle_foreground,
                                ..Default::default()
                            },
                        );
                        ui.text_with(
                            &dashboard::next_text(next),
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: if next.is_some() {
                                    kit::frame_palette().muted_foreground
                                } else {
                                    kit::frame_palette().subtle_foreground
                                },
                                ..Default::default()
                            },
                        );
                    },
                );
            } else {
                ui.column(
                    &cui::Column {
                        width: Size::px(210.0),
                        ..Default::default()
                    },
                    |ui| {
                        ui.text(&tp.name);
                        ui.text_with(
                            &tp.tin_display,
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: kit::frame_palette().subtle_foreground,
                                tabular: true,
                                ..Default::default()
                            },
                        );
                    },
                );
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        min_width: 120.0,
                        gap: 4.0,
                        ..Default::default()
                    },
                    |ui| {
                        ui.progress_with(
                            pct,
                            &cui::Progress {
                                height: 6.0,
                                tone: Tone::Success,
                                ..Default::default()
                            },
                        );
                        ui.text_with(
                            &dashboard::progress_text(row),
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: kit::frame_palette().subtle_foreground,
                                ..Default::default()
                            },
                        );
                    },
                );
                ui.column(
                    &cui::Column {
                        width: Size::px(170.0),
                        ..Default::default()
                    },
                    |ui| {
                        ui.text_with(
                            &dashboard::next_text(next),
                            &cui::Text {
                                size: crate::theme::r#type::META.0,
                                color: if next.is_some() {
                                    kit::frame_palette().muted_foreground
                                } else {
                                    kit::frame_palette().subtle_foreground
                                },
                                ..Default::default()
                            },
                        );
                        if let (Some(n), Some(days)) = (next, next_days) {
                            if days <= 14 {
                                kit::draw_chip(ui, &status_chip("due_soon", Some(days)));
                            } else {
                                ui.text_with(
                                    &format!("In {days} days"),
                                    &cui::Text {
                                        size: crate::theme::r#type::META.0,
                                        color: kit::frame_palette().subtle_foreground,
                                        ..Default::default()
                                    },
                                );
                            }
                            let _ = n;
                        }
                    },
                );
            }
            if let Some(n) = next {
                let label = if n.status == "Draft" {
                    "Open next"
                } else {
                    "Start next"
                };
                let d = n.clone();
                if kit::small_button(ui, label, ButtonVariant::Default, None) {
                    open_deadline(app, &d);
                }
            }
        },
    );
}

/// An as-needed form a branch files (parts.tsx's AsNeededHead).
fn draw_as_needed_head(ui: &mut Ui, app: &mut AppState, f: &AsNeededForm, compact: bool) {
    let _g = ui.push_id(format!("as-needed-{}-{}", f.tin, f.form_code));
    let start = |ui: &mut Ui, app: &mut AppState| {
        if kit::small_button(
            ui,
            "Start a return",
            ButtonVariant::Default,
            Some(crate::ui::icons::glyph("plus")),
        ) {
            start_as_needed(app, f);
        }
    };
    if compact {
        ui.column(
            &cui::Column {
                width: Size::FILL,
                gap: 6.0,
                ..Default::default()
            },
            |ui| {
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text(&format!("{} · {}", f.form_code, f.form_name));
                        ui.muted(&who_line(f.branch_count, &f.taxpayer_name, &f.branch_name));
                    },
                );
                ui.row(
                    &cui::Row {
                        width: Size::FILL,
                        justify: cui::Justify::End,
                        ..Default::default()
                    },
                    |ui| start(ui, app),
                );
            },
        );
    } else {
        ui.row(
            &cui::Row {
                width: Size::FILL,
                gap: crate::theme::space(3),
                ..Default::default()
            },
            |ui| {
                ui.box_(
                    &BoxOpts {
                        width: Size::px(68.0),
                        ..Default::default()
                    },
                    |ui| ui.code(&f.form_code),
                );
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        ui.text(&format!("{} · {}", f.form_code, f.form_name));
                        ui.muted(&format!(
                            "{} · {}",
                            who_line(f.branch_count, &f.taxpayer_name, &f.branch_name),
                            f.due_rule
                        ));
                    },
                );
                start(ui, app);
            },
        );
    }
}

/// A saved as-needed return that holds no trigger date yet (parts.tsx's AsNeededUndated).
fn draw_as_needed_undated(
    ui: &mut Ui,
    app: &mut AppState,
    f: &AsNeededForm,
    r: &AsNeededReturn,
    compact: bool,
) {
    let _g = ui.push_id(format!(
        "as-needed-{}-{}-{}",
        f.tin, f.form_code, r.draft_id
    ));
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: crate::theme::space(3),
            ..Default::default()
        },
        |ui| {
            if !compact {
                ui.box_(
                    &BoxOpts {
                        width: Size::px(68.0),
                        ..Default::default()
                    },
                    |_| {},
                );
            }
            ui.column(
                &cui::Column {
                    width: Size::FILL,
                    ..Default::default()
                },
                |ui| {
                    ui.text(&if let Some(t) = &f.trigger {
                        format!("No {} date yet", t.to_lowercase())
                    } else {
                        "Saved return".to_owned()
                    });
                    ui.muted(&format!("Saved {}", short_date(&r.updated_at)));
                },
            );
            if !compact {
                ui.box_(
                    &BoxOpts {
                        width: Size::px(88.0),
                        align: Align::End,
                        ..Default::default()
                    },
                    |ui| kit::draw_chip(ui, &chip_for(status_kind(&r.status))),
                );
            }
            if kit::small_button(ui, "Open", ButtonVariant::Ghost, None) {
                open_undated(app, f, r);
            }
        },
    );
}

// ---------------------------------------------------------------- dashboard

fn draw_dashboard(ui: &mut Ui, app: &mut AppState) {
    let now = chrono::Local::now();
    let width = first_width(app);
    let data = load(app);
    let view = pick();
    let picked = dashboard::picked_deadlines(&view, &data);
    let specs = dashboard::tile_specs(&data);
    let filed: Vec<ReturnRow> = crate::state::returns("filed").into_iter().take(3).collect();
    let entries = as_needed_entries(as_needed(now.year() as i64));
    let cols = tile_columns(width);
    let tile_w = ((width - TILE_GAP * (cols - 1) as f32) / cols as f32).floor();
    let today = today_iso();

    // PageHeader: the greeting and the long date.
    ui.heading(&format!("{}", greeting(&now)), 1);
    ui.muted(&format!("{}.", long_today(&now)));

    // The five tiles.
    ui.gap(crate::theme::space(6));
    let mut tile_pressed: Option<String> = None;
    ui.row(
        &cui::Row {
            wrap: true,
            gap: TILE_GAP,
            ..Default::default()
        },
        |ui| {
            for spec in &specs {
                let is_picked = view.pick.as_deref() == Some(spec.key);
                let key = spec.key.to_owned();
                if draw_stat_tile(ui, spec, tile_w, is_picked) {
                    tile_pressed = Some(key);
                }
            }
        },
    );
    if let Some(key) = tile_pressed {
        press_tile(app, &key, &today);
    }

    // Next 30 days.
    ui.gap(crate::theme::space(7));
    ui.section_title("Next 30 days");
    if let Some(iso) = draw_strip(ui, &data, &view, width) {
        toggle_day(&iso);
    }
    ui.gap(crate::theme::space(4));
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: 8.0,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                &picked.title,
                &cui::Text {
                    size: crate::theme::r#type::EMPH.0,
                    ..Default::default()
                },
            );
            ui.spacer();
            if view.pick.is_some() && kit::small_button(ui, "Show all", ButtonVariant::Ghost, None)
            {
                clear_pick();
            }
            if kit::small_button(
                ui,
                "Open Calendar",
                ButtonVariant::Ghost,
                Some(crate::ui::icons::glyph("calendar")),
            ) {
                app.go(ViewName::Calendar, Default::default());
            }
        },
    );
    ui.gap(crate::theme::space(2));
    kit::list_group(ui, |ui| {
        if picked.rows.is_empty() {
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    pad_y: crate::theme::space(3),
                    pad_x: crate::theme::space(3),
                    ..Default::default()
                },
                |ui| ui.muted(&picked.empty),
            );
        } else {
            for (i, d) in picked.rows.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                if draw_due_row(ui, d, width < 560.0) {
                    open_deadline(app, d);
                }
            }
        }
    });

    // Each taxpayer at a glance.
    ui.section_title("Each taxpayer at a glance");
    kit::list_group(ui, |ui| {
        if data.taxpayers.is_empty() {
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    pad_y: crate::theme::space(3),
                    pad_x: crate::theme::space(3),
                    ..Default::default()
                },
                |ui| ui.muted("Archived taxpayers are left out of Home."),
            );
        } else {
            for (i, row) in data.taxpayers.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                draw_taxpayer_row(ui, app, row, width < 720.0);
            }
        }
    });

    // As needed.
    if !entries.is_empty() {
        ui.section_title("As needed");
        kit::list_group(ui, |ui| {
            for (i, e) in entries.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                match (&e.undated, e.undated.as_ref()) {
                    (None, _) => draw_as_needed_head(ui, app, &e.form, width < 560.0),
                    (Some(_), Some(r)) => {
                        draw_as_needed_undated(ui, app, &e.form, r, width < 560.0)
                    }
                    _ => {}
                }
            }
        });
    }

    // Recently filed.
    ui.section_title("Recently filed");
    kit::list_group(ui, |ui| {
        if filed.is_empty() {
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    pad_y: crate::theme::space(3),
                    pad_x: crate::theme::space(3),
                    ..Default::default()
                },
                |ui| ui.muted("Filed returns show here."),
            );
        } else {
            for (i, r) in filed.iter().enumerate() {
                if i > 0 {
                    kit::row_sep(ui);
                }
                draw_filed_row(ui, app, r, width < 640.0);
            }
        }
    });

    // Home draws from what the store holds at once; the drafts are read again once per visit.
    if !app.saved_returns_forgotten {
        app.saved_returns_forgotten = true;
        crate::store::forget("saved_returns");
    }
}

/// The welcome on a fresh install (Home.tsx's Welcome): three numbered steps in a list group.
fn draw_welcome(ui: &mut Ui, app: &mut AppState) {
    ui.heading("Welcome to eBIRForms", 1);
    ui.muted(
        "Prepare, print and track BIR returns for every taxpayer you handle. Everything stays on this computer.",
    );
    ui.gap(crate::theme::space(7));
    kit::list_group(ui, |ui| {
        for (i, (icon, title, line)) in STEPS.iter().enumerate() {
            if i > 0 {
                kit::row_sep(ui);
            }
            let _g = ui.push_id(format!("step-{i}"));
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: crate::theme::space(3),
                    pad_y: crate::theme::space(2),
                    pad_x: crate::theme::space(3),
                    ..Default::default()
                },
                |ui| {
                    step_circle(ui, i);
                    ui.icon(
                        crate::ui::icons::glyph(icon),
                        18.0,
                        kit::frame_palette().muted_foreground,
                    );
                    ui.column(
                        &cui::Column {
                            width: Size::FILL,
                            ..Default::default()
                        },
                        |ui| {
                            ui.text(title);
                            ui.muted(line);
                        },
                    );
                },
            );
        }
        kit::row_sep(ui);
        ui.row(
            &cui::Row {
                width: Size::FILL,
                pad_y: crate::theme::space(2),
                pad_x: crate::theme::space(3),
                ..Default::default()
            },
            |ui| {
                if ui.primary_button("Add your first taxpayer", true) {
                    app.go(ViewName::NewTaxpayer, Default::default());
                }
            },
        );
    });
}

/// The Home view: the dashboard, or the welcome while no taxpayer exists.
pub fn draw(ui: &mut Ui, app: &mut AppState) {
    if app.is_fresh() {
        draw_welcome(ui, app);
    } else {
        draw_dashboard(ui, app);
    }
}
