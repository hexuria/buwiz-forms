use crate::bir_xml::{generate_bir_xml, parse_bir_xml};
use crate::crypto::{BIR_IAF_PASSPHRASE, compress_and_encrypt, decrypt_and_decompress};
use crate::transport::submit_iaf;
use anyhow::{Result, anyhow};
use chrono::Local;
use std::fs;
use std::path::Path;

pub struct OfficialSavefile {
    pub tin: String,
    pub form_type: String,
    pub period_code: String,
    pub email: String,
    pub year: u16,
    pub quarter: Option<u8>,
    pub month: Option<u8>,
}

pub async fn import_and_submit_savefile(
    file_path: &Path,
    fallback_email: Option<&str>,
) -> Result<OfficialSavefile> {
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid file name"))?;

    // Example: 010558054000-2551Qv2018-122026Q1.xml
    let parts: Vec<&str> = file_name.trim_end_matches(".xml").split('-').collect();
    if parts.len() < 3 {
        return Err(anyhow!(
            "Filename does not match expected format TIN-FORM-PERIOD.xml"
        ));
    }

    let tin = parts[0].to_string();
    let form_type = parts[1].to_string();
    let period_code = parts[2..].join("-");

    let ciphertext = fs::read(file_path)?;
    let plaintext_bytes = decrypt_and_decompress(&ciphertext, BIR_IAF_PASSPHRASE)?;
    let plaintext_str = String::from_utf8_lossy(&plaintext_bytes);

    let mut fields = parse_bir_xml(&plaintext_str);

    // Extract or fallback email
    let email = if let Some(e) = fields.get("txtEmail").filter(|s| !s.is_empty()) {
        e.clone()
    } else if let Some(fb) = fallback_email {
        fb.to_string()
    } else {
        return Err(anyhow!(
            "No email found in savefile and no fallback provided"
        ));
    };

    // Inject dynamic date (Heartbeat)
    let now = Local::now();
    let dynamic_date = now.format("%m/%d/%Y %H:%M:%S").to_string();
    fields.insert("txtDateIssue".to_string(), dynamic_date);

    // Ensure email is in fields
    fields.insert("txtEmail".to_string(), email.clone());

    let new_xml = generate_bir_xml(&fields);
    let encrypted = compress_and_encrypt(new_xml.as_bytes(), BIR_IAF_PASSPHRASE)?;

    let submit_filename = format!("{}-{}-{}#{}#.xml", tin, form_type, period_code, email);

    // Transmit to Remote Gateway
    submit_iaf(&form_type, &submit_filename, &encrypted).await?;

    let (year, quarter, month) = parse_period_code(&period_code);

    Ok(OfficialSavefile {
        tin,
        form_type,
        period_code,
        email,
        year,
        quarter,
        month,
    })
}

// Helper to extract year/quarter/month from period code
pub fn parse_period_code(period_code: &str) -> (u16, Option<u8>, Option<u8>) {
    // 122026Q1 -> year 2026, month 12, quarter 1
    // 122026 -> year 2026, month 12
    if period_code.len() >= 6 {
        let month_str = &period_code[0..2];
        let year_str = &period_code[2..6];

        let month: Option<u8> = month_str.parse().ok();
        let year: u16 = year_str.parse().unwrap_or(
            chrono::Local::now()
                .format("%Y")
                .to_string()
                .parse()
                .unwrap_or(2024),
        );

        let mut quarter = None;
        if period_code.len() > 6 && period_code.contains('Q') {
            let q_part = period_code.split('Q').next_back().unwrap_or("");
            quarter = q_part.parse().ok();
        }

        (year, quarter, month)
    } else {
        (
            chrono::Local::now()
                .format("%Y")
                .to_string()
                .parse()
                .unwrap_or(2024),
            None,
            None,
        )
    }
}
