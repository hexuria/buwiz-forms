//! Date helpers, in one file (lib/dates.ts). Local time throughout. Pure: the clock is a parameter.

use chrono::{Datelike, Local, NaiveDate, TimeZone};

/// A calendar day: the year, the month (1-12) and the day of the month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ymd {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

/// A date split into the month, day and year boxes that hold it (oneDateText). `day` is `None` when the
/// form has no day box.
pub struct OneDateParts<'a> {
    pub month: &'a str,
    pub day: Option<&'a str>,
    pub year: &'a str,
}

pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub const MONTHS_LONG: [&str; 12] = [
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

/// What a date box says when its text is not a date.
pub const DATE_ERROR: &str = "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)";

/// ISO date (YYYY-MM-DD) in local time.
pub fn iso_date(date: chrono::DateTime<Local>) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// The date `days` after (or, negative, before) the given one.
pub fn add_days(date: NaiveDate, days: i64) -> NaiveDate {
    date + chrono::Duration::days(days)
}

/// Whole days from today (local midnight) to an ISO date. `now` is the clock.
pub fn days_until(iso: &str, now: chrono::DateTime<Local>) -> i64 {
    let today = now.date_naive();
    let target = NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap_or(today);
    (target - today).num_days()
}

/// "Oct 9, 2026" from an ISO date or a SQLite timestamp.
pub fn short_date(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    // SQLite timestamps ("2026-10-14 22:10:03") are UTC; show the local day.
    let bytes = text.as_bytes();
    if bytes.len() >= 16
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && (bytes[10] == b' ' || bytes[10] == b'T')
    {
        if let (Ok(y), Ok(mo), Ok(d), Ok(h), Ok(mi)) = (
            text[0..4].parse::<i32>(),
            text[5..7].parse::<u32>(),
            text[8..10].parse::<u32>(),
            text[11..13].parse::<u32>(),
            text[14..16].parse::<u32>(),
        ) && let Some(utc) =
            chrono::NaiveDate::from_ymd_opt(y, mo, d).and_then(|date| date.and_hms_opt(h, mi, 0))
        {
            let local = Local.from_utc_datetime(&utc);
            return format!(
                "{} {}, {}",
                MONTHS[local.month() as usize - 1],
                local.day(),
                local.year()
            );
        }
    }
    let date_part = text.split([' ', 'T']).next().unwrap_or(text);
    let mut parts = date_part.split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return text.to_owned();
    };
    let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>()) else {
        return text.to_owned();
    };
    if y == 0 || m == 0 || m > 12 || d == 0 {
        return text.to_owned();
    }
    format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y)
}

/// A timestamp starting with an ISO date reads as "Oct 1, 2026"; anything else as given.
pub fn short_iso_date(text: &str) -> String {
    let b = text.as_bytes();
    if b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && let (Ok(y), Ok(m), Ok(d)) = (
            text[0..4].parse::<i32>(),
            text[5..7].parse::<u32>(),
            text[8..10].parse::<u32>(),
        )
        && (1..=12).contains(&m)
        && d >= 1
    {
        return format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y);
    }
    text.to_owned()
}

/// A date the filer typed, read the way the old tax-profile date input read it: 12012020, 12/01/2020,
/// 12-01-2020, 2020-12-01 or Dec 01 2020 all mean December 1, 2020. `None` when it is not a real day.
pub fn parse_typed_date(text: &str) -> Option<Ymd> {
    let v = text.trim();
    if v.is_empty() {
        return None;
    }
    const MON: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let (y, m, d);
    if v.len() == 8 && v.bytes().all(|b| b.is_ascii_digit()) {
        // 12012020
        m = v[0..2].parse().ok()?;
        d = v[2..4].parse().ok()?;
        y = v[4..8].parse().ok()?;
    } else if let Some((yy, rest)) = v.split_once('-')
        && yy.len() == 4
        && yy.bytes().all(|b| b.is_ascii_digit())
    {
        // 2020-12-01 or 2020-12-1
        let (mm, dd) = rest.split_once('-')?;
        if dd.contains('-') {
            return None;
        }
        y = yy.parse().ok()?;
        m = mm.parse().ok()?;
        d = dd.parse().ok()?;
    } else if v.contains('/') || (v.contains('-') && v.split('-').count() == 3) {
        // 12/01/2020 or 12-01-2020
        let sep = if v.contains('/') { '/' } else { '-' };
        let parts: Vec<&str> = v.split(sep).collect();
        if parts.len() != 3 {
            return None;
        }
        m = parts[0].parse().ok()?;
        d = parts[1].parse().ok()?;
        y = parts[2].parse().ok()?;
    } else {
        // "Dec 01 2020" / "December 1, 2020"
        let lower = v.to_lowercase();
        let parts: Vec<&str> = lower
            .split(|c: char| c == ' ' || c == ',')
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let name = parts[0].trim_end_matches('.');
        m = MON.iter().position(|mon| name.starts_with(mon))? as u32 + 1;
        d = parts[1].parse().ok()?;
        y = parts[2].parse().ok()?;
    }
    if !(1900..=2100).contains(&y) || !(1..=12).contains(&m) || d < 1 {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(y, m, d)?;
    // Day must exist in the month (from_ymd_opt already rejects e.g. Feb 30).
    Some(Ymd {
        y,
        m,
        d: date.day(),
    })
}

/// The form's MM/DD/YYYY text for a parsed date.
pub fn format_mdy(date: Ymd) -> String {
    format!("{:02}/{:02}/{}", date.m, date.d, date.y)
}

/// How a date box shows a parsed date: "Dec 10, 2020".
pub fn format_long_date(date: Ymd) -> String {
    format!("{} {}, {}", MONTHS[(date.m - 1) as usize], date.d, date.y)
}

/// What a date box shows for the draft's text: the date as "Dec 10, 2020" when it reads as one, else the
/// text as it is. Display only: the draft keeps the form's own format.
pub fn shown_date(text: &str) -> String {
    match parse_typed_date(text) {
        Some(date) => format_long_date(date),
        None => text.to_owned(),
    }
}

/// The three boxes' values as the MM/DD/YYYY text a date box reads ("" when all are blank).
pub fn one_date_text(values: &dyn Fn(&str) -> String, date: &OneDateParts) -> String {
    let mut parts = Vec::with_capacity(3);
    parts.push(values(date.month).trim().to_owned());
    if let Some(day) = date.day {
        parts.push(values(day).trim().to_owned());
    }
    parts.push(values(date.year).trim().to_owned());
    if parts.iter().all(|p| p.is_empty()) {
        String::new()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_dates() {
        let dec1 = Ymd {
            y: 2020,
            m: 12,
            d: 1,
        };
        for text in [
            "12012020",
            "12/01/2020",
            "12-01-2020",
            "2020-12-01",
            "Dec 01 2020",
            "December 1, 2020",
        ] {
            assert_eq!(parse_typed_date(text), Some(dec1), "{text}");
        }
        assert_eq!(parse_typed_date("2/30/2020"), None);
        assert_eq!(parse_typed_date(""), None);
        assert_eq!(format_mdy(dec1), "12/01/2020");
        assert_eq!(format_long_date(dec1), "Dec 1, 2020");
        assert_eq!(shown_date("12/01/2020"), "Dec 1, 2020");
        assert_eq!(shown_date(""), "");
    }

    #[test]
    fn short_dates() {
        assert_eq!(short_iso_date("2026-10-01T00:00:00"), "Oct 1, 2026");
        assert_eq!(short_iso_date("nope"), "nope");
        assert_eq!(short_date("2026-10-09"), "Oct 9, 2026");
    }
}
