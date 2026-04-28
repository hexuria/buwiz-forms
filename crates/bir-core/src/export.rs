use crate::db::{Database, DbError};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

pub fn export_profile_data(db: &Database, tin: &str, export_file: &Path) -> Result<(), DbError> {
    let file = fs::File::create(export_file)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    write_profile_to_zip(&mut zip, db, tin, "", options)?;

    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}

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

fn write_profile_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    db: &Database,
    tin: &str,
    base_dir: &str,
    options: SimpleFileOptions,
) -> Result<(), DbError> {
    // 1. Profile JSON
    if let Some(profile) = db.get_profile_by_tin(tin)? {
        let profile_json = serde_json::to_string_pretty(&profile)?;
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

    Ok(())
}

pub fn export_database_zip(db: &Database, zip_path: &Path) -> Result<(), DbError> {
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_db_path = temp_dir.join(format!("bir_unencrypted_{}.db", timestamp));

    // Export current encrypted DB to a temporary unencrypted DB
    db.conn.execute(
        "ATTACH DATABASE ?1 AS plaintext KEY '';",
        rusqlite::params![temp_db_path.to_str().unwrap()],
    )?;
    db.conn
        .execute_batch("SELECT sqlcipher_export('plaintext');")?;
    db.conn.execute("DETACH DATABASE plaintext;", [])?;

    // Zip the unencrypted database
    let mut file = fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(&mut file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("bir_data.db", options)
        .map_err(|e| DbError::Other(e.to_string()))?;

    let mut db_file = fs::File::open(&temp_db_path)?;
    let mut buffer = Vec::new();
    db_file.read_to_end(&mut buffer)?;

    zip.write_all(&buffer)
        .map_err(|e| DbError::Other(e.to_string()))?;
    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    // Clean up temporary file
    let _ = fs::remove_file(&temp_db_path);

    Ok(())
}
