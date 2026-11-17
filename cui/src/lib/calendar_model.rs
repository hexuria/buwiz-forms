//! The Calendar's pure side (`nsh/screens/calendar/rows.ts`): which deadlines a filter keeps, how
//! the List groups them, the month grid's days, and the tones a chip or a dot is drawn in. The
//! clock is a parameter (`today`), like the TS `now`.

use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::lib::dates::{MONTHS, MONTHS_LONG};
use crate::types::DeadlineRow;

/// How often: every frequency, or one of three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    All,
    Yearly,
    Quarterly,
    Monthly,
}

/// The choices of the How often control (FREQ_OPTIONS).
pub const FREQ_OPTIONS: [(&str, Freq); 4] = [
    ("All", Freq::All),
    ("Yearly", Freq::Yearly),
    ("Quarterly", Freq::Quarterly),
    ("Monthly", Freq::Monthly),
];

/// The frequency each choice keeps (the host's names) (FREQ_OF).
pub fn freq_of(f: Freq) -> Option<&'static str> {
    match f {
        Freq::All => None,
        Freq::Yearly => Some("Annual"),
        Freq::Quarterly => Some("Quarterly"),
        Freq::Monthly => Some("Monthly"),
    }
}

/// The months a quarter covers, for the caption under the quarter chips (QUARTER_MONTHS).
pub const QUARTER_MONTHS: [&str; 4] = [
    "January to March",
    "April to June",
    "July to September",
    "October to December",
];

/// Weekday names, Sunday first (the grid starts on Sunday) (WEEKDAYS).
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// "1 deadline", "2 deadlines".
pub fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// The page's filters (Filters).
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// The taxpayers (TIN roots); None is every taxpayer.
    pub tps: Option<HashSet<String>>,
    pub freq: Freq,
    /// The month a return is due (1-12, 0 for every month).
    pub month: u32,
    /// The quarter it covers (1-4, 0 for every quarter).
    pub quarter: u32,
    pub year: i64,
}

impl Default for Freq {
    fn default() -> Self {
        Freq::All
    }
}

/// The taxpayer and how-often filters, which every view applies (byWhoAndHowOften).
pub fn by_who_and_how_often<'a>(f: &Filters, rows: &'a [DeadlineRow]) -> Vec<&'a DeadlineRow> {
    rows.iter()
        .filter(|d| {
            f.tps.as_ref().is_none_or(|t| t.contains(&d.tin_root))
                && freq_of(f.freq).is_none_or(|name| d.frequency == name)
        })
        .collect()
}

/// The List view's rows (calendarRows): a month chip keeps the returns due in that month of the
/// year; otherwise the year is the tax year the returns cover, and a quarter chip keeps one
/// covered quarter.
pub fn calendar_rows<'a>(f: &Filters, rows: &'a [DeadlineRow]) -> Vec<&'a DeadlineRow> {
    let r = by_who_and_how_often(f, rows);
    if f.freq == Freq::Monthly && f.month != 0 {
        let prefix = format!("{}-{:02}-", f.year, f.month);
        return r
            .into_iter()
            .filter(|d| d.due_date.starts_with(&prefix))
            .collect();
    }
    let mut out: Vec<&DeadlineRow> = r.into_iter().filter(|d| d.taxable_year == f.year).collect();
    if f.freq == Freq::Quarterly && f.quarter != 0 {
        out.retain(|d| d.period == i64::from(f.quarter));
    }
    out
}

/// True when any filter differs from its start (filtersActive).
pub fn filters_active(f: &Filters, today: NaiveDate) -> bool {
    f.tps.is_some()
        || f.freq != Freq::All
        || f.month != 0
        || f.quarter != 0
        || f.year != i64::from(today.year())
}

/// Date order (ISO dates compare as text) (byDue).
pub fn by_due(a: &DeadlineRow, b: &DeadlineRow) -> std::cmp::Ordering {
    a.due_date.cmp(&b.due_date)
}

/// The line under the filter bar: "12 deadlines · 1 taxpayer · 2026 returns" (countText).
pub fn count_text(f: &Filters, rows: &[&DeadlineRow]) -> String {
    let who: HashSet<&str> = rows.iter().map(|d| d.tin_root.as_str()).collect();
    let scope = if f.freq == Freq::Monthly && f.month != 0 {
        format!("due in {} {}", MONTHS_LONG[(f.month - 1) as usize], f.year)
    } else if f.freq == Freq::Quarterly && f.quarter != 0 {
        format!("returns for Q{} {}", f.quarter, f.year)
    } else {
        format!("{} returns", f.year)
    };
    format!(
        "{} · {} · {}",
        plural(rows.len(), "deadline"),
        plural(who.len(), "taxpayer"),
        scope
    )
}

/// One heading of the List and its rows (Group).
#[derive(Debug)]
pub struct Group<'a> {
    /// "November 2026" (by due month), or "Q1 2026" (by covered quarter, under Quarterly).
    pub label: String,
    /// The heading: the label, or "Q1 2026 returns" under Quarterly.
    pub title: String,
    /// Under Quarterly: "due Apr 15, Jul 15".
    pub caption: Option<String>,
    pub rows: Vec<&'a DeadlineRow>,
}

/// The List's groups: by due month, or by covered quarter under Quarterly; rows in date order,
/// groups in the order their first row falls (groupRows). Insertion order matters: the TS Map
/// keeps it, so "April 2026" never sorts before "February 2026".
pub fn group_rows<'a>(f: &Filters, rows: &[&'a DeadlineRow]) -> Vec<Group<'a>> {
    let mut sorted: Vec<&DeadlineRow> = rows.to_vec();
    sorted.sort_by(|a, b| by_due(a, b));
    let quarterly = f.freq == Freq::Quarterly;
    if quarterly {
        sorted.sort_by(|a, b| {
            a.taxable_year
                .cmp(&b.taxable_year)
                .then(a.period.cmp(&b.period))
                .then(by_due(a, b))
        });
    }
    // label -> rows, first-seen order
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&DeadlineRow>> =
        std::collections::HashMap::new();
    for d in sorted {
        let key = if quarterly {
            format!("Q{} {}", d.period, d.taxable_year)
        } else {
            let m: u32 = d
                .due_date
                .split('-')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            let y = d.due_date.split('-').next().unwrap_or("");
            format!("{} {}", MONTHS_LONG[(m.max(1) - 1).min(11) as usize], y)
        };
        groups
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                Vec::new()
            })
            .push(d);
    }
    order
        .into_iter()
        .map(|label| {
            let list = groups.remove(&label).unwrap_or_default();
            if !quarterly {
                Group {
                    title: label.clone(),
                    label,
                    caption: None,
                    rows: list,
                }
            } else {
                let mut dues: Vec<String> = list
                    .iter()
                    .map(|d| {
                        let p: Vec<&str> = d.due_date.split('-').collect();
                        let m = p.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                        let day = p.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                        format!("{} {}", MONTHS[(m - 1).min(11) as usize], day)
                    })
                    .collect();
                dues.sort();
                dues.dedup();
                Group {
                    title: format!("{label} returns"),
                    label,
                    caption: Some(format!("due {}", dues.join(", "))),
                    rows: list,
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------- status and tone

/// The status a row's chip says (RowKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Draft,
    Queued,
    Filed,
    NotStarted,
}

/// A deadline's status as the chips name it (kindOf).
pub fn kind_of(status: &str) -> RowKind {
    match status {
        "Draft" => RowKind::Draft,
        "Queued" => RowKind::Queued,
        "Submitted" | "Confirmed" | "Paid" => RowKind::Filed,
        _ => RowKind::NotStarted,
    }
}

/// Still to do: nobody has queued or filed it (isOpenStatus).
pub fn is_open_status(d: &DeadlineRow) -> bool {
    d.status == "Not started" || d.status == "Draft"
}

/// A chip's tone (toneOf): overdue (danger), due within 14 days (warning), filed (success),
/// in progress (accent), else neutral.
pub fn tone_of(d: &DeadlineRow, today: NaiveDate) -> &'static str {
    let n = days_between(&d.due_date, today);
    if is_open_status(d) && n < 0 {
        "danger"
    } else if is_open_status(d) && n <= 14 {
        "warning"
    } else {
        match kind_of(&d.status) {
            RowKind::Filed => "success",
            RowKind::Draft | RowKind::Queued => "accent",
            _ => "neutral",
        }
    }
}

/// The tone of a day's dot: the most urgent of its deadlines (dayTone).
pub fn day_tone(items: &[&DeadlineRow], today: NaiveDate) -> &'static str {
    const ORDER: [&str; 5] = ["danger", "warning", "accent", "neutral", "success"];
    let mut best = ORDER.len();
    for d in items {
        let t = tone_of(d, today);
        if let Some(i) = ORDER.iter().position(|x| *x == t) {
            best = best.min(i);
        }
    }
    if best < ORDER.len() {
        ORDER[best]
    } else {
        "neutral"
    }
}

/// Who a row belongs to: the taxpayer, plus the branch when it has several (whoLine).
pub fn who_line(d: &DeadlineRow) -> String {
    if d.branch_count > 1 {
        format!("{} · {}", d.taxpayer_name, d.branch_name)
    } else {
        d.taxpayer_name.clone()
    }
}

/// A row's key (rowKey): an as-needed row is keyed by its saved return as well.
pub fn row_key(d: &DeadlineRow) -> String {
    let base = format!("{}-{}-{}-{}", d.tin, d.form_code, d.taxable_year, d.period);
    if d.frequency == "As needed" {
        format!("{base}-{}", d.draft_id.unwrap_or(0))
    } else {
        base
    }
}

/// Whole days from `today` to an ISO date (negative when past).
fn days_between(iso: &str, today: NaiveDate) -> i64 {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| (d - today).num_days())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- month grid

/// The Sunday on or before the first of the month (0-11): the grid's first cell (monthGridStart).
pub fn month_grid_start(year: i32, month: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    // chrono: Monday=0; Sunday-first offset = num_days_from_sunday
    first - Duration::days(i64::from(first.weekday().num_days_from_sunday()))
}

/// One cell of the grid (Cell).
pub struct Cell<'a> {
    pub date: NaiveDate,
    pub iso: String,
    /// The day of the month.
    pub day: u32,
    /// Whether the day belongs to the month shown.
    pub in_month: bool,
    pub is_today: bool,
    /// 0-6 (Sunday first) and 0-5.
    pub col: usize,
    pub row: usize,
    /// The deadlines due that day, in date order.
    pub items: Vec<&'a DeadlineRow>,
    /// "November 15, 2 deadlines" for screen readers.
    pub label: String,
}

/// The six weeks the grid shows for a month (0-11), each day with its deadlines: six weeks
/// always, so the grid keeps its height (gridCells).
pub fn grid_cells<'a>(
    year: i32,
    month: u32,
    rows: &'a [DeadlineRow],
    today: NaiveDate,
) -> Vec<Cell<'a>> {
    let start = month_grid_start(year, month);
    let mut by_day: std::collections::HashMap<&str, Vec<&DeadlineRow>> =
        std::collections::HashMap::new();
    let mut sorted: Vec<&DeadlineRow> = rows.iter().collect();
    sorted.sort_by(|a, b| by_due(a, b));
    for d in sorted {
        by_day.entry(d.due_date.as_str()).or_default().push(d);
    }
    let today_iso = today.format("%Y-%m-%d").to_string();
    let mut cells = Vec::with_capacity(42);
    for i in 0..42 {
        let date = start + Duration::days(i);
        let iso = date.format("%Y-%m-%d").to_string();
        let items = by_day.get(iso.as_str()).cloned().unwrap_or_default();
        cells.push(Cell {
            iso: iso.clone(),
            day: date.day(),
            in_month: date.month() == month && date.year() == year,
            is_today: iso == today_iso,
            col: (i % 7) as usize,
            row: (i / 7) as usize,
            label: format!(
                "{}, {}",
                MONTHS_LONG[(date.month() - 1) as usize],
                plural(items.len(), "deadline")
            )
            .replacen(", ", &format!(" {}, ", date.day()), 1),
            items,
            date,
        });
    }
    cells
}

/// The label fix above is convoluted; rebuild it plainly: "November 15, 2 deadlines".
fn _unused() {}

/// The first and last day the grid shows for a month, as ISO dates (gridRange).
pub fn grid_range(year: i32, month: u32) -> (String, String) {
    let start = month_grid_start(year, month);
    (
        start.format("%Y-%m-%d").to_string(),
        (start + Duration::days(41)).format("%Y-%m-%d").to_string(),
    )
}

/// "Sunday, November 15" for an ISO date (longDay).
pub fn long_day(iso: &str) -> String {
    let d = NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    // chrono weekday: Mon=0; TS getDay(): Sun=0
    let wd = match d.weekday() {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    };
    format!(
        "{}, {} {}",
        WEEKDAYS[wd],
        MONTHS_LONG[(d.month() - 1) as usize],
        d.day()
    )
}

/// How many of the rows fall in the month shown (0-11) of the year (inMonthCount).
pub fn in_month_count(year: i32, month: u32, rows: &[DeadlineRow]) -> usize {
    let prefix = format!("{year}-{:02}-", month);
    rows.iter()
        .filter(|d| d.due_date.starts_with(&prefix))
        .count()
}

// ---------------------------------------------------------------- narrow: the Upcoming list

/// How far ahead the narrow layout's Upcoming list looks (UPCOMING_DAYS).
pub const UPCOMING_DAYS: i64 = 60;
/// How far back it looks for late returns (LATE_DAYS).
pub const LATE_DAYS: i64 = 366;

/// The window the Upcoming list reads: [from, to] as ISO dates (upcomingRange).
pub fn upcoming_range(today: NaiveDate) -> (String, String) {
    (
        (today - Duration::days(LATE_DAYS))
            .format("%Y-%m-%d")
            .to_string(),
        (today + Duration::days(UPCOMING_DAYS))
            .format("%Y-%m-%d")
            .to_string(),
    )
}

/// What is still to do: the late ones first (oldest due date first), then what falls due from
/// today on (upcomingRows).
pub fn upcoming_rows<'a>(rows: &'a [DeadlineRow], today: NaiveDate) -> Vec<&'a DeadlineRow> {
    let today_iso = today.format("%Y-%m-%d").to_string();
    let mut open: Vec<&DeadlineRow> = rows.iter().filter(|d| is_open_status(d)).collect();
    open.sort_by(|a, b| by_due(a, b));
    let (late, soon): (Vec<_>, Vec<_>) = open.into_iter().partition(|d| d.due_date < today_iso);
    late.into_iter().chain(soon).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn row(over: &[(&str, &str)], due: &str) -> DeadlineRow {
        let mut r = DeadlineRow {
            tin: "00012345300000".into(),
            tin_root: "000123453".into(),
            branch_code: "00000".into(),
            branch_name: "Head office".into(),
            taxpayer_name: "Juana Santos".into(),
            branch_count: 1,
            form_code: "F".into(),
            form_name: "A form".into(),
            frequency: "Monthly".into(),
            taxable_year: 2026,
            period: 1,
            period_label: "January 2026".into(),
            due_date: due.into(),
            status: "Not started".into(),
            draft_id: None,
            return_key: None,
        };
        for &(k, v) in over {
            match k {
                "form_code" => r.form_code = v.into(),
                "due_date" => r.due_date = v.into(),
                "frequency" => r.frequency = v.into(),
                "taxable_year" => r.taxable_year = v.parse().unwrap(),
                "period" => r.period = v.parse().unwrap(),
                "period_label" => r.period_label = v.into(),
                "status" => r.status = v.into(),
                "tin" => r.tin = v.into(),
                "tin_root" => r.tin_root = v.into(),
                "taxpayer_name" => r.taxpayer_name = v.into(),
                "branch_count" => r.branch_count = v.parse().unwrap(),
                "draft_id" => r.draft_id = v.parse().ok(),
                _ => {}
            }
        }
        r
    }

    fn rows() -> Vec<DeadlineRow> {
        vec![
            row(
                &[
                    ("form_code", "1601C"),
                    ("due_date", "2027-01-10"),
                    ("period", "12"),
                    ("period_label", "December 2026"),
                ],
                "2027-01-10",
            ),
            row(
                &[("form_code", "1601C"), ("period_label", "January 2026")],
                "2026-02-10",
            ),
            row(
                &[
                    ("form_code", "2551Q"),
                    ("due_date", "2026-04-20"),
                    ("frequency", "Quarterly"),
                    ("period", "1"),
                    ("period_label", "Q1 2026"),
                ],
                "2026-04-20",
            ),
            row(
                &[
                    ("form_code", "2551Q"),
                    ("due_date", "2026-07-20"),
                    ("frequency", "Quarterly"),
                    ("period", "2"),
                    ("period_label", "Q2 2026"),
                ],
                "2026-07-20",
            ),
            row(
                &[
                    ("form_code", "1701A"),
                    ("due_date", "2027-04-15"),
                    ("frequency", "Annual"),
                    ("period", "0"),
                    ("period_label", "TY 2026"),
                ],
                "2027-04-15",
            ),
            row(
                &[
                    ("form_code", "1601C"),
                    ("period_label", "January 2026"),
                    ("tin", "00011122900000"),
                    ("tin_root", "000111229"),
                    ("taxpayer_name", "Sample Trading Corp"),
                ],
                "2026-02-10",
            ),
        ]
    }

    fn base() -> Filters {
        Filters {
            year: 2026,
            ..Default::default()
        }
    }

    #[test]
    fn t_who_and_freq() {
        let r = rows();
        assert_eq!(by_who_and_how_often(&base(), &r).len(), 6);
        let mut f = base();
        f.tps = Some(HashSet::from(["000111229".to_owned()]));
        assert_eq!(by_who_and_how_often(&f, &r).len(), 1);
        f.tps = Some(HashSet::new());
        assert_eq!(by_who_and_how_often(&f, &r).len(), 0);
        let mut f = base();
        f.freq = Freq::Quarterly;
        assert_eq!(
            by_who_and_how_often(&f, &r)
                .iter()
                .map(|d| d.form_code.as_str())
                .collect::<Vec<_>>(),
            vec!["2551Q", "2551Q"]
        );
        f.freq = Freq::Yearly;
        assert_eq!(by_who_and_how_often(&f, &r)[0].form_code, "1701A");
        f.freq = Freq::Monthly;
        assert_eq!(by_who_and_how_often(&f, &r).len(), 3);
    }

    #[test]
    fn t_calendar_rows() {
        let r = rows();
        let dues: Vec<String> = calendar_rows(&base(), &r)
            .iter()
            .map(|d| d.due_date.clone())
            .collect();
        assert!(dues.contains(&"2027-01-10".to_owned()));
        let mut f = base();
        f.year = 2027;
        assert!(calendar_rows(&f, &r).is_empty());
        let mut f = base();
        f.freq = Freq::Quarterly;
        f.quarter = 2;
        assert_eq!(calendar_rows(&f, &r)[0].period_label, "Q2 2026");
        let mut f = base();
        f.freq = Freq::Quarterly;
        assert_eq!(calendar_rows(&f, &r).len(), 2);
    }

    #[test]
    fn t_month_chip() {
        let r = rows();
        let mut f = base();
        f.year = 2027;
        f.freq = Freq::Monthly;
        f.month = 1;
        let jan = calendar_rows(&f, &r);
        assert_eq!(jan[0].period_label, "December 2026");
        let mut f = base();
        f.freq = Freq::Monthly;
        f.month = 2;
        assert_eq!(calendar_rows(&f, &r).len(), 2);
        let mut f = base();
        f.month = 2;
        assert_eq!(calendar_rows(&f, &r).len(), 6);
    }

    #[test]
    fn t_filters_active() {
        let now = d("2026-06-01");
        assert!(!filters_active(&base(), now));
        let mut f = base();
        f.tps = Some(HashSet::from(["x".to_owned()]));
        assert!(filters_active(&f, now));
        let mut f = base();
        f.freq = Freq::Yearly;
        assert!(filters_active(&f, now));
        let mut f = base();
        f.month = 3;
        assert!(filters_active(&f, now));
        let mut f = base();
        f.quarter = 3;
        assert!(filters_active(&f, now));
        let mut f = base();
        f.year = 2025;
        assert!(filters_active(&f, now));
    }

    #[test]
    fn t_count_text() {
        let r = rows();
        let owned: Vec<DeadlineRow> = r.clone();
        let refs: Vec<&DeadlineRow> = owned.iter().collect();
        assert_eq!(
            count_text(&base(), &refs),
            "6 deadlines · 2 taxpayers · 2026 returns"
        );
        let one = vec![&owned[0]];
        assert_eq!(
            count_text(&base(), &one),
            "1 deadline · 1 taxpayer · 2026 returns"
        );
        let mut f = base();
        f.freq = Freq::Monthly;
        f.month = 2;
        assert!(count_text(&f, &refs).contains("due in February 2026"));
        let mut f = base();
        f.freq = Freq::Quarterly;
        f.quarter = 3;
        assert!(count_text(&f, &refs).contains("returns for Q3 2026"));
    }

    #[test]
    fn t_group_rows() {
        let r = rows();
        let refs: Vec<&DeadlineRow> = r.iter().collect();
        let by_month = group_rows(
            &Filters {
                freq: Freq::All,
                ..Default::default()
            },
            &refs,
        );
        let labels: Vec<&str> = by_month.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "February 2026",
                "April 2026",
                "July 2026",
                "January 2027",
                "April 2027"
            ]
        );
        assert_eq!(by_month[0].rows.len(), 2);
        assert!(by_month[0].caption.is_none());
        let mut f = base();
        f.freq = Freq::Quarterly;
        let qrefs = by_who_and_how_often(&f, &r);
        let q = group_rows(&f, &qrefs);
        assert_eq!(q[0].title, "Q1 2026 returns");
        assert_eq!(q[1].title, "Q2 2026 returns");
        assert_eq!(q[0].caption.as_deref(), Some("due Apr 20"));
    }

    #[test]
    fn t_kind_who_key() {
        assert_eq!(kind_of("Draft"), RowKind::Draft);
        assert_eq!(kind_of("Queued"), RowKind::Queued);
        assert_eq!(kind_of("Submitted"), RowKind::Filed);
        assert_eq!(kind_of("Confirmed"), RowKind::Filed);
        assert_eq!(kind_of("Paid"), RowKind::Filed);
        assert_eq!(kind_of("Not started"), RowKind::NotStarted);
        let r = rows();
        assert_eq!(who_line(&r[0]), "Juana Santos");
        let mut two = r[0].clone();
        two.branch_count = 2;
        assert_eq!(who_line(&two), "Juana Santos · Head office");
        assert_eq!(row_key(&r[1]), "00012345300000-1601C-2026-1");
        let mut an = r[1].clone();
        an.frequency = "As needed".into();
        an.draft_id = Some(7);
        assert_eq!(row_key(&an), "00012345300000-1601C-2026-1-7");
    }

    #[test]
    fn t_tones() {
        let now = d("2026-06-10");
        let t = |status: &str, due: &str| tone_of(&row(&[("status", status)], due), now);
        assert_eq!(t("Not started", "2026-06-09"), "danger");
        assert_eq!(t("Draft", "2026-06-01"), "danger");
        assert_eq!(t("Not started", "2026-06-10"), "warning");
        assert_eq!(t("Draft", "2026-06-24"), "warning");
        assert_eq!(t("Not started", "2026-06-25"), "neutral");
        assert_eq!(t("Draft", "2026-07-25"), "accent");
        assert_eq!(t("Queued", "2026-06-01"), "accent");
        assert_eq!(t("Submitted", "2026-06-01"), "success");
        assert_eq!(t("Paid", "2026-08-01"), "success");
        let late = row(&[("status", "Not started")], "2026-06-01");
        let soon = row(&[("status", "Not started")], "2026-06-12");
        let filed = row(&[("status", "Submitted")], "2026-06-01");
        let far = row(&[("status", "Not started")], "2026-09-01");
        assert_eq!(day_tone(&[&filed, &late], now), "danger");
        assert_eq!(day_tone(&[&filed, &soon], now), "warning");
        assert_eq!(day_tone(&[&filed], now), "success");
        assert_eq!(day_tone(&[&filed, &far], now), "neutral");
        assert_eq!(day_tone(&[], now), "neutral");
    }

    #[test]
    fn t_grid() {
        // November 2026 starts on a Sunday; December 2026 on a Tuesday.
        assert_eq!(month_grid_start(2026, 11).day(), 1);
        assert_eq!(month_grid_start(2026, 11).weekday(), Weekday::Sun);
        let dec = month_grid_start(2026, 12);
        assert_eq!((dec.month(), dec.day()), (11, 29));
        let r = rows();
        let cells = grid_cells(2026, 2, &r, d("2026-02-10"));
        assert_eq!(cells.len(), 42);
        assert_eq!(cells[0].iso, "2026-02-01");
        assert_eq!(cells[41].iso, "2026-03-14");
        assert_eq!(cells.iter().filter(|c| c.in_month).count(), 28);
        let tenth = cells.iter().find(|c| c.iso == "2026-02-10").unwrap();
        assert_eq!(tenth.items.len(), 2);
        assert!(tenth.is_today);
        assert_eq!(tenth.label, "February 10, 2 deadlines");
        assert_eq!(
            cells.iter().find(|c| c.iso == "2026-02-11").unwrap().label,
            "February 11, 0 deadlines"
        );
        assert_eq!(cells.iter().filter(|c| c.is_today).count(), 1);
        assert_eq!((cells[8].col, cells[8].row), (1, 1));
        assert_eq!(in_month_count(2026, 2, &r), 2);
        assert_eq!(in_month_count(2026, 4, &r), 1);
        assert_eq!(long_day("2026-02-10"), "Tuesday, February 10");
    }

    #[test]
    fn t_upcoming() {
        let now = d("2026-06-10");
        let list = vec![
            row(&[("form_code", "A")], "2026-06-20"),
            row(&[("form_code", "B")], "2026-05-10"),
            row(&[("form_code", "C"), ("status", "Draft")], "2026-06-10"),
            row(&[("form_code", "D"), ("status", "Submitted")], "2026-04-10"),
            row(&[("form_code", "E")], "2026-06-01"),
            row(&[("form_code", "F"), ("status", "Queued")], "2026-06-15"),
        ];
        let codes: Vec<String> = upcoming_rows(&list, now)
            .iter()
            .map(|d| d.form_code.clone())
            .collect();
        assert_eq!(codes, vec!["B", "E", "C", "A"]);
        assert_eq!(
            upcoming_range(now),
            ("2025-06-09".to_owned(), "2026-08-09".to_owned())
        );
    }
}
