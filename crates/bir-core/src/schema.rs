//! Form schema engine — JSON-driven form definitions.
//!
//! Each BIR form is defined as a JSON schema with fields, types,
//! validation rules, and layout information. 

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSchema {
    /// Form identifier (e.g., "2551Qv2018")
    pub id: String,
    /// Human-readable title
    pub title: String,
    /// Description or instructions
    pub description: Option<String>,
    /// Array of pages in the form
    pub pages: Vec<FormPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormPage {
    /// Page title (e.g., "Part I - Background Information")
    pub title: String,
    /// Sections within the page
    pub sections: Vec<FormSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSection {
    /// Section title
    pub title: String,
    /// Fields in this section
    pub fields: Vec<FormField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FieldType {
    /// Standard text input
    Text { 
        max_length: Option<usize>,
        pattern: Option<String>,
    },
    /// Numeric input (amounts)
    Currency,
    /// Radio button group
    Radio { 
        options: Vec<FieldOption> 
    },
    /// Checkbox boolean
    Checkbox,
    /// TIN segmented input
    Tin,
    /// RDO selector dropdown
    RdoSelect,
    /// ATC selector dropdown
    AtcSelect,
    /// Date picker
    Date,
    /// Read-only computed field
    Computed {
        /// Expression to evaluate (e.g., "field_12 + field_13")
        formula: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// The XML key name for the BIR payload (e.g., "frm:txtTIN1")
    pub name: String,
    /// The label displayed in the UI
    pub label: String,
    /// Field type and constraints
    pub field_type: FieldType,
    /// Whether the field is mandatory
    pub required: bool,
    /// The default value, if any
    pub default_value: Option<String>,
    /// Help text or tooltip
    pub help_text: Option<String>,
}

impl FormSchema {
    /// Parses a JSON schema string into a `FormSchema`
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_deserialization() {
        let json = r#"{
            "id": "2551Qv2018",
            "title": "Quarterly Percentage Tax Return",
            "pages": [{
                "title": "Part I",
                "sections": [{
                    "title": "Background",
                    "fields": [
                        {
                            "name": "frm:txtTIN1",
                            "label": "Taxpayer Identification Number",
                            "field_type": { "type": "Tin" },
                            "required": true
                        }
                    ]
                }]
            }]
        }"#;

        let schema = FormSchema::from_json(json).expect("Failed to parse schema");
        assert_eq!(schema.id, "2551Qv2018");
        assert_eq!(schema.pages[0].sections[0].fields[0].name, "frm:txtTIN1");
    }
}
