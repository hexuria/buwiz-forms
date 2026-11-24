//! A period whose last day precedes the business start date is not owed
//! and is not listed: the conservative start-of-obligation rule shared
//! by the dashboard feed and the calendar events builder
//! (`DeadlineResolver::deadlines_for_forms_since`).

use bir_core::calendar_rules::{DeadlinePeriod, DeadlineResolver};
use chrono::NaiveDate;

fn start(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    Some(NaiveDate::from_ymd_opt(year, month, day).unwrap())
}

fn quarters(codes: &[&str], year: i32, business_start: Option<NaiveDate>) -> Vec<u8> {
    let codes: Vec<String> = codes.iter().map(|code| code.to_string()).collect();
    DeadlineResolver::deadlines_for_forms_since(&codes, year, &[], business_start)
        .iter()
        .filter_map(|deadline| match deadline.period {
            DeadlinePeriod::Quarterly { quarter, .. } => Some(quarter),
            _ => None,
        })
        .collect()
}

fn months(codes: &[&str], year: i32, business_start: Option<NaiveDate>) -> Vec<u8> {
    let codes: Vec<String> = codes.iter().map(|code| code.to_string()).collect();
    DeadlineResolver::deadlines_for_forms_since(&codes, year, &[], business_start)
        .iter()
        .filter_map(|deadline| match deadline.period {
            DeadlinePeriod::Monthly { month, .. } => Some(month),
            _ => None,
        })
        .collect()
}

fn annuals(codes: &[&str], year: i32, business_start: Option<NaiveDate>) -> Vec<String> {
    let codes: Vec<String> = codes.iter().map(|code| code.to_string()).collect();
    DeadlineResolver::deadlines_for_forms_since(&codes, year, &[], business_start)
        .iter()
        .filter(|deadline| matches!(deadline.period, DeadlinePeriod::Annual { .. }))
        .map(|deadline| deadline.form_code.clone())
        .collect()
}

/// The iPad fault: a business that began 2026-10-01 owes the quarter that
/// contains the start and the start year's annual return, and nothing
/// earlier. 1701Q has no Q4 — the annual ITR covers it — so an October
/// start owes no 1701Q quarter at all.
#[test]
fn a_start_on_a_quarters_first_day_lists_that_quarter_and_nothing_earlier() {
    for code in ["2550Q", "2551Q"] {
        assert_eq!(quarters(&[code], 2026, start(2026, 10, 1)), [4], "{code}");
    }
    assert!(
        quarters(&["1701Q"], 2026, start(2026, 10, 1)).is_empty(),
        "1701Q has only Q1-Q3, all ended before the start"
    );
    assert_eq!(annuals(&["1701"], 2026, start(2026, 10, 1)), ["1701"]);
}

/// A start on a quarter's LAST day leaves that quarter owed: the period
/// does not end before the business began. (Q3 is 1701Q's last quarter.)
#[test]
fn a_start_on_a_quarters_last_day_keeps_that_quarter_owed() {
    for code in ["2550Q", "2551Q"] {
        assert_eq!(
            quarters(&[code], 2026, start(2026, 9, 30)),
            [3, 4],
            "{code}"
        );
    }
    assert_eq!(quarters(&["1701Q"], 2026, start(2026, 9, 30)), [3]);
}

/// The first day of Q3 keeps Q3 and Q4 owed (Q3 only for 1701Q, which has
/// no Q4).
#[test]
fn a_start_on_the_first_day_of_a_middle_quarter() {
    for code in ["2550Q", "2551Q"] {
        assert_eq!(quarters(&[code], 2026, start(2026, 7, 1)), [3, 4], "{code}");
    }
    assert_eq!(quarters(&["1701Q"], 2026, start(2026, 7, 1)), [3]);
}

/// A month that ends before the start is not owed; the start month is.
#[test]
fn a_monthly_form_drops_the_months_ended_before_the_start() {
    assert_eq!(months(&["1601C"], 2026, start(2026, 10, 15)), [10, 11, 12]);
}

/// A start on a month's last day keeps that month owed.
#[test]
fn a_start_on_a_months_last_day_keeps_that_month_owed() {
    assert_eq!(
        months(&["1601C"], 2026, start(2026, 9, 30)),
        [9, 10, 11, 12]
    );
}

/// No start date: identical to `deadlines_for_forms`.
#[test]
fn no_start_date_lists_every_period() {
    assert_eq!(quarters(&["2551Q"], 2026, None), [1, 2, 3, 4]);
    assert_eq!(months(&["1601C"], 2026, None), (1..=12).collect::<Vec<_>>());
    assert_eq!(annuals(&["1701"], 2026, None), ["1701"]);
}

/// A start after the viewed year: nothing is owed that year, the annual
/// return included.
#[test]
fn a_start_in_a_later_year_leaves_the_year_unowed() {
    let codes = vec!["1701".to_string(), "2551Q".to_string(), "1601C".to_string()];
    assert!(
        DeadlineResolver::deadlines_for_forms_since(&codes, 2026, &[], start(2027, 5, 1))
            .is_empty()
    );
}

/// A start in an earlier year: everything is owed that year.
#[test]
fn a_start_in_an_earlier_year_owes_everything() {
    assert_eq!(quarters(&["2551Q"], 2026, start(2025, 5, 1)), [1, 2, 3, 4]);
    assert_eq!(
        months(&["1601C"], 2026, start(2025, 5, 1)),
        (1..=12).collect::<Vec<_>>()
    );
    assert_eq!(annuals(&["1701"], 2026, start(2025, 5, 1)), ["1701"]);
}

/// An as-needed obligation names no period and is listed as before.
#[test]
fn as_needed_obligations_are_not_filtered() {
    let codes = vec!["1600WP".to_string()];
    let listed = DeadlineResolver::deadlines_for_forms_since(&codes, 2026, &[], start(2027, 1, 1));
    assert_eq!(listed.len(), 1);
    assert!(matches!(listed[0].period, DeadlinePeriod::EventBased));
}
