//! Legacy schema-backed form-data prototype.
//!
//! Production typed forms live under `crate::forms`. The reviewed rule runtime
//! lives in `bir-rules`; this compatibility module must not become a second
//! validation implementation.

use crate::schema::{FieldType, FormSchema};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum FormError {
    #[error("Required field '{0}' is missing")]
    MissingRequired(String),
    #[error("Field '{field}' failed validation: {reason}")]
    ValidationFailed { field: String, reason: String },
    #[error("Schema not found: {0}")]
    SchemaNotFound(String),
}

/// Runtime form data: a thin wrapper around BTreeMap<String, String>
/// that bridges schema definitions to the BIR XML payload.
#[derive(Debug, Clone, PartialEq)]
pub struct FormData {
    pub schema_id: String,
    pub fields: BTreeMap<String, String>,
}

impl FormData {
    pub fn new(schema_id: &str) -> Self {
        Self {
            schema_id: schema_id.to_string(),
            fields: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(|s| s.as_str())
    }

    /// Validate all fields against the schema.
    pub fn validate(&self, schema: &FormSchema) -> Result<(), Vec<FormError>> {
        let mut errors = Vec::new();
        for page in &schema.pages {
            for section in &page.sections {
                for field in &section.fields {
                    let val = self.get(&field.name).unwrap_or("");
                    if field.required && val.is_empty() {
                        errors.push(FormError::MissingRequired(field.name.clone()));
                        continue;
                    }
                    if !val.is_empty()
                        && let FieldType::Text {
                            max_length,
                            pattern,
                        } = &field.field_type
                    {
                        if let Some(max) = max_length
                            && val.len() > *max
                        {
                            errors.push(FormError::ValidationFailed {
                                field: field.name.clone(),
                                reason: format!("exceeds max length of {}", max),
                            });
                        }
                        if let Some(pat) = pattern
                            && let Ok(regex) = regex::Regex::new(pat)
                            && !regex.is_match(val)
                        {
                            errors.push(FormError::ValidationFailed {
                                field: field.name.clone(),
                                reason: format!("does not match pattern {}", pat),
                            });
                        }
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Convert to BIR pseudo-XML payload.
    pub fn to_bir_xml(&self) -> String {
        crate::bir_xml::generate_bir_xml(&self.fields)
    }

    /// Load from BIR pseudo-XML payload.
    pub fn from_bir_xml(
        schema_id: &str,
        xml: &str,
    ) -> Result<Self, crate::bir_xml::BirXmlParseError> {
        Ok(Self {
            schema_id: schema_id.to_string(),
            fields: crate::bir_xml::parse_bir_xml_checked(xml)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_data_roundtrip() {
        let mut form = FormData::new("2551Qv2018");
        form.set("frm:txtName", "JOHN DOE");
        let xml = form.to_bir_xml();
        let parsed = FormData::from_bir_xml("2551Qv2018", &xml).expect("generated payload parses");
        assert_eq!(parsed.get("frm:txtName"), Some("JOHN DOE"));
    }

    #[test]
    fn malformed_form_data_payload_fails_closed() {
        assert!(FormData::from_bir_xml("2551Qv2018", "<div>broken</div>").is_err());
    }
}
