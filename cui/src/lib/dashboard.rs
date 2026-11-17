//! Home's dashboard, pure data (port of lib/dashboard.ts): the numbers the five tiles show, the
//! 30-day deadline strip, the list the picked tile or day narrows to, and each taxpayer's progress
//! and next deadline. The clock is a parameter.

use chrono::{Datelike, Local, NaiveDate, TimeZone};

use super::dates::{MONTHS, MONTHS_LONG, Ymd, add_days, iso_date};
use crate::types::{DeadlineRow, ReturnRow, Taxpayer};

/// One day of the 30-day strip.
#[derive(Debug, Clone)]
pub struct StripDay {
    /// The day, as YYYY-MM-DD.
    pub iso: String,
    pub date: NaiveDate,
    /// The open deadlines due that day.
    pub rows: Vec<DeadlineRow>,
}

/// One taxpayer's row: filed of due this year, and the next open deadline within 60 days.
#[derive(Debug, Clone)]
pub struct TaxpayerRow {
    pub taxpayer: Taxpayer,
    pub due: i64,
    pub done: i64,
    pub next: Option<DeadlineRow>,
}

/// Everything the dashboard shows.
#[derive(Debug, Default)]
pub struct Dashboard {
    pub today: String,
    pub in7: String,
    pub overdue: Vec<DeadlineRow>,
    pub week: Vec<DeadlineRow>,
    pub drafts: usize,
    pub waiting: usize,
    pub filed_month: usize,
    pub strip: Vec<StripDay>,
    pub upcoming: Vec<DeadlineRow>,
    pub any_deadlines: bool,
    pub taxpayers: Vec<TaxpayerRow>,
}

const STRIP_DAYS: i64 = 30;
const FILED: [&str; 3] = ["Submitted", "Confirmed", "Paid"];

fn is_open(d: &DeadlineRow) -> bool {
    d.status == "Not started" || d.status == "Draft"
}

fn is_filed(status: &str) -> bool {
    FILED.contains(&status)
}

/// [year, month 1-12] of a return's update, in local time (SQLite timestamps are UTC).
fn local_month(text: &str) -> (i32, u32) {
    let b = text.as_bytes();
    if b.len() >= 16
        && b[4] == b'-'
        && b[7] == b'-'
        && (b[10] == b' ' || b[10] == b'T')
        && let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi)) = (
            text[0..4].parse::<i32>(),
            text[5..7].parse::<u32>(),
            text[8..10].parse::<u32>(),
            text[11..13].parse::<u32>(),
            text[14..16].parse::<u32>(),
        )
        && let Some(utc) =
            NaiveDate::from_ymd_opt(y, mo, d).and_then(|date| date.and_hms_opt(h, mi, 0))
    {
        let local = Local.from_utc_datetime(&utc);
        return (local.year(), local.month());
    }
    let mut parts = text.split('-');
    let y = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let m = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    (y, m)
}

/// Everything the dashboard shows, from the reads the screens make (deadlines, returns, taxpayers).
pub fn load_dashboard(
    deadlines: &dyn Fn(&str, &str) -> Vec<DeadlineRow>,
    saved_returns: &dyn Fn() -> Vec<ReturnRow>,
    returns_filed: &dyn Fn() -> Vec<ReturnRow>,
    taxpayers: &dyn Fn() -> Vec<Taxpayer>,
    at: chrono::DateTime<Local>,
) -> Dashboard {
    let now = at.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let now = Local.from_local_datetime(&now).unwrap();
    let today = iso_date(now);
    let in7 = iso_date(now + chrono::Duration::days(7));

    let near: Vec<DeadlineRow> = deadlines(&iso_date(now - chrono::Duration::days(366)), &in7)
        .into_iter()
        .filter(|d| is_open(d))
        .collect();
    let overdue: Vec<DeadlineRow> = near
        .iter()
        .filter(|d| d.due_date < today)
        .cloned()
        .collect();
    let week: Vec<DeadlineRow> = near
        .iter()
        .filter(|d| d.due_date >= today && d.due_date <= in7)
        .cloned()
        .collect();

    let drafts = saved_returns().len();
    let filed = returns_filed();
    let waiting = filed.iter().filter(|r| r.status == "Queued").count();
    let (y, m) = (now.year(), now.month());
    let filed_month = filed
        .iter()
        .filter(|r| is_filed(&r.status) && local_month(&r.updated_at) == (y, m))
        .count();

    let strip_all = deadlines(
        &today,
        &iso_date(now + chrono::Duration::days(STRIP_DAYS - 1)),
    );
    let upcoming: Vec<DeadlineRow> = strip_all.iter().filter(|d| is_open(d)).cloned().collect();
    let mut strip = Vec::with_capacity(STRIP_DAYS as usize);
    for i in 0..STRIP_DAYS {
        let date = add_days(now.date_naive(), i);
        let iso = date.format("%Y-%m-%d").to_string();
        strip.push(StripDay {
            iso: iso.clone(),
            date,
            rows: upcoming
                .iter()
                .filter(|d| d.due_date == iso)
                .cloned()
                .collect(),
        });
    }

    let since_jan = format!("{y}-01-01");
    let past: Vec<DeadlineRow> = deadlines(&since_jan, &today)
        .into_iter()
        .filter(|d| d.due_date < today)
        .collect();
    let ahead: Vec<DeadlineRow> = deadlines(&today, &iso_date(now + chrono::Duration::days(60)))
        .into_iter()
        .filter(|d| is_open(d))
        .collect();
    let taxpayers = taxpayers()
        .into_iter()
        .filter(|t| !t.archived)
        .map(|t| {
            let past_rows: Vec<&DeadlineRow> =
                past.iter().filter(|d| d.tin_root == t.tin_root).collect();
            TaxpayerRow {
                due: past_rows.len() as i64,
                done: past_rows.iter().filter(|d| is_filed(&d.status)).count() as i64,
                next: ahead.iter().find(|d| d.tin_root == t.tin_root).cloned(),
                taxpayer: t,
            }
        })
        .collect();

    Dashboard {
        today,
        in7,
        overdue,
        week,
        drafts,
        waiting,
        filed_month,
        strip,
        upcoming,
        any_deadlines: !strip_all.is_empty() || !near.is_empty(),
        taxpayers,
    }
}

/// One tile: its key, its texts, its icon, its tone and its count.
#[derive(Debug, Clone)]
pub struct TileSpec {
    pub key: &'static str,
    pub label: String,
    pub sub: String,
    pub tone: &'static str,
    pub icon: &'static str,
    pub n: usize,
}

/// The five tiles: their keys, texts, icons, tones and counts, in the order they show.
pub fn tile_specs(d: &Dashboard) -> Vec<TileSpec> {
    let in7_parts: Vec<&str> = d.in7.split('-').collect();
    let (mm, dd) = (
        in7_parts
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
        in7_parts
            .get(2)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1),
    );
    let month_idx = d
        .today
        .split('-')
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1)
        .saturating_sub(1) as usize;
    let month_name = MONTHS.get(month_idx).copied().unwrap_or("");
    vec![
        TileSpec {
            key: "overdue",
            label: "Overdue".into(),
            sub: "past the due date".into(),
            tone: "danger",
            icon: "triangle-alert",
            n: d.overdue.len(),
        },
        TileSpec {
            key: "week",
            label: "Due in 7 days".into(),
            sub: format!(
                "by {} {}",
                MONTHS.get((mm - 1) as usize).copied().unwrap_or(""),
                dd
            ),
            tone: "warning",
            icon: "calendar",
            n: d.week.len(),
        },
        TileSpec {
            key: "drafts",
            label: "Drafts".into(),
            sub: "started, not queued".into(),
            tone: "accent",
            icon: "pencil",
            n: d.drafts,
        },
        TileSpec {
            key: "waiting",
            label: "Waiting to send".into(),
            sub: "queued to send".into(),
            tone: "fg2",
            icon: "send",
            n: d.waiting,
        },
        TileSpec {
            key: "filed",
            label: "Filed this month".into(),
            sub: format!("submitted in {month_name}"),
            tone: "success",
            icon: "circle-check",
            n: d.filed_month,
        },
    ]
}

/// What the home view has picked: a tile (overdue or week), a day of the strip, or nothing.
#[derive(Debug, Clone, Default)]
pub struct HomeView {
    pub pick: Option<String>, // "overdue" | "week" | "day"
    pub day: Option<String>,  // YYYY-MM-DD
}

/// The list under the strip for the tile or day picked.
pub struct Picked {
    pub title: String,
    pub rows: Vec<DeadlineRow>,
    pub empty: String,
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The deadlines under the strip for the tile or day picked.
pub fn picked_deadlines(view: &HomeView, d: &Dashboard) -> Picked {
    match view.pick.as_deref() {
        Some("overdue") => Picked {
            title: "Overdue".into(),
            rows: d.overdue.clone(),
            empty: "Nothing is overdue.".into(),
        },
        Some("week") => Picked {
            title: "Due in the next 7 days".into(),
            rows: d.week.clone(),
            empty: "Nothing is due in the next 7 days.".into(),
        },
        Some("day") => {
            if let Some(day) = d
                .strip
                .iter()
                .find(|s| Some(s.iso.as_str()) == view.day.as_deref())
            {
                let wd = WEEKDAYS[day.date.weekday().num_days_from_sunday() as usize];
                Picked {
                    title: format!(
                        "{wd}, {} {}",
                        MONTHS_LONG[(day.date.month() - 1) as usize],
                        day.date.day()
                    ),
                    rows: day.rows.clone(),
                    empty: "Nothing due this day.".into(),
                }
            } else {
                open_deadlines(d)
            }
        }
        _ => open_deadlines(d),
    }
}

fn open_deadlines(d: &Dashboard) -> Picked {
    Picked {
        title: "Open deadlines".into(),
        rows: d.upcoming.clone(),
        empty: if d.any_deadlines {
            "Nothing is due in the next 30 days.".into()
        } else {
            "Deadlines show here once a taxpayer has forms for a tax year.".into()
        },
    }
}

/// "1 of 4 filed this year" once something was due this year.
pub fn progress_text(row: &TaxpayerRow) -> String {
    if row.due > 0 {
        format!("{} of {} filed this year", row.done, row.due)
    } else {
        "Nothing was due yet this year".into()
    }
}

/// "Next: 1601C · Nov 10" for the next open deadline, or "Nothing due in 60 days".
pub fn next_text(next: Option<&DeadlineRow>) -> String {
    match next {
        None => "Nothing due in 60 days".into(),
        Some(n) => {
            let parts: Vec<&str> = n.due_date.split('-').collect();
            let (m, d) = (
                parts
                    .get(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1),
                parts
                    .get(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1),
            );
            format!(
                "Next: {} · {} {}",
                n.form_code,
                MONTHS.get((m - 1) as usize).copied().unwrap_or(""),
                d
            )
        }
    }
}

/// A YMD helper re-export for screens.
pub fn ymd(y: i32, m: u32, d: u32) -> Ymd {
    Ymd { y, m, d }
}
