use crate::db::{Database, DbError, Submission};
use crate::profile::TaxpayerProfile;
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::info;
use zip::ZipArchive;

/// Parse a semver string like "0.0.1" into (major, minor, patch).
/// Returns (0, 0, 0) for unparseable strings.
fn parse_semver(s: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = s.trim().split('.').collect();
    let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Apply sequential JSON-level migrations to a profile Value before deserialization.
///
/// Each migration step transforms the JSON structure to match the current schema.
/// The `from_version` is the semver tuple from the archive's manifest.json.
///
/// When you bump the Cargo package version AND change the export schema, add a new
/// `if from_version < (M, N, P)` block here with the corresponding field transformations.
fn migrate_profile_json(
    _value: &mut serde_json::Value,
    from_version: (u32, u32, u32),
) -> Result<(), DbError> {
    // Example for future use:
    // if from_version < (0, 2, 0) {
    //     // < 0.2.0: renamed `rdo_code` → `rdo`
    //     if let Some(old) = value.get("rdo_code").cloned() {
    //         value["rdo"] = old;
    //         value.as_object_mut().unwrap().remove("rdo_code");
    //     }
    // }
    if from_version < (0, 0, 1) {
        // Pre-0.0.1 (legacy unversioned): no structural changes needed.
        // All newer fields have #[serde(default)].
    }
    Ok(())
}

/// Apply sequential JSON-level migrations to a submission Value before deserialization.
fn migrate_submission_json(
    _value: &mut serde_json::Value,
    from_version: (u32, u32, u32),
) -> Result<(), DbError> {
    if from_version < (0, 0, 1) {
        // Pre-0.0.1: no changes needed
    }
    Ok(())
}

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

        // 0. Read manifest.json for export version (defaults to (0,0,0) for legacy archives)
        let export_version = match read_from_zip("manifest.json") {
            Ok(manifest_str) => {
                let m: serde_json::Value = serde_json::from_str(&manifest_str)?;
                let ver_str = m
                    .get("export_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0");
                let ver = parse_semver(ver_str);
                info!(
                    "Import archive manifest: export_version={}.{}.{}",
                    ver.0, ver.1, ver.2
                );
                ver
            }
            Err(_) => {
                info!("No manifest.json found — treating as legacy (0.0.0) export");
                (0, 0, 0)
            }
        };

        // 1. Read profile.json — parse as Value, migrate, then deserialize
        let profile_json = read_from_zip("profile.json")?;
        let mut profile_value: serde_json::Value = serde_json::from_str(&profile_json)?;
        migrate_profile_json(&mut profile_value, export_version)?;
        let profile: TaxpayerProfile = serde_json::from_value(profile_value)?;
        let tin = profile.tin.full();
        db.save_profile(profile)?;

        // 2. Read submissions.json — deduplicate via (tin, form_type, period, submitted_at)
        if let Ok(submissions_json) = read_from_zip("submissions.json") {
            let mut submissions: Vec<serde_json::Value> = serde_json::from_str(&submissions_json)?;
            for sub_value in &mut submissions {
                migrate_submission_json(sub_value, export_version)?;
            }
            let typed: Vec<Submission> = submissions
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<_, _>>()?;

            for sub in typed {
                let json_data = serde_json::to_string(&sub.form_data)?;
                let submitted_at_key = sub.submitted_at.clone().unwrap_or_default();

                // Check if a matching submission already exists (using COALESCE to match the index)
                let exists: bool = db
                    .conn
                    .query_row(
                        "SELECT 1 FROM submissions
                     WHERE tin = ?1 AND form_type = ?2 AND period = ?3
                       AND COALESCE(submitted_at, '') = ?4",
                        rusqlite::params![tin, sub.form_type, sub.period, submitted_at_key],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);

                if exists {
                    db.conn.execute(
                        "UPDATE submissions SET status = ?1, form_data = ?2, filename = ?3,
                                updated_at = datetime('now')
                         WHERE tin = ?4 AND form_type = ?5 AND period = ?6
                           AND COALESCE(submitted_at, '') = ?7",
                        rusqlite::params![
                            sub.status,
                            json_data,
                            sub.filename,
                            tin,
                            sub.form_type,
                            sub.period,
                            submitted_at_key
                        ],
                    )?;
                } else {
                    db.conn.execute(
                        "INSERT INTO submissions (tin, form_type, period, status, form_data, submitted_at, filename)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![
                            tin, sub.form_type, sub.period, sub.status,
                            json_data, sub.submitted_at, sub.filename
                        ],
                    )?;
                }
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

        // 5. Read data_providers.json (added in export v1)
        if let Ok(providers_json) = read_from_zip("data_providers.json") {
            let providers: Vec<serde_json::Value> = serde_json::from_str(&providers_json)?;
            for p in providers {
                if let (Some(provider_id), Some(name), Some(credentials_json)) = (
                    p.get("provider_id").and_then(|v| v.as_str()),
                    p.get("name").and_then(|v| v.as_str()),
                    p.get("credentials").map(|v| v.to_string()),
                ) {
                    let _ = db.conn.execute(
                        "INSERT INTO data_providers (profile_tin, provider_id, name, credentials_json)
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![tin, provider_id, name, credentials_json],
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
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
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
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
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

    #[test]
    fn test_legacy_profile_json_deserializes_with_defaults() {
        // A minimal JSON without any of the newer #[serde(default)] fields
        let legacy_json = r#"{
            "full_name": "Legacy User",
            "tin": {"segment1":"111","segment2":"222","segment3":"333","branch":"000"},
            "rdo_code": "039",
            "line_of_business": "IT",
            "registered_address": "Manila",
            "zip_code": "1000",
            "phone": "0912",
            "email": "legacy@test.com",
            "default_form_type": "2551Q"
        }"#;

        // Parse as Value, run migration, then deserialize
        let mut value: serde_json::Value = serde_json::from_str(legacy_json).unwrap();
        migrate_profile_json(&mut value, (0, 0, 0)).unwrap();
        let profile: TaxpayerProfile = serde_json::from_value(value).unwrap();

        assert_eq!(profile.full_name, "Legacy User");
        assert_eq!(profile.tax_classification, None);
        assert!(!profile.has_8_percent_election(2026));
        assert!(!profile.is_archived);
        assert!(!profile.email_tracking_enabled);
    }

    #[test]
    fn test_import_idempotent_no_duplicate_submissions() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_idem.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Idem User".to_string(),
            tin: crate::naming::Tin {
                segment1: "555".into(),
                segment2: "555".into(),
                segment3: "555".into(),
                branch: "000".into(),
            },
            rdo_code: "039".to_string(),
            line_of_business: "IT".to_string(),
            registered_address: "QC".to_string(),
            zip_code: "1103".to_string(),
            phone: "0999".to_string(),
            email: "idem@test.com".to_string(),
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
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile).unwrap();

        let sub = Submission {
            id: None,
            tin: "555555555000".to_string(),
            form_type: "2551Q".to_string(),
            period: "2024Q1".to_string(),
            status: "Draft".to_string(),
            form_data: std::collections::BTreeMap::new(),
            submitted_at: None,
            filename: None,
            created_at: None,
            updated_at: None,
        };
        db.save_submission(sub).unwrap();

        // Export
        let export_zip = dir.path().join("idem_export.zip");
        crate::export::export_profile_data(&db, "555555555000", &export_zip).unwrap();

        // Import into a new DB twice
        let db2_path = dir.path().join("test_idem2.db");
        let (db2, _) = Database::open_or_recreate(&db2_path).unwrap();
        import_profile_data(&db2, &export_zip).unwrap();
        // Second import — should NOT create duplicate submissions
        import_profile_data(&db2, &export_zip).unwrap();

        let submissions = db2.list_submissions_for_tin("555555555000").unwrap();
        assert_eq!(
            submissions.len(),
            1,
            "Re-importing should not create duplicate submissions"
        );
    }

    #[test]
    fn test_parse_semver_valid() {
        assert_eq!(parse_semver("0.0.1"), (0, 0, 1));
        assert_eq!(parse_semver("1.2.3"), (1, 2, 3));
        assert_eq!(parse_semver("10.20.30"), (10, 20, 30));
    }

    #[test]
    fn test_parse_semver_edge_cases() {
        // Missing parts default to 0
        assert_eq!(parse_semver("1"), (1, 0, 0));
        assert_eq!(parse_semver("1.2"), (1, 2, 0));
        // Empty / garbage defaults to (0,0,0)
        assert_eq!(parse_semver(""), (0, 0, 0));
        assert_eq!(parse_semver("not-a-version"), (0, 0, 0));
        // Whitespace is trimmed
        assert_eq!(parse_semver("  1.2.3  "), (1, 2, 3));
    }

    #[test]
    fn test_manifest_contains_cargo_version() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_manifest.db");
        let (db, _) = Database::open_or_recreate(&db_path).unwrap();

        let profile = TaxpayerProfile {
            id: None,
            full_name: "Manifest Test".to_string(),
            tin: crate::naming::Tin {
                segment1: "777".into(),
                segment2: "777".into(),
                segment3: "777".into(),
                branch: "000".into(),
            },
            rdo_code: "039".to_string(),
            line_of_business: "IT".to_string(),
            registered_address: "BGC".to_string(),
            zip_code: "1634".to_string(),
            phone: "0917".to_string(),
            email: "manifest@test.com".to_string(),
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
            totp_secret: None,
            tax_classification: None,
            eopt_tier: None,
            is_bmbe: false,
            is_gpp_partner: false,
            is_create_msme: false,
            is_expanded_withholding_agent: false,
            atc_codes: vec![],
            excise_tax_categories: vec![],
            tax_elections: vec![],
            _opted_for_8_percent_flat_rate_compat: None,
            has_employees: false,
            is_dormant: false,
            has_single_employer: false,
            withholds_compensation: false,
            withholds_expanded: false,
            withholds_final: false,
            is_top_withholding_agent: false,
            is_government_withholding_entity: false,
            registration_activity_status: Default::default(),
        };
        db.save_profile(profile).unwrap();

        // Export
        let export_zip = dir.path().join("manifest_test.zip");
        crate::export::export_profile_data(&db, "777777777000", &export_zip).unwrap();

        // Read the ZIP and extract manifest.json directly
        let file = std::fs::File::open(&export_zip).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut manifest_str = String::new();
        manifest_file.read_to_string(&mut manifest_str).unwrap();

        let manifest: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();

        // export_version should be the Cargo package version string
        let version = manifest.get("export_version").unwrap();
        assert!(version.is_string(), "export_version should be a string");
        assert_eq!(
            version.as_str().unwrap(),
            env!("CARGO_PKG_VERSION"),
            "export_version should match CARGO_PKG_VERSION"
        );

        // exported_at should be present
        assert!(manifest.get("exported_at").is_some());

        // Verify the version parses correctly
        let ver = parse_semver(version.as_str().unwrap());
        assert_eq!(ver, parse_semver(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_semver_comparison_ordering() {
        // Verify that tuple comparison works correctly for migration gates
        assert!((0, 0, 0) < (0, 0, 1));
        assert!((0, 0, 1) < (0, 1, 0));
        assert!((0, 1, 0) < (1, 0, 0));
        assert!((0, 0, 1) < (0, 2, 0));
        assert!(!((0, 0, 1) < (0, 0, 1))); // equal, not less
        assert!((1, 0, 0) < (1, 0, 1));
    }
}
