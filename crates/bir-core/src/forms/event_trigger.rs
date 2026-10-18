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

/// `MM/DD/YYYY` as typed.
fn date_from_text(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%m/%d/%Y").ok()
}

/// The trigger date of a saved return of `form_code` from its stored
/// `form_drafts.data_json`. `None` for a form that is not as needed or has
/// no trigger-date rule, a return that does not hold the date yet, or JSON
/// that does not parse.
pub fn event_trigger_from_json(form_code: &str, data_json: &str) -> Option<EventTrigger> {
    if !DeadlineResolver::is_as_needed(form_code) {
        return None;
    }
    let mut rule = event_due_rule(form_code)?;
    let draft: Value = serde_json::from_str(data_json).ok()?;
    let date = match form_code {
        // Item 1: the removal date (the excise returns), the withholding
        // date (1600WP).
        "2200A" | "2200AN" | "2200M" | "2200P" | "2200S" | "2200T" | "1600WP" => {
            date_from(&draft, "year", "month", "day")
        }
        "0605" => date_from(draft.get("due_date")?, "year", "month", "day"),
        "1706" | "1707" => date_from(
            &draft,
            "transaction_year",
            "transaction_month",
            "transaction_day",
        ),
        "2552" => {
            // Item 11 picks the rule: a primary offering is due from the
            // listing date, the LSE and a secondary offering from collection.
            match draft.get("transaction").and_then(Value::as_str) {
                Some("primary_offering") => rule = FORM_2552_PRIMARY_OFFERING_RULE,
                Some("local_stock_exchange" | "secondary_offering") => {}
                _ => return None,
            }
            date_from(
                &draft,
                "transaction_year",
                "transaction_month",
                "transaction_day",
            )
        }
        "2000OT" => date_from_text(draft.get("transaction_date")?.as_str()?),
        "1800" => date_from(&draft, "donation_year", "donation_month", "donation_day"),
        "1801" => date_from(&draft, "death_year", "death_month", "death_day"),
        _ => None,
    }?;
    Some(EventTrigger { date, rule })
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
                let mut draft = <$ty>::new_from_profile(&p, 2026, 10);
                draft.month = 10;
                draft.day = 7;
                draft.year = 2026;
                let trigger = trigger_of($code, &draft).expect($code);
                assert_eq!(trigger.date, ymd(2026, 10, 7), $code);
                assert_eq!(trigger.rule.form_code, $code);
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
        dst.transaction_date = "12/03/2026".to_string();
        assert_eq!(trigger_of("2000OT", &dst).unwrap().date, ymd(2026, 12, 3));

        let mut stock = super::super::form_2552::Form2552Draft::new_from_profile(&p, 2026, 10, 8);
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
            event_trigger_from_json("2200A", r#"{"month":0,"day":0,"year":0}"#),
            None,
            "an unset date is no trigger"
        );
        assert_eq!(
            event_trigger_from_json("2200A", r#"{"month":2,"day":30,"year":2026}"#),
            None
        );
        assert_eq!(
            event_trigger_from_json("2553", r#"{"month":2,"day":3,"year":2026}"#),
            None,
            "an as-needed form with no trigger-date rule"
        );
    }
}
