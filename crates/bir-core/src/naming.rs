//! File naming conventions for BIR files.

use serde::{Deserialize, Serialize};

/// Structured Philippine TIN (Tax Identification Number).
/// Format: XXX-XXX-XXX-XXX (3-3-3-3 digits)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tin {
    pub segment1: String,
    pub segment2: String,
    pub segment3: String,
    pub branch: String,
}

impl Tin {
    /// Full TIN as a single string (e.g., "010558054000")
    pub fn full(&self) -> String {
        format!(
            "{}{}{}{}",
            self.segment1, self.segment2, self.segment3, self.branch
        )
    }

    /// Formatted TIN with dashes (e.g., "010-558-054-000")
    pub fn formatted(&self) -> String {
        format!(
            "{}-{}-{}-{}",
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
    fn test_tin_full() {
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
    fn test_iaf_filename() {
        let tin = Tin {
            segment1: "010".into(),
            segment2: "558".into(),
            segment3: "054".into(),
            branch: "000".into(),
        };
        let name = iaf_filename(&tin, "2551Qv2018", "122026Q1", "test@mail.com");
        assert_eq!(name, "010558054000-2551Qv2018-122026Q1#test@mail.com#.xml");
    }
}
