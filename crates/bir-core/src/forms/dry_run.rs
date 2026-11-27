//! A dry run is not a filing. The submission worker marks a send it only
//! simulated (a dry run, a practice account) with
//! `submitted_dry_run`: the return is stored Submitted, but nothing reached
//! the BIR. Every rule that asks "was this return filed?" asks it here, so a
//! dry run never stands in for a real filing. A real send (no flag), a send
//! with a relay receipt, Confirmed and Paid count as filed, as before.
//!
//! The flag sits at the top level of the stored JSON (2551Q's and 1601C's
//! inline lifecycle, and the generic lifecycle, which is flattened), or under
//! `lifecycle` (read the same way as `submitted_via_relay`).

use serde_json::Value;

use super::FilingStatus;

/// SQL that is true for a stored row (`data_json`) marked as a dry run, top
/// level or under `lifecycle`; false for unreadable JSON and a missing or
/// false flag. Pair it with the row's status: only a Submitted one is a dry
/// run (`counts_as_filed`).
/// (`CASE` so a malformed row is never handed to `json_type`, which would
/// fail the whole query: SQLite does not promise to stop an `AND` early.)
pub const STORED_DRY_RUN_SQL: &str = "(CASE WHEN json_valid(data_json) THEN COALESCE(NULLIF(json_type(data_json, '$.submitted_dry_run'), 'null'), json_type(data_json, '$.lifecycle.submitted_dry_run'), 'false') = 'true' ELSE 0 END)";

/// SQL that is true for a stored row that counts as filed: Submitted (or the
/// legacy "Filed") and not a dry run, Confirmed or Paid.
pub const COUNTS_AS_FILED_SQL: &str = "(status IN ('Confirmed', 'Paid') OR (status IN ('Submitted', 'Filed') AND NOT (CASE WHEN json_valid(data_json) THEN COALESCE(NULLIF(json_type(data_json, '$.submitted_dry_run'), 'null'), json_type(data_json, '$.lifecycle.submitted_dry_run'), 'false') = 'true' ELSE 0 END)))";

/// Whether a stored return's JSON carries the dry-run mark (top level or
/// under `lifecycle`). Says nothing about its status.
pub fn json_marks_dry_run(draft: &Value) -> bool {
    match draft.get("submitted_dry_run").and_then(Value::as_bool) {
        Some(flag) => flag,
        None => draft
            .get("lifecycle")
            .and_then(|lifecycle| lifecycle.get("submitted_dry_run"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Whether a return in `status` with the dry-run mark `dry_run` counts as
/// filed: a Submitted return that is not a dry run, Confirmed or Paid.
pub fn counts_as_filed(status: &FilingStatus, dry_run: bool) -> bool {
    match status {
        FilingStatus::Submitted => !dry_run,
        FilingStatus::Confirmed | FilingStatus::Paid => true,
        FilingStatus::Draft | FilingStatus::Queued => false,
    }
}

/// Whether a return in `status` is a dry run: Submitted with the mark.
pub fn is_dry_run(status: &FilingStatus, dry_run: bool) -> bool {
    matches!(status, FilingStatus::Submitted) && dry_run
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_mark_is_read_at_the_top_level_or_under_lifecycle() {
        assert!(json_marks_dry_run(&json!({"submitted_dry_run": true})));
        assert!(json_marks_dry_run(
            &json!({"lifecycle": {"submitted_dry_run": true}})
        ));
        assert!(json_marks_dry_run(
            &json!({"submitted_dry_run": null, "lifecycle": {"submitted_dry_run": true}})
        ));
        assert!(!json_marks_dry_run(&json!({})));
        assert!(!json_marks_dry_run(&json!({"submitted_dry_run": false})));
        assert!(!json_marks_dry_run(&json!({"submitted_dry_run": "true"})));
    }

    #[test]
    fn only_a_submitted_dry_run_is_not_filed() {
        assert!(!counts_as_filed(&FilingStatus::Submitted, true));
        assert!(counts_as_filed(&FilingStatus::Submitted, false));
        for status in [FilingStatus::Confirmed, FilingStatus::Paid] {
            assert!(counts_as_filed(&status, true));
            assert!(!is_dry_run(&status, true));
        }
        for status in [FilingStatus::Draft, FilingStatus::Queued] {
            assert!(!counts_as_filed(&status, false));
            assert!(!is_dry_run(&status, true));
        }
        assert!(is_dry_run(&FilingStatus::Submitted, true));
    }

    #[test]
    fn the_two_sql_texts_read_the_same_mark() {
        assert!(COUNTS_AS_FILED_SQL.contains(&STORED_DRY_RUN_SQL[1..STORED_DRY_RUN_SQL.len() - 1]));
    }

    #[test]
    fn the_sql_reads_the_mark_as_the_json_reader_does() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        for (status, data, filed) in [
            ("Submitted", r#"{"submitted_dry_run":true}"#, false),
            (
                "Submitted",
                r#"{"lifecycle":{"submitted_dry_run":true}}"#,
                false,
            ),
            ("Filed", r#"{"submitted_dry_run":true}"#, false),
            ("Submitted", "{}", true),
            ("Submitted", r#"{"relay_receipt":"a.b.c"}"#, true),
            ("Submitted", "{broken", true),
            ("Confirmed", r#"{"submitted_dry_run":true}"#, true),
            ("Paid", r#"{"submitted_dry_run":true}"#, true),
            ("Draft", "{}", false),
            ("Queued", "{}", false),
        ] {
            let got: bool = conn
                .query_row(
                    &format!(
                        "SELECT {COUNTS_AS_FILED_SQL} FROM (SELECT ?1 AS status, ?2 AS data_json)"
                    ),
                    rusqlite::params![status, data],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(got, filed, "{status} {data}");
            let dry: bool = conn
                .query_row(
                    &format!("SELECT {STORED_DRY_RUN_SQL} FROM (SELECT ?1 AS data_json)"),
                    rusqlite::params![data],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(dry, data.contains("submitted_dry_run\":true"), "{data}");
        }
    }
}
