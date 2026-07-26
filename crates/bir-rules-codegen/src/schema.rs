use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{CodegenError, Result};
use crate::files::{json_files, read_bytes};
use crate::json::{JsonValue, canonical_bytes, parse_strict};
use crate::path::normalized_relative_path;

#[derive(Clone, Copy, Debug)]
enum NestedSchemas {
    /// The v2 schema set must be flat.
    Reject,
    /// The v1 schema directory contains the v2 subtree, which it does not own.
    Skip,
}

#[derive(Clone, Debug)]
pub struct SchemaSet {
    root: PathBuf,
    documents: BTreeMap<String, JsonValue>,
    canonical_documents: BTreeMap<String, Vec<u8>>,
}

impl SchemaSet {
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_with(root, NestedSchemas::Reject)
    }

    /// Loads only the schemas directly under `root`, ignoring nested
    /// directories rather than rejecting them.
    ///
    /// The v1 corpus keeps its schemas in `rules/schema/` while the v2 IR keeps
    /// its own closed set in `rules/schema/v2/`. [`Self::load`] must keep
    /// rejecting nested documents so the v2 set stays flat; the v1 audit simply
    /// skips the v2 subtree it does not own.
    pub fn load_top_level(root: &Path) -> Result<Self> {
        Self::load_with(root, NestedSchemas::Skip)
    }

    fn load_with(root: &Path, nested: NestedSchemas) -> Result<Self> {
        let mut documents = BTreeMap::new();
        let mut canonical_documents = BTreeMap::new();
        for path in json_files(root)? {
            let relative = normalized_relative_path(root, &path)?;
            if relative.contains('/') {
                match nested {
                    NestedSchemas::Skip => continue,
                    NestedSchemas::Reject => {
                        return Err(CodegenError::new(format!(
                            "v2 schema `{relative}` must be directly under the schema directory"
                        )));
                    }
                }
            }
            let bytes = read_bytes(&path)?;
            let value = parse_strict(&bytes, &path)?;
            if !matches!(value, JsonValue::Object(_)) {
                return Err(CodegenError::new(format!(
                    "v2 schema `{relative}` must be a JSON object"
                )));
            }
            canonical_documents.insert(relative.clone(), canonical_bytes(&value));
            documents.insert(relative, value);
        }
        if documents.is_empty() {
            return Err(CodegenError::new(format!(
                "v2 schema directory `{}` contains no JSON schemas",
                root.display()
            )));
        }
        Ok(Self {
            root: root.to_path_buf(),
            documents,
            canonical_documents,
        })
    }

    pub fn canonical_documents(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.canonical_documents
    }

    pub fn validate(&self, schema_name: &str, instance: &JsonValue) -> Result<()> {
        let schema = self.documents.get(schema_name).ok_or_else(|| {
            CodegenError::new(format!(
                "required v2 schema `{schema_name}` is missing from `{}`",
                self.root.display()
            ))
        })?;
        let mut errors = Vec::new();
        let mut ref_stack = Vec::new();
        self.validate_node(
            schema_name,
            schema,
            instance,
            "$",
            &mut ref_stack,
            &mut errors,
        );
        if errors.is_empty() {
            Ok(())
        } else {
            errors.sort();
            errors.dedup();
            Err(CodegenError::new(format!(
                "`{schema_name}` validation failed:\n{}",
                errors
                    .into_iter()
                    .map(|error| format!("- {error}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )))
        }
    }

    fn validate_node(
        &self,
        document_name: &str,
        schema: &JsonValue,
        instance: &JsonValue,
        instance_path: &str,
        ref_stack: &mut Vec<String>,
        errors: &mut Vec<String>,
    ) {
        let JsonValue::Object(schema) = schema else {
            errors.push(format!(
                "{instance_path}: schema node in `{document_name}` is not an object"
            ));
            return;
        };

        if let Some(reference) = string_property(schema, "$ref") {
            let reference_key = format!("{document_name}:{reference}");
            if ref_stack.len() > 512 {
                errors.push(format!(
                    "{instance_path}: schema reference nesting exceeds 512 nodes"
                ));
                return;
            }
            let Some((target_name, target)) = self.resolve_ref(document_name, reference) else {
                errors.push(format!(
                    "{instance_path}: unresolved schema reference `{reference}` in `{document_name}`"
                ));
                return;
            };
            ref_stack.push(reference_key);
            self.validate_node(
                target_name,
                target,
                instance,
                instance_path,
                ref_stack,
                errors,
            );
            ref_stack.pop();
        }

        if let Some(JsonValue::Array(variants)) = schema.get("oneOf") {
            let mut passing = 0usize;
            let mut variant_errors = Vec::new();
            for variant in variants {
                let mut current = Vec::new();
                self.validate_node(
                    document_name,
                    variant,
                    instance,
                    instance_path,
                    ref_stack,
                    &mut current,
                );
                if current.is_empty() {
                    passing += 1;
                } else {
                    variant_errors.push(current);
                }
            }
            if passing != 1 {
                errors.push(format!(
                    "{instance_path}: expected exactly one `oneOf` branch to match, found {passing}"
                ));
                if passing == 0 {
                    let mut concise = BTreeSet::new();
                    for variant in variant_errors {
                        if let Some(first) = variant.into_iter().next() {
                            concise.insert(first);
                        }
                    }
                    for detail in concise.into_iter().take(4) {
                        errors.push(format!("{instance_path}: branch mismatch: {detail}"));
                    }
                }
            }
            // All constraints of each oneOf branch have already been checked.
            // Continue for sibling constraints permitted by draft 2020-12.
        }

        if let Some(JsonValue::Array(variants)) = schema.get("anyOf") {
            let mut passing = 0usize;
            let mut variant_errors = Vec::new();
            for variant in variants {
                let mut current = Vec::new();
                self.validate_node(
                    document_name,
                    variant,
                    instance,
                    instance_path,
                    ref_stack,
                    &mut current,
                );
                if current.is_empty() {
                    passing += 1;
                } else {
                    variant_errors.push(current);
                }
            }
            if passing == 0 {
                errors.push(format!(
                    "{instance_path}: expected at least one `anyOf` branch to match"
                ));
                let mut concise = BTreeSet::new();
                for variant in variant_errors {
                    if let Some(first) = variant.into_iter().next() {
                        concise.insert(first);
                    }
                }
                for detail in concise.into_iter().take(4) {
                    errors.push(format!("{instance_path}: branch mismatch: {detail}"));
                }
            }
            // Continue for sibling constraints permitted by draft 2020-12.
        }

        if let Some(JsonValue::Array(constraints)) = schema.get("allOf") {
            for constraint in constraints {
                self.validate_node(
                    document_name,
                    constraint,
                    instance,
                    instance_path,
                    ref_stack,
                    errors,
                );
            }
        }

        if let Some(condition) = schema.get("if") {
            let mut condition_errors = Vec::new();
            self.validate_node(
                document_name,
                condition,
                instance,
                instance_path,
                ref_stack,
                &mut condition_errors,
            );
            let selected = if condition_errors.is_empty() {
                schema.get("then")
            } else {
                schema.get("else")
            };
            if let Some(selected) = selected {
                self.validate_node(
                    document_name,
                    selected,
                    instance,
                    instance_path,
                    ref_stack,
                    errors,
                );
            }
        }

        if let Some(expected) = schema.get("type") {
            if !matches_type(expected, instance) {
                errors.push(format!(
                    "{instance_path}: expected type {}, found {}",
                    describe_type_constraint(expected),
                    type_name(instance)
                ));
                return;
            }
        }

        if let Some(expected) = schema.get("const") {
            if expected != instance {
                errors.push(format!(
                    "{instance_path}: value does not match schema `const`"
                ));
            }
        }

        if let Some(JsonValue::Array(expected)) = schema.get("enum") {
            if !expected.iter().any(|candidate| candidate == instance) {
                errors.push(format!(
                    "{instance_path}: value is not one of the closed enum variants"
                ));
            }
        }

        match instance {
            JsonValue::Object(object) => self.validate_object(
                document_name,
                schema,
                object,
                instance_path,
                ref_stack,
                errors,
            ),
            JsonValue::Array(array) => self.validate_array(
                document_name,
                schema,
                array,
                instance_path,
                ref_stack,
                errors,
            ),
            JsonValue::String(value) => self.validate_string(schema, value, instance_path, errors),
            JsonValue::Number(value) => self.validate_number(schema, value, instance_path, errors),
            JsonValue::Null | JsonValue::Bool(_) => {}
        }
    }

    fn validate_object(
        &self,
        document_name: &str,
        schema: &BTreeMap<String, JsonValue>,
        object: &BTreeMap<String, JsonValue>,
        instance_path: &str,
        ref_stack: &mut Vec<String>,
        errors: &mut Vec<String>,
    ) {
        if let Some(JsonValue::Array(required)) = schema.get("required") {
            for key in required.iter().filter_map(JsonValue::as_str) {
                if !object.contains_key(key) {
                    errors.push(format!(
                        "{instance_path}: missing required property `{key}`"
                    ));
                }
            }
        }

        let properties = match schema.get("properties") {
            Some(JsonValue::Object(properties)) => Some(properties),
            _ => None,
        };

        if schema.get("additionalProperties") == Some(&JsonValue::Bool(false)) {
            for key in object.keys() {
                if !properties.is_some_and(|properties| properties.contains_key(key)) {
                    errors.push(format!(
                        "{instance_path}: unknown property `{key}` in closed object"
                    ));
                }
            }
        }

        if let Some(properties) = properties {
            for (key, property_schema) in properties {
                if let Some(value) = object.get(key) {
                    let child_path = format!("{instance_path}/{}", escape_json_pointer(key));
                    self.validate_node(
                        document_name,
                        property_schema,
                        value,
                        &child_path,
                        ref_stack,
                        errors,
                    );
                }
            }
        }
    }

    fn validate_array(
        &self,
        document_name: &str,
        schema: &BTreeMap<String, JsonValue>,
        array: &[JsonValue],
        instance_path: &str,
        ref_stack: &mut Vec<String>,
        errors: &mut Vec<String>,
    ) {
        if let Some(minimum) = integer_property(schema, "minItems") {
            if array.len() < minimum {
                errors.push(format!(
                    "{instance_path}: array length {} is below minItems {minimum}",
                    array.len()
                ));
            }
        }
        if let Some(maximum) = integer_property(schema, "maxItems") {
            if array.len() > maximum {
                errors.push(format!(
                    "{instance_path}: array length {} exceeds maxItems {maximum}",
                    array.len()
                ));
            }
        }
        if schema.get("uniqueItems") == Some(&JsonValue::Bool(true)) {
            let mut seen = BTreeSet::new();
            for (index, value) in array.iter().enumerate() {
                if !seen.insert(canonical_bytes(value)) {
                    errors.push(format!(
                        "{instance_path}/{index}: duplicate array item violates uniqueItems"
                    ));
                }
            }
        }
        if let Some(item_schema) = schema.get("items") {
            for (index, value) in array.iter().enumerate() {
                self.validate_node(
                    document_name,
                    item_schema,
                    value,
                    &format!("{instance_path}/{index}"),
                    ref_stack,
                    errors,
                );
            }
        }
    }

    fn validate_string(
        &self,
        schema: &BTreeMap<String, JsonValue>,
        value: &str,
        instance_path: &str,
        errors: &mut Vec<String>,
    ) {
        let character_count = value.chars().count();
        if let Some(minimum) = integer_property(schema, "minLength") {
            if character_count < minimum {
                errors.push(format!(
                    "{instance_path}: string length {character_count} is below minLength {minimum}"
                ));
            }
        }
        if let Some(maximum) = integer_property(schema, "maxLength") {
            if character_count > maximum {
                errors.push(format!(
                    "{instance_path}: string length {character_count} exceeds maxLength {maximum}"
                ));
            }
        }
        if let Some(pattern) = string_property(schema, "pattern") {
            match Regex::new(pattern) {
                Ok(pattern) if !pattern.is_match(value) => errors.push(format!(
                    "{instance_path}: string does not match required pattern `{}`",
                    pattern.as_str()
                )),
                Ok(_) => {}
                Err(source) => errors.push(format!(
                    "{instance_path}: schema contains invalid regex `{pattern}`: {source}"
                )),
            }
        }
        if string_property(schema, "format") == Some("date") && !is_iso_date(value) {
            errors.push(format!(
                "{instance_path}: `{value}` is not a valid ISO calendar date"
            ));
        }
    }

    fn validate_number(
        &self,
        schema: &BTreeMap<String, JsonValue>,
        value: &serde_json::Number,
        instance_path: &str,
        errors: &mut Vec<String>,
    ) {
        if let Some(minimum) = number_property(schema, "minimum") {
            if number_as_f64(value).is_some_and(|value| value < minimum) {
                errors.push(format!(
                    "{instance_path}: number is below minimum {minimum}"
                ));
            }
        }
        if let Some(maximum) = number_property(schema, "maximum") {
            if number_as_f64(value).is_some_and(|value| value > maximum) {
                errors.push(format!("{instance_path}: number exceeds maximum {maximum}"));
            }
        }
    }

    fn resolve_ref<'a>(
        &'a self,
        current_document: &str,
        reference: &str,
    ) -> Option<(&'a str, &'a JsonValue)> {
        let (document_name, fragment) = match reference.split_once('#') {
            Some(("", fragment)) => (current_document, fragment),
            Some((document, fragment)) => (document, fragment),
            None => (reference, ""),
        };
        let document = self.documents.get(document_name)?;
        if fragment.is_empty() {
            return Some((
                self.documents.get_key_value(document_name)?.0.as_str(),
                document,
            ));
        }
        let target = resolve_pointer(document, fragment)?;
        Some((
            self.documents.get_key_value(document_name)?.0.as_str(),
            target,
        ))
    }
}

fn matches_type(constraint: &JsonValue, instance: &JsonValue) -> bool {
    match constraint {
        JsonValue::String(expected) => is_type(expected, instance),
        JsonValue::Array(expected) => expected
            .iter()
            .filter_map(JsonValue::as_str)
            .any(|expected| is_type(expected, instance)),
        _ => false,
    }
}

fn is_type(expected: &str, instance: &JsonValue) -> bool {
    match expected {
        "null" => matches!(instance, JsonValue::Null),
        "boolean" => matches!(instance, JsonValue::Bool(_)),
        "string" => matches!(instance, JsonValue::String(_)),
        "array" => matches!(instance, JsonValue::Array(_)),
        "object" => matches!(instance, JsonValue::Object(_)),
        "number" => matches!(instance, JsonValue::Number(_)),
        "integer" => match instance {
            JsonValue::Number(number) => number.as_i64().is_some() || number.as_u64().is_some(),
            _ => false,
        },
        _ => false,
    }
}

fn describe_type_constraint(constraint: &JsonValue) -> String {
    match constraint {
        JsonValue::String(value) => format!("`{value}`"),
        JsonValue::Array(values) => values
            .iter()
            .filter_map(JsonValue::as_str)
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(" or "),
        _ => "a valid schema type".to_owned(),
    }
}

fn type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            "integer"
        }
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn string_property<'a>(object: &'a BTreeMap<String, JsonValue>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(JsonValue::as_str)
}

fn integer_property(object: &BTreeMap<String, JsonValue>, key: &str) -> Option<usize> {
    object
        .get(key)
        .and_then(|value| match value {
            JsonValue::Number(number) => number.as_u64(),
            _ => None,
        })
        .and_then(|value| usize::try_from(value).ok())
}

fn number_property(object: &BTreeMap<String, JsonValue>, key: &str) -> Option<f64> {
    object.get(key).and_then(|value| match value {
        JsonValue::Number(number) => number_as_f64(number),
        _ => None,
    })
}

fn number_as_f64(number: &serde_json::Number) -> Option<f64> {
    number
        .as_f64()
        .or_else(|| number.as_i64().map(|value| value as f64))
        .or_else(|| number.as_u64().map(|value| value as f64))
}

fn resolve_pointer<'a>(document: &'a JsonValue, fragment: &str) -> Option<&'a JsonValue> {
    if fragment.is_empty() {
        return Some(document);
    }
    let pointer = fragment.strip_prefix('/')?;
    let mut current = document;
    for encoded in pointer.split('/') {
        let component = encoded.replace("~1", "/").replace("~0", "~");
        current = match current {
            JsonValue::Object(object) => object.get(&component)?,
            JsonValue::Array(array) => array.get(component.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u32>() else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days).contains(&day)
}

#[cfg(test)]
mod tests {
    use bir_rules::FieldId;
    use bir_rules::serialization::{ArtifactVariantId, BodyCodec};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::SchemaSet;
    use crate::json::{JsonValue, parse_strict};
    use crate::model::SerializationBodyCodec;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn common_identifier_schema_matches_runtime_stable_id_alphabet_and_boundaries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let valid_maximum = format!("a{}Z", "/B".repeat(126) + "B");
        assert_eq!(valid_maximum.len(), 255);
        let invalid_too_long = format!("a{}Z", "B".repeat(254));
        assert_eq!(invalid_too_long.len(), 256);
        let candidates = [
            "a".to_owned(),
            "frm2550qv2024:txtTIN1".to_owned(),
            "schedule/row_A-1.value".to_owned(),
            valid_maximum,
            String::new(),
            "-leading".to_owned(),
            "trailing/".to_owned(),
            "contains space".to_owned(),
            "unicodé".to_owned(),
            invalid_too_long,
        ];

        for candidate in candidates {
            let predicate: JsonValue = serde_json::from_value(serde_json::json!({
                "kind": "coercion-failed",
                "field": {
                    "field_id": candidate.clone(),
                    "instance": {"kind": "singleton"}
                }
            }))
            .expect("build field-reference predicate");
            let schema_accepts = schemas
                .validate("predicate.schema.json", &predicate)
                .is_ok();
            let runtime_accepts = FieldId::parse(candidate.clone()).is_ok();
            assert_eq!(
                schema_accepts, runtime_accepts,
                "schema/runtime stable identifier parity for {candidate:?}"
            );
        }
    }

    #[test]
    fn candidate_schema_requires_digest_fixture_and_matching_executable_policy_branch() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let index_path = root.join("../../ir/v2/index.json");
        let mut index = parse_strict(&fs::read(&index_path).expect("read v2 index"), &index_path)
            .expect("parse v2 index");
        let JsonValue::Array(snapshots) = index.object_mut().unwrap().get_mut("snapshots").unwrap()
        else {
            panic!("index snapshots are an array");
        };
        let snapshot = snapshots[0].object_mut().unwrap();
        snapshot.insert(
            "review_status".to_owned(),
            JsonValue::String("candidate".to_owned()),
        );
        snapshot.insert(
            "source_set_sha256".to_owned(),
            JsonValue::String(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ),
        );
        snapshot
            .get_mut("profile_states")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                JsonValue::String("executable".to_owned()),
            );
        schemas
            .validate("index.schema.json", &index)
            .expect("candidate index pins digest and one executable profile");
        let mut index_without_executable = index;
        let JsonValue::Array(snapshots) = index_without_executable
            .object_mut()
            .unwrap()
            .get_mut("snapshots")
            .unwrap()
        else {
            panic!("index snapshots are an array");
        };
        snapshots[0]
            .object_mut()
            .unwrap()
            .get_mut("profile_states")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                JsonValue::String("documented_only".to_owned()),
            );
        schemas
            .validate("index.schema.json", &index_without_executable)
            .expect_err("candidate index must expose at least one executable profile");

        let scaffold_path = root.join("../../ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let mut candidate = parse_strict(
            &fs::read(&scaffold_path).expect("read scaffold rule set"),
            &scaffold_path,
        )
        .expect("parse scaffold rule set");
        let document = candidate.object_mut().unwrap();
        document.insert(
            "review_status".to_owned(),
            JsonValue::String("candidate".to_owned()),
        );
        document
            .get_mut("identity")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "source_set_sha256".to_owned(),
                JsonValue::String(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                ),
            );
        document.insert(
            "fixtures".to_owned(),
            serde_json::from_value(serde_json::json!([
                "ir/v2/2550q-v2024-p7.9.6.0/fixtures/candidate.json"
            ]))
            .unwrap(),
        );
        document
            .get_mut("profile_status")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                serde_json::from_value(serde_json::json!({
                    "state": "executable",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                }))
                .unwrap(),
            );
        document
            .get_mut("evaluation_policy")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                serde_json::from_value(serde_json::json!({
                    "state": "executable",
                    "effect_mode": "apply-all",
                    "review_decision": {"source_id": "v1-validations"},
                    "source_refs": [{"source_id": "v1-validations"}]
                }))
                .unwrap(),
            );

        schemas
            .validate("rule-set.schema.json", &candidate)
            .expect("complete candidate shape validates");

        let mut missing_digest = candidate.clone();
        missing_digest
            .object_mut()
            .unwrap()
            .get_mut("identity")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert("source_set_sha256".to_owned(), JsonValue::Null);
        schemas
            .validate("rule-set.schema.json", &missing_digest)
            .expect_err("candidate digest must be pinned");

        let mut missing_fixture = candidate.clone();
        missing_fixture
            .object_mut()
            .unwrap()
            .insert("fixtures".to_owned(), JsonValue::Array(vec![]));
        schemas
            .validate("rule-set.schema.json", &missing_fixture)
            .expect_err("candidate must have at least one fixture");

        let mut candidate_cannot_claim_reviewed = candidate.clone();
        candidate_cannot_claim_reviewed
            .object_mut()
            .unwrap()
            .insert(
                "review_status".to_owned(),
                JsonValue::String("reviewed".to_owned()),
            );
        schemas
            .validate("rule-set.schema.json", &candidate_cannot_claim_reviewed)
            .expect_err("partially executable candidate cannot be relabeled reviewed");

        let mut no_executable_policy = candidate;
        no_executable_policy
            .object_mut()
            .unwrap()
            .get_mut("evaluation_policy")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "official".to_owned(),
                serde_json::from_value(serde_json::json!({
                    "state": "unresolved",
                    "reason": "not reviewed",
                    "source_refs": [{"source_id": "v1-validations"}]
                }))
                .unwrap(),
            );
        schemas
            .validate("rule-set.schema.json", &no_executable_policy)
            .expect_err("candidate profile cannot borrow a non-executable policy branch");
    }

    #[test]
    fn legacy_record_classification_schema_is_closed_and_source_bound() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let rule_set_path = root.join("../../ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let mut rule_set = parse_strict(
            &fs::read(&rule_set_path).expect("read landed rule set"),
            &rule_set_path,
        )
        .expect("parse landed rule set");
        let classifications = serde_json::from_value(serde_json::json!([{
            "outcome": "non-runtime",
            "artifact": "validations",
            "legacy_id": "example-rule",
            "locator": "#/rules/0",
            "reason": "proven-unreachable",
            "source_refs": [{
                "source_id": "v1-validations",
                "locator": "#/rules/0"
            }]
        }]))
        .expect("classification JSON");
        rule_set
            .object_mut()
            .unwrap()
            .get_mut("legacy_v1")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert("record_classifications".to_owned(), classifications);
        schemas
            .validate("rule-set.schema.json", &rule_set)
            .expect("closed non-runtime classification validates");

        let mut workflow_rule_set = rule_set.clone();
        let workflow_classifications = serde_json::from_value(serde_json::json!([{
            "outcome": "non-runtime",
            "artifact": "workflow",
            "locator": "#/phases/0",
            "reason": "non-validation-ui-behavior",
            "source_refs": [{
                "source_id": "v1-workflow",
                "locator": "#/phases/0"
            }]
        }]))
        .expect("workflow classification JSON");
        workflow_rule_set
            .object_mut()
            .unwrap()
            .get_mut("legacy_v1")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert(
                "record_classifications".to_owned(),
                workflow_classifications,
            );
        schemas
            .validate("rule-set.schema.json", &workflow_rule_set)
            .expect("ID-less workflow classification validates by locator");

        let workflow_classifications = workflow_rule_set
            .object_mut()
            .unwrap()
            .get_mut("legacy_v1")
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("record_classifications")
            .unwrap();
        let JsonValue::Array(workflow_classifications) = workflow_classifications else {
            panic!("workflow record classifications are an array");
        };
        workflow_classifications[0].object_mut().unwrap().insert(
            "legacy_id".to_owned(),
            JsonValue::String("invented-phase-id".to_owned()),
        );
        schemas
            .validate("rule-set.schema.json", &workflow_rule_set)
            .expect_err("workflow classification cannot invent a legacy ID");

        let classifications = rule_set
            .object_mut()
            .unwrap()
            .get_mut("legacy_v1")
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("record_classifications")
            .unwrap();
        let JsonValue::Array(classifications) = classifications else {
            panic!("record classifications are an array");
        };
        classifications[0].object_mut().unwrap().remove("legacy_id");
        schemas
            .validate("rule-set.schema.json", &rule_set)
            .expect_err("ID-bearing artifact classification requires legacy_id");

        let classifications = rule_set
            .object_mut()
            .unwrap()
            .get_mut("legacy_v1")
            .unwrap()
            .object_mut()
            .unwrap()
            .get_mut("record_classifications")
            .unwrap();
        let JsonValue::Array(classifications) = classifications else {
            panic!("record classifications are an array");
        };
        classifications[0].object_mut().unwrap().insert(
            "legacy_id".to_owned(),
            JsonValue::String("example-rule".to_owned()),
        );
        classifications[0].object_mut().unwrap().insert(
            "reason".to_owned(),
            JsonValue::String("whatever-makes-the-count-pass".to_owned()),
        );
        schemas
            .validate("rule-set.schema.json", &rule_set)
            .expect_err("classification reason vocabulary is closed");
    }

    fn serialization_contract_with_node(node: serde_json::Value) -> JsonValue {
        serde_json::from_value(serde_json::json!({
            "contract_version": "1.0.0",
            "artifacts": [{
                "artifact_id": "artifact",
                "target": "editable-save",
                "variant_id": "default",
                "official": {
                    "state": "executable",
                    "nodes": [node],
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "documented_only",
                    "summary": "not executable",
                    "source_refs": [{"source_id": "review"}]
                },
                "source_refs": [{"source_id": "review"}]
            }]
        }))
        .expect("build serialization contract")
    }

    #[test]
    fn validator_enforces_closed_objects_and_external_refs() {
        let root = std::env::temp_dir().join(format!(
            "bir-rules-codegen-schema-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create schema directory");
        fs::write(
            root.join("common.json"),
            br#"{"$defs":{"id":{"type":"string","pattern":"^[a-z]+$"}}}"#,
        )
        .expect("write common schema");
        fs::write(
            root.join("root.json"),
            br#"{
                "type":"object",
                "required":["id"],
                "properties":{"id":{"$ref":"common.json#/$defs/id"}},
                "additionalProperties":false
            }"#,
        )
        .expect("write root schema");

        let schemas = SchemaSet::load(&root).expect("load schemas");
        let valid = parse_strict(br#"{"id":"abc"}"#, Path::new("valid.json"))
            .expect("parse valid instance");
        schemas
            .validate("root.json", &valid)
            .expect("valid instance");

        let invalid = parse_strict(br#"{"id":"ABC","extra":true}"#, Path::new("invalid.json"))
            .expect("parse invalid instance");
        let error = schemas
            .validate("root.json", &invalid)
            .expect_err("invalid instance must fail");
        assert!(error.message().contains("unknown property `extra`"));
        assert!(error.message().contains("required pattern"));

        fs::remove_dir_all(root).expect("remove schema directory");
    }

    #[test]
    fn iso_date_checks_real_calendar_days() {
        let schema = crate::json::JsonValue::Object(BTreeMap::from([
            (
                "type".to_owned(),
                crate::json::JsonValue::String("string".to_owned()),
            ),
            (
                "format".to_owned(),
                crate::json::JsonValue::String("date".to_owned()),
            ),
        ]));
        let root = std::env::temp_dir().join(format!(
            "bir-rules-codegen-date-schema-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create schema directory");
        fs::write(
            root.join("date.json"),
            crate::json::canonical_bytes(&schema),
        )
        .expect("write date schema");
        let schemas = SchemaSet::load(&root).expect("load schemas");

        let leap = crate::json::JsonValue::String("2024-02-29".to_owned());
        schemas
            .validate("date.json", &leap)
            .expect("valid leap day");
        let invalid = crate::json::JsonValue::String("2023-02-29".to_owned());
        assert!(schemas.validate("date.json", &invalid).is_err());
        fs::remove_dir_all(root).expect("remove schema directory");
    }

    #[test]
    fn expression_schema_binds_decimal_division_policy_exclusively_to_divide() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");

        let valid = parse_strict(
            br#"{
                "kind":"binary",
                "result_type":"decimal",
                "operator":"divide",
                "division_policy":{"scale":2,"rounding":"half-even"},
                "left":{"kind":"literal","value":{"type":"decimal","value":"1"}},
                "right":{"kind":"literal","value":{"type":"decimal","value":"3"}}
            }"#,
            Path::new("valid-division.json"),
        )
        .expect("parse valid division");
        schemas
            .validate("expression.schema.json", &valid)
            .expect("policy-bound decimal division");

        let missing = parse_strict(
            br#"{
                "kind":"binary",
                "result_type":"decimal",
                "operator":"divide",
                "left":{"kind":"literal","value":{"type":"decimal","value":"1"}},
                "right":{"kind":"literal","value":{"type":"decimal","value":"3"}}
            }"#,
            Path::new("missing-policy.json"),
        )
        .expect("parse missing policy");
        assert!(
            schemas
                .validate("expression.schema.json", &missing)
                .expect_err("divide without policy must fail")
                .message()
                .contains("division_policy")
        );

        let unexpected = parse_strict(
            br#"{
                "kind":"binary",
                "result_type":"decimal",
                "operator":"add",
                "division_policy":{"scale":2,"rounding":"half-up"},
                "left":{"kind":"literal","value":{"type":"decimal","value":"1"}},
                "right":{"kind":"literal","value":{"type":"decimal","value":"3"}}
            }"#,
            Path::new("unexpected-policy.json"),
        )
        .expect("parse unexpected policy");
        schemas
            .validate("expression.schema.json", &unexpected)
            .expect_err("non-divide policy must fail");

        let invalid_scale = parse_strict(
            br#"{
                "kind":"binary",
                "result_type":"decimal",
                "operator":"divide",
                "division_policy":{"scale":19,"rounding":"half-up"},
                "left":{"kind":"literal","value":{"type":"decimal","value":"1"}},
                "right":{"kind":"literal","value":{"type":"decimal","value":"3"}}
            }"#,
            Path::new("invalid-scale.json"),
        )
        .expect("parse invalid scale");
        schemas
            .validate("expression.schema.json", &invalid_scale)
            .expect_err("division scale above 18 must fail");

        let open_policy = parse_strict(
            br#"{
                "kind":"binary",
                "result_type":"decimal",
                "operator":"divide",
                "division_policy":{"scale":2,"rounding":"half-up","extra":true},
                "left":{"kind":"literal","value":{"type":"decimal","value":"1"}},
                "right":{"kind":"literal","value":{"type":"decimal","value":"3"}}
            }"#,
            Path::new("open-policy.json"),
        )
        .expect("parse open policy");
        schemas
            .validate("expression.schema.json", &open_policy)
            .expect_err("division policy must remain closed");
    }

    #[test]
    fn expression_schema_requires_derived_selector_and_expression_aggregate_value() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");

        let derived: JsonValue = serde_json::from_value(serde_json::json!({
            "kind": "derived",
            "result_type": "decimal",
            "calculation_id": "subtotal",
            "output_id": "value",
            "instance": {"kind": "singleton"}
        }))
        .expect("build derived expression");
        schemas
            .validate("expression.schema.json", &derived)
            .expect("explicit singleton derived selector validates");

        let mut missing_selector = derived.clone();
        missing_selector.object_mut().unwrap().remove("instance");
        schemas
            .validate("expression.schema.json", &missing_selector)
            .expect_err("derived selector must never default");

        let aggregate: JsonValue = serde_json::from_value(serde_json::json!({
            "kind": "group-aggregate",
            "result_type": "decimal",
            "operator": "sum",
            "group_id": "rows",
            "value": {
                "kind": "field",
                "result_type": "decimal",
                "field": {
                    "field_id": "row-amount",
                    "instance": {"kind": "current-group-instance"}
                }
            }
        }))
        .expect("build aggregate expression");
        schemas
            .validate("expression.schema.json", &aggregate)
            .expect("aggregate accepts a typed expression value");

        let mut count_with_decimal_result = aggregate.clone();
        let count = count_with_decimal_result.object_mut().unwrap();
        count.insert("operator".to_owned(), JsonValue::String("count".to_owned()));
        schemas
            .validate("expression.schema.json", &count_with_decimal_result)
            .expect_err("count aggregate result_type must be integer");

        let old_field_only: JsonValue = serde_json::from_value(serde_json::json!({
            "kind": "group-aggregate",
            "result_type": "decimal",
            "operator": "sum",
            "group_id": "rows",
            "field_id": "row-amount"
        }))
        .expect("build obsolete aggregate");
        schemas
            .validate("expression.schema.json", &old_field_only)
            .expect_err("obsolete field-only aggregate must fail closed");
    }

    #[test]
    fn rule_set_schema_requires_explicit_calculation_and_rule_scopes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let scaffold_path = root.join("../../ir/v2/2550q-v2024-p7.9.6.0/rule-set.json");
        let mut rule_set = parse_strict(
            &fs::read(&scaffold_path).expect("read scaffold rule set"),
            &scaffold_path,
        )
        .expect("parse scaffold rule set");
        let document = rule_set.object_mut().expect("rule set is an object");
        document.insert(
            "evaluation_order".to_owned(),
            serde_json::from_value(serde_json::json!(["subtotal"]))
                .expect("build evaluation order"),
        );
        document.insert(
            "calculations".to_owned(),
            serde_json::from_value(serde_json::json!([{
                "calculation_id": "subtotal",
                "scope": {"kind": "singleton"},
                "output_ids": ["value"],
                "depends_on": [],
                "phases": ["validate"],
                "profiles": {
                    "official": {
                        "state": "documented_only",
                        "summary": "not reviewed",
                        "source_refs": [{"source_id": "v1-calculations"}]
                    },
                    "filing_safe": {
                        "state": "unresolved",
                        "reason": "not reviewed",
                        "source_refs": [{"source_id": "v1-calculations"}]
                    }
                },
                "source_refs": [{"source_id": "v1-calculations"}]
            }]))
            .expect("build scoped calculation"),
        );
        document.insert(
            "rules".to_owned(),
            serde_json::from_value(serde_json::json!([{
                "rule_id": "amount-valid",
                "scope": {"kind": "each-group", "group_id": "rows"},
                "order": 1,
                "phases": ["validate"],
                "field_ids": [],
                "profiles": {
                    "official": {
                        "state": "documented_only",
                        "summary": "not reviewed",
                        "source_refs": [{"source_id": "v1-validations"}]
                    },
                    "filing_safe": {
                        "state": "unresolved",
                        "reason": "not reviewed",
                        "source_refs": [{"source_id": "v1-validations"}]
                    }
                },
                "source_refs": [{"source_id": "v1-validations"}]
            }]))
            .expect("build scoped rule"),
        );
        schemas
            .validate("rule-set.schema.json", &rule_set)
            .expect("both explicit closed scopes validate structurally");

        let mut missing = rule_set.clone();
        let JsonValue::Array(calculations) = missing
            .object_mut()
            .unwrap()
            .get_mut("calculations")
            .unwrap()
        else {
            panic!("calculations are an array");
        };
        let calculation = calculations[0].object_mut().unwrap();
        calculation.remove("scope");
        schemas
            .validate("rule-set.schema.json", &missing)
            .expect_err("calculation scope must never default");

        let mut open = rule_set;
        let JsonValue::Array(rules) = open.object_mut().unwrap().get_mut("rules").unwrap() else {
            panic!("rules are an array");
        };
        let rule = rules[0].object_mut().unwrap();
        rule.get_mut("scope")
            .unwrap()
            .object_mut()
            .unwrap()
            .insert("fallback".to_owned(), JsonValue::Bool(true));
        schemas
            .validate("rule-set.schema.json", &open)
            .expect_err("evaluation scope objects remain closed");
    }

    #[test]
    fn evaluation_result_schema_binds_exact_nullable_group_identity() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let result = |instance: serde_json::Value| {
            serde_json::from_value::<JsonValue>(serde_json::json!({
                "report": {
                    "rule_set": {
                        "rule_set_id": "test-v1",
                        "form_code": "TEST",
                        "form_revision": "2024-01-01",
                        "official_package_version": "1.0.0",
                        "source_set_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "context": {"phase": "validate", "profile": "official"},
                    "input_revision": 1,
                    "context_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "expected_rules": [{
                        "execution": {"rule_id": "rule", "instance": instance.clone()},
                        "order": 1
                    }],
                    "evaluated_rules": [{
                        "rule_id": "rule",
                        "instance": instance.clone()
                    }],
                    "violations": [{
                        "execution": {"rule_id": "rule", "instance": instance.clone()},
                        "phase": "validate",
                        "order": {"rule_order": 1, "occurrence": 0},
                        "fields": [],
                        "official_message": null,
                        "message": "invalid",
                        "assessment": "verified-correct",
                        "severity": "blocking",
                        "profile": "official"
                    }]
                },
                "canonical_inputs": [],
                "expected_outputs": [{
                    "calculation_id": "calculation",
                    "output_id": "output",
                    "instance": instance.clone()
                }],
                "derived_outputs": [{
                    "calculation_id": "calculation",
                    "output_id": "output",
                    "instance": instance,
                    "value": {"type": "integer", "value": 1}
                }]
            }))
            .expect("build evaluation result")
        };

        schemas
            .validate(
                "evaluation-result.schema.json",
                &result(serde_json::Value::Null),
            )
            .expect("singleton result carries explicit null identity");
        schemas
            .validate(
                "evaluation-result.schema.json",
                &result(serde_json::json!({
                    "group_id": "rows",
                    "instance_id": "row-1"
                })),
            )
            .expect("group-scoped result carries exact full identity");

        let mut missing = result(serde_json::Value::Null);
        let JsonValue::Array(outputs) = missing
            .object_mut()
            .unwrap()
            .get_mut("expected_outputs")
            .unwrap()
        else {
            panic!("expected_outputs are an array");
        };
        outputs[0].object_mut().unwrap().remove("instance");
        schemas
            .validate("evaluation-result.schema.json", &missing)
            .expect_err("derived expectation identity must never default");
    }

    #[test]
    fn coercion_failed_predicate_requires_one_exact_field_reference() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");

        let valid = parse_strict(
            br#"{
                "kind":"coercion-failed",
                "field":{
                    "field_id":"amount",
                    "instance":{"kind":"singleton"}
                }
            }"#,
            Path::new("valid-coercion-failed.json"),
        )
        .expect("parse valid predicate");
        schemas
            .validate("predicate.schema.json", &valid)
            .expect("exact field reference is valid");

        let expression_instead_of_field = parse_strict(
            br#"{
                "kind":"coercion-failed",
                "field":{
                    "kind":"field",
                    "result_type":"decimal",
                    "field":{
                        "field_id":"amount",
                        "instance":{"kind":"singleton"}
                    }
                }
            }"#,
            Path::new("expression-coercion-failed.json"),
        )
        .expect("parse expression-shaped field");
        schemas
            .validate("predicate.schema.json", &expression_instead_of_field)
            .expect_err("an expression must not substitute for the exact field reference");

        let open = parse_strict(
            br#"{
                "kind":"coercion-failed",
                "field":{
                    "field_id":"amount",
                    "instance":{"kind":"singleton"}
                },
                "extra":true
            }"#,
            Path::new("open-coercion-failed.json"),
        )
        .expect("parse open predicate");
        schemas
            .validate("predicate.schema.json", &open)
            .expect_err("coercion-failed predicate must remain closed");
    }

    #[test]
    fn javascript_parse_float_predicate_binds_operator_and_decimal_operand_shape() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let input = serde_json::json!({
            "kind": "field",
            "result_type": "string",
            "field": {
                "field_id": "item-19-amount",
                "instance": {"kind": "singleton"}
            }
        });
        for valid in [
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "is-nan",
                "input": input.clone()
            }),
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "strict-equal",
                "input": input.clone(),
                "operand": {"type": "decimal", "value": "0"}
            }),
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "greater-than",
                "input": input.clone(),
                "operand": {"type": "decimal", "value": "1000"}
            }),
        ] {
            let valid = serde_json::from_value(valid).expect("build valid predicate");
            schemas
                .validate("predicate.schema.json", &valid)
                .expect("closed JavaScript parseFloat predicate is valid");
        }

        for invalid in [
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "is-nan",
                "input": input.clone(),
                "operand": {"type": "decimal", "value": "0"}
            }),
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "strict-equal",
                "input": input.clone()
            }),
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "greater-than",
                "input": input.clone(),
                "operand": {"type": "integer", "value": 0}
            }),
            serde_json::json!({
                "kind": "javascript-parse-float",
                "operator": "less-than",
                "input": input.clone(),
                "operand": {"type": "decimal", "value": "0"}
            }),
        ] {
            let invalid = serde_json::from_value(invalid).expect("build invalid predicate");
            schemas
                .validate("predicate.schema.json", &invalid)
                .expect_err("operator/operand shape must fail closed");
        }
    }

    #[test]
    fn serialization_variant_schema_matches_runtime_artifact_variant_id() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let valid_maximum = format!("a{}z", "b".repeat(126));
        let invalid_too_long = format!("a{}z", "b".repeat(127));
        let candidates = [
            "a".to_owned(),
            "save-path_v1.2".to_owned(),
            valid_maximum,
            String::new(),
            "-leading".to_owned(),
            "trailing-".to_owned(),
            "UPPER".to_owned(),
            "contains:colon".to_owned(),
            "contains/slash".to_owned(),
            "contains space".to_owned(),
            "unicodé".to_owned(),
            invalid_too_long,
        ];

        for candidate in candidates {
            let contract: crate::json::JsonValue = serde_json::from_value(serde_json::json!({
                "contract_version": "1.0.0",
                "artifacts": [{
                    "artifact_id": "artifact",
                    "target": "editable-save",
                    "variant_id": candidate.clone(),
                    "official": {
                        "state": "documented_only",
                        "summary": "not executable",
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "unresolved",
                        "reason": "not reviewed",
                        "source_refs": [{"source_id": "review"}]
                    },
                    "source_refs": [{"source_id": "review"}]
                }]
            }))
            .expect("build serialization contract");
            let schema_accepts = schemas
                .validate("serialization.schema.json", &contract)
                .is_ok();
            let runtime_accepts = ArtifactVariantId::parse(candidate.clone()).is_ok();
            assert_eq!(
                schema_accepts, runtime_accepts,
                "schema/runtime variant-ID parity for {candidate:?}"
            );
        }
    }

    #[test]
    fn serialization_schema_allows_top_level_but_rejects_nested_dynamic_groups() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let literal = |ordinal| {
            serde_json::json!({
                "kind": "reviewed-literal",
                "ordinal": ordinal,
                "exact_bytes": [10],
                "review_decision": {"source_id": "review"},
                "source_refs": [{"source_id": "review"}]
            })
        };
        let group = |ordinal, group_id: &str, nodes: Vec<serde_json::Value>| {
            serde_json::json!({
                "kind": "dynamic-group",
                "ordinal": ordinal,
                "group_id": group_id,
                "instance_order": "stable-instance-id-ascending",
                "min_occurs": 0,
                "max_occurs": 2,
                "nodes": nodes,
                "review_decision": {"source_id": "review"},
                "source_refs": [{"source_id": "review"}]
            })
        };

        let top_level = serialization_contract_with_node(group(1, "rows", vec![literal(2)]));
        schemas
            .validate("serialization.schema.json", &top_level)
            .expect("one top-level dynamic group is valid");

        let nested = serialization_contract_with_node(group(
            1,
            "parent-rows",
            vec![group(2, "child-rows", vec![literal(3)])],
        ));
        schemas
            .validate("serialization.schema.json", &nested)
            .expect_err("a nested dynamic group must fail even with the other branch documented");
    }

    #[test]
    fn serialization_body_codec_schema_model_and_runtime_alphabets_match() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let candidates = [
            "raw-literal",
            "legacy-javascript-escape",
            "utf8-percent-rfc3986-unreserved",
            "legacy-java-script-escape",
            "unknown",
            "",
        ];

        for candidate in candidates {
            let contract = serialization_contract_with_node(serde_json::json!({
                "kind": "pseudo-xml-field",
                "ordinal": 1,
                "key_projection": {"kind": "exact", "key": "Field"},
                "occurrence_projection": {"kind": "fixed", "occurrence": 1},
                "value_projection": {
                    "kind": "constant",
                    "value": {"type": "string", "value": "value"},
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                },
                "semantic_format": {
                    "absent": "reject",
                    "blank": "reject",
                    "present": {"kind": "text"}
                },
                "body_codec": candidate,
                "presence": {"kind": "always"},
                "source_refs": [{"source_id": "review"}]
            }));
            let schema_accepts = schemas
                .validate("serialization.schema.json", &contract)
                .is_ok();
            let model_accepts =
                serde_json::from_value::<SerializationBodyCodec>(serde_json::json!(candidate))
                    .is_ok();
            let runtime_accepts =
                serde_json::from_value::<BodyCodec>(serde_json::json!(candidate)).is_ok();
            assert_eq!(
                schema_accepts, model_accepts,
                "schema/model body-codec parity for {candidate:?}"
            );
            assert_eq!(
                model_accepts, runtime_accepts,
                "model/runtime body-codec parity for {candidate:?}"
            );
        }
    }

    #[test]
    fn serialization_derived_projection_requires_closed_instance_selector() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/schema/v2");
        let schemas = SchemaSet::load(&root).expect("load repository v2 schemas");
        let node = |instance| {
            serde_json::json!({
                "kind": "metadata-element",
                "ordinal": 1,
                "exact_tag": "Amount",
                "value_projection": {
                    "kind": "derived",
                    "calculation_id": "calculation",
                    "output_id": "output",
                    "instance": instance
                },
                "semantic_format": {
                    "absent": "reject",
                    "blank": "reject",
                    "present": {"kind": "base10-integer"}
                },
                "body_codec": "raw-literal",
                "presence": {"kind": "always"},
                "source_refs": [{"source_id": "review"}]
            })
        };

        schemas
            .validate(
                "serialization.schema.json",
                &serialization_contract_with_node(node(serde_json::json!({
                    "kind": "singleton"
                }))),
            )
            .expect("explicit singleton derived instance is valid");
        schemas
            .validate(
                "serialization.schema.json",
                &serialization_contract_with_node(node(serde_json::json!({
                    "kind": "stable-instance-id",
                    "instance_id": "row-1"
                }))),
            )
            .expect("explicit stable derived instance is valid");
        schemas
            .validate(
                "serialization.schema.json",
                &serialization_contract_with_node(node(serde_json::json!({
                    "kind": "current-group-instance"
                }))),
            )
            .expect("explicit current-group derived instance is valid");
        schemas
            .validate(
                "serialization.schema.json",
                &serialization_contract_with_node(node(serde_json::Value::Null)),
            )
            .expect_err("derived instance must never default from null");
    }
}
