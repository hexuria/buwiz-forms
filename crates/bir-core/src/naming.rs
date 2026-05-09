//! File naming conventions for BIR files.

use serde::{Deserialize, Serialize};

/// Structured Philippine TIN (Tax Identification Number).
/// Format: XXX-XXX-XXX-XXXXX (3-3-3-5 digits)
///
/// The latest eBIRForms uses a 14-digit format where the branch code
/// is 5 digits. Older 12-digit TINs (3-digit branch) are still accepted
/// for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tin {
    pub segment1: String,
    pub segment2: String,
    pub segment3: String,
    pub branch: String,
}

impl Tin {
    /// Full TIN as a single string (e.g., "01055805400000")
    pub fn full(&self) -> String {
        format!(
            "{}{}{}{:0>5}",
            self.segment1, self.segment2, self.segment3, self.branch
        )
    }

    /// Formatted TIN with dashes (e.g., "010-558-054-00000")
    pub fn formatted(&self) -> String {
        format!(
            "{}-{}-{}-{:0>5}",
            self.segment1, self.segment2, self.segment3, self.branch
        )
    }
}

/// Generate savefile name: {TIN}-{FormType}-{Period}.xml
pub fn savefile_name(tin: &Tin, form_type: &str, period: &str) -> String {
    format!("{}-{}-{}.xml", tin.full(), form_type, period)
}

/// Generate IAF filename: {TIN}-{FormType}-{Period}#{email}#.xml
pub fn iaf_filename(tin: &Tin, form_type: &str, period: &str, email: &str) -> String {
    format!("{}-{}-{}#{}#.xml", tin.full(), form_type, period, email)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tin_full_legacy_12_digit() {
        let tin = Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        };
        assert_eq!(tin.full(), "010558054000");
        assert_eq!(tin.formatted(), "010-558-054-000");
    }

    #[test]
    fn test_tin_full_new_14_digit() {
        let tin = Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "00000".into(),
        };
        assert_eq!(tin.full(), "01055805400000");
        assert_eq!(tin.formatted(), "010-558-054-00000");
        assert_eq!(tin.full().len(), 14);
    }

    #[test]
    fn test_iaf_filename() {
        let tin = Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "00000".into(),
        };
        let name = iaf_filename(&tin, "2551Qv2018", "122026Q1", "test@mail.com");
        assert_eq!(
            name,
            "01055805400000-2551Qv2018-122026Q1#test@mail.com#.xml"
        );
    }
}
