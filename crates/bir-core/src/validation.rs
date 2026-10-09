use crate::naming::Tin;
use crate::profile::{ComplianceSourceMode, TaxProfileVersionStatus, TaxpayerProfile};
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

/// Oldest taxable year the January-2018-and-later eBIRForms accept. The
/// official forms alert "Please file using the old version of the form."
pub const OFFICIAL_MIN_FORM_YEAR: u16 = 2018;

/// Revenue District Office codes accepted by the official eBIRForms RDO
/// dropdown, extracted verbatim from the official `xml/rdo.xml` (138 entries,
/// v7.9.5.0). "000" is the blank placeholder and is intentionally absent.
pub const RDO_CODES: &[&str] = &[
    "001", "002", "003", "004", "005", "006", "007", "008", "009", "010", "011", "012", "013",
    "014", "015", "016", "018", "019", "020", "022", "024", "026", "027", "028", "029", "030",
    "031", "032", "033", "034", "035", "036", "037", "038", "039", "040", "041", "042", "043",
    "044", "045", "046", "047", "048", "049", "050", "051", "052", "055", "056", "057", "058",
    "059", "060", "061", "062", "063", "064", "065", "066", "067", "068", "069", "070", "071",
    "072", "073", "074", "075", "076", "077", "078", "079", "080", "081", "082", "083", "084",
    "085", "086", "087", "088", "089", "090", "091", "092", "094", "095", "096", "097", "098",
    "099", "100", "101", "102", "103", "104", "105", "106", "107", "108", "109", "110", "111",
    "112", "113", "114", "115", "116", "117", "118", "119", "120", "121", "122", "123", "124",
    "125", "126", "127", "132", "17A", "17B", "21A", "21B", "21C", "23A", "23B", "25A", "25B",
    "43A", "43B", "53A", "53B", "54A", "54B", "93A", "93B",
];

/// Official RDO dropdown domain: a real RDO code, never the "000" placeholder.
pub fn valid_rdo_code(code: &str) -> bool {
    RDO_CODES.contains(&code.trim())
}

/// Dev-mode escape hatch: set `EBIR_RELAXED_VALIDATION=1` to skip checks that
/// gate against reference data BIR ultimately enforces server-side (strict
/// TIN composition, RDO-code membership). Format and submission rules the
/// official form validates client-side still apply. Intended for development
/// and test filings only.
pub fn relaxed_dev_mode() -> bool {
    relaxed_dev_mode_enabled(std::env::var("EBIR_RELAXED_VALIDATION").ok().as_deref())
}

fn relaxed_dev_mode_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// Maps a tax-relief specification to the option code the official form
/// serializes: "1" (Special Rate), "2" (International Tax Treaty), and — on
/// 1601C only — "3" (Both). Accepts the codes verbatim plus the labels users
/// typed when the field was free-text.
pub fn official_tax_relief_code(spec: &str, allow_both: bool) -> Option<String> {
    let normalized = spec.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "special rate" | "special" => Some("1".to_string()),
        "2" | "international tax treaty" | "tax treaty" | "international treaty" => {
            Some("2".to_string())
        }
        "3" | "both" if allow_both => Some("3".to_string()),
        _ => None,
    }
}

/// Official field-length limits used by the eBIRForms `maxlength` attributes.
/// Reaching BIR with an over-long value truncates or corrupts the wire file, so
/// we reject it instead.
pub fn fits_official_maxlength(value: &str, max_chars: usize) -> bool {
    value.trim().chars().count() <= max_chars
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

    let mut confirmed_versions: Vec<_> = profile
        .profile_versions
        .iter()
        .filter(|version| version.status == TaxProfileVersionStatus::Confirmed)
        .collect();

    if profile.compliance_source_mode == ComplianceSourceMode::CorVersioned
        && confirmed_versions.is_empty()
    {
        errors.push(ValidationError::new(
            "profile_versions",
            "COR-managed compliance requires at least one confirmed profile version",
        ));
    }

    for version in &confirmed_versions {
        if version.effective_from.is_none() {
            errors.push(ValidationError::new(
                "profile_versions",
                format!(
                    "Confirmed profile version '{}' must have an effective start date",
                    version.label
                ),
            ));
        }
        if let (Some(effective_from), Some(effective_until)) =
            (version.effective_from, version.effective_until)
            && effective_until < effective_from
        {
            errors.push(ValidationError::new(
                "profile_versions",
                format!(
                    "Confirmed profile version '{}' has an effective end date before its start date",
                    version.label
                ),
            ));
        }
    }

    for version in &profile.profile_versions {
        for override_rule in &version.obligation_overrides {
            if override_rule.form_code.trim().is_empty() {
                errors.push(ValidationError::new(
                    "profile_versions",
                    format!(
                        "Profile version '{}' has an obligation override without a form code",
                        version.label
                    ),
                ));
            }
            if override_rule.reason.trim().is_empty()
                || override_rule
                    .source_reference
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                errors.push(ValidationError::new(
                    "profile_versions",
                    format!(
                        "Profile obligation override '{}' on '{}' requires a reason and source",
                        override_rule.form_code, version.label
                    ),
                ));
            }
        }

        for override_rule in &version.deadline_overrides {
            if override_rule.title.trim().is_empty()
                || override_rule.source_reference.trim().is_empty()
                || override_rule.affected_form_codes.is_empty()
            {
                errors.push(ValidationError::new(
                    "profile_versions",
                    format!(
                        "Profile deadline override on '{}' requires a title, source, and form code",
                        version.label
                    ),
                ));
            }
        }
    }

    confirmed_versions.sort_by(|a, b| {
        a.effective_from
            .cmp(&b.effective_from)
            .then(a.id.cmp(&b.id))
    });
    for pair in confirmed_versions.windows(2) {
        let previous = pair[0];
        let next = pair[1];
        let overlaps = match (previous.effective_until, next.effective_from) {
            (Some(previous_end), Some(next_start)) => previous_end >= next_start,
            _ => true,
        };

        if overlaps {
            errors.push(ValidationError::new(
                "profile_versions",
                format!(
                    "Confirmed profile versions '{}' and '{}' overlap",
                    previous.label, next.label
                ),
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdo_code_list_matches_official_dropdown_domain() {
        assert!(valid_rdo_code("001"));
        assert!(valid_rdo_code("018"));
        assert!(valid_rdo_code("93B"));
        assert!(!valid_rdo_code("18"));
        assert!(!valid_rdo_code("000"));
        assert!(!valid_rdo_code("999"));
        assert!(!valid_rdo_code("abc"));
        assert!(!valid_rdo_code(""));
        assert_eq!(RDO_CODES.len(), 138);
    }

    #[test]
    fn relaxed_dev_mode_parses_only_truthy_values() {
        for v in ["1", "true", "yes", "on", "TRUE", " 1 "] {
            assert!(relaxed_dev_mode_enabled(Some(v)), "{v}");
        }
        for v in ["0", "false", "no", "off", "", "enabled"] {
            assert!(!relaxed_dev_mode_enabled(Some(v)), "{v}");
        }
        assert!(!relaxed_dev_mode_enabled(None));
    }

    #[test]
    fn tax_relief_spec_maps_labels_and_codes_to_official_options() {
        assert_eq!(official_tax_relief_code("1", true).as_deref(), Some("1"));
        assert_eq!(
            official_tax_relief_code("Special Rate", false).as_deref(),
            Some("1")
        );
        assert_eq!(
            official_tax_relief_code("International Tax Treaty", true).as_deref(),
            Some("2")
        );
        assert_eq!(official_tax_relief_code("both", true).as_deref(), Some("3"));
        // 2551Q's dropdown has no "Both" option.
        assert_eq!(official_tax_relief_code("3", false), None);
        assert_eq!(official_tax_relief_code("Special Law 123", true), None);
        assert_eq!(official_tax_relief_code("", true), None);
    }

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
