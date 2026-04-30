use super::{Database, DbError};
use crate::integration::providers::ProviderConfig;
use rusqlite::params;

impl Database {
    pub fn save_data_provider(&self, config: ProviderConfig) -> Result<i64, DbError> {
        let credentials_json = serde_json::to_string(&config.credentials)?;

        if let Some(id) = config.id {
            self.conn.execute(
                "UPDATE data_providers 
                 SET profile_tin = ?1, provider_id = ?2, name = ?3, credentials_json = ?4 
                 WHERE id = ?5",
                params![
                    config.profile_tin,
                    config.provider_id,
                    config.name,
                    credentials_json,
                    id
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO data_providers (profile_tin, provider_id, name, credentials_json) 
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    config.profile_tin,
                    config.provider_id,
                    config.name,
                    credentials_json
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn get_data_providers(&self, profile_tin: &str) -> Result<Vec<ProviderConfig>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, profile_tin, provider_id, name, credentials_json 
             FROM data_providers 
             WHERE profile_tin = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![profile_tin], |row| {
            let credentials_json: String = row.get(4)?;
            let credentials = serde_json::from_str(&credentials_json).unwrap_or_default();

            Ok(ProviderConfig {
                id: row.get(0)?,
                profile_tin: row.get(1)?,
                provider_id: row.get(2)?,
                name: row.get(3)?,
                credentials,
            })
        })?;

        let mut providers = Vec::new();
        for provider in rows {
            providers.push(provider?);
        }

        Ok(providers)
    }

    pub fn delete_data_provider(&self, id: i64) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM data_providers WHERE id = ?1", params![id])?;
        Ok(())
    }
}
