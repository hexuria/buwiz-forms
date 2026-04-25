//! BIR pseudo-XML parser and generator.
//!
//! The BIR application uses a non-standard XML-like format:
//! ```xml
//! <?xml version='1.0'?>
//!     <div>key=url_encoded_valuekey=</div>
//! ```
//!
//! Values are URL-encoded. Keys appear twice as delimiters.

use std::collections::BTreeMap;

/// Parse BIR pseudo-XML content into a key-value map.
pub fn parse_bir_xml(content: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<div>") && trimmed.ends_with("</div>") {
            let inner = &trimmed[5..trimmed.len() - 6];
            if let Some(first_eq) = inner.find('=') {
                let key = &inner[..first_eq];
                let last_eq = inner.rfind('=').unwrap();
                if last_eq > first_eq {
                    let value_with_key = &inner[first_eq + 1..last_eq];
                    if let Some(value) = value_with_key.strip_suffix(key) {
                        fields.insert(
                            key.to_string(),
                            urlencoding::decode(value)
                                .unwrap_or(std::borrow::Cow::Borrowed(value))
                                .into_owned(),
                        );
                    } else {
                        fields.insert(
                            key.to_string(),
                            urlencoding::decode(value_with_key)
                                .unwrap_or(std::borrow::Cow::Borrowed(value_with_key))
                                .into_owned(),
                        );
                    }
                }
            }
        }
    }
    fields
}

/// Generate BIR pseudo-XML from a key-value map.
pub fn generate_bir_xml(fields: &BTreeMap<String, String>) -> String {
    let mut output = String::from("<?xml version='1.0'?>\t\n");
    for (key, value) in fields {
        let encoded_value = urlencoding::encode(value);
        output.push_str(&format!(
            "            <div>{}={}{}=</div>\t\n",
            key, encoded_value, key
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let mut fields = BTreeMap::new();
        fields.insert("frm:txtName".to_string(), "JOHN DOE".to_string());
        fields.insert("frm:txtTIN1".to_string(), "010".to_string());

        let xml = generate_bir_xml(&fields);
        let parsed = parse_bir_xml(&xml);

        assert_eq!(parsed.get("frm:txtName").unwrap(), "JOHN DOE");
        assert_eq!(parsed.get("frm:txtTIN1").unwrap(), "010");
    }

    #[test]
    fn test_url_encoding() {
        let mut fields = BTreeMap::new();
        fields.insert("fn".to_string(), "GALANG, ANDREA MAE".to_string());

        let xml = generate_bir_xml(&fields);
        assert!(xml.contains("GALANG%2C%20ANDREA%20MAE"));

        let parsed = parse_bir_xml(&xml);
        assert_eq!(parsed.get("fn").unwrap(), "GALANG, ANDREA MAE");
    }
}
