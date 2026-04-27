use crate::db::{Database, DbError};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

pub fn export_profile_data(db: &Database, tin: &str, export_file: &Path) -> Result<(), DbError> {
    let file = fs::File::create(export_file)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Profile JSON
    if let Some(profile) = db.get_profile_by_tin(tin)? {
        let profile_json = serde_json::to_string_pretty(&profile)?;
        zip.start_file("profile.json", options)?;
        zip.write_all(profile_json.as_bytes())?;
    }

    // 2. Submissions JSON
    let submissions = db.list_submissions_for_tin(tin)?;
    let submissions_json = serde_json::to_string_pretty(&submissions)?;
    zip.start_file("submissions.json", options)?;
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
    for draft in drafts_iter {
        if let Ok(d) = draft {
            drafts.push(d);
        }
    }
    let drafts_json = serde_json::to_string_pretty(&drafts)?;
    zip.start_file("drafts.json", options)?;
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
        zip.start_file(format!("Receipts/{}.txt", filename), options)?;
        zip.write_all(raw_text.as_bytes())?;
        if let Some(html) = raw_html {
            zip.start_file(format!("Receipts/{}.html", filename), options)?;
            zip.write_all(html.as_bytes())?;
        }
    }

    let metadata_json = serde_json::to_string_pretty(&receipts_metadata)?;
    zip.start_file("receipts_manifest.json", options)?;
    zip.write_all(metadata_json.as_bytes())?;
    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}

pub fn export_database_zip(db_path: &Path, zip_path: &Path) -> Result<(), DbError> {
    let mut file = fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(&mut file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("bir_data.db", options)
        .map_err(|e| DbError::Other(e.to_string()))?;

    let mut db_file = fs::File::open(db_path)?;
    let mut buffer = Vec::new();
    db_file.read_to_end(&mut buffer)?;

    zip.write_all(&buffer)
        .map_err(|e| DbError::Other(e.to_string()))?;
    zip.finish().map_err(|e| DbError::Other(e.to_string()))?;

    Ok(())
}
