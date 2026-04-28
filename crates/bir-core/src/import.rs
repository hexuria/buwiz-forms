use crate::db::{Database, DbError, Submission};
use crate::profile::TaxpayerProfile;
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn import_profile_data(db: &Database, import_file: &Path) -> Result<(), DbError> {
    let file = fs::File::open(import_file)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DbError::Other(e.to_string()))?;

    let mut base_dirs = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name();
            if name.ends_with("profile.json") {
                let base_dir = name.strip_suffix("profile.json").unwrap();
                base_dirs.push(base_dir.to_string());
            }
        }
    }

    if base_dirs.is_empty() {
        return Err(DbError::Other(
            "No profile.json found in archive".to_string(),
        ));
    }

    for base_dir in base_dirs {
        // Helper to read file from archive
        let mut read_from_zip = |name: &str| -> Result<String, DbError> {
            let full_name = format!("{}{}", base_dir, name);
            let mut file = archive
                .by_name(&full_name)
                .map_err(|e| DbError::Other(e.to_string()))?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            Ok(content)
        };

        // 1. Read profile.json
        let profile_json = read_from_zip("profile.json")?;
        let profile: TaxpayerProfile = serde_json::from_str(&profile_json)?;
        let tin = profile.tin.full();
        db.save_profile(profile)?;

        // 2. Read submissions.json
        if let Ok(submissions_json) = read_from_zip("submissions.json") {
            let submissions: Vec<Submission> = serde_json::from_str(&submissions_json)?;
            for mut sub in submissions {
                sub.id = None; // clear ID to insert as new
                db.save_submission(sub)?;
            }
        }

        // 3. Read drafts.json
        if let Ok(drafts_json) = read_from_zip("drafts.json") {
            let drafts: Vec<serde_json::Value> = serde_json::from_str(&drafts_json)?;

            for draft in drafts {
                if let (Some(form_code), Some(taxable_year), Some(status), Some(data_json)) = (
                    draft.get("form_code").and_then(|v| v.as_str()),
                    draft.get("taxable_year").and_then(|v| v.as_i64()),
                    draft.get("status").and_then(|v| v.as_str()),
                    draft.get("data_json").and_then(|v| v.as_str()),
                ) {
                    let quarter = draft.get("quarter").and_then(|v| v.as_i64());

                    db.conn.execute(
                        "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT(tin, form_code, taxable_year, quarter)
                         DO UPDATE SET status = excluded.status,
                                       data_json = excluded.data_json,
                                       updated_at = datetime('now')",
                        rusqlite::params![
                            tin,
                            form_code,
                            taxable_year,
                            quarter,
                            status,
                            data_json
                        ],
                    )?;
                }
            }
        }

        // 4. Read receipts
        if let Ok(manifest_json) = read_from_zip("receipts_manifest.json") {
            let manifest: Vec<String> = serde_json::from_str(&manifest_json)?;

            for filename in manifest {
                let txt_name = format!("Receipts/{}.txt", filename);
                let html_name = format!("Receipts/{}.html", filename);

                if let Ok(raw_text) = read_from_zip(&txt_name) {
                    let raw_html = read_from_zip(&html_name).ok();

                    let mut form_type = "Unknown".to_string();
                    let mut period = "Unknown".to_string();
                    let mut received_date = "Unknown".to_string();
                    let mut received_time = "Unknown".to_string();

                    if let Ok(parsed) =
                        crate::receipt::parse_bir_receipt_email(&raw_text, raw_html.clone())
                    {
                        received_date = parsed.date_received.to_string();
                        received_time = parsed.time_received.to_string();
                    }

                    if let Some(split) = crate::receipt::split_bir_filename(&filename) {
                        form_type = split.1;
                        period = split.2;
                    }

                    let _ = db.conn.execute(
                        "INSERT INTO submission_receipts (filename, tin, form_type, period, received_date, received_time, source_from, raw_text, raw_html)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                         ON CONFLICT(filename) DO UPDATE SET raw_text = excluded.raw_text, raw_html = excluded.raw_html",
                        rusqlite::params![
                            filename,
                            tin,
                            form_type,
                            period,
                            received_date,
                            received_time,
                            "Import",
                            raw_text,
                            raw_html
                        ],
                    );
                }
            }
        }
    }

    Ok(())
}

pub fn extract_database_zip(zip_path: &Path, out_db_path: &Path) -> Result<(), DbError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DbError::Other(e.to_string()))?;

    let mut db_file_in_zip = archive
        .by_name("bir_data.db")
        .map_err(|e| DbError::Other(e.to_string()))?;

    // Extract to temp unencrypted db
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_db_path = temp_dir.join(format!("bir_unencrypted_import_{}.db", timestamp));

    let mut temp_file = fs::File::create(&temp_db_path)?;
    std::io::copy(&mut db_file_in_zip, &mut temp_file)?;

    // Delete the current database so we can cleanly export into a new one
    if out_db_path.exists() {
        let _ = fs::remove_file(out_db_path);
    }
    let wal_path = out_db_path.with_extension("db-wal");
    let shm_path = out_db_path.with_extension("db-shm");
    if wal_path.exists() {
        let _ = fs::remove_file(wal_path);
    }
    if shm_path.exists() {
        let _ = fs::remove_file(shm_path);
    }

    // Open the unencrypted db
    let conn = rusqlite::Connection::open(&temp_db_path)?;
    let key_hex = Database::get_or_create_master_key()?;

    // Attach the target db (encrypted) and export to it
    conn.execute(
        &format!("ATTACH DATABASE ?1 AS encrypted KEY \"x'{}'\";", key_hex),
        rusqlite::params![out_db_path.to_str().unwrap()],
    )?;
    let mut stmt = conn.prepare("SELECT sqlcipher_export('encrypted');")?;
    let _ = stmt.query([])?.next()?;
    drop(stmt);
    conn.execute("DETACH DATABASE encrypted;", [])?;

    // Explicitly close the connection to ensure all data is flushed
    drop(conn);

    // Small delay to ensure file system operations complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Clean up
    let _ = fs::remove_file(&temp_db_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, Submission};
    use crate::profile::{TaxpayerProfile, TaxpayerType};
    use tempfile::tempdir;

    #[test]
    fn test_profile_export_import_zip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        // Create mock profile
        let profile = TaxpayerProfile {
            id: None,
            full_name: "John Doe".to_string(),
            tin: crate::naming::Tin {
                segment1: "123".into(),
                segment2: "456".into(),
                segment3: "789".into(),
                branch: "000".into(),
            },
            rdo_code: "123".to_string(),
            line_of_business: "Tech".to_string(),
            registered_address: "123 Main St".to_string(),
            zip_code: "1000".to_string(),
            phone: "09123456789".to_string(),
            email: "test@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_pin_hash: None,
            tax_classification: None,
            opted_for_8_percent_flat_rate: false,
        };
        db.save_profile(profile.clone()).unwrap();

        // Create mock submission
        let sub = Submission {
            id: None,
            tin: "123456789000".to_string(),
            form_type: "2551Q".to_string(),
            period: "2024Q1".to_string(),
            status: "Draft".to_string(),
            form_data: std::collections::BTreeMap::new(),
            submitted_at: None,
            filename: None,
            created_at: None,
            updated_at: None,
        };
        db.save_submission(sub.clone()).unwrap();

        // Export to zip
        let export_zip = dir.path().join("export.zip");
        crate::export::export_profile_data(&db, "123456789000", &export_zip).unwrap();
        assert!(export_zip.exists());

        // Import into new DB
        let db2_path = dir.path().join("test2.db");
        let (db2, _) = Database::open_or_recreate(&db2_path).unwrap();
        import_profile_data(&db2, &export_zip).unwrap();

        // Verify
        let imported_profile = db2.get_profile_by_tin("123456789000").unwrap().unwrap();
        assert_eq!(imported_profile.full_name, "John Doe".to_string());

        let submissions = db2.list_submissions_for_tin("123456789000").unwrap();
        assert_eq!(submissions.len(), 1);
        assert_eq!(submissions[0].form_type, "2551Q");
    }

    #[test]
    fn test_database_export_import_zip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("original.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Jane Doe".to_string(),
            tin: crate::naming::Tin {
                segment1: "999".into(),
                segment2: "999".into(),
                segment3: "999".into(),
                branch: "000".into(),
            },
            rdo_code: "123".to_string(),
            line_of_business: "Tech".to_string(),
            registered_address: "123 Main St".to_string(),
            zip_code: "1000".to_string(),
            phone: "09123456789".to_string(),
            email: "test2@example.com".to_string(),
            default_form_type: "2551Q".to_string(),
            taxpayer_type: TaxpayerType::Individual,
            is_vat_registered: false,
            business_start_date: None,
            is_archived: false,
            email_tracking_enabled: false,
            email_auth_method: crate::profile::EmailAuthMethod::AppPassword,
            imap_email: None,
            imap_host: None,
            _imap_enabled_compat: None,
            test_notification_enabled: false,
            imap_app_password: None,
            oauth_access_token: None,
            oauth_refresh_token: None,
            profile_pin_hash: None,
            tax_classification: None,
            opted_for_8_percent_flat_rate: false,
        };
        db.save_profile(profile).unwrap();
        db.checkpoint().unwrap();

        let backup_zip = dir.path().join("backup.db.zip");
        crate::export::export_database_zip(&db, &backup_zip).unwrap();
        assert!(backup_zip.exists());

        let restored_db_path = dir.path().join("restored.db");
        extract_database_zip(&backup_zip, &restored_db_path).unwrap();

        let (db2, _) = Database::open_or_recreate(&restored_db_path).unwrap();
        let profiles = db2.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].tin.full(), "999999999000");
    }
}
