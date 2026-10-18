//! The official non-working holidays the app ships, one table per year, and
//! the deadline overrides they seed (decision D8a, 2026-10-18).
//!
//! A year is one added [`HolidayYear`] in [`OFFICIAL_HOLIDAY_YEARS`]; new
//! years arrive in app updates. [`crate::db::Database::seed_official_holidays`]
//! writes them into the stored deadline overrides on first run and adds only
//! what is missing afterwards; it never touches an override the user saved.

use chrono::{Datelike, Duration, NaiveDate};

use crate::calendar_rules::{
    BusinessDayCalendar, DeadlineOverride, DeadlineResolver, NonWorkingDay, canonical_form_code,
};

/// One official non-working day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holiday {
    pub month: u32,
    pub day: u32,
    pub name: &'static str,
}

/// One year's official list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HolidayYear {
    pub year: i32,
    /// The proclamation that declares the list, e.g. "Proclamation No. 727,
    /// s. 2024". `None` while the number is not on record in the repo; the
    /// seeded override then cites [`HolidayYear::source_reference`]'s
    /// fallback.
    pub proclamation: Option<&'static str>,
    pub days: &'static [Holiday],
}

impl HolidayYear {
    /// What a seeded override cites as its source.
    pub fn source_reference(&self) -> String {
        match self.proclamation {
            Some(proclamation) => proclamation.to_string(),
            None => format!(
                "Official {} regular holidays (proclamation number not yet recorded)",
                self.year
            ),
        }
    }

    pub fn dates(&self) -> impl Iterator<Item = (NaiveDate, &'static str)> + '_ {
        self.days.iter().filter_map(move |holiday| {
            NaiveDate::from_ymd_opt(self.year, holiday.month, holiday.day)
                .map(|date| (date, holiday.name))
        })
    }
}

/// 2026: the regular holidays the app already carried (the list
/// `seed_2026_holiday_overrides` seeded). The repo records no proclamation
/// number for it, and no special non-working days.
const HOLIDAYS_2026: HolidayYear = HolidayYear {
    year: 2026,
    proclamation: None,
    days: &[
        Holiday {
            month: 1,
            day: 1,
            name: "New Year's Day",
        },
        Holiday {
            month: 4,
            day: 2,
            name: "Maundy Thursday",
        },
        Holiday {
            month: 4,
            day: 3,
            name: "Good Friday",
        },
        Holiday {
            month: 4,
            day: 9,
            name: "Araw ng Kagitingan",
        },
        Holiday {
            month: 5,
            day: 1,
            name: "Labor Day",
        },
        Holiday {
            month: 6,
            day: 12,
            name: "Independence Day",
        },
        Holiday {
            month: 8,
            day: 31,
            name: "National Heroes Day",
        },
        Holiday {
            month: 11,
            day: 30,
            name: "Bonifacio Day",
        },
        Holiday {
            month: 12,
            day: 25,
            name: "Christmas Day",
        },
        Holiday {
            month: 12,
            day: 30,
            name: "Rizal Day",
        },
    ],
};

/// Every year the app ships a list for, oldest first.
pub const OFFICIAL_HOLIDAY_YEARS: &[HolidayYear] = &[HOLIDAYS_2026];

/// Whether `date` is a weekend or one of `years`' holidays.
fn is_non_working(date: NaiveDate, years: &[HolidayYear]) -> bool {
    matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
        || years
            .iter()
            .any(|year| year.dates().any(|(holiday, _)| holiday == date))
}

/// The next working day after `date`: past weekends and every listed holiday.
pub fn next_working_day(date: NaiveDate, years: &[HolidayYear]) -> NaiveDate {
    let mut next = date + Duration::days(1);
    while is_non_working(next, years) {
        next += Duration::days(1);
    }
    next
}

/// The working-day calendar of every shipped year: weekends and the listed
/// holidays are not working days (what banking days are counted on).
pub fn business_day_calendar() -> BusinessDayCalendar {
    BusinessDayCalendar::with_non_working_days(
        OFFICIAL_HOLIDAY_YEARS
            .iter()
            .flat_map(|year| {
                year.dates()
                    .map(|(date, name)| NonWorkingDay {
                        source_reference: Some(year.source_reference()),
                        ..NonWorkingDay::regular_holiday(date, name)
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    )
}

/// The stable id of the override a holiday seeds.
pub fn seed_id(date: NaiveDate) -> String {
    format!("official-holiday-{date}")
}

/// Every canonical form code the calendar has a rule for: a holiday moves
/// any of their deadlines that fall on it.
fn affected_form_codes() -> Vec<String> {
    let mut codes: Vec<String> = DeadlineResolver::official_rules()
        .iter()
        .flat_map(|rule| {
            rule.form_nos
                .iter()
                .map(|no| canonical_form_code(no).to_string())
        })
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

/// The overrides `years` seed: one per holiday, moving a deadline on it to
/// the next working day.
pub fn holiday_overrides(years: &[HolidayYear]) -> Vec<DeadlineOverride> {
    let codes = affected_form_codes();
    let mut out = Vec::new();
    for year in years {
        for (date, name) in year.dates() {
            out.push(DeadlineOverride {
                id: seed_id(date),
                title: format!("{name} {}", year.year),
                source_reference: year.source_reference(),
                affected_form_codes: codes.clone(),
                original_deadline: date,
                adjusted_deadline: next_working_day(date, years),
                affected_regions: Vec::new(),
                affected_taxpayer_types: Vec::new(),
                effective_from: None,
                effective_until: None,
                expires_at: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_date_is_real_and_in_its_year() {
        for year in OFFICIAL_HOLIDAY_YEARS {
            assert_eq!(year.dates().count(), year.days.len(), "{}", year.year);
            assert!(year.dates().all(|(date, _)| date.year() == year.year));
        }
    }

    #[test]
    fn the_working_calendar_skips_the_shipped_holidays() {
        let calendar = business_day_calendar();
        assert!(calendar.is_non_working_day(NaiveDate::from_ymd_opt(2026, 6, 12).unwrap()));
        assert!(calendar.is_business_day(NaiveDate::from_ymd_opt(2026, 6, 11).unwrap()));
    }

    #[test]
    fn a_holiday_moves_past_the_next_holiday_and_the_weekend() {
        // Maundy Thursday 2026 is followed by Good Friday, then the weekend.
        let overrides = holiday_overrides(OFFICIAL_HOLIDAY_YEARS);
        let maundy = overrides
            .iter()
            .find(|o| o.original_deadline == NaiveDate::from_ymd_opt(2026, 4, 2).unwrap())
            .unwrap();
        assert_eq!(
            maundy.adjusted_deadline,
            NaiveDate::from_ymd_opt(2026, 4, 6).unwrap()
        );
        assert_eq!(maundy.id, "official-holiday-2026-04-02");
        assert!(!maundy.source_reference.trim().is_empty());
        assert!(maundy.affected_form_codes.iter().any(|c| c == "1601C"));
        assert!(maundy.affected_form_codes.iter().any(|c| c == "2200C"));
    }
}
