use crate::naming::Tin;
use crate::profile::TaxpayerProfile;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

pub fn validate_required(field: &'static str, label: &str, value: &str) -> Option<ValidationError> {
    if value.trim().is_empty() {
        Some(ValidationError::new(field, format!("{label} is required")))
    } else {
        None
    }
}

pub fn validate_zip(zip: &str) -> bool {
    zip.len() == 4 && zip.chars().all(|c| c.is_ascii_digit())
}

pub fn validate_email(email: &str) -> bool {
    static EMAIL_RE: OnceLock<Regex> = OnceLock::new();
    EMAIL_RE
        .get_or_init(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").expect("valid email regex"))
        .is_match(email.trim())
}

pub fn validate_ph_phone(phone: &str) -> bool {
    let compact: String = phone
        .trim()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    static MOBILE_RE: OnceLock<Regex> = OnceLock::new();
    static LANDLINE_RE: OnceLock<Regex> = OnceLock::new();

    let mobile = MOBILE_RE.get_or_init(|| {
        Regex::new(r"^(09\d{9}|\+639\d{9}|639\d{9})$").expect("valid mobile regex")
    });
    let landline = LANDLINE_RE.get_or_init(|| {
        Regex::new(r"^((02|\+632|632)?\d{8}|0[3-8]\d{1,2}\d{7}|\+63[3-8]\d{1,2}\d{7}|63[3-8]\d{1,2}\d{7})$")
            .expect("valid landline regex")
    });

    mobile.is_match(&compact) || landline.is_match(&compact)
}

/// Validate TIN allowing both legacy 12-digit and new 14-digit formats.
/// Use this for loading/importing existing data.
pub fn validate_tin(tin: &Tin) -> bool {
    let full = tin.full();
    (full.len() == 12 || full.len() == 13 || full.len() == 14)
        && full.chars().all(|c| c.is_ascii_digit())
}

/// Validate TIN strictly as the new 14-digit format (3-3-3-5).
/// Use this for new filings on the latest eBIRForms.
pub fn validate_tin_14(tin: &Tin) -> bool {
    let full = tin.full();
    full.len() == 14
        && full.chars().all(|c| c.is_ascii_digit())
        && tin.segment1.len() == 3
        && tin.segment2.len() == 3
        && tin.segment3.len() == 3
        && tin.branch.len() == 5
}

pub fn validate_profile(profile: &TaxpayerProfile) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if !validate_tin(&profile.tin) {
        errors.push(ValidationError::new(
            "tin",
            "TIN must have 12 to 14 digits including branch code",
        ));
    }

    for err in [
        validate_required("rdo_code", "RDO", &profile.rdo_code),
        validate_required(
            "line_of_business",
            "Line of business",
            &profile.line_of_business,
        ),
        validate_required("full_name", "Taxpayer name", &profile.full_name),
        validate_required(
            "registered_address",
            "Registered address",
            &profile.registered_address,
        ),
        validate_required("zip_code", "ZIP code", &profile.zip_code),
        validate_required("phone", "Phone number", &profile.phone),
        validate_required("email", "Email", &profile.email),
    ]
    .into_iter()
    .flatten()
    {
        errors.push(err);
    }

    if !profile.zip_code.trim().is_empty() && !validate_zip(profile.zip_code.trim()) {
        errors.push(ValidationError::new(
            "zip_code",
            "ZIP code must be 4 digits",
        ));
    }

    if !profile.phone.trim().is_empty() && !validate_ph_phone(&profile.phone) {
        errors.push(ValidationError::new(
            "phone",
            "Phone must be a valid Philippine mobile or landline number",
        ));
    }

    if !profile.email.trim().is_empty() && !validate_email(&profile.email) {
        errors.push(ValidationError::new("email", "Email address is invalid"));
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_zip_and_phone_patterns() {
        assert!(validate_zip("2200"));
        assert!(!validate_zip("22000"));
        assert!(validate_ph_phone("09156837000"));
        assert!(validate_ph_phone("+639156837000"));
        assert!(validate_ph_phone("02 8123 4567"));
        assert!(validate_ph_phone("(032) 123 4567"));
        assert!(!validate_ph_phone("123"));
    }
}
