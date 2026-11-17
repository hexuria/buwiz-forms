//! Filing periods as the screens name them (lib/periods.ts). The canonical period of a return is the
//! number the host takes: month 1-12 for monthly forms, quarter 1-4 for quarterly ones, 0 for annual
//! returns and 1 for an as-needed return's first open-ended slot. Pure: the clock is a parameter.

use super::dates::{MONTHS, MONTHS_LONG};
use chrono::{Datelike, Local};

/// How often a form is filed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Monthly,
    Quarterly,
    Annual,
    AsNeeded,
}

impl Frequency {
    pub fn parse(value: &str) -> Frequency {
        match value {
            "Monthly" => Frequency::Monthly,
            "Quarterly" => Frequency::Quarterly,
            "As needed" => Frequency::AsNeeded,
            _ => Frequency::Annual,
        }
    }

    /// The spelling the forms index and the host use.
    pub fn word(self) -> &'static str {
        match self {
            Frequency::Monthly => "Monthly",
            Frequency::Quarterly => "Quarterly",
            Frequency::Annual => "Annual",
            Frequency::AsNeeded => "As needed",
        }
    }
}

/// One line of the forms index: a form code and its frequency.
#[derive(Debug, Clone)]
pub struct FormIndexEntry {
    pub c: String,
    pub freq: Frequency,
}

/// A period the start-a-return dialog offers: the value the host takes and the label the dialog shows.
#[derive(Debug, Clone)]
pub struct PeriodOption {
    pub value: i64,
    pub label: String,
}

/// The frequency of a form code in the forms index; `Annual` when the index does not list it.
pub fn frequency_of<'a>(index: &'a [FormIndexEntry], code: &str) -> Frequency {
    index
        .iter()
        .find(|f| f.c == code)
        .map(|f| f.freq)
        .unwrap_or(Frequency::Annual)
}

/// Periods the start-a-return dialog offers for a form in a tax year, newest first. The current year
/// offers the months or quarters up to now; a past year all of them; a later year only January or Q1.
pub fn period_options(
    code: &str,
    freq: Frequency,
    year: i32,
    now: chrono::DateTime<Local>,
) -> Vec<PeriodOption> {
    let _ = code;
    let last_month = if year < now.year() {
        12
    } else if year > now.year() {
        1
    } else {
        now.month()
    };
    match freq {
        Frequency::Monthly => (1..=last_month)
            .rev()
            .map(|m| PeriodOption {
                value: i64::from(m),
                label: format!("{} {}", MONTHS_LONG[(m - 1) as usize], year),
            })
            .collect(),
        Frequency::Quarterly => {
            let last_quarter = last_month.div_ceil(3);
            (1..=last_quarter)
                .rev()
                .map(|q| PeriodOption {
                    value: i64::from(q),
                    label: format!("Q{q} {year}"),
                })
                .collect()
        }
        Frequency::AsNeeded => vec![PeriodOption {
            value: 1,
            label: format!("{year}, first return"),
        }],
        Frequency::Annual => vec![PeriodOption {
            value: 0,
            label: format!("TY {year}"),
        }],
    }
}

/// The human label for a period of a form with frequency `freq`. `short` gives "Sep 2026".
pub fn period_label(freq: Frequency, year: i32, period: i64, short: bool) -> String {
    match freq {
        Frequency::Monthly if (1..=12).contains(&period) => {
            let name = if short { MONTHS } else { MONTHS_LONG }[(period - 1) as usize];
            format!("{name} {year}")
        }
        Frequency::Quarterly if (1..=4).contains(&period) => format!("Q{period} {year}"),
        Frequency::AsNeeded => format!("{year} #{}", if period == 0 { 1 } else { period }),
        _ => format!("TY {year}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn labels() {
        assert_eq!(
            period_label(Frequency::Monthly, 2026, 9, false),
            "September 2026"
        );
        assert_eq!(period_label(Frequency::Monthly, 2026, 9, true), "Sep 2026");
        assert_eq!(
            period_label(Frequency::Quarterly, 2026, 2, false),
            "Q2 2026"
        );
        assert_eq!(period_label(Frequency::Annual, 2026, 0, false), "TY 2026");
        assert_eq!(period_label(Frequency::AsNeeded, 2026, 0, false), "2026 #1");
    }

    #[test]
    fn options_follow_the_clock() {
        let now = Local.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let monthly = period_options("1601C", Frequency::Monthly, 2026, now);
        assert_eq!(monthly.len(), 6);
        assert_eq!(monthly[0].label, "June 2026");
        let past = period_options("1601C", Frequency::Monthly, 2025, now);
        assert_eq!(past.len(), 12);
        let future = period_options("1601C", Frequency::Monthly, 2027, now);
        assert_eq!(future.len(), 1);
        assert_eq!(future[0].label, "January 2027");
    }
}
