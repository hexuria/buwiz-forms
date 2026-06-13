//! Persistence for per-profile Google Calendar links and managed events.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileCalendarLink {
    pub profile_tin: String,
    pub google_calendar_id: String,
    pub calendar_name: String,
    pub enabled: bool,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalendarEventLink {
    pub profile_tin: String,
    pub obligation_key: String,
    pub google_event_id: String,
    pub content_hash: String,
    pub taxable_year: u16,
    pub form_code: String,
    pub period_label: String,
}

impl Database {
    pub fn request_google_calendar_sync(&self) -> Result<(), DbError> {
        self.set_setting("google_calendar_sync_requested", "true")?;
        crate::background_cron::wake();
        Ok(())
    }

    pub fn get_profile_calendar_link(
        &self,
        tin: &str,
    ) -> Result<Option<ProfileCalendarLink>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT profile_tin, google_calendar_id, calendar_name, enabled,
                    last_synced_at, last_error
             FROM profile_calendar_links WHERE profile_tin = ?1",
        )?;
        let mut rows = stmt.query(params![tin])?;
        match rows.next()? {
            Some(row) => Ok(Some(ProfileCalendarLink {
                profile_tin: row.get(0)?,
                google_calendar_id: row.get(1)?,
                calendar_name: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                last_synced_at: row.get(4)?,
                last_error: row.get(5)?,
            })),
            None => Ok(None),
        }
    }

    pub fn list_profile_calendar_links(&self) -> Result<Vec<ProfileCalendarLink>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT profile_tin, google_calendar_id, calendar_name, enabled,
                    last_synced_at, last_error
             FROM profile_calendar_links WHERE enabled = 1 ORDER BY profile_tin",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProfileCalendarLink {
                profile_tin: row.get(0)?,
                google_calendar_id: row.get(1)?,
                calendar_name: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                last_synced_at: row.get(4)?,
                last_error: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn save_profile_calendar_link(&self, link: &ProfileCalendarLink) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO profile_calendar_links
                (profile_tin, google_calendar_id, calendar_name, enabled,
                 last_synced_at, last_error, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(profile_tin) DO UPDATE SET
                google_calendar_id = excluded.google_calendar_id,
                calendar_name = excluded.calendar_name,
                enabled = excluded.enabled,
                last_synced_at = excluded.last_synced_at,
                last_error = excluded.last_error,
                updated_at = datetime('now')",
            params![
                link.profile_tin,
                link.google_calendar_id,
                link.calendar_name,
                link.enabled as i64,
                link.last_synced_at,
                link.last_error,
            ],
        )?;
        Ok(())
    }

    pub fn set_profile_calendar_sync_result(
        &self,
        tin: &str,
        error: Option<&str>,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE profile_calendar_links
             SET last_synced_at = CASE WHEN ?2 IS NULL THEN datetime('now') ELSE last_synced_at END,
                 last_error = ?2,
                 updated_at = datetime('now')
             WHERE profile_tin = ?1",
            params![tin, error],
        )?;
        Ok(())
    }

    pub fn delete_profile_calendar_link(&self, tin: &str) -> Result<(), DbError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM profile_calendar_events WHERE profile_tin = ?1",
            params![tin],
        )?;
        tx.execute(
            "DELETE FROM profile_calendar_links WHERE profile_tin = ?1",
            params![tin],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_calendar_event_links(&self, tin: &str) -> Result<Vec<CalendarEventLink>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT profile_tin, obligation_key, google_event_id, content_hash,
                    taxable_year, form_code, period_label
             FROM profile_calendar_events WHERE profile_tin = ?1",
        )?;
        let rows = stmt.query_map(params![tin], |row| {
            Ok(CalendarEventLink {
                profile_tin: row.get(0)?,
                obligation_key: row.get(1)?,
                google_event_id: row.get(2)?,
                content_hash: row.get(3)?,
                taxable_year: row.get::<_, i64>(4)? as u16,
                form_code: row.get(5)?,
                period_label: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn save_calendar_event_link(&self, link: &CalendarEventLink) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO profile_calendar_events
                (profile_tin, obligation_key, google_event_id, content_hash,
                 taxable_year, form_code, period_label, last_synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(profile_tin, obligation_key) DO UPDATE SET
                google_event_id = excluded.google_event_id,
                content_hash = excluded.content_hash,
                taxable_year = excluded.taxable_year,
                form_code = excluded.form_code,
                period_label = excluded.period_label,
                last_synced_at = datetime('now')",
            params![
                link.profile_tin,
                link.obligation_key,
                link.google_event_id,
                link.content_hash,
                link.taxable_year,
                link.form_code,
                link.period_label,
            ],
        )?;
        Ok(())
    }

    pub fn delete_calendar_event_link(
        &self,
        tin: &str,
        obligation_key: &str,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM profile_calendar_events
             WHERE profile_tin = ?1 AND obligation_key = ?2",
            params![tin, obligation_key],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn calendar_links_and_events_round_trip() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        db.conn
            .execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, '{}')",
                ["123456789000"],
            )
            .unwrap();
        let link = ProfileCalendarLink {
            profile_tin: "123456789000".into(),
            google_calendar_id: "calendar@example.com".into(),
            calendar_name: "eBIRForms - Test (...9000)".into(),
            enabled: true,
            last_synced_at: None,
            last_error: None,
        };
        db.save_profile_calendar_link(&link).unwrap();
        assert_eq!(
            db.get_profile_calendar_link("123456789000")
                .unwrap()
                .unwrap()
                .calendar_name,
            link.calendar_name
        );

        let event = CalendarEventLink {
            profile_tin: "123456789000".into(),
            obligation_key: "2026:0619E:m01".into(),
            google_event_id: "event-1".into(),
            content_hash: "abc".into(),
            taxable_year: 2026,
            form_code: "0619E".into(),
            period_label: "2026-01".into(),
        };
        db.save_calendar_event_link(&event).unwrap();
        assert_eq!(
            db.list_calendar_event_links("123456789000").unwrap(),
            vec![event]
        );

        db.delete_profile_calendar_link("123456789000").unwrap();
        assert!(
            db.get_profile_calendar_link("123456789000")
                .unwrap()
                .is_none()
        );
        assert!(
            db.list_calendar_event_links("123456789000")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn profile_deletion_is_blocked_while_calendar_is_linked() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        db.conn
            .execute(
                "INSERT INTO profiles (tin, data_json) VALUES (?1, '{}')",
                ["123456789000"],
            )
            .unwrap();
        db.save_profile_calendar_link(&ProfileCalendarLink {
            profile_tin: "123456789000".into(),
            google_calendar_id: "calendar@example.com".into(),
            calendar_name: "Test Calendar".into(),
            enabled: true,
            last_synced_at: None,
            last_error: None,
        })
        .unwrap();

        let error = db.delete_profile("123456789000").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Delete or unlink the profile's Google Calendar")
        );
    }

    #[test]
    fn deadline_override_changes_request_calendar_sync() {
        let file = NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).unwrap();
        db.set_setting("google_calendar_sync_requested", "false")
            .unwrap();

        db.set_deadline_overrides(&[]).unwrap();

        assert_eq!(
            db.get_setting("google_calendar_sync_requested").unwrap(),
            Some("true".to_string())
        );
    }
}
