use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;

use crate::error::{CodegenError, Result};

pub const CANONICALIZATION_ID: &str = "bir-json-c14n-v1";

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_serde(self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(value),
            Self::Number(value) => serde_json::Value::Number(value),
            Self::String(value) => serde_json::Value::String(value),
            Self::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(Self::into_serde).collect())
            }
            Self::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_serde()))
                    .collect(),
            ),
        }
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| E::custom("non-finite numbers are not valid JSON"))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        Ok(JsonValue::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element()? {
            values.push(value);
        }
        Ok(JsonValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key `{key}`")));
            }
            let value = map.next_value()?;
            values.insert(key, value);
        }
        Ok(JsonValue::Object(values))
    }
}

impl Serialize for JsonValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => values.serialize(serializer),
            Self::Object(values) => values.serialize(serializer),
        }
    }
}

pub fn parse_strict(bytes: &[u8], path: &Path) -> Result<JsonValue> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = JsonValue::deserialize(&mut deserializer).map_err(|source| {
        CodegenError::with_source(
            format!("strict JSON parse of `{}` failed", path.display()),
            source,
        )
    })?;
    deserializer.end().map_err(|source| {
        CodegenError::with_source(
            format!(
                "strict JSON parse of `{}` found trailing content",
                path.display()
            ),
            source,
        )
    })?;
    Ok(value)
}

pub fn parse_typed<T>(bytes: &[u8], path: &Path) -> Result<(T, JsonValue)>
where
    T: for<'de> Deserialize<'de>,
{
    let value = parse_strict(bytes, path)?;
    let typed = serde_json::from_value(value.clone().into_serde()).map_err(|source| {
        CodegenError::with_source(
            format!("closed-structure load of `{}` failed", path.display()),
            source,
        )
    })?;
    Ok((typed, value))
}

pub fn canonical_bytes(value: &JsonValue) -> Vec<u8> {
    let mut output = Vec::new();
    write_canonical(value, &mut output);
    output
}

fn write_canonical(value: &JsonValue, output: &mut Vec<u8>) {
    match value {
        JsonValue::Null => output.extend_from_slice(b"null"),
        JsonValue::Bool(false) => output.extend_from_slice(b"false"),
        JsonValue::Bool(true) => output.extend_from_slice(b"true"),
        JsonValue::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        JsonValue::String(value) => {
            let encoded =
                serde_json::to_string(value).expect("serializing a JSON string cannot fail");
            output.extend_from_slice(encoded.as_bytes());
        }
        JsonValue::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(value, output);
            }
            output.push(b']');
        }
        JsonValue::Object(values) => {
            output.push(b'{');
            for (index, (key, value)) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                let encoded =
                    serde_json::to_string(key).expect("serializing a JSON key cannot fail");
                output.extend_from_slice(encoded.as_bytes());
                output.push(b':');
                write_canonical(value, output);
            }
            output.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde::Deserialize;

    use super::{canonical_bytes, parse_strict, parse_typed};

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Closed {
        value: u32,
    }

    #[test]
    fn rejects_duplicate_keys_at_any_depth() {
        let error = parse_strict(
            br#"{"outer":{"same":1,"same":2}}"#,
            Path::new("duplicate.json"),
        )
        .expect_err("duplicate keys must fail");
        let details = format!("{error:?}");
        assert!(details.contains("duplicate object key `same`"), "{details}");
    }

    #[test]
    fn closed_typed_load_rejects_unknown_keys() {
        let error =
            parse_typed::<Closed>(br#"{"value":1,"surprise":true}"#, Path::new("closed.json"))
                .expect_err("unknown fields must fail");
        let details = format!("{error:?}");
        assert!(details.contains("unknown field `surprise`"), "{details}");
    }

    #[test]
    fn canonicalization_sorts_keys_and_preserves_array_order() {
        let value = parse_strict(
            r#"{ "z": 0, "a": [3, 2, 1], "é": "ok" }"#.as_bytes(),
            Path::new("canonical.json"),
        )
        .expect("valid JSON");
        assert_eq!(
            canonical_bytes(&value),
            r#"{"a":[3,2,1],"z":0,"é":"ok"}"#.as_bytes()
        );
    }

    #[test]
    fn typed_value_is_loaded() {
        let (value, _) = parse_typed::<Closed>(br#"{"value":42}"#, Path::new("closed.json"))
            .expect("known field");
        assert_eq!(value.value, 42);
    }
}
