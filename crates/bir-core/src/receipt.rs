use chrono::{NaiveDate, NaiveTime};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BirReceiptConfirmation {
    pub filename: String,
    pub date_received: NaiveDate,
    pub time_received: NaiveTime,
    pub source_from: Option<String>,
    pub raw_text: String,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ReceiptParseError {
    #[error("missing BIR receipt filename")]
    MissingFilename,
    #[error("missing BIR received date")]
    MissingDate,
    #[error("missing BIR received time")]
    MissingTime,
}

pub fn parse_bir_receipt_email(raw: &str) -> Result<BirReceiptConfirmation, ReceiptParseError> {
    static FILENAME_RE: OnceLock<Regex> = OnceLock::new();
    static DATE_RE: OnceLock<Regex> = OnceLock::new();
    static TIME_RE: OnceLock<Regex> = OnceLock::new();
    static FROM_RE: OnceLock<Regex> = OnceLock::new();

    let filename_re = FILENAME_RE.get_or_init(|| {
        Regex::new(r"(?im)^\s*File name:\s*([^\s]+\.xml)\s*$").expect("valid filename regex")
    });
    let date_re = DATE_RE.get_or_init(|| {
        Regex::new(r"(?im)^\s*Date received by BIR:\s*(\d{1,2}\s+[A-Za-z]+\s+\d{4})\s*$")
            .expect("valid date regex")
    });
    let time_re = TIME_RE.get_or_init(|| {
        Regex::new(r"(?im)^\s*Time received by BIR:\s*(\d{1,2}:\d{2}\s*[AP]M)\s*$")
            .expect("valid time regex")
    });
    let from_re = FROM_RE.get_or_init(|| {
        Regex::new(r"(?im)^\s*from:\s*([^\s]+@[^\s]+)\s*$|^\s*(ebirforms-noreply@bir\.gov\.ph)\s*$")
            .expect("valid from regex")
    });

    let filename = filename_re
        .captures(raw)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
        .ok_or(ReceiptParseError::MissingFilename)?;

    let date_str = date_re
        .captures(raw)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
        .ok_or(ReceiptParseError::MissingDate)?;
    let date_received = NaiveDate::parse_from_str(&date_str, "%d %B %Y")
        .or_else(|_| NaiveDate::parse_from_str(&date_str, "%e %B %Y"))
        .map_err(|_| ReceiptParseError::MissingDate)?;

    let time_str = time_re
        .captures(raw)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_uppercase())
        .ok_or(ReceiptParseError::MissingTime)?;
    let time_received = NaiveTime::parse_from_str(&time_str, "%I:%M %p")
        .map_err(|_| ReceiptParseError::MissingTime)?;

    let source_from = from_re.captures(raw).and_then(|cap| {
        cap.get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str().trim().to_string())
    });

    Ok(BirReceiptConfirmation {
        filename,
        date_received,
        time_received,
        source_from,
        raw_text: raw.to_string(),
    })
}

pub fn split_bir_filename(filename: &str) -> Option<(String, String, String)> {
    let stem = filename.strip_suffix(".xml").unwrap_or(filename);
    let mut parts = stem.splitn(3, '-');
    let tin = parts.next()?.to_string();
    let form = parts.next()?.to_string();
    let period = parts.next()?.to_string();
    Some((tin, form, period))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_confirmation_email() {
        let raw = r#"ebirforms-noreply@bir.gov.ph

This confirms receipt of your submission with the following details subject to validation by BIR:

File name: 779025068000-2551Qv2018-122026Q1.xml
Date received by BIR: 24 April 2026
Time received by BIR: 05:18 AM
"#;
        let receipt = parse_bir_receipt_email(raw).unwrap();
        assert_eq!(receipt.filename, "779025068000-2551Qv2018-122026Q1.xml");
        assert_eq!(receipt.date_received.to_string(), "2026-04-24");
        assert_eq!(receipt.time_received.format("%H:%M").to_string(), "05:18");
        assert_eq!(
            split_bir_filename(&receipt.filename),
            Some((
                "779025068000".to_string(),
                "2551Qv2018".to_string(),
                "122026Q1".to_string()
            ))
        );
    }
}
