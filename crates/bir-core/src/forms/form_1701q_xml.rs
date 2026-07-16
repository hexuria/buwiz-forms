//! Fail-closed transport boundary for exact form `1701Qv2018`.
//!
//! The reviewed source pack contains the official January 2018 PDF but no
//! exact-revision saved XML. The available `BIRForm1701QScript.js` describes an
//! older Items 26-41 layout and therefore cannot establish this revision's
//! field names, option indexes, final flag, or round-trip behavior.

use std::collections::BTreeMap;

use super::form_1701q::Form1701QDraft;

pub const XML_UNSUPPORTED_REASON: &str =
    "1701Qv2018 XML is unavailable: no exact-revision saved XML has been reviewed";

impl Form1701QDraft {
    /// Returns no transport fields. Callers that need export must use the
    /// checked API and handle its explicit unsupported result.
    pub fn to_bir_field_map(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    pub fn to_bir_field_map_checked(&self) -> Result<BTreeMap<String, String>, String> {
        Err(XML_UNSUPPORTED_REASON.to_string())
    }

    pub fn to_bir_xml_payload(&self) -> Result<String, String> {
        Err(XML_UNSUPPORTED_REASON.to_string())
    }

    pub fn from_bir_xml_payload(_xml: &str) -> Result<Self, String> {
        Err(XML_UNSUPPORTED_REASON.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_revision_field_map_should_be_empty_without_source_evidence() {
        let draft = Form1701QDraft::default();

        assert!(draft.to_bir_field_map().is_empty());
    }

    #[test]
    fn xml_export_should_fail_closed() {
        let draft = Form1701QDraft::default();

        let error = draft
            .to_bir_xml_payload()
            .expect_err("XML export must remain disabled");

        assert!(error.contains("no exact-revision saved XML"));
    }

    #[test]
    fn xml_import_should_fail_closed() {
        let error = Form1701QDraft::from_bir_xml_payload("<xml />")
            .expect_err("XML import must remain disabled");

        assert!(error.contains("1701Qv2018 XML is unavailable"));
    }
}
