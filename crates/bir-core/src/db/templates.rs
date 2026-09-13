//! Per-form templates keyed by `(tin, form_code)`.

use rusqlite::{OptionalExtension, params};
use std::collections::BTreeMap;

use super::{Database, DbError};
use crate::forms::templates::FormTemplate;

impl Database {
    pub fn get_form_template(
        &self,
        tin: &str,
        form_code: &str,
    ) -> Result<Option<FormTemplate>, DbError> {
        let code = form_code.trim().to_ascii_uppercase();
        let mut stmt = self.conn.prepare(
            "SELECT data_json FROM form_templates WHERE tin = ?1 AND form_code = ?2",
        )?;
        let json: Option<String> = stmt
            .query_row(params![tin, code], |row| row.get(0))
            .optional()?;
        let Some(json) = json else {
            return Ok(None);
        };
        let values: BTreeMap<String, String> = serde_json::from_str(&json)?;
        Ok(Some(FormTemplate {
            tin: tin.to_string(),
            form_code: code,
            values,
        }))
    }

    pub fn save_form_template(
        &self,
        tin: &str,
        form_code: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<(), DbError> {
        let code = form_code.trim().to_ascii_uppercase();
        let json = serde_json::to_string(values)?;
        self.conn.execute(
            "INSERT INTO form_templates (tin, form_code, data_json, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(tin, form_code) DO UPDATE SET
                data_json = excluded.data_json,
                updated_at = datetime('now')",
            params![tin, code, json],
        )?;
        Ok(())
    }

    pub fn delete_form_template(&self, tin: &str, form_code: &str) -> Result<(), DbError> {
        let code = form_code.trim().to_ascii_uppercase();
        self.conn.execute(
            "DELETE FROM form_templates WHERE tin = ?1 AND form_code = ?2",
            params![tin, code],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::inventory::load_spec;
    use crate::forms::templates::filter_template_values;
    use crate::profile::TaxpayerProfile;

    fn sample_profile() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Template Taxpayer",
            "tin": { "segment1": "261", "segment2": "708", "segment3": "015", "branch": "00000" },
            "rdo_code": "018",
            "line_of_business": "Software",
            "registered_address": "New Cabalan",
            "zip_code": "2200",
            "phone": "09156837000",
            "email": "tax@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01"
        }))
        .unwrap()
    }

    #[test]
    fn form_template_round_trips_without_a_tax_year() {
        let db = Database::open_in_memory_for_tests().unwrap();
        let profile = sample_profile();
        let tin = profile.tin.full();
        let spec = load_spec("2551Q").unwrap();
        let mut values = BTreeMap::new();
        values.insert("drpATC1".into(), "PT040".into());
        values.insert("frm2551Qv2018:txtTIN1".into(), "999".into());
        let filtered = filter_template_values(&spec, &values);
        db.save_form_template(&tin, "2551Q", &filtered).unwrap();

        let loaded = db.get_form_template(&tin, "2551q").unwrap().unwrap();
        assert_eq!(loaded.form_code, "2551Q");
        assert_eq!(loaded.values.get("drpATC1").map(String::as_str), Some("PT040"));
        assert!(!loaded.values.keys().any(|k| k.contains("TIN")));
        assert_eq!(loaded.tin, tin);
    }

    #[test]
    fn save_profile_does_not_copy_the_profile_onto_the_template() {
        let db = Database::open_in_memory_for_tests().unwrap();
        let profile = sample_profile();
        let tin = profile.tin.full();
        db.save_form_template(&tin, "2551Q", &BTreeMap::from([(
            "drpATC1".into(),
            "PT010".into(),
        )]))
        .unwrap();
        let loaded = db.get_form_template(&tin, "2551Q").unwrap().unwrap();
        assert!(!loaded.values.keys().any(|k| k.contains("TaxpayerName")));
        assert!(!loaded.values.keys().any(|k| k.contains("TIN")));
    }
}
