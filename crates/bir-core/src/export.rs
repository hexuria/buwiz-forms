use crate::db::{Database, DbError};
use crate::tax_profile::TaxProfileError;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// Write `tin`'s profile archive. Like every profile archive it carries no
/// credential and no profile or provider row id (see `write_profile_to_zip`).
///
/// Exporting a profile whose taxpayer has a PIN requires that PIN (the
/// maintainer's 2026-10-20 decision): the check runs before the archive file
/// is created, so a refusal leaves nothing on disk. A taxpayer without a PIN
/// — one with only an authenticator included — exports ungated.
pub fn export_profile_data(
    db: &Database,
    tin: &str,
    export_file: &Path,
    pin: Option<&str>,
) -> Result<(), DbError> {
    check_export_pin(db, tin, pin)?;
    write_profile_zip(db, tin, export_file)
}

/// The administrator's delete-with-backup export — bir-shell's
/// `export_and_delete_profile` is its one caller: the same archive
/// [`export_profile_data`] writes, without the taxpayer PIN gate, because
/// that flow already confirmed the administrator's app-lock credential and
/// its save dialog is the deletion's confirmation.
pub fn export_profile_backup_as_admin(
    db: &Database,
    tin: &str,
    export_file: &Path,
) -> Result<(), DbError> {
    write_profile_zip(db, tin, export_file)
}

/// The gate [`export_profile_data`] applies before it creates `export_file`:
/// when the profile's taxpayer (the TIN root's `taxpayers` row) has a
/// `pin_hash`, `pin` must match it. A missing taxpayer row means no PIN to
/// check, and a taxpayer without a `pin_hash` — including one with only an
/// authenticator — is ungated.
///
/// `taxpayers.pin_hash` is authoritative when it holds a PIN. A legacy
/// `profile_pin_hash` still sitting on the profile row — what the desktop
/// profile manager writes until the next launch's v22 repair lifts it onto
/// the taxpayer — gates the export the same way, so a PIN is never bypassed
/// between the save that sets it and the open that lifts it.
fn check_export_pin(db: &Database, tin: &str, pin: Option<&str>) -> Result<(), DbError> {
    let root: String = tin.chars().filter(|c| c.is_ascii_digit()).take(9).collect();
    let stored = match db.taxpayer_credentials(&root) {
        Ok(credentials) => credentials.pin_hash,
        Err(TaxProfileError::NotFound(_) | TaxProfileError::Invalid(_)) => None,
        Err(TaxProfileError::Db(error)) => return Err(error),
        Err(error) => return Err(DbError::Other(error.to_string())),
    };
    let required = match stored {
        Some(hash) => Some(hash),
        None => db
            .get_profile(tin)?
            .and_then(|mut profile| profile.profile_pin_hash.take()),
    };
    let Some(pin_hash) = required else {
        return Ok(());
    };
    match pin {
        None => Err(DbError::PinRequired),
        Some(pin) if crate::crypto::hash_pin(pin) == pin_hash => Ok(()),
        Some(_) => Err(DbError::PinMismatch),
    }
}

fn write_profile_zip(db: &Database, tin: &str, export_file: &Path) -> Result<(), DbError> {
    let file = fs::File::create(export_file)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    write_profile_to_zip(&mut zip, db, tin, "", options)?;

    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}

/// Write every profile's archive under `Profiles/<tin>/`, each without
/// credentials or row ids, as [`export_profile_data`] writes one.
pub fn export_all_profiles_data(db: &Database, export_file: &Path) -> Result<(), DbError> {
    let file = fs::File::create(export_file)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let profiles = db.list_profiles()?;
    for profile in profiles {
        let tin = profile.tin.full();
        let base_dir = format!("Profiles/{}/", tin);
        write_profile_to_zip(&mut zip, db, &tin, &base_dir, options)?;
    }

    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}

/// One profile's archive entries under `base_dir`.
///
/// A profile export carries no credentials: profile.json has none of the
/// five credential keys, and data_providers.json has each provider's
/// provider_id and name only. Nor does it carry the profile's or a
/// provider's row id, which names a row in this database and means nothing
/// in another. The full database backup ([`export_database_zip`]) is a
/// whole-database copy by design and keeps everything.
fn write_profile_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    db: &Database,
    tin: &str,
    base_dir: &str,
    options: SimpleFileOptions,
) -> Result<(), DbError> {
    // 0. Manifest — version tracks Cargo.toml package version
    let manifest = serde_json::json!({
        "export_version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });
    zip.start_file(format!("{}manifest.json", base_dir), options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;

    // 1. Profile JSON, without the credentials and the row id
    if let Some(mut profile) = db.get_profile_by_tin(tin)? {
        let mut record = profile.record_without_credentials()?;
        if let Some(fields) = record.as_object_mut() {
            fields.remove("id");
        }
        let profile_json = serde_json::to_string_pretty(&record)?;
        zip.start_file(format!("{}profile.json", base_dir), options)?;
        zip.write_all(profile_json.as_bytes())?;
    }

    // 2. Submissions JSON
    let submissions = db.list_submissions_for_tin(tin)?;
    let submissions_json = serde_json::to_string_pretty(&submissions)?;
    zip.start_file(format!("{}submissions.json", base_dir), options)?;
    zip.write_all(submissions_json.as_bytes())?;

    // 3. Drafts JSON
    let mut drafts_stmt = db
        .conn
        .prepare("SELECT form_code, taxable_year, quarter, status, data_json FROM form_drafts WHERE tin = ?1")?;
    let drafts_iter = drafts_stmt.query_map([tin], |row| {
        Ok(serde_json::json!({
            "form_code": row.get::<_, String>(0)?,
            "taxable_year": row.get::<_, i64>(1)?,
            "quarter": row.get::<_, Option<i64>>(2)?,
            "status": row.get::<_, String>(3)?,
            "data_json": row.get::<_, String>(4)?,
        }))
    })?;
    let mut drafts = Vec::new();
    for d in drafts_iter.flatten() {
        drafts.push(d);
    }
    let drafts_json = serde_json::to_string_pretty(&drafts)?;
    zip.start_file(format!("{}drafts.json", base_dir), options)?;
    zip.write_all(drafts_json.as_bytes())?;

    // 4. Receipts JSON and HTMLs
    let mut stmt = db
        .conn
        .prepare("SELECT filename, raw_text, raw_html FROM submission_receipts WHERE tin = ?1")?;
    let receipts_iter = stmt.query_map([tin], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut receipts_metadata = Vec::new();
    for receipt_res in receipts_iter {
        let (filename, raw_text, raw_html) = receipt_res?;
        receipts_metadata.push(filename.clone());
        zip.start_file(format!("{}Receipts/{}.txt", base_dir, filename), options)?;
        zip.write_all(raw_text.as_bytes())?;
        if let Some(html) = raw_html {
            zip.start_file(format!("{}Receipts/{}.html", base_dir, filename), options)?;
            zip.write_all(html.as_bytes())?;
        }
    }

    let metadata_json = serde_json::to_string_pretty(&receipts_metadata)?;
    zip.start_file(format!("{}receipts_manifest.json", base_dir), options)?;
    zip.write_all(metadata_json.as_bytes())?;

    // 5. Data Providers JSON: each provider's provider_id and name only,
    // read without its credentials, in `Database::get_data_providers` order
    let mut providers_stmt = db.conn.prepare(
        "SELECT provider_id, name FROM data_providers
         WHERE profile_tin = ?1 ORDER BY created_at DESC",
    )?;
    let providers = providers_stmt
        .query_map([tin], |row| {
            Ok(serde_json::json!({
                "provider_id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !providers.is_empty() {
        let providers_json = serde_json::to_string_pretty(&providers)?;
        zip.start_file(format!("{}data_providers.json", base_dir), options)?;
        zip.write_all(providers_json.as_bytes())?;
    }

    Ok(())
}

pub fn export_database_zip(db: &Database, zip_path: &Path) -> Result<(), DbError> {
    let temp_db_path = temporary_plaintext_database_path("bir_unencrypted");

    let result = (|| {
        // Export current encrypted DB to a temporary unencrypted DB
        db.conn.execute(
            "ATTACH DATABASE ?1 AS plaintext KEY '';",
            rusqlite::params![path_as_utf8(&temp_db_path)?],
        )?;
        let mut stmt = db.conn.prepare("SELECT sqlcipher_export('plaintext');")?;
        let _ = stmt.query([])?.next()?;
        drop(stmt);
        db.conn.execute("DETACH DATABASE plaintext;", [])?;

        zip_plaintext_database(&temp_db_path, zip_path)
    })();

    let _ = fs::remove_file(&temp_db_path);
    result
}

/// Exports an existing SQLCipher database while the source is attached with `mode=ro`.
///
/// Unlike [`export_database_zip`], this path does not open the source through the normal
/// application lifecycle, so it cannot create, migrate, recover, or write the live database.
pub fn export_existing_database_zip(
    source_database: &Path,
    zip_path: &Path,
) -> Result<(), DbError> {
    if !source_database.is_file() {
        return Err(DbError::Other(format!(
            "Database does not exist: {}",
            source_database.display()
        )));
    }

    let temp_db_path = temporary_plaintext_database_path("bir_read_only_export");
    let result = (|| {
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
            | rusqlite::OpenFlags::SQLITE_OPEN_URI;
        let connection = rusqlite::Connection::open_with_flags(&temp_db_path, flags)?;
        let key_hex = Database::get_existing_master_key()?;
        let source_uri = sqlite_read_only_uri(source_database)?;

        connection.execute(
            &format!(
                "ATTACH DATABASE ?1 AS encrypted_source KEY \"x'{}'\";",
                key_hex
            ),
            rusqlite::params![source_uri],
        )?;
        let _: i64 = connection.query_row(
            "SELECT count(*) FROM encrypted_source.sqlite_master",
            [],
            |row| row.get(0),
        )?;
        let mut statement =
            connection.prepare("SELECT sqlcipher_export('main', 'encrypted_source');")?;
        let _ = statement.query([])?.next()?;
        drop(statement);
        connection.execute("DETACH DATABASE encrypted_source;", [])?;
        drop(connection);

        zip_plaintext_database(&temp_db_path, zip_path)
    })();

    let _ = fs::remove_file(&temp_db_path);
    result
}

fn temporary_plaintext_database_path(prefix: &str) -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{timestamp}.db"))
}

fn path_as_utf8(path: &Path) -> Result<&str, DbError> {
    path.to_str()
        .ok_or_else(|| DbError::Other("Database path is not valid UTF-8".into()))
}

fn sqlite_read_only_uri(path: &Path) -> Result<String, DbError> {
    let normalized = path_as_utf8(path)?.replace('\\', "/");
    let mut encoded = String::with_capacity(normalized.len());
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~' | b':') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}")
                .map_err(|error| DbError::Other(error.to_string()))?;
        }
    }
    Ok(format!("file:{encoded}?mode=ro"))
}

fn zip_plaintext_database(database_path: &Path, zip_path: &Path) -> Result<(), DbError> {
    let mut file = fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(&mut file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("bir_data.db", options)
        .map_err(|e| DbError::Other(e.to_string()))?;

    let mut db_file = fs::File::open(database_path)?;
    let mut buffer = Vec::new();
    db_file.read_to_end(&mut buffer)?;

    zip.write_all(&buffer)
        .map_err(|e| DbError::Other(e.to_string()))?;
    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_source_can_be_exported_without_mutation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source_path = directory.path().join("source.db");
        let archive_path = directory.path().join("backup.zip");
        let extracted_path = directory.path().join("extracted.db");

        let source = Database::open(&source_path).expect("create encrypted source");
        source
            .set_setting("export_probe", "preserved")
            .expect("seed source");
        source.close().expect("close encrypted source");

        export_existing_database_zip(&source_path, &archive_path)
            .expect("export source through a read-only attachment");

        let read_only =
            Database::open_existing_read_only(&source_path).expect("reopen source read-only");
        assert_eq!(
            read_only
                .get_setting("export_probe")
                .expect("read preserved source value"),
            Some("preserved".to_string())
        );

        let archive_file = fs::File::open(&archive_path).expect("open exported archive");
        let mut archive = zip::ZipArchive::new(archive_file).expect("read exported archive");
        let mut database_entry = archive.by_name("bir_data.db").expect("database entry");
        let mut extracted_file = fs::File::create(&extracted_path).expect("create extracted file");
        std::io::copy(&mut database_entry, &mut extracted_file).expect("extract database");

        let extracted = rusqlite::Connection::open(&extracted_path).expect("open plaintext export");
        let value: String = extracted
            .query_row(
                "SELECT value FROM settings WHERE key = 'export_probe'",
                [],
                |row| row.get(0),
            )
            .expect("read exported value");
        assert_eq!(value, "preserved");
    }

    // ---------- a profile export carries no credential and no row id ----------

    use crate::integration::providers::ProviderConfig;
    use crate::profile::{PROFILE_CREDENTIAL_KEYS, TaxpayerProfile};
    use std::collections::BTreeMap;

    const TIN: &str = "12345678800000";

    /// Distinctive values stored on the profile, in `PROFILE_CREDENTIAL_KEYS`
    /// order.
    const PROFILE_SECRETS: [&str; 5] = [
        "export-totp-K7Q2",
        "export-pass-W9X4",
        "export-access-M3R8",
        "export-refresh-T5V1",
        "export-pin-hash-H6N3",
    ];

    /// Distinctive values stored as a provider's credentials.
    const PROVIDER_SECRETS: [&str; 2] = ["export-provider-key-R2S6", "export-provider-token-V7N5"];

    /// Every entry of the archive at `path`, as (name, contents).
    fn archive_entries(path: &Path) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|index| {
                let mut entry = archive.by_index(index).unwrap();
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents).unwrap();
                (entry.name().to_string(), contents)
            })
            .collect()
    }

    fn contains(haystack: &[u8], needle: &str) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    }

    /// `TIN`'s profile as the export tests write it: credentials unset.
    fn export_sample() -> TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Export Sample Corp",
            "tin": { "segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000" },
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Sample Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "books@example.com",
            "default_form_type": "2551Q",
        }))
        .unwrap()
    }

    /// Both profile exports write profile.json without the five credential
    /// keys and without `id`, data_providers.json with each provider's
    /// provider_id and name only, and no stored secret in any entry's name or
    /// bytes. A failure names the entry and the key, never a value.
    #[test]
    fn a_profile_export_carries_no_credential_and_no_row_id() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let mut profile = export_sample();
        let [totp, password, access, refresh, _pin] =
            PROFILE_SECRETS.map(|secret| Some(secret.to_string()));
        profile.totp_secret = totp;
        profile.imap_app_password = password;
        profile.oauth_access_token = access;
        profile.oauth_refresh_token = refresh;
        // A profile-row PIN gates this export, so what is stored is the hash
        // of the PIN the call below passes.
        let pin = "0817";
        let stored_pin_hash = crate::crypto::hash_pin(pin);
        profile.profile_pin_hash = Some(stored_pin_hash.clone());
        db.save_profile(profile).unwrap();
        db.save_data_provider(ProviderConfig {
            id: None,
            profile_tin: TIN.into(),
            provider_id: "generic".into(),
            name: "Books".into(),
            credentials: BTreeMap::from([
                ("api_key".to_string(), PROVIDER_SECRETS[0].to_string()),
                ("token".to_string(), PROVIDER_SECRETS[1].to_string()),
            ]),
        })
        .unwrap();

        let single = directory.path().join("profile.zip");
        export_profile_data(&db, TIN, &single, Some(pin)).unwrap();
        let all = directory.path().join("all-profiles.zip");
        export_all_profiles_data(&db, &all).unwrap();

        for (archive, base) in [(single, String::new()), (all, format!("Profiles/{TIN}/"))] {
            let entries = archive_entries(&archive);
            for (name, contents) in &entries {
                for (key, secret) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(PROFILE_SECRETS) {
                    let stored = if key == "profile_pin_hash" {
                        stored_pin_hash.as_str()
                    } else {
                        secret
                    };
                    assert!(
                        !name.contains(stored) && !contains(contents, stored),
                        "{name} carries the stored {key}"
                    );
                }
                for secret in PROVIDER_SECRETS {
                    assert!(
                        !name.contains(secret) && !contains(contents, secret),
                        "{name} carries a stored provider credential"
                    );
                }
            }
            let entry = |file: &str| -> serde_json::Value {
                let name = format!("{base}{file}");
                let (_, contents) = entries
                    .iter()
                    .find(|(entry, _)| *entry == name)
                    .unwrap_or_else(|| panic!("{name} is in the archive"));
                serde_json::from_slice(contents).unwrap()
            };
            let profile = entry("profile.json");
            for key in PROFILE_CREDENTIAL_KEYS.into_iter().chain(["id"]) {
                assert!(
                    profile.get(key).is_none(),
                    "{base}profile.json has the {key} key"
                );
            }
            assert_eq!(profile["full_name"], "Export Sample Corp");
            assert!(
                entry("data_providers.json")
                    == serde_json::json!([{ "provider_id": "generic", "name": "Books" }]),
                "{base}data_providers.json carries more than each provider's provider_id and name"
            );
        }
    }

    // ---------- a taxpayer PIN gates the export, decided 2026-10-20 ----------

    /// `TIN`'s profile in a database whose taxpayer row carries the hash of
    /// `pin` (or no PIN at all): what `check_export_pin` reads.
    fn db_with_taxpayer_pin(pin: Option<&str>) -> Database {
        let db = Database::open_ephemeral().unwrap();
        db.save_profile(export_sample()).unwrap();
        if let Some(pin) = pin {
            db.set_taxpayer_pin_hash("123456788", Some(&crate::crypto::hash_pin(pin)))
                .unwrap();
        }
        db
    }

    /// No PIN given: refused as `PinRequired` before the file exists; a wrong
    /// PIN: `PinMismatch` the same way; the right PIN writes the same
    /// credential-free archive.
    #[test]
    fn exporting_a_pinned_taxpayers_profile_needs_the_pin() {
        let directory = tempfile::tempdir().unwrap();
        let db = db_with_taxpayer_pin(Some("0817"));

        let missing = directory.path().join("no-pin.zip");
        let refusal = export_profile_data(&db, TIN, &missing, None).unwrap_err();
        assert!(matches!(refusal, DbError::PinRequired), "{refusal}");
        assert_eq!(
            refusal.to_string(),
            "This profile's PIN is required to export it."
        );
        assert!(!missing.exists(), "a refused export left a file");

        let wrong = directory.path().join("wrong-pin.zip");
        let refusal = export_profile_data(&db, TIN, &wrong, Some("1234")).unwrap_err();
        assert!(matches!(refusal, DbError::PinMismatch), "{refusal}");
        assert_eq!(refusal.to_string(), "Wrong PIN.");
        assert!(!wrong.exists(), "a refused export left a file");

        let right = directory.path().join("profile.zip");
        export_profile_data(&db, TIN, &right, Some("0817")).unwrap();
        let entries = archive_entries(&right);
        let (_, contents) = entries
            .iter()
            .find(|(name, _)| name == "profile.json")
            .expect("profile.json is in the archive");
        let profile: serde_json::Value = serde_json::from_slice(contents).unwrap();
        for key in PROFILE_CREDENTIAL_KEYS.into_iter().chain(["id"]) {
            assert!(profile.get(key).is_none(), "profile.json has the {key} key");
        }
    }

    /// A legacy `profile_pin_hash` still on the profile row — the desktop
    /// profile manager's store, lifted onto the taxpayer only at the next
    /// launch — gates the export the same way, and never overrules a PIN the
    /// taxpayer row holds.
    #[test]
    fn a_profile_row_pin_gates_the_export_until_it_is_lifted() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open_ephemeral().unwrap();
        let mut profile = export_sample();
        profile.profile_pin_hash = Some(crate::crypto::hash_pin("6810"));
        db.save_profile(profile).unwrap();

        let missing = directory.path().join("no-pin.zip");
        let refusal = export_profile_data(&db, TIN, &missing, None).unwrap_err();
        assert!(matches!(refusal, DbError::PinRequired), "{refusal}");
        assert!(!missing.exists(), "a refused export left a file");

        export_profile_data(
            &db,
            TIN,
            &directory.path().join("profile.zip"),
            Some("6810"),
        )
        .unwrap();

        // Once the taxpayer holds its own PIN, it alone answers the gate.
        db.set_taxpayer_pin_hash("123456788", Some(&crate::crypto::hash_pin("0817")))
            .unwrap();
        let stale = directory.path().join("stale.zip");
        let refusal = export_profile_data(&db, TIN, &stale, Some("6810")).unwrap_err();
        assert!(matches!(refusal, DbError::PinMismatch), "{refusal}");
        assert!(!stale.exists(), "a refused export left a file");
        export_profile_data(&db, TIN, &directory.path().join("ok.zip"), Some("0817")).unwrap();
    }

    /// With no PIN on the taxpayer the export is ungated — and an
    /// authenticator alone is not a PIN, so the gate stays off for it.
    #[test]
    fn a_taxpayer_without_a_pin_exports_ungated() {
        let directory = tempfile::tempdir().unwrap();
        let db = db_with_taxpayer_pin(None);
        export_profile_data(&db, TIN, &directory.path().join("plain.zip"), None).unwrap();

        db.set_taxpayer_totp_secret("123456788", Some("JBSWY3DPEHPK3PXP"))
            .unwrap();
        export_profile_data(&db, TIN, &directory.path().join("totp.zip"), None).unwrap();
        export_profile_data(&db, TIN, &directory.path().join("totp2.zip"), Some("9999")).unwrap();
    }

    /// The administrator's delete-with-backup export writes the archive
    /// without the taxpayer's PIN.
    #[test]
    fn the_admin_backup_exports_a_pinned_profile_without_the_pin() {
        let directory = tempfile::tempdir().unwrap();
        let db = db_with_taxpayer_pin(Some("0817"));
        let backup = directory.path().join("backup.zip");
        export_profile_backup_as_admin(&db, TIN, &backup).unwrap();
        assert!(
            archive_entries(&backup)
                .iter()
                .any(|(name, _)| name == "profile.json"),
            "the admin backup wrote the profile"
        );
    }
}
