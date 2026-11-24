//! Forms-owned fallback mapping from a BIR form box to a Buwiz Books account (ADR 0009).
//!
//! Used only for form boxes that do not have an authoritative Books measure rule.
//! Stored inside Buwiz Forms' own database; Books never reads or writes this table.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{Database, DbError};

/// One saved fallback mapping from a BIR form box to a Books account (or subtree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BooksBoxMapping {
    pub tin: String,
    pub form_type: String,
    pub box_key: String,
    pub book_id: String,
    pub account_id: String,
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    pub include_subtree: bool,
    #[serde(default)]
    pub updated_at: String,
}

impl Database {
    /// List all saved fallback Books account mappings for a taxpayer and form type.
    pub fn list_books_box_mappings(
        &self,
        tin: &str,
        form_type: &str,
    ) -> Result<Vec<BooksBoxMapping>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT tin, form_type, box_key, book_id, account_id, account_code, account_name, account_type, include_subtree, updated_at
             FROM books_box_mappings
             WHERE tin = ?1 AND form_type = ?2
             ORDER BY box_key ASC",
        )?;
        let rows = stmt
            .query_map(params![tin, form_type], |row| {
                Ok(BooksBoxMapping {
                    tin: row.get(0)?,
                    form_type: row.get(1)?,
                    box_key: row.get(2)?,
                    book_id: row.get(3)?,
                    account_id: row.get(4)?,
                    account_code: row.get(5)?,
                    account_name: row.get(6)?,
                    account_type: row.get(7)?,
                    include_subtree: row.get::<_, i64>(8)? != 0,
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Save (insert or update) a fallback Books account mapping for `(tin, form_type, box_key)`.
    pub fn upsert_books_box_mapping(&self, m: &BooksBoxMapping) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO books_box_mappings
                (tin, form_type, box_key, book_id, account_id, account_code, account_name, account_type, include_subtree, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
             ON CONFLICT(tin, form_type, box_key) DO UPDATE SET
                book_id = excluded.book_id,
                account_id = excluded.account_id,
                account_code = excluded.account_code,
                account_name = excluded.account_name,
                account_type = excluded.account_type,
                include_subtree = excluded.include_subtree,
                updated_at = datetime('now')",
            params![
                m.tin,
                m.form_type,
                m.box_key,
                m.book_id,
                m.account_id,
                m.account_code,
                m.account_name,
                m.account_type,
                i64::from(m.include_subtree),
            ],
        )?;
        Ok(())
    }

    /// Remove a fallback Books account mapping. Returns `true` if a row was deleted.
    pub fn delete_books_box_mapping(
        &self,
        tin: &str,
        form_type: &str,
        box_key: &str,
    ) -> Result<bool, DbError> {
        let changed = self.conn.execute(
            "DELETE FROM books_box_mappings WHERE tin = ?1 AND form_type = ?2 AND box_key = ?3",
            params![tin, form_type, box_key],
        )?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn books_box_mapping_crud_roundtrip() {
        let db = Database::open_in_memory_for_tests().unwrap();
        let tin = "00012345300000";
        let m = BooksBoxMapping {
            tin: tin.to_owned(),
            form_type: "2551Q".to_owned(),
            box_key: "txtATCAmt1".to_owned(),
            book_id: "book_demo".to_owned(),
            account_id: "acc_41000".to_owned(),
            account_code: "41000".to_owned(),
            account_name: "Sales Revenue".to_owned(),
            account_type: "income".to_owned(),
            include_subtree: true,
            updated_at: String::new(),
        };
        db.upsert_books_box_mapping(&m).unwrap();
        let listed = db.list_books_box_mappings(tin, "2551Q").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].account_code, "41000");
        assert!(listed[0].include_subtree);
        assert!(!listed[0].updated_at.is_empty());

        // Upsert replaces the same (tin, form_type, box_key)
        let mut updated = m.clone();
        updated.account_id = "acc_41010".to_owned();
        updated.account_code = "41010".to_owned();
        updated.account_name = "Sales to Government".to_owned();
        updated.include_subtree = false;
        db.upsert_books_box_mapping(&updated).unwrap();
        let listed2 = db.list_books_box_mappings(tin, "2551Q").unwrap();
        assert_eq!(listed2.len(), 1);
        assert_eq!(listed2[0].account_code, "41010");
        assert!(!listed2[0].include_subtree);

        // Delete removes it
        assert!(
            db.delete_books_box_mapping(tin, "2551Q", "txtATCAmt1")
                .unwrap()
        );
        assert!(
            !db.delete_books_box_mapping(tin, "2551Q", "txtATCAmt1")
                .unwrap()
        );
        assert!(db.list_books_box_mappings(tin, "2551Q").unwrap().is_empty());
    }
}
