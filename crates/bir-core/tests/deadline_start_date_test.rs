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

// ---------------------------------------------------------------------------
// The earliest-start construction and the saved-return guard.
// ---------------------------------------------------------------------------

use bir_core::forms::{FilingStatus, FormDraftSummary};
use bir_core::naming::Tin;
use bir_core::profile::{TaxpayerProfile, TaxpayerType};

fn profile_with_start(business_start_date: Option<NaiveDate>) -> TaxpayerProfile {
    TaxpayerProfile {
        id: Some(1),
        full_name: "Start Test".into(),
        tin: Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        },
        rdo_code: "039".into(),
        line_of_business: "Services".into(),
        registered_address: "QC".into(),
        zip_code: "1100".into(),
        phone: "09170000000".into(),
        email: "start@example.com".into(),
        default_form_type: "2551Q".into(),
        taxpayer_type: TaxpayerType::Individual,
        is_vat_registered: false,
        business_start_date,
        tax_classification: None,
        eopt_tier: None,
        is_bmbe: false,
        is_gpp_partner: false,
        is_create_msme: false,
        is_expanded_withholding_agent: false,
        atc_codes: vec![],
        excise_tax_categories: vec![],
        tax_elections: vec![],
        has_employees: false,
        is_dormant: false,
        has_single_employer: false,
        withholds_compensation: false,
        withholds_expanded: false,
        withholds_final: false,
        is_top_withholding_agent: false,
        is_government_withholding_entity: false,
        registration_activity_status: Default::default(),
        is_archived: false,
        profile_pin_hash: None,
        totp_secret: None,
        email_tracking_enabled: false,
        email_auth_method: Default::default(),
        imap_email: None,
        imap_host: None,
        test_notification_enabled: false,
        imap_app_password: None,
        oauth_access_token: None,
        oauth_refresh_token: None,
        profile_versions: vec![],
        birth_date: None,
        compliance_source_mode: Default::default(),
        per_year_forms: Default::default(),
    }
}

fn version_dated(
    profile: &TaxpayerProfile,
    registration: NaiveDate,
) -> bir_core::profile::TaxProfileVersion {
    let mut version = bir_core::profile::TaxProfileVersion::from_profile_backfill(profile);
    version.cor.registration_date = Some(registration);
    version
}

/// Two certificate versions (a 2015 registration, a 2026 re-issue): the
/// earliest date governs, so every 2026 period stays owed.
#[test]
fn two_versions_use_the_earliest_registration_date() {
    let mut profile = profile_with_start(None);
    profile.profile_versions = vec![
        version_dated(&profile, NaiveDate::from_ymd_opt(2015, 3, 2).unwrap()),
        version_dated(&profile, NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()),
    ];
    let start = profile.earliest_business_start(None);
    assert_eq!(start, NaiveDate::from_ymd_opt(2015, 3, 2));
    assert_eq!(quarters(&["2551Q"], 2026, start), [1, 2, 3, 4]);
    assert_eq!(
        months(&["1601C"], 2026, start),
        (1..=12).collect::<Vec<_>>()
    );
}

/// A single certificate dated 2026-10-01: the start is that date.
#[test]
fn a_single_version_dates_the_start() {
    let mut profile = profile_with_start(None);
    profile.profile_versions = vec![version_dated(
        &profile,
        NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
    )];
    assert_eq!(
        profile.earliest_business_start(None),
        NaiveDate::from_ymd_opt(2026, 10, 1)
    );
}

/// A later version whose certificate carries an EARLIER date than the
/// first moves the start earlier.
#[test]
fn a_later_version_with_an_earlier_date_moves_the_start_earlier() {
    let mut profile = profile_with_start(None);
    profile.profile_versions = vec![
        version_dated(&profile, NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()),
        version_dated(&profile, NaiveDate::from_ymd_opt(2015, 3, 2).unwrap()),
    ];
    assert_eq!(
        profile.earliest_business_start(None),
        NaiveDate::from_ymd_opt(2015, 3, 2)
    );
}

/// The flat profile field is one of the candidates, and the year's facts
/// the user corrected is another.
#[test]
fn the_flat_field_and_the_year_fact_are_candidates() {
    let mut profile = profile_with_start(NaiveDate::from_ymd_opt(2015, 3, 2));
    profile.profile_versions = vec![version_dated(
        &profile,
        NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
    )];
    assert_eq!(
        profile.earliest_business_start(None),
        NaiveDate::from_ymd_opt(2015, 3, 2),
        "the re-issued certificate never moves the start later"
    );
    let no_version = profile_with_start(None);
    assert_eq!(
        no_version.earliest_business_start(NaiveDate::from_ymd_opt(2026, 10, 1)),
        NaiveDate::from_ymd_opt(2026, 10, 1),
        "the year fact stands when nothing else is known"
    );
}

fn filing(form_code: &str, quarter: Option<u8>, month: Option<u8>) -> FormDraftSummary {
    FormDraftSummary {
        id: 1,
        tin: "010558054000".into(),
        form_code: form_code.into(),
        taxable_year: 2026,
        quarter,
        month,
        status: FilingStatus::Draft,
        updated_at: String::new(),
    }
}

fn all_quarters_2551q() -> Vec<bir_core::calendar_rules::ResolvedTaxDeadline> {
    DeadlineResolver::deadlines_for_forms(&["2551Q".to_string()], 2026, &[])
}

/// The guard: a saved return for a pre-start period proves the start
/// wrong — the whole list is shown unfiltered.
#[test]
fn a_saved_return_before_the_start_lifts_the_filter() {
    let filings = vec![filing("2551Q", Some(1), None)];
    let listed = DeadlineResolver::filter_deadlines_for_listing(
        all_quarters_2551q(),
        start(2026, 10, 1),
        &filings,
    );
    let quarters: Vec<u8> = listed
        .iter()
        .filter_map(|d| match d.period {
            DeadlinePeriod::Quarterly { quarter, .. } => Some(quarter),
            _ => None,
        })
        .collect();
    assert_eq!(quarters, [1, 2, 3, 4]);
}

/// A saved return in a post-start period changes nothing.
#[test]
fn a_saved_return_after_the_start_keeps_the_filter() {
    let filings = vec![filing("2551Q", Some(4), None)];
    let listed = DeadlineResolver::filter_deadlines_for_listing(
        all_quarters_2551q(),
        start(2026, 10, 1),
        &filings,
    );
    assert_eq!(listed.len(), 1);
}

/// A saved return of another form changes nothing.
#[test]
fn a_saved_return_of_another_form_keeps_the_filter() {
    let filings = vec![filing("1601C", None, Some(2))];
    let listed = DeadlineResolver::filter_deadlines_for_listing(
        all_quarters_2551q(),
        start(2026, 10, 1),
        &filings,
    );
    assert_eq!(listed.len(), 1);
}

/// No start date: the guard never applies.
#[test]
fn no_start_lists_everything_regardless_of_filings() {
    let listed = DeadlineResolver::filter_deadlines_for_listing(all_quarters_2551q(), None, &[]);
    assert_eq!(listed.len(), 4);
}
