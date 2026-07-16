//! BIR pseudo-XML parser and generator.
//!
//! The BIR application uses a non-standard XML-like format:
//! ```xml
//! <?xml version='1.0'?>
//!     <div>key=url_encoded_valuekey=</div>
//! ```
//!
//! Values are URL-encoded. Keys appear twice as delimiters. Official payloads
//! are not guaranteed to contain line breaks, so parsing is tag-oriented.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BirXmlParseError {
    #[error("BIR payload contains no field divs")]
    EmptyPayload,
    #[error("malformed BIR div at byte {offset}: {reason}")]
    MalformedDiv { offset: usize, reason: String },
    #[error("BIR div at byte {offset} has no field ID")]
    MissingFieldId { offset: usize },
    #[error(
        "BIR div at byte {offset} declares field ID {attribute_id:?} but its body uses {body_id:?}"
    )]
    MismatchedFieldId {
        offset: usize,
        attribute_id: String,
        body_id: String,
    },
    #[error("BIR payload contains duplicate field ID {field_id:?}")]
    DuplicateFieldId { field_id: String },
    #[error("BIR field {field_id:?} has invalid URL encoding")]
    InvalidEncoding { field_id: String },
}

/// Parse every BIR field div and reject malformed, duplicate, or unidentified
/// fields. Text outside the divs is retained only as envelope noise; official
/// payloads commonly contain an XML declaration and a trailing copyright line.
pub fn parse_bir_xml_checked(content: &str) -> Result<BTreeMap<String, String>, BirXmlParseError> {
    let mut fields = BTreeMap::new();
    let mut cursor = 0;

    while let Some(relative_start) = content[cursor..].find("<div") {
        let start = cursor + relative_start;
        if content[cursor..start].contains("</div>") {
            return Err(BirXmlParseError::MalformedDiv {
                offset: cursor,
                reason: "closing tag appears before an opening tag".to_string(),
            });
        }

        let opening_end = content[start..]
            .find('>')
            .map(|index| start + index)
            .ok_or(BirXmlParseError::MalformedDiv {
                offset: start,
                reason: "opening tag is not terminated".to_string(),
            })?;
        let attributes = &content[start + "<div".len()..opening_end];
        let body_start = opening_end + 1;
        let closing_start = content[body_start..]
            .find("</div>")
            .map(|index| body_start + index)
            .ok_or(BirXmlParseError::MalformedDiv {
                offset: start,
                reason: "opening tag has no closing tag".to_string(),
            })?;

        let body = &content[body_start..closing_start];
        if body.contains("<div") {
            return Err(BirXmlParseError::MalformedDiv {
                offset: start,
                reason: "nested field divs are not supported".to_string(),
            });
        }

        let attribute_id = parse_id_attribute(attributes, start)?;
        let (field_id, encoded_value) = match attribute_id {
            Some(attribute_id) => parse_attributed_body(attribute_id, body, start)?,
            None => parse_delimited_body(body, start)?,
        };
        validate_field_id(&field_id, start)?;

        let value = urlencoding::decode(&encoded_value)
            .map(Cow::into_owned)
            .map_err(|_| BirXmlParseError::InvalidEncoding {
                field_id: field_id.clone(),
            })?;
        if fields.insert(field_id.clone(), value).is_some() {
            return Err(BirXmlParseError::DuplicateFieldId { field_id });
        }

        cursor = closing_start + "</div>".len();
    }

    if content[cursor..].contains("</div>") {
        return Err(BirXmlParseError::MalformedDiv {
            offset: cursor,
            reason: "closing tag has no opening tag".to_string(),
        });
    }
    if fields.is_empty() {
        return Err(BirXmlParseError::EmptyPayload);
    }

    Ok(fields)
}

fn parse_id_attribute(attributes: &str, offset: usize) -> Result<Option<&str>, BirXmlParseError> {
    let attributes = attributes.trim();
    if attributes.is_empty() {
        return Ok(None);
    }

    static ID_ATTRIBUTE: OnceLock<Regex> = OnceLock::new();
    let expression = ID_ATTRIBUTE.get_or_init(|| {
        Regex::new(r#"(?i)(?:^|\s)id\s*=\s*(?:\"([^\"]*)\"|'([^']*)')"#)
            .expect("the field ID attribute regex is valid")
    });
    let mut matches = expression.captures_iter(attributes);
    let Some(captures) = matches.next() else {
        return Err(BirXmlParseError::MissingFieldId { offset });
    };
    if matches.next().is_some() {
        return Err(BirXmlParseError::MalformedDiv {
            offset,
            reason: "opening tag declares more than one ID attribute".to_string(),
        });
    }
    let id = captures
        .get(1)
        .or_else(|| captures.get(2))
        .map(|capture| capture.as_str())
        .unwrap_or_default();
    if id.is_empty() {
        return Err(BirXmlParseError::MissingFieldId { offset });
    }
    Ok(Some(id))
}

fn parse_attributed_body(
    attribute_id: &str,
    body: &str,
    offset: usize,
) -> Result<(String, String), BirXmlParseError> {
    if body.contains('=') {
        let (body_id, encoded_value) = parse_delimited_body(body, offset)?;
        if body_id != attribute_id {
            return Err(BirXmlParseError::MismatchedFieldId {
                offset,
                attribute_id: attribute_id.to_string(),
                body_id,
            });
        }
        Ok((attribute_id.to_string(), encoded_value))
    } else {
        Ok((attribute_id.to_string(), body.to_string()))
    }
}

fn parse_delimited_body(body: &str, offset: usize) -> Result<(String, String), BirXmlParseError> {
    let first_equals = body
        .find('=')
        .ok_or(BirXmlParseError::MissingFieldId { offset })?;
    let field_id = &body[..first_equals];
    if field_id.is_empty() {
        return Err(BirXmlParseError::MissingFieldId { offset });
    }
    let value_with_delimiter = &body[first_equals + 1..];
    let closing_delimiter = format!("{field_id}=");
    let encoded_value = value_with_delimiter
        .strip_suffix(&closing_delimiter)
        .ok_or(BirXmlParseError::MalformedDiv {
            offset,
            reason: format!("field {field_id:?} is missing its repeated closing delimiter"),
        })?;
    Ok((field_id.to_string(), encoded_value.to_string()))
}

fn validate_field_id(field_id: &str, offset: usize) -> Result<(), BirXmlParseError> {
    if field_id.is_empty()
        || field_id.chars().any(|character| {
            character.is_whitespace()
                || matches!(character, '=' | '<' | '>' | '\'' | '"' | '/' | '\\')
        })
    {
        return Err(BirXmlParseError::MalformedDiv {
            offset,
            reason: format!("invalid field ID {field_id:?}"),
        });
    }
    Ok(())
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
    fn generated_payload_round_trips_through_checked_parser() {
        let mut fields = BTreeMap::new();
        fields.insert("frm:txtName".to_string(), "JOHN DOE".to_string());
        fields.insert("frm:txtTIN1".to_string(), "010".to_string());

        let xml = generate_bir_xml(&fields);
        assert_eq!(parse_bir_xml_checked(&xml).unwrap(), fields);
    }

    #[test]
    fn parses_xml_declaration_and_all_fields_on_one_line() {
        let xml = "<?xml version='1.0'?><div>frm0619E:txtMonth=04frm0619E:txtMonth=</div><div>frm0619E:txtTaxpayerName=JUAN%20DELA%20CRUZfrm0619E:txtTaxpayerName=</div>All Rights Reserved BIR 2012.0";

        let parsed = parse_bir_xml_checked(xml).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["frm0619E:txtMonth"], "04");
        assert_eq!(parsed["frm0619E:txtTaxpayerName"], "JUAN DELA CRUZ");
    }

    #[test]
    fn parses_attributed_field_divs() {
        let xml =
            "<div id=\"frm0619E:txtMonth\">04</div><div id='txtEmail'>test%40example.com</div>";

        let parsed = parse_bir_xml_checked(xml).unwrap();
        assert_eq!(parsed["frm0619E:txtMonth"], "04");
        assert_eq!(parsed["txtEmail"], "test@example.com");
    }

    #[test]
    fn rejects_duplicate_field_ids() {
        let xml = "<div>txtEmail=a%40example.comtxtEmail=</div><div id=\"txtEmail\">b%40example.com</div>";

        assert_eq!(
            parse_bir_xml_checked(xml),
            Err(BirXmlParseError::DuplicateFieldId {
                field_id: "txtEmail".to_string(),
            })
        );
    }

    #[test]
    fn rejects_malformed_and_unidentified_fields() {
        assert!(matches!(
            parse_bir_xml_checked("<div>txtEmail=value</div>"),
            Err(BirXmlParseError::MalformedDiv { .. })
        ));
        assert!(matches!(
            parse_bir_xml_checked("<div>=value=</div>"),
            Err(BirXmlParseError::MissingFieldId { .. })
        ));
        assert!(matches!(
            parse_bir_xml_checked("<div id=\"\">value</div>"),
            Err(BirXmlParseError::MissingFieldId { .. })
        ));
        assert!(matches!(
            parse_bir_xml_checked("<div>txtEmail=valuetxtEmail="),
            Err(BirXmlParseError::MalformedDiv { .. })
        ));
    }

    #[test]
    fn url_encoding_is_decoded_once_into_semantic_values() {
        let mut fields = BTreeMap::new();
        fields.insert("fn".to_string(), "GALANG, ANDREA MAE".to_string());

        let xml = generate_bir_xml(&fields);
        assert!(xml.contains("GALANG%2C%20ANDREA%20MAE"));
        assert_eq!(parse_bir_xml_checked(&xml).unwrap(), fields);
    }
}
