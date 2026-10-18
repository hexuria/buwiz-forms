//! The trigger date an as-needed return holds (decision D8c, 2026-10-18).
//!
//! An as-needed return is due relative to an event, not a period: the
//! removal date for the excise returns 2200A, 2200AN, 2200M, 2200P, 2200S
//! and 2200T (Item 1), the withholding date for 1600WP (Item 1), and each
//! other form's own typed date (0605's due date, the transaction date of
//! 1706, 1707, 2000OT and 2552, the donation date of 1800, the date of death
//! of 1801). Once a saved return holds that date it has a computed due date
//! ([`crate::calendar_rules::DeadlineResolver::event_deadline`]); until then
//! it has none.
//!
//! **Only a date the filer entered counts.** Several constructors pre-fill
//! a real date (the excise returns and 1600WP: today or the month's last
//! day; 1706: today or January 1; 2552: the date its caller passes), so a
//! stored date alone does not say the event happened then. For those forms
//! ([`PREFILLED_TRIGGER_FORMS`]) a Draft counts only once the page recorded
//! that the filer wrote the date (`trigger_date_entered`, a flag on the
//! flattened `SubmissionLifecycle`, absent and so byte-identical on every
//! draft saved before it existed). A return that left Draft files the date
//! it holds, typed or not, so it is dated by that date; a cancelled queue
//! makes it a Draft again, and without the flag it is undated again. The
//! other as-needed forms start with the date blank (0, empty or `None`), so
//! any date they hold was typed.
//!
//! No migration: a Draft of a pre-filled form saved by the first D8c build
//! (which dated any held date) and never re-typed has no flag and is now
//! undated. The D8c build was never released (its branch chain is
//! unmerged), so no released return depends on the old reading.

use chrono::NaiveDate;
use serde_json::Value;

use crate::calendar_rules::{
    DeadlineResolver, EventDueRule, FORM_2552_PRIMARY_OFFERING_RULE, event_due_rule,
};

/// The date an as-needed return holds and the rule its due date follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTrigger {
    pub date: NaiveDate,
    pub rule: EventDueRule,
}

fn number(draft: &Value, key: &str) -> Option<u32> {
    draft
        .get(key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

/// A date held as three numeric fields; `None` while any is unset (0) or the
/// day does not exist.
fn date_from(draft: &Value, year: &str, month: &str, day: &str) -> Option<NaiveDate> {
    let year = i32::try_from(number(draft, year)?).ok()?;
    NaiveDate::from_ymd_opt(year, number(draft, month)?, number(draft, day)?)
}

/// 2000OT's Item 1 as stored, dated only when the form's own validator
/// takes it: `MM/DD/YYYY` (its `date_parts`), not before 2018 and not after
/// `today` (`Form2000OTDraft::validate`). Anything else (`M/D/YYYY`, which
/// the desktop view can store as typed, another separator, an impossible
/// day, a year out of range) is no date.
fn date_from_text(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    let (month, day, year) = super::form_2000ot::date_parts(text)?;
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    (year >= 2018 && date <= today).then_some(date)
}

/// The as-needed forms whose constructor pre-fills the trigger date.
pub const PREFILLED_TRIGGER_FORMS: &[&str] = &[
    "2200A", "2200AN", "2200M", "2200P", "2200S", "2200T", "1600WP", "1706", "2552",
];

/// The top-level draft fields that hold `form_code`'s trigger date (what
/// the page records `trigger_date_entered` for). Empty for a form without one.
pub fn trigger_paths(form_code: &str) -> &'static [&'static str] {
    match form_code {
        "2200A" | "2200AN" | "2200M" | "2200P" | "2200S" | "2200T" | "1600WP" => {
            &["month", "day", "year"]
        }
        "0605" => &["due_date"],
        "1706" | "1707" | "2552" => &["transaction_month", "transaction_day", "transaction_year"],
        "2000OT" => &["transaction_date"],
        "1800" => &["donation_month", "donation_day", "donation_year"],
        "1801" => &["death_month", "death_day", "death_year"],
        _ => &[],
    }
}

/// Whether the date a draft holds was entered (see the module docs).
fn date_was_entered(form_code: &str, draft: &Value) -> bool {
    if !PREFILLED_TRIGGER_FORMS.contains(&form_code) {
        return true;
    }
    let filed = draft
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "Draft");
    filed
        || draft
            .get("trigger_date_entered")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

/// The trigger date of a saved return of `form_code` from its stored
/// `form_drafts.data_json`. `None` for a form that is not as needed or has
/// no trigger-date rule, a return that does not hold an entered date yet, or
/// JSON that does not parse.
pub fn event_trigger_from_json(form_code: &str, data_json: &str) -> Option<EventTrigger> {
    if !DeadlineResolver::is_as_needed(form_code) {
        return None;
    }
    let mut rule = event_due_rule(form_code)?;
    let draft: Value = serde_json::from_str(data_json).ok()?;
    if !date_was_entered(form_code, &draft) {
        return None;
    }
    if form_code == "2552" {
        // Item 11 picks the rule: a primary offering is due from the
        // listing date, the LSE and a secondary offering from collection.
        match draft.get("transaction").and_then(Value::as_str) {
            Some("primary_offering") => rule = FORM_2552_PRIMARY_OFFERING_RULE,
            Some("local_stock_exchange" | "secondary_offering") => {}
            _ => return None,
        }
    }
    let date = held_trigger_date(form_code, &draft)?;
    Some(EventTrigger { date, rule })
}

/// The trigger date a draft of `form_code` holds in its fields, entered or
/// pre-filled (what [`event_trigger_from_json`] reads once it counts).
pub fn held_trigger_date(form_code: &str, draft: &Value) -> Option<NaiveDate> {
    match form_code {
        // Item 1: the removal date (the excise returns), the withholding
        // date (1600WP).
        "2200A" | "2200AN" | "2200M" | "2200P" | "2200S" | "2200T" | "1600WP" => {
            date_from(draft, "year", "month", "day")
        }
        "0605" => date_from(draft.get("due_date")?, "year", "month", "day"),
        "1706" | "1707" | "2552" => date_from(
            draft,
            "transaction_year",
            "transaction_month",
            "transaction_day",
        ),
        "2000OT" => date_from_text(
            draft.get("transaction_date")?.as_str()?,
            super::penalty_items::today(),
        ),
        "1800" => date_from(draft, "donation_year", "donation_month", "donation_day"),
        "1801" => date_from(draft, "death_year", "death_month", "death_day"),
        _ => None,
    }
}

/// The overrides a branch's as-needed deadlines are resolved with: the
/// stored global ones, then the profile's own for every year of its Forms
/// Set. The Home and Calendar feed and the Google Calendar sync both use it,
/// so they agree on a moved date.
pub fn event_overrides(
    db: &crate::db::Database,
    profile: &crate::profile::TaxpayerProfile,
    global: &[crate::calendar_rules::DeadlineOverride],
) -> Result<Vec<crate::calendar_rules::DeadlineOverride>, crate::db::DbError> {
    let mut overrides = global.to_vec();
    for year in db.list_forms_set_years(&profile.tin.full())? {
        overrides.extend(crate::integration::profile_deadline_overrides_for_year(
            profile, year,
        ));
    }
    Ok(overrides)
}

/// A saved as-needed return that holds its entered trigger date, with the
/// due date that date gives.
#[derive(Debug, Clone)]
pub struct DatedEventReturn {
    pub summary: super::FormDraftSummary,
    pub trigger: EventTrigger,
    pub deadline: crate::calendar_rules::ResolvedTaxDeadline,
}

/// Every saved as-needed return of `tin`, whatever its taxable year, that
/// holds an entered trigger date, with its due date
/// ([`DeadlineResolver::event_deadline`]: `overrides` applied, banking days
/// counted on `working`). What Home, the Calendar and the Google Calendar
/// sync list as dated as-needed rows.
pub fn dated_event_returns(
    db: &crate::db::Database,
    tin: &str,
    overrides: &[crate::calendar_rules::DeadlineOverride],
    working: &crate::calendar_rules::BusinessDayCalendar,
) -> Result<Vec<DatedEventReturn>, crate::db::DbError> {
    let mut out = Vec::new();
    for summary in db.list_draft_summaries_all_years(tin)? {
        if !DeadlineResolver::is_as_needed(&summary.form_code) {
            continue;
        }
        let Some(trigger) = db.draft_event_trigger(summary.id)? else {
            continue;
        };
        let Some(deadline) =
            DeadlineResolver::event_deadline(&trigger.rule, trigger.date, overrides, working)
        else {
            continue;
        };
        out.push(DatedEventReturn {
            summary,
            trigger,
            deadline,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar_rules::DueFromTrigger;
    use crate::profile::TaxpayerProfile;

    fn profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Example Trading Corp",
            "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Trading",
            "registered_address": "1 Example Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "office@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Corporation"
        }))
        .unwrap()
    }

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn trigger_of(code: &str, draft: &impl serde::Serialize) -> Option<EventTrigger> {
        event_trigger_from_json(code, &serde_json::to_string(draft).unwrap())
    }

    /// Each form's trigger is read from the field its draft keeps it in:
    /// the real draft types, serialized as they are stored.
    #[test]
    fn each_as_needed_draft_yields_its_typed_trigger_date() {
        let p = profile();
        macro_rules! item1 {
            ($ty:ty, $code:literal) => {{
                // As the constructor builds it: a pre-filled date, not entered.
                let mut draft = <$ty>::new_from_profile(&p, 2026, 10);
                assert_eq!(trigger_of($code, &draft), None, "{} untouched", $code);
                assert!(
                    !serde_json::to_string(&draft)
                        .unwrap()
                        .contains("trigger_date_entered"),
                    "an unmarked draft stores no marker"
                );
                draft.month = 10;
                draft.day = 7;
                draft.year = 2026;
                draft.lifecycle.trigger_date_entered = true;
                let trigger = trigger_of($code, &draft).expect($code);
                assert_eq!(trigger.date, ymd(2026, 10, 7), $code);
                assert_eq!(trigger.rule.form_code, $code);
                // A return that left Draft was filed with its date.
                draft.lifecycle.trigger_date_entered = false;
                draft.lifecycle.status = crate::forms::FilingStatus::Queued;
                assert!(trigger_of($code, &draft).is_some(), "{} queued", $code);
            }};
        }
        item1!(super::super::form_2200a::Form2200ADraft, "2200A");
        item1!(super::super::form_2200an::Form2200ANDraft, "2200AN");
        item1!(super::super::form_2200m::Form2200MDraft, "2200M");
        item1!(super::super::form_2200p::Form2200PDraft, "2200P");
        item1!(super::super::form_2200s::Form2200SDraft, "2200S");
        item1!(super::super::form_2200t::Form2200TDraft, "2200T");
        item1!(super::super::form_1600wp::Form1600WpDraft, "1600WP");

        let mut wp = super::super::form_1600wp::Form1600WpDraft::new_from_profile(&p, 2026, 10);
        wp.day = 5;
        wp.lifecycle.trigger_date_entered = true;
        assert_eq!(
            trigger_of("1600WP", &wp).unwrap().rule.due,
            DueFromTrigger::DaysAfter(20)
        );

        let mut f0605 = super::super::form_0605::Form0605Draft::new_from_profile(&p, 2026, 1);
        f0605.due_date = None;
        assert_eq!(trigger_of("0605", &f0605), None, "no due date typed yet");
        f0605.due_date = Some(super::super::form_0605::Form0605Date::new(2026, 10, 15).unwrap());
        assert_eq!(trigger_of("0605", &f0605).unwrap().date, ymd(2026, 10, 15));

        let mut f1706 = super::super::form_1706::Form1706Draft::new_from_profile(&p, 2026, 1);
        assert_eq!(trigger_of("1706", &f1706), None, "1706 untouched");
        f1706.lifecycle.trigger_date_entered = true;
        (
            f1706.transaction_month,
            f1706.transaction_day,
            f1706.transaction_year,
        ) = (9, 1, 2026);
        assert_eq!(trigger_of("1706", &f1706).unwrap().date, ymd(2026, 9, 1));

        let mut f1707 = super::super::form_1707::Form1707Draft::new_from_profile(&p, 2026, 1);
        (
            f1707.transaction_month,
            f1707.transaction_day,
            f1707.transaction_year,
        ) = (9, 2, 2026);
        assert_eq!(trigger_of("1707", &f1707).unwrap().date, ymd(2026, 9, 2));

        let mut dst = super::super::form_2000ot::Form2000OTDraft::new_from_profile(&p, 2026);
        assert_eq!(trigger_of("2000OT", &dst), None, "no transaction date yet");
        dst.transaction_date = "09/03/2026".to_string();
        assert_eq!(trigger_of("2000OT", &dst).unwrap().date, ymd(2026, 9, 3));

        let mut stock = super::super::form_2552::Form2552Draft::new_from_profile(&p, 2026, 10, 8);
        stock.set_transaction(super::super::form_2552::Form2552Transaction::LocalStockExchange);
        assert_eq!(trigger_of("2552", &stock), None, "2552 untouched");
        stock.lifecycle.trigger_date_entered = true;
        stock.set_transaction(super::super::form_2552::Form2552Transaction::Unspecified);
        assert_eq!(
            trigger_of("2552", &stock),
            None,
            "Item 11 not picked: no rule yet"
        );
        stock.set_transaction(super::super::form_2552::Form2552Transaction::LocalStockExchange);
        let lse = trigger_of("2552", &stock).unwrap();
        assert_eq!(lse.date, ymd(2026, 10, 8));
        assert_eq!(lse.rule.due, DueFromTrigger::BankingDaysAfter(5));
        stock.set_transaction(super::super::form_2552::Form2552Transaction::PrimaryOffering);
        assert_eq!(
            trigger_of("2552", &stock).unwrap().rule,
            FORM_2552_PRIMARY_OFFERING_RULE
        );

        let mut gift = super::super::form_1800::Form1800Draft::new_from_profile(&p, 2026, 1);
        (gift.donation_month, gift.donation_day, gift.donation_year) = (9, 15, 2026);
        assert_eq!(trigger_of("1800", &gift).unwrap().date, ymd(2026, 9, 15));

        let mut estate = super::super::form_1801::Form1801Draft::new_from_profile(&p, 2026, 1);
        (estate.death_month, estate.death_day, estate.death_year) = (2, 28, 2026);
        assert_eq!(trigger_of("1801", &estate).unwrap().date, ymd(2026, 2, 28));
    }

    /// 2000OT's Item 1 is dated exactly when the form's own validator takes
    /// it (no Item 1 error), so the two cannot drift. Both read the real
    /// clock: the future dates are relative to today.
    #[test]
    fn form_2000ot_dates_only_what_its_validator_accepts() {
        use super::super::FormValidator as _;
        let today = chrono::Local::now().date_naive();
        let as_text = |date: NaiveDate| date.format("%m/%d/%Y").to_string();
        let mut accepted = 0;
        for text in [
            "09/03/2025".to_string(),
            "  09/03/2025 ".to_string(),
            as_text(today),
            as_text(today + chrono::Duration::days(1)),
            "01/01/2018".to_string(),
            "12/31/2017".to_string(),
            "01/01/0001".to_string(),
            "1/2/2026".to_string(),
            "6/5/2025".to_string(),
            "2025-09-03".to_string(),
            "09-03-2025".to_string(),
            "09/03/25".to_string(),
            "02/30/2025".to_string(),
            "09/03/2025 x".to_string(),
            String::new(),
        ] {
            let mut draft =
                super::super::form_2000ot::Form2000OTDraft::new_from_profile(&profile(), 2025);
            draft.transaction_date = text.clone();
            let validator_takes_it = !draft
                .validate()
                .iter()
                .any(|(field, _)| field == "transaction_date");
            let dated = trigger_of("2000OT", &draft);
            assert_eq!(dated.is_some(), validator_takes_it, "{text:?}");
            if let Some(trigger) = dated {
                assert_eq!(as_text(trigger.date), text.trim(), "{text:?}");
                accepted += 1;
            }
        }
        assert_eq!(accepted, 4, "the four valid dates");
    }

    #[test]
    fn periodic_forms_and_unknown_json_have_no_trigger() {
        assert_eq!(
            event_trigger_from_json("1601C", r#"{"month":1,"day":1,"year":2026}"#),
            None
        );
        assert_eq!(
            event_trigger_from_json("1606", r#"{"transaction_month":1}"#),
            None,
            "1606 is still filed monthly"
        );
        assert_eq!(event_trigger_from_json("2200A", "not json"), None);
        assert_eq!(
            event_trigger_from_json(
                "2200A",
                r#"{"month":0,"day":0,"year":0,"trigger_date_entered":true}"#
            ),
            None,
            "an unset date is no trigger"
        );
        assert_eq!(
            event_trigger_from_json(
                "2200A",
                r#"{"month":2,"day":30,"year":2026,"trigger_date_entered":true}"#
            ),
            None
        );
        assert_eq!(
            event_trigger_from_json("2200A", r#"{"month":2,"day":3,"year":2026}"#),
            None,
            "a pre-filled date nobody entered"
        );
        assert_eq!(
            event_trigger_from_json("2553", r#"{"month":2,"day":3,"year":2026}"#),
            None,
            "an as-needed form with no trigger-date rule"
        );
    }
}
