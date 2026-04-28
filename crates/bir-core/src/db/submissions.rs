//! Submission repository — save and list form submissions.

use rusqlite::params;
use std::collections::BTreeMap;

use super::{Database, DbError, Submission};

impl Database {
    /// Save a submission.
    pub fn save_submission(&self, mut sub: Submission) -> Result<Submission, DbError> {
        let json_data = serde_json::to_string(&sub.form_data)?;

        if let Some(id) = sub.id {
            self.conn.execute(
                "UPDATE submissions SET form_type = ?1, period = ?2, status = ?3, form_data = ?4, submitted_at = ?5, filename = ?6, updated_at = datetime('now') WHERE id = ?7",
                params![sub.form_type, sub.period, sub.status, json_data, sub.submitted_at, sub.filename, id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO submissions (tin, form_type, period, status, form_data, submitted_at, filename) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![sub.tin, sub.form_type, sub.period, sub.status, json_data, sub.submitted_at, sub.filename],
            )?;
            sub.id = Some(self.conn.last_insert_rowid());
        }

        Ok(sub)
    }

    /// List submissions for a specific TIN.
    pub fn list_submissions_for_tin(&self, tin: &str) -> Result<Vec<Submission>, DbError> {
        let mut stmt = self.conn.prepare("SELECT id, tin, form_type, period, status, form_data, submitted_at, filename, created_at, updated_at FROM submissions WHERE tin = ?1 ORDER BY created_at DESC")?;
        let rows = stmt.query_map(params![tin], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;

        let mut submissions = Vec::new();
        for row_result in rows {
            let (
                id,
                tin,
                form_type,
                period,
                status,
                json_data,
                submitted_at,
                filename,
                created_at,
                updated_at,
            ) = row_result?;
            let form_data: BTreeMap<String, String> = serde_json::from_str(&json_data)?;

            submissions.push(Submission {
                id: Some(id),
                tin,
                form_type,
                period,
                status,
                form_data,
                submitted_at,
                filename,
                created_at,
                updated_at,
            });
        }
        Ok(submissions)
    }
}
