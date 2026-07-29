//! Actionable application health events.
//!
//! These are conditions *this app* hit that a user can do something about — an
//! expired Google token, a profile row this build cannot deserialize. They are
//! deliberately separate from `bir_notices`, which carries BIR's own external
//! announcements and is not about the health of this installation.
//!
//! Before this existed such conditions only reached the log file, so the email
//! cron could fail every 60 seconds for a week with nothing visible in the UI.

use super::{Database, DbError};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// How much attention an alert deserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Something is broken and a feature is not working.
    Error,
    /// Working, but degraded or heading for trouble.
    Warning,
    /// Worth knowing, nothing is wrong.
    Info,
}

impl AlertSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertSeverity::Error => "Error",
            AlertSeverity::Warning => "Warning",
            AlertSeverity::Info => "Info",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "Error" => AlertSeverity::Error,
            "Warning" => AlertSeverity::Warning,
            _ => AlertSeverity::Info,
        }
    }
}

/// What the user can do about an alert, as a stable key the UI maps to a button.
///
/// Kept as an enum rather than a free string so a typo cannot produce an alert
/// with a button that goes nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertAction {
    /// Open Profile → Email Settings to re-run the Google OAuth consent flow.
    ReconnectGoogleAccount,
    /// Open the profile manager; a stored profile needs attention.
    OpenProfileManager,
    /// Nothing to click; the text is the whole message.
    None,
}

impl AlertAction {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertAction::ReconnectGoogleAccount => "reconnect_google_account",
            AlertAction::OpenProfileManager => "open_profile_manager",
            AlertAction::None => "none",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "reconnect_google_account" => AlertAction::ReconnectGoogleAccount,
            "open_profile_manager" => AlertAction::OpenProfileManager,
            _ => AlertAction::None,
        }
    }

    /// The button label, or `None` when the alert is purely informational.
    pub fn label(self) -> Option<&'static str> {
        match self {
            AlertAction::ReconnectGoogleAccount => Some("Reconnect Google Account"),
            AlertAction::OpenProfileManager => Some("Open Profile Manager"),
            AlertAction::None => None,
        }
    }
}

/// A recorded application health event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppAlert {
    pub id: i64,
    /// `None` means the alert is application-wide rather than about one profile.
    pub tin: Option<String>,
    /// Stable machine key identifying the condition; also the dedup key.
    pub kind: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub detail: String,
    pub action: AlertAction,
    /// How many times this condition has been reported since it was first seen.
    pub occurrences: i64,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

/// Stable `kind` keys. Centralised so recording and resolving cannot drift apart —
/// a mismatch would leave an alert on screen forever after the cause was fixed.
pub mod kinds {
    /// Google rejected the OAuth refresh token; email polling is not working.
    pub const GOOGLE_OAUTH_REFRESH_FAILED: &str = "google_oauth_refresh_failed";
    /// A profile row could not be deserialized and was skipped from the list.
    pub const PROFILE_ROW_UNREADABLE: &str = "profile_row_unreadable";
}

impl Database {
    /// Record an alert, or bump the existing one for the same `(tin, kind)`.
    ///
    /// Callers fire repeatedly by nature — the email cron retries every 60
    /// seconds — so this upserts rather than inserts. `occurrences` counts and
    /// `last_seen_at` moves; `first_seen_at` is preserved so the UI can say how
    /// long a condition has persisted.
    ///
    /// Re-recording also clears `resolved_at`: a condition that recurs after
    /// being dismissed is active again and must reappear.
    pub fn record_alert(
        &self,
        tin: Option<&str>,
        kind: &str,
        severity: AlertSeverity,
        title: &str,
        detail: &str,
        action: AlertAction,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO app_alerts (tin, kind, severity, title, detail, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (COALESCE(tin, ''), kind) DO UPDATE SET
                 severity    = excluded.severity,
                 title       = excluded.title,
                 detail      = excluded.detail,
                 action      = excluded.action,
                 occurrences = app_alerts.occurrences + 1,
                 last_seen_at = datetime('now'),
                 resolved_at = NULL",
            params![tin, kind, severity.as_str(), title, detail, action.as_str()],
        )?;
        Ok(())
    }

    /// Active alerts for one profile, plus application-wide ones.
    ///
    /// Application-wide alerts (`tin IS NULL`) are always included: a broken
    /// Google connection or an unreadable database affects the user whichever
    /// profile happens to be selected, and hiding it behind profile switching
    /// would be how it goes unnoticed for a week.
    pub fn list_active_alerts(&self, tin: Option<&str>) -> Result<Vec<AppAlert>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tin, kind, severity, title, detail, action,
                    occurrences, first_seen_at, last_seen_at
             FROM app_alerts
             WHERE resolved_at IS NULL
               AND (tin IS NULL OR tin = ?1)
             ORDER BY CASE severity
                        WHEN 'Error' THEN 0
                        WHEN 'Warning' THEN 1
                        ELSE 2
                      END,
                      last_seen_at DESC",
        )?;

        let rows = stmt.query_map(params![tin], |row| {
            Ok(AppAlert {
                id: row.get(0)?,
                tin: row.get(1)?,
                kind: row.get(2)?,
                severity: AlertSeverity::from_str(&row.get::<_, String>(3)?),
                title: row.get(4)?,
                detail: row.get(5)?,
                action: AlertAction::from_str(&row.get::<_, String>(6)?),
                occurrences: row.get(7)?,
                first_seen_at: row.get(8)?,
                last_seen_at: row.get(9)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    /// Mark a condition resolved so it leaves the list.
    ///
    /// Takes the `kind` rather than a row id so the code that *fixes* a problem
    /// can clear it without having to know which row it produced — the OAuth
    /// success path calls this without ever having seen the alert.
    pub fn resolve_alert(&self, tin: Option<&str>, kind: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE app_alerts SET resolved_at = datetime('now')
             WHERE kind = ?2 AND COALESCE(tin, '') = COALESCE(?1, '')
               AND resolved_at IS NULL",
            params![tin, kind],
        )?;
        Ok(())
    }

    /// Dismiss one alert by id — the user acknowledging it rather than fixing it.
    ///
    /// If the underlying condition recurs, `record_alert` clears `resolved_at`
    /// and it comes back. Dismissing something still broken hides it until the
    /// next occurrence, not forever.
    pub fn dismiss_alert(&self, id: i64) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE app_alerts SET resolved_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIN_A: &str = "111456789000";
    const TIN_B: &str = "222456789000";

    fn db() -> Database {
        Database::open_in_memory_for_tests().expect("in-memory db")
    }

    fn record_oauth_failure(db: &Database, tin: Option<&str>) {
        db.record_alert(
            tin,
            kinds::GOOGLE_OAUTH_REFRESH_FAILED,
            AlertSeverity::Error,
            "Gmail connection needs attention",
            "invalid_grant: Token has been expired or revoked.",
            AlertAction::ReconnectGoogleAccount,
        )
        .expect("alert records");
    }

    /// The whole reason this table has a unique index. The email cron retries
    /// every 60 seconds, so a broken token reports ~1,440 times a day. Without
    /// upsert the page would be unusable within an hour.
    #[test]
    fn repeated_reports_collapse_into_one_row_with_a_count() {
        let db = db();
        for _ in 0..500 {
            record_oauth_failure(&db, Some(TIN_A));
        }

        let alerts = db.list_active_alerts(Some(TIN_A)).unwrap();
        assert_eq!(alerts.len(), 1, "500 reports must not make 500 rows");
        assert_eq!(alerts[0].occurrences, 500);
    }

    #[test]
    fn first_seen_is_preserved_while_last_seen_advances() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));
        let first = db.list_active_alerts(Some(TIN_A)).unwrap()[0].clone();
        record_oauth_failure(&db, Some(TIN_A));
        let again = db.list_active_alerts(Some(TIN_A)).unwrap()[0].clone();

        assert_eq!(
            first.first_seen_at, again.first_seen_at,
            "first_seen must not move, or the UI cannot say how long this has been broken"
        );
        assert_eq!(again.occurrences, 2);
    }

    #[test]
    fn alerts_are_scoped_to_their_profile() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));

        assert_eq!(db.list_active_alerts(Some(TIN_A)).unwrap().len(), 1);
        assert!(
            db.list_active_alerts(Some(TIN_B)).unwrap().is_empty(),
            "a different profile must not see another profile's alerts"
        );
    }

    /// Application-wide alerts must appear whichever profile is selected -
    /// otherwise a broken database or connection hides behind profile switching.
    #[test]
    fn application_wide_alerts_show_under_every_profile() {
        let db = db();
        record_oauth_failure(&db, None);

        for tin in [Some(TIN_A), Some(TIN_B), None] {
            assert_eq!(
                db.list_active_alerts(tin).unwrap().len(),
                1,
                "app-wide alert must be visible for {tin:?}"
            );
        }
    }

    /// SQLite treats NULLs as distinct in a plain UNIQUE index, so without the
    /// COALESCE in the index every app-wide report would insert a new row.
    #[test]
    fn app_wide_alerts_deduplicate_despite_null_tin() {
        let db = db();
        for _ in 0..10 {
            record_oauth_failure(&db, None);
        }

        let alerts = db.list_active_alerts(None).unwrap();
        assert_eq!(alerts.len(), 1, "NULL tin must still deduplicate");
        assert_eq!(alerts[0].occurrences, 10);
    }

    #[test]
    fn distinct_kinds_stay_separate() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));
        db.record_alert(
            Some(TIN_A),
            kinds::PROFILE_ROW_UNREADABLE,
            AlertSeverity::Warning,
            "A profile could not be read",
            "Written by a newer version of the app.",
            AlertAction::OpenProfileManager,
        )
        .unwrap();

        assert_eq!(db.list_active_alerts(Some(TIN_A)).unwrap().len(), 2);
    }

    #[test]
    fn resolving_removes_it_from_the_active_list() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));
        db.resolve_alert(Some(TIN_A), kinds::GOOGLE_OAUTH_REFRESH_FAILED)
            .unwrap();

        assert!(db.list_active_alerts(Some(TIN_A)).unwrap().is_empty());
    }

    /// A condition that comes back after being resolved is active again.
    /// Otherwise fixing then re-breaking Gmail would leave the user with no
    /// indication the second time.
    #[test]
    fn a_recurrence_after_resolution_reappears() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));
        db.resolve_alert(Some(TIN_A), kinds::GOOGLE_OAUTH_REFRESH_FAILED)
            .unwrap();
        assert!(db.list_active_alerts(Some(TIN_A)).unwrap().is_empty());

        record_oauth_failure(&db, Some(TIN_A));
        let alerts = db.list_active_alerts(Some(TIN_A)).unwrap();
        assert_eq!(alerts.len(), 1, "a recurring condition must resurface");
        assert_eq!(alerts[0].occurrences, 2, "and keep its history");
    }

    #[test]
    fn dismissing_by_id_hides_only_that_alert() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));
        db.record_alert(
            Some(TIN_A),
            kinds::PROFILE_ROW_UNREADABLE,
            AlertSeverity::Warning,
            "A profile could not be read",
            "detail",
            AlertAction::OpenProfileManager,
        )
        .unwrap();

        let first = db.list_active_alerts(Some(TIN_A)).unwrap()[0].id;
        db.dismiss_alert(first).unwrap();

        let left = db.list_active_alerts(Some(TIN_A)).unwrap();
        assert_eq!(left.len(), 1);
        assert_ne!(left[0].id, first);
    }

    /// Errors must sort above warnings so the actionable thing is at the top.
    #[test]
    fn errors_sort_above_warnings() {
        let db = db();
        db.record_alert(
            Some(TIN_A),
            kinds::PROFILE_ROW_UNREADABLE,
            AlertSeverity::Warning,
            "warning",
            "d",
            AlertAction::None,
        )
        .unwrap();
        record_oauth_failure(&db, Some(TIN_A));

        let alerts = db.list_active_alerts(Some(TIN_A)).unwrap();
        assert_eq!(alerts[0].severity, AlertSeverity::Error);
        assert_eq!(alerts[1].severity, AlertSeverity::Warning);
    }

    #[test]
    fn severity_and_action_round_trip_through_the_database() {
        let db = db();
        record_oauth_failure(&db, Some(TIN_A));

        let a = &db.list_active_alerts(Some(TIN_A)).unwrap()[0];
        assert_eq!(a.severity, AlertSeverity::Error);
        assert_eq!(a.action, AlertAction::ReconnectGoogleAccount);
        assert_eq!(a.action.label(), Some("Reconnect Google Account"));
    }
}
