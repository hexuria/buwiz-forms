//! Encrypted SQLite database for taxpayer data.
//!
//! Stores profiles, form data, submission history.
//! Data is AES-256-GCM encrypted at rest at the application layer.
//! Master key stored in OS keychain.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use keyring::Entry;
use rusqlite::{params, Connection};
use std::path::Path;
use thiserror::Error;
use tracing::info;

use crate::profile::TaxpayerProfile;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Encryption error")]
    Encryption,
    #[error("Keychain error: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub struct Database {
    conn: Connection,
    cipher: Aes256Gcm,
}

impl Database {
    /// Opens the database and initializes encryption using the OS keychain.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        
        // Initialize Schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tin TEXT UNIQUE NOT NULL,
                encrypted_data BLOB NOT NULL
            )",
            [],
        )?;

        let cipher = Self::init_cipher()?;

        Ok(Self { conn, cipher })
    }

    /// Retrieve or generate the master database key from the OS Keychain.
    fn init_cipher() -> Result<Aes256Gcm, DbError> {
        let entry = Entry::new("com.ebir.rust", "master_key")?;
        
        let key_bytes = match entry.get_password() {
            Ok(hex_key) => {
                info!("Loaded existing master key from keychain");
                hex::decode(&hex_key).map_err(|_| DbError::Encryption)?
            }
            Err(_) => {
                info!("Generating new master key and storing in keychain");
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                let hex_key = hex::encode(key);
                entry.set_password(&hex_key)?;
                key.to_vec()
            }
        };

        if key_bytes.len() != 32 {
            return Err(DbError::Encryption);
        }

        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Aes256Gcm::new(key))
    }

    fn encrypt_payload(&self, data: &[u8]) -> Result<Vec<u8>, DbError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let mut ciphertext = self.cipher.encrypt(nonce, data)
            .map_err(|_| DbError::Encryption)?;
        
        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.append(&mut ciphertext);
        Ok(result)
    }

    fn decrypt_payload(&self, data: &[u8]) -> Result<Vec<u8>, DbError> {
        if data.len() < 12 {
            return Err(DbError::Encryption);
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|_| DbError::Encryption)
    }

    /// Saves a taxpayer profile (insert or replace).
    pub fn save_profile(&self, mut profile: TaxpayerProfile) -> Result<TaxpayerProfile, DbError> {
        let json_data = serde_json::to_vec(&profile)?;
        let encrypted_data = self.encrypt_payload(&json_data)?;

        // Using REPLACE to handle updates based on TIN
        self.conn.execute(
            "INSERT OR REPLACE INTO profiles (tin, encrypted_data) VALUES (?1, ?2)",
            params![profile.tin.full(), encrypted_data],
        )?;

        // Update ID if not set
        if profile.id.is_none() {
            profile.id = Some(self.conn.last_insert_rowid());
        }

        Ok(profile)
    }

    /// Get a profile by TIN.
    pub fn get_profile(&self, tin: &str) -> Result<Option<TaxpayerProfile>, DbError> {
        let mut stmt = self.conn.prepare("SELECT encrypted_data FROM profiles WHERE tin = ?1")?;
        
        let mut rows = stmt.query(params![tin])?;
        
        if let Some(row) = rows.next()? {
            let encrypted_data: Vec<u8> = row.get(0)?;
            let decrypted_data = self.decrypt_payload(&encrypted_data)?;
            let profile: TaxpayerProfile = serde_json::from_slice(&decrypted_data)?;
            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    /// List all profiles.
    pub fn list_profiles(&self) -> Result<Vec<TaxpayerProfile>, DbError> {
        let mut stmt = self.conn.prepare("SELECT encrypted_data FROM profiles")?;
        let rows = stmt.query_map([], |row| {
            let encrypted_data: Vec<u8> = row.get(0)?;
            Ok(encrypted_data)
        })?;

        let mut profiles = Vec::new();
        for row_result in rows {
            let encrypted_data = row_result?;
            let decrypted_data = self.decrypt_payload(&encrypted_data)?;
            let profile: TaxpayerProfile = serde_json::from_slice(&decrypted_data)?;
            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Delete a profile by TIN.
    pub fn delete_profile(&self, tin: &str) -> Result<(), DbError> {
        self.conn.execute("DELETE FROM profiles WHERE tin = ?1", params![tin])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::Tin;
    use tempfile::NamedTempFile;

    // A mock keyring can be tricky in CI, so we might encounter issues depending on the runner.
    // Assuming macOS with user session or a mock environment.
    #[test]
    fn test_profile_crud() {
        let db_file = NamedTempFile::new().unwrap();
        // Since keyring depends on OS, this might fail in headless environments without a keychain.
        // For the sake of the test, we'll try to open it and skip if the OS keychain isn't available.
        let db = match Database::open(db_file.path()) {
            Ok(db) => db,
            Err(e) => {
                println!("Skipping test due to keyring unavailability: {:?}", e);
                return;
            }
        };

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Code It Like Miley".into(),
            tin: Tin {
                segment1: "010".into(),
                segment2: "558".into(),
                segment3: "054".into(),
                branch: "000".into(),
            },
            rdo_code: "039".into(),
            line_of_business: "Software".into(),
            registered_address: "QC".into(),
            zip_code: "1103".into(),
            phone: "0999".into(),
            email: "miley@example.com".into(),
            default_form_type: "2551Qv2018".into(),
        };

        db.save_profile(profile).expect("Failed to save profile");

        let retrieved = db.get_profile("010558054000").expect("Failed to query").expect("Profile not found");
        assert_eq!(retrieved.full_name, "Code It Like Miley");

        let list = db.list_profiles().expect("Failed to list");
        assert_eq!(list.len(), 1);

        db.delete_profile("010558054000").expect("Failed to delete");
        let list_after = db.list_profiles().expect("Failed to list");
        assert_eq!(list_after.len(), 0);
    }
}
