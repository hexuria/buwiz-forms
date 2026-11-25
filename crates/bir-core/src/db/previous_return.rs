//! The previous return of a form for a branch (owner's decision 9): the
//! return a new one starts from. The nearest EARLIER period of the same form
//! for the same branch, across years; never "the most recently saved" (a
//! draft's `updated_at` moves when it is merely opened).

use rusqlite::params;
use serde_json::Value;

use super::drafts::filing_status_from_db;
use super::{Database, DbError};
use crate::forms::registry::canonical_form_code;
use crate::forms::{FilingPeriod, FilingStatus};

/// One stored return, as the carry reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviousReturn {
    pub id: i64,
    pub taxable_year: u16,
    /// The period within its year: the month, the quarter, the open-ended
    /// entry number, or 0 for an annual return.
    pub period: u32,
    pub status: FilingStatus,
    pub draft: Value,
}

/// A row's place in time and its rank (year, period, filed rank, id), with
/// its status and stored draft.
type Candidate = ((u16, u32, u8, i64), FilingStatus);

/// A period's place within its year: the month, the quarter, the entry
/// number, 0 for the year.
fn ordinal(period: &FilingPeriod) -> u32 {
    match period {
        FilingPeriod::Monthly(month) => u32::from(*month),
        FilingPeriod::Quarterly(quarter) => u32::from(*quarter),
        FilingPeriod::Annual => 0,
        FilingPeriod::OpenEnded(entry) => *entry,
    }
}

/// How far a return got: a filed one wins over a queued one, a queued one
/// over a draft, within one period. A dry run (Submitted, only simulated:
/// [`crate::forms::dry_run`]) was never filed: it ranks with the drafts.
fn filed_rank(status: &FilingStatus, dry_run: bool) -> u8 {
    if crate::forms::dry_run::is_dry_run(status, dry_run) {
        return 0;
    }
    match status {
        FilingStatus::Submitted | FilingStatus::Confirmed | FilingStatus::Paid => 2,
        FilingStatus::Queued => 1,
        FilingStatus::Draft => 0,
    }
}

impl Database {
    /// The return of `form_code` for branch `tin` whose period comes right
    /// before (`year`, `period`) among the stored ones: the latest earlier
    /// period of the same year, else of the nearest earlier year. `period`
    /// is the month, quarter or entry number, 0 for an annual form (whose
    /// previous return is then a previous year's). Within one period a
    /// filed return wins over a draft. `None` for a first return.
    pub fn previous_return(
        &self,
        tin: &str,
        form_code: &str,
        year: u16,
        period: u32,
    ) -> Result<Option<PreviousReturn>, DbError> {
        let code = canonical_form_code(form_code);
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id, form_code, taxable_year, quarter, period_key, status, {}
             FROM form_drafts WHERE tin = ?1 AND taxable_year <= ?2",
            crate::forms::dry_run::STORED_DRY_RUN_SQL
        ))?;
        let rows = stmt.query_map(params![tin, i64::from(year)], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, bool>(6)?,
            ))
        })?;
        let mut best: Option<Candidate> = None;
        for row in rows {
            let (id, row_code, row_year, quarter, period_key, status, dry_run) = row?;
            if canonical_form_code(&row_code) != code {
                continue;
            }
            let Ok(row_year) = u16::try_from(row_year) else {
                continue;
            };
            let row_period = match period_key
                .as_deref()
                .and_then(FilingPeriod::from_period_key)
            {
                Some(found) => ordinal(&found),
                None => quarter
                    .and_then(|slot| u32::try_from(slot).ok())
                    .unwrap_or(0),
            };
            if (row_year, row_period) >= (year, period) {
                continue;
            }
            let status = filing_status_from_db(&status);
            let order = (row_year, row_period, filed_rank(&status, dry_run), id);
            if best.as_ref().is_none_or(|(current, _)| order > *current) {
                best = Some((order, status));
            }
        }
        let Some(((taxable_year, period, _, id), status)) = best else {
            return Ok(None);
        };
        // Only the chosen return's answers are read, not every return's.
        let data_json: String = self.conn.query_row(
            "SELECT data_json FROM form_drafts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(Some(PreviousReturn {
            id,
            taxable_year,
            period,
            status,
            draft: serde_json::from_str(&data_json)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIN: &str = "00012345300000";

    fn insert(
        db: &Database,
        code: &str,
        year: i64,
        quarter: Option<i64>,
        key: &str,
        status: &str,
    ) -> i64 {
        db.conn
            .execute(
                "INSERT INTO form_drafts
                    (tin, form_code, taxable_year, quarter, period_key, status, data_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![
                    TIN,
                    code,
                    year,
                    quarter,
                    key,
                    status,
                    serde_json::json!({ "marker": format!("{code} {year} {key} {status}") })
                        .to_string()
                ],
            )
            .unwrap();
        db.conn.last_insert_rowid()
    }

    fn previous(db: &Database, code: &str, year: u16, period: u32) -> Option<String> {
        db.previous_return(TIN, code, year, period)
            .unwrap()
            .map(|found| found.draft["marker"].as_str().unwrap().to_string())
    }

    #[test]
    fn the_previous_return_is_the_nearest_earlier_period_across_years() {
        let db = Database::open_in_memory_for_tests().unwrap();
        assert_eq!(previous(&db, "1601C", 2026, 3), None, "a first return");
        insert(&db, "1601C", 2025, Some(11), "M11", "Confirmed");
        insert(&db, "1601C", 2026, Some(1), "M01", "Draft");
        // A later period saved (and opened) most recently is not "previous".
        let later = insert(&db, "1601C", 2026, Some(5), "M05", "Draft");
        db.conn
            .execute(
                "UPDATE form_drafts SET updated_at = datetime('now', '+1 day') WHERE id = ?1",
                [later],
            )
            .unwrap();
        // Another form and another branch never count.
        insert(&db, "0619E", 2026, Some(2), "M02", "Confirmed");
        db.conn
            .execute(
                "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, period_key, status, data_json)
                 VALUES ('00012345300001', '1601C', 2026, 2, 'M02', 'Confirmed', '{\"marker\":\"branch\"}')",
                [],
            )
            .unwrap();
        assert_eq!(
            previous(&db, "1601C", 2026, 3).as_deref(),
            Some("1601C 2026 M01 Draft")
        );
        assert_eq!(
            previous(&db, "1601C", 2026, 1).as_deref(),
            Some("1601C 2025 M11 Confirmed")
        );
        assert_eq!(previous(&db, "1601C", 2025, 11), None);
        assert_eq!(
            previous(&db, "1601c", 2026, 12).as_deref(),
            Some("1601C 2026 M05 Draft")
        );
    }

    #[test]
    fn a_filed_return_wins_over_a_draft_of_the_same_period() {
        let db = Database::open_in_memory_for_tests().unwrap();
        // An annual form: its previous return is the previous year's, and of
        // two rows of that year (a legacy NULL-period one and a new one) the
        // filed one wins.
        insert(&db, "1701", 2024, None, "A", "Confirmed");
        insert(&db, "1701", 2025, None, "A", "Submitted");
        insert(&db, "1701", 2025, Some(0), "A", "Draft");
        assert_eq!(
            previous(&db, "1701", 2026, 0).as_deref(),
            Some("1701 2025 A Submitted")
        );
        assert_eq!(
            previous(&db, "1701", 2025, 0).as_deref(),
            Some("1701 2024 A Confirmed")
        );
        let found = db.previous_return(TIN, "1701", 2026, 0).unwrap().unwrap();
        assert_eq!((found.taxable_year, found.period), (2025, 0));
        assert_eq!(found.status, FilingStatus::Submitted);
    }

    /// Mark the row `id` as a dry run, at the top level or under `lifecycle`.
    fn mark_dry_run(db: &Database, id: i64, nested: bool) {
        let path = if nested {
            "$.lifecycle"
        } else {
            "$.submitted_dry_run"
        };
        let value = if nested {
            r#"{"submitted_dry_run":true}"#
        } else {
            "true"
        };
        db.conn
            .execute(
                &format!("UPDATE form_drafts SET data_json = json_set(data_json, '{path}', json('{value}')) WHERE id = ?1"),
                [id],
            )
            .unwrap();
    }

    /// Within one period, a real filing outranks a dry run; a dry run ranks
    /// with the drafts (the later row of the two wins, as between drafts) and
    /// below a Queued return.
    #[test]
    fn a_dry_run_ranks_with_drafts_and_a_real_filing_outranks_it() {
        for nested in [false, true] {
            // A dry run and a real send of the same period: the real one.
            let db = Database::open_in_memory_for_tests().unwrap();
            insert(&db, "1701", 2025, Some(0), "A", "Submitted");
            let dry = insert(&db, "1701", 2025, None, "A", "Submitted");
            mark_dry_run(&db, dry, nested);
            assert_eq!(
                previous(&db, "1701", 2026, 0).as_deref(),
                Some("1701 2025 A Submitted"),
                "{nested}"
            );
            assert_ne!(
                db.previous_return(TIN, "1701", 2026, 0)
                    .unwrap()
                    .unwrap()
                    .id,
                dry
            );
            for (other, status) in [
                ("Confirmed", "Confirmed"),
                ("Paid", "Paid"),
                ("Queued", "Queued"),
            ] {
                let db = Database::open_in_memory_for_tests().unwrap();
                let filed = insert(&db, "1701", 2025, Some(0), "A", status);
                let dry = insert(&db, "1701", 2025, None, "A", "Submitted");
                mark_dry_run(&db, dry, nested);
                assert_eq!(
                    db.previous_return(TIN, "1701", 2026, 0)
                        .unwrap()
                        .unwrap()
                        .id,
                    filed,
                    "{other} wins over a dry run"
                );
            }
            // A dry run and a Draft: they rank together, the later row wins.
            let db = Database::open_in_memory_for_tests().unwrap();
            let dry = insert(&db, "1701", 2025, None, "A", "Submitted");
            mark_dry_run(&db, dry, nested);
            let draft = insert(&db, "1701", 2025, Some(0), "A", "Draft");
            assert_eq!(
                db.previous_return(TIN, "1701", 2026, 0)
                    .unwrap()
                    .unwrap()
                    .id,
                draft
            );
            // Alone, a dry run is still the previous return (its answers carry).
            let db = Database::open_in_memory_for_tests().unwrap();
            let dry = insert(&db, "1701", 2025, None, "A", "Submitted");
            mark_dry_run(&db, dry, nested);
            let found = db.previous_return(TIN, "1701", 2026, 0).unwrap().unwrap();
            assert_eq!(found.id, dry);
            assert_eq!(found.status, FilingStatus::Submitted);
        }
        // A real send with a relay receipt is a real filing.
        let db = Database::open_in_memory_for_tests().unwrap();
        let relay = insert(&db, "1701", 2025, None, "A", "Submitted");
        db.conn
            .execute(
                "UPDATE form_drafts SET data_json = json_set(data_json, '$.relay_receipt', 'a.b.c', '$.submitted_dry_run', json('false')) WHERE id = ?1",
                [relay],
            )
            .unwrap();
        let draft = insert(&db, "1701", 2025, Some(0), "A", "Draft");
        assert_ne!(draft, relay);
        assert_eq!(
            db.previous_return(TIN, "1701", 2026, 0)
                .unwrap()
                .unwrap()
                .id,
            relay
        );
    }
}
