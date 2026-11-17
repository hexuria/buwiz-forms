use std::collections::BTreeMap;

use crate::audit::AuditedSnapshot;
use crate::error::{CodegenError, Result};
use crate::json::JsonValue;
use crate::model::{
    DerivedInstanceSelector, EffectEvaluationMode as ModelEffectMode, EvaluationPolicyBranch,
    ProfileStatusBranch, SerializationAbsentPolicy, SerializationArtifact,
    SerializationArtifactBranch, SerializationArtifactTarget, SerializationBlankPolicy,
    SerializationBodyCodec, SerializationContract, SerializationDatePattern,
    SerializationDecimalSeparator, SerializationGroupInstanceOrder, SerializationGrouping,
    SerializationKeyProjection, SerializationNegativeRepresentation, SerializationNode,
    SerializationOccurrenceProjection, SerializationPresence, SerializationPresentFormat,
    SerializationRoundingMode, SerializationSemanticFormat, SerializationValueProjection,
};

/// Render the executable portion of one reviewed or test-only candidate snapshot.
///
/// Every policy branch retains its explicit source state. A documented-only or
/// unresolved candidate branch is emitted as non-executable and can never
/// borrow the other profile's effect policy.
pub(crate) fn render_static_rule_set(snapshot: &AuditedSnapshot) -> Result<String> {
    let emitter = Emitter {
        rule_set_id: &snapshot.document.identity.rule_set_id,
    };
    let profile_status = format!(
        "Profiled {{ official: {}, filing_safe: {} }}",
        emitter.render_profile_status_branch(&snapshot.document.profile_status.official),
        emitter.render_profile_status_branch(&snapshot.document.profile_status.filing_safe),
    );
    let effect_mode = format!(
        "Profiled {{ official: {}, filing_safe: {} }}",
        emitter.render_effect_mode_branch(&snapshot.document.evaluation_policy.official),
        emitter.render_effect_mode_branch(&snapshot.document.evaluation_policy.filing_safe),
    );
    let context_values = emitter.render_list(
        &snapshot.document.context_values,
        "$.context_values",
        |emitter, value, path| emitter.render_context_value(value, path),
    )?;
    let field_groups = emitter.render_list(
        &snapshot.document.field_groups,
        "$.field_groups",
        |emitter, value, path| emitter.render_field_group(value, path),
    )?;
    let fields = emitter.render_list(
        &snapshot.document.fields,
        "$.fields",
        |emitter, value, path| emitter.render_field(value, path),
    )?;
    let field_event_programs = emitter.render_list(
        &snapshot.document.field_event_programs,
        "$.field_event_programs",
        |emitter, value, path| emitter.render_field_event_program(value, path),
    )?;
    let evaluation_order = emitter.render_string_slice(&snapshot.document.evaluation_order);
    let calculations = emitter.render_list(
        &snapshot.document.calculations,
        "$.calculations",
        |emitter, value, path| emitter.render_calculation(value, path),
    )?;
    let rules = emitter.render_list(
        &snapshot.document.rules,
        "$.rules",
        |emitter, value, path| emitter.render_rule(value, path),
    )?;
    let workflow = emitter.render_workflow(&snapshot.document.workflow, "$.workflow")?;
    let serialization = emitter.render_serialization_contract(
        &snapshot.document.serialization,
        &snapshot.serialization_contract_sha256,
        "$.serialization",
    )?;

    Ok(format!(
        "\
pub static STATIC_SERIALIZATION_CONTRACT: StaticSerializationContract = {serialization};

pub static STATIC_RULE_SET_SPEC: StaticRuleSetSpec = StaticRuleSetSpec {{
    profile_status: {profile_status},
    effect_mode: {effect_mode},
    serialization: &STATIC_SERIALIZATION_CONTRACT,
    context_values: {context_values},
    field_groups: {field_groups},
    fields: {fields},
    field_event_programs: {field_event_programs},
    evaluation_order: {evaluation_order},
    calculations: {calculations},
    rules: {rules},
    workflow: {workflow},
}};

pub(super) static COMPILED_RULE_SET: LazyLock<StaticCompiledRuleSet> = LazyLock::new(|| {{
    StaticCompiledRuleSet::new(
        FormRevisionKey::parse(
            RULE_SET_ID,
            FORM_CODE,
            FORM_REVISION,
            OFFICIAL_PACKAGE_VERSION,
            SOURCE_SET_SHA256,
        )
        .expect(\"audited generated rule-set identity\"),
        &STATIC_RULE_SET_SPEC,
    )
}});
"
    ))
}

struct Emitter<'a> {
    rule_set_id: &'a str,
}

impl Emitter<'_> {
    fn error(&self, path: &str, message: impl std::fmt::Display) -> CodegenError {
        CodegenError::new(format!(
            "cannot emit rule-set snapshot `{}`: {message} at `{path}`",
            self.rule_set_id
        ))
    }

    fn unsupported(&self, path: &str, node: &str, reason: &str) -> CodegenError {
        self.error(
            path,
            format!("unsupported executable node `{node}`: {reason}"),
        )
    }

    fn render_profile_status_branch(&self, branch: &ProfileStatusBranch) -> &'static str {
        match branch {
            ProfileStatusBranch::Executable { .. } => "Branch::Executable(())",
            ProfileStatusBranch::DocumentedOnly { .. } => "Branch::DocumentedOnly",
            ProfileStatusBranch::Unresolved { .. } => "Branch::Unresolved",
        }
    }

    fn render_effect_mode(&self, mode: ModelEffectMode) -> &'static str {
        match mode {
            ModelEffectMode::ApplyAll => "EffectEvaluationMode::ApplyAll",
            ModelEffectMode::StopEffectsAfterFirstBlockingIssue => {
                "EffectEvaluationMode::StopEffectsAfterFirstBlockingIssue"
            }
        }
    }

    fn render_effect_mode_branch(&self, branch: &EvaluationPolicyBranch) -> String {
        match branch {
            EvaluationPolicyBranch::Executable { effect_mode, .. } => {
                format!(
                    "Branch::Executable({})",
                    self.render_effect_mode(*effect_mode)
                )
            }
            EvaluationPolicyBranch::DocumentedOnly { .. } => "Branch::DocumentedOnly".to_owned(),
            EvaluationPolicyBranch::Unresolved { .. } => "Branch::Unresolved".to_owned(),
        }
    }

    fn render_serialization_contract(
        &self,
        contract: &SerializationContract,
        canonical_sha256: &str,
        path: &str,
    ) -> Result<String> {
        let mut artifacts = Vec::with_capacity(contract.artifacts.len());
        for (index, artifact) in contract.artifacts.iter().enumerate() {
            artifacts.push(
                self.render_serialization_artifact(
                    artifact,
                    &format!("{path}.artifacts[{index}]"),
                )?,
            );
        }
        Ok(format!(
            "StaticSerializationContract {{ contract_version: {}, canonical_sha256: Some({}), artifacts: &[{}] }}",
            rust_string(&contract.contract_version),
            rust_string(canonical_sha256),
            artifacts.join(", "),
        ))
    }

    fn render_serialization_artifact(
        &self,
        artifact: &SerializationArtifact,
        path: &str,
    ) -> Result<String> {
        Ok(format!(
            "SerializationArtifactSpec {{ artifact_id: {}, target: {}, variant_id: {}, branches: Profiled {{ official: {}, filing_safe: {} }} }}",
            rust_string(&artifact.artifact_id),
            self.render_serialization_target(artifact.target),
            rust_string(&artifact.variant_id),
            self.render_serialization_branch(&artifact.official, &format!("{path}.official"))?,
            self.render_serialization_branch(
                &artifact.filing_safe,
                &format!("{path}.filing_safe")
            )?,
        ))
    }

    fn render_serialization_target(&self, target: SerializationArtifactTarget) -> &'static str {
        match target {
            SerializationArtifactTarget::EditableSave => {
                "SerializationArtifactTarget::EditableSave"
            }
            SerializationArtifactTarget::FinalizedSave => {
                "SerializationArtifactTarget::FinalizedSave"
            }
            SerializationArtifactTarget::EncryptedFinalCopy => {
                "SerializationArtifactTarget::EncryptedFinalCopy"
            }
            SerializationArtifactTarget::SubmissionPayload => {
                "SerializationArtifactTarget::SubmissionPayload"
            }
            SerializationArtifactTarget::HistoricalImportCompatibility => {
                "SerializationArtifactTarget::HistoricalImportCompatibility"
            }
        }
    }

    fn render_serialization_branch(
        &self,
        branch: &SerializationArtifactBranch,
        path: &str,
    ) -> Result<String> {
        match branch {
            SerializationArtifactBranch::Executable { nodes, .. } => Ok(format!(
                "Branch::Executable(SerializationPlan {{ nodes: {} }})",
                self.render_serialization_nodes(nodes, &format!("{path}.nodes"))?
            )),
            SerializationArtifactBranch::DocumentedOnly { .. } => {
                Ok("Branch::DocumentedOnly".to_owned())
            }
            SerializationArtifactBranch::Unresolved { .. } => Ok("Branch::Unresolved".to_owned()),
        }
    }

    fn render_serialization_nodes(
        &self,
        nodes: &[SerializationNode],
        path: &str,
    ) -> Result<String> {
        let mut rendered = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            rendered.push(self.render_serialization_node(node, &format!("{path}[{index}]"))?);
        }
        Ok(format!("&[{}]", rendered.join(", ")))
    }

    fn render_serialization_node(&self, node: &SerializationNode, path: &str) -> Result<String> {
        match node {
            SerializationNode::PseudoXmlField {
                ordinal,
                key_projection,
                occurrence_projection,
                value_projection,
                semantic_format,
                body_codec,
                presence,
                ..
            } => Ok(format!(
                "SerializationNode::PseudoXmlField(PseudoXmlFieldNode {{ ordinal: {ordinal}u32, key_projection: {}, occurrence_projection: {}, value_projection: {}, semantic_format: {}, body_codec: {}, presence: {} }})",
                self.render_serialization_key_projection(
                    key_projection,
                    &format!("{path}.key_projection")
                )?,
                self.render_serialization_occurrence_projection(
                    occurrence_projection,
                    &format!("{path}.occurrence_projection")
                )?,
                self.render_serialization_value_projection(
                    value_projection,
                    &format!("{path}.value_projection")
                )?,
                self.render_serialization_semantic_format(
                    semantic_format,
                    &format!("{path}.semantic_format")
                ),
                self.render_serialization_body_codec(*body_codec),
                self.render_serialization_presence(presence, &format!("{path}.presence"))?,
            )),
            SerializationNode::MetadataElement {
                ordinal,
                exact_tag,
                value_projection,
                semantic_format,
                body_codec,
                presence,
                ..
            } => Ok(format!(
                "SerializationNode::MetadataElement(MetadataElementNode {{ ordinal: {ordinal}u32, exact_tag: {}, value_projection: {}, semantic_format: {}, body_codec: {}, presence: {} }})",
                rust_string(exact_tag),
                self.render_serialization_value_projection(
                    value_projection,
                    &format!("{path}.value_projection")
                )?,
                self.render_serialization_semantic_format(
                    semantic_format,
                    &format!("{path}.semantic_format")
                ),
                self.render_serialization_body_codec(*body_codec),
                self.render_serialization_presence(presence, &format!("{path}.presence"))?,
            )),
            SerializationNode::ReviewedLiteral {
                ordinal,
                exact_bytes,
                ..
            } => Ok(format!(
                "SerializationNode::ReviewedLiteral(ReviewedLiteralNode {{ ordinal: {ordinal}u32, exact_bytes: &[{}] }})",
                exact_bytes
                    .iter()
                    .map(|byte| format!("{byte}u8"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            SerializationNode::DynamicGroup {
                ordinal,
                group_id,
                instance_order,
                min_occurs,
                max_occurs,
                nodes,
                ..
            } => Ok(format!(
                "SerializationNode::DynamicGroup(DynamicGroupNode {{ ordinal: {ordinal}u32, group_id: {}, instance_order: {}, min_occurs: {min_occurs}usize, max_occurs: {}, nodes: {} }})",
                rust_string(group_id),
                self.render_serialization_group_order(*instance_order),
                max_occurs
                    .map(|maximum| format!("Some({maximum}usize)"))
                    .unwrap_or_else(|| "None".to_owned()),
                self.render_serialization_nodes(nodes, &format!("{path}.nodes"))?,
            )),
        }
    }

    fn render_serialization_group_order(
        &self,
        order: SerializationGroupInstanceOrder,
    ) -> &'static str {
        match order {
            SerializationGroupInstanceOrder::StableInstanceIdAscending => {
                "SerializationGroupInstanceOrder::StableInstanceIdAscending"
            }
        }
    }

    fn render_serialization_key_projection(
        &self,
        projection: &SerializationKeyProjection,
        _path: &str,
    ) -> Result<String> {
        Ok(match projection {
            SerializationKeyProjection::Exact { key } => {
                format!("SerializationKeyProjection::Exact({})", rust_string(key))
            }
            SerializationKeyProjection::GroupIndexed {
                group_id,
                index_base,
                index_step,
                padding,
                prefix,
                suffix,
                ..
            } => format!(
                "SerializationKeyProjection::GroupIndexed(IndexedKeyProjection {{ group_id: {}, index_base: {index_base}u32, index_step: {index_step}u32, padding: {padding}u32, prefix: {}, suffix: {} }})",
                rust_string(group_id),
                rust_string(prefix),
                rust_string(suffix),
            ),
        })
    }

    fn render_serialization_occurrence_projection(
        &self,
        projection: &SerializationOccurrenceProjection,
        _path: &str,
    ) -> Result<String> {
        Ok(match projection {
            SerializationOccurrenceProjection::Fixed { occurrence } => {
                format!("SerializationOccurrenceProjection::Fixed({occurrence}u32)")
            }
            SerializationOccurrenceProjection::GroupIndexed {
                group_id,
                index_base,
                index_step,
                ..
            } => format!(
                "SerializationOccurrenceProjection::GroupIndexed(IndexedOccurrenceProjection {{ group_id: {}, index_base: {index_base}u32, index_step: {index_step}u32 }})",
                rust_string(group_id),
            ),
        })
    }

    fn render_serialization_value_projection(
        &self,
        projection: &SerializationValueProjection,
        path: &str,
    ) -> Result<String> {
        match projection {
            SerializationValueProjection::Field { field } => Ok(format!(
                "SerializationValueProjection::Field({})",
                self.render_field_ref(field, path)?
            )),
            SerializationValueProjection::Derived {
                calculation_id,
                output_id,
                instance,
            } => Ok(format!(
                "SerializationValueProjection::Derived {{ calculation_id: {}, output_id: {}, instance: {} }}",
                rust_string(calculation_id),
                rust_string(output_id),
                self.render_derived_instance_selector(instance),
            )),
            SerializationValueProjection::Context { context_value_id } => Ok(format!(
                "SerializationValueProjection::Context {{ context_value_id: {} }}",
                rust_string(context_value_id)
            )),
            SerializationValueProjection::Constant { value, .. } => Ok(format!(
                "SerializationValueProjection::Constant({})",
                self.render_typed_value(value, path)?
            )),
            SerializationValueProjection::Default { value, .. } => Ok(format!(
                "SerializationValueProjection::Default({})",
                self.render_typed_value(value, path)?
            )),
        }
    }

    fn render_serialization_semantic_format(
        &self,
        format: &SerializationSemanticFormat,
        _path: &str,
    ) -> String {
        format!(
            "SerializationSemanticFormat {{ absent: {}, blank: {}, present: {} }}",
            match format.absent {
                SerializationAbsentPolicy::Reject => "AbsentValuePolicy::Reject",
                SerializationAbsentPolicy::OmitOccurrence => {
                    "AbsentValuePolicy::OmitOccurrence"
                }
            },
            match format.blank {
                SerializationBlankPolicy::Reject => "BlankValuePolicy::Reject",
                SerializationBlankPolicy::EmitEmptyBody => "BlankValuePolicy::EmitEmptyBody",
                SerializationBlankPolicy::OmitOccurrence => {
                    "BlankValuePolicy::OmitOccurrence"
                }
            },
            self.render_serialization_present_format(&format.present),
        )
    }

    fn render_serialization_present_format(&self, format: &SerializationPresentFormat) -> String {
        match format {
            SerializationPresentFormat::Text => "SerializationPresentFormat::Text".to_owned(),
            SerializationPresentFormat::Boolean {
                true_text,
                false_text,
            } => format!(
                "SerializationPresentFormat::Boolean {{ true_text: {}, false_text: {} }}",
                rust_string(true_text),
                rust_string(false_text),
            ),
            SerializationPresentFormat::Base10Integer => {
                "SerializationPresentFormat::Base10Integer".to_owned()
            }
            SerializationPresentFormat::Decimal {
                scale,
                rounding,
                grouping,
                decimal_separator,
                negative,
            } => format!(
                "SerializationPresentFormat::Decimal(SerializationDecimalFormat {{ scale: {scale}u32, rounding: {}, grouping: {}, decimal_separator: {}, negative: {} }})",
                self.render_serialization_rounding(*rounding),
                self.render_serialization_grouping(*grouping),
                self.render_serialization_decimal_separator(*decimal_separator),
                self.render_serialization_negative(*negative),
            ),
            SerializationPresentFormat::Date { pattern } => format!(
                "SerializationPresentFormat::Date({})",
                self.render_serialization_date(*pattern)
            ),
        }
    }

    fn render_serialization_rounding(&self, rounding: SerializationRoundingMode) -> &'static str {
        match rounding {
            SerializationRoundingMode::None => "RoundingMode::None",
            SerializationRoundingMode::HalfUp => "RoundingMode::HalfUp",
            SerializationRoundingMode::HalfEven => "RoundingMode::HalfEven",
            SerializationRoundingMode::TowardZero => "RoundingMode::TowardZero",
            SerializationRoundingMode::AwayFromZero => "RoundingMode::AwayFromZero",
            SerializationRoundingMode::Floor => "RoundingMode::Floor",
            SerializationRoundingMode::Ceiling => "RoundingMode::Ceiling",
        }
    }

    fn render_serialization_grouping(&self, grouping: SerializationGrouping) -> &'static str {
        match grouping {
            SerializationGrouping::None => "SerializationGrouping::None",
            SerializationGrouping::Comma => "SerializationGrouping::Comma",
            SerializationGrouping::Period => "SerializationGrouping::Period",
            SerializationGrouping::Space => "SerializationGrouping::Space",
        }
    }

    fn render_serialization_decimal_separator(
        &self,
        separator: SerializationDecimalSeparator,
    ) -> &'static str {
        match separator {
            SerializationDecimalSeparator::Period => "SerializationDecimalSeparator::Period",
            SerializationDecimalSeparator::Comma => "SerializationDecimalSeparator::Comma",
        }
    }

    fn render_serialization_negative(
        &self,
        negative: SerializationNegativeRepresentation,
    ) -> &'static str {
        match negative {
            SerializationNegativeRepresentation::LeadingMinus => {
                "SerializationNegativeRepresentation::LeadingMinus"
            }
            SerializationNegativeRepresentation::TrailingMinus => {
                "SerializationNegativeRepresentation::TrailingMinus"
            }
            SerializationNegativeRepresentation::Parentheses => {
                "SerializationNegativeRepresentation::Parentheses"
            }
        }
    }

    fn render_serialization_date(&self, pattern: SerializationDatePattern) -> &'static str {
        match pattern {
            SerializationDatePattern::YyyyMmDdHyphen => "ExactDatePattern::YyyyMmDdHyphen",
            SerializationDatePattern::YyyyMmDdSlash => "ExactDatePattern::YyyyMmDdSlash",
            SerializationDatePattern::MmDdYyyySlash => "ExactDatePattern::MmDdYyyySlash",
            SerializationDatePattern::DdMmYyyySlash => "ExactDatePattern::DdMmYyyySlash",
            SerializationDatePattern::YyyyMmDdCompact => "ExactDatePattern::YyyyMmDdCompact",
        }
    }

    fn render_serialization_body_codec(&self, codec: SerializationBodyCodec) -> &'static str {
        match codec {
            SerializationBodyCodec::RawLiteral => "BodyCodec::RawLiteral",
            SerializationBodyCodec::LegacyJavascriptEscape => "BodyCodec::LegacyJavaScriptEscape",
            SerializationBodyCodec::Utf8PercentRfc3986Unreserved => {
                "BodyCodec::Utf8PercentRfc3986Unreserved"
            }
        }
    }

    fn render_serialization_presence(
        &self,
        presence: &SerializationPresence,
        path: &str,
    ) -> Result<String> {
        match presence {
            SerializationPresence::Always => Ok("SerializationPresence::Always".to_owned()),
            SerializationPresence::When { predicate } => Ok(format!(
                "SerializationPresence::When({})",
                self.render_predicate(predicate, &format!("{path}.predicate"))?
            )),
            SerializationPresence::Omitted => Ok("SerializationPresence::Omitted".to_owned()),
        }
    }

    fn render_list<F>(&self, values: &[JsonValue], path: &str, render: F) -> Result<String>
    where
        F: Fn(&Self, &JsonValue, &str) -> Result<String>,
    {
        let mut rendered = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            rendered.push(render(self, value, &format!("{path}[{index}]"))?);
        }
        Ok(format!("&[{}]", rendered.join(", ")))
    }

    fn render_context_value(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &["context_value_id", "required", "source_refs", "value_type"],
            path,
        )?;
        Ok(format!(
            "ContextValueSpec {{ context_value_id: {}, value_type: {}, required: {} }}",
            rust_string(self.string(object, "context_value_id", path)?),
            self.render_value_type(self.string(object, "value_type", path)?, path)?,
            self.boolean(object, "required", path)?,
        ))
    }

    fn render_field_group(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "group_id",
                "instance_identity",
                "max_occurs",
                "members",
                "min_occurs",
                "source_refs",
            ],
            path,
        )?;
        let max_occurs = match self.required(object, "max_occurs", path)? {
            JsonValue::Null => "None".to_owned(),
            value => format!("Some({})", self.usize_value(value, path)?),
        };
        Ok(format!(
            "FieldGroupSpec {{ group_id: {}, min_occurs: {}, max_occurs: {max_occurs}, members: {} }}",
            rust_string(self.string(object, "group_id", path)?),
            self.usize(object, "min_occurs", path)?,
            self.render_json_string_slice(self.array(object, "members", path)?, path)?,
        ))
    }

    fn render_field(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "behavior",
                "calculation_id",
                "control_kind",
                "field_id",
                "group_id",
                "requiredness",
                "serialized",
                "source_refs",
                "value_type",
            ],
            path,
        )?;
        let group_id =
            self.render_optional_string(self.required(object, "group_id", path)?, path)?;
        let calculation_id =
            self.render_optional_string(self.required(object, "calculation_id", path)?, path)?;
        let behavior = self.render_profiled_branches(
            self.required(object, "behavior", path)?,
            &format!("{path}.behavior"),
            |emitter, branch, branch_path| emitter.render_field_behavior(branch, branch_path),
        )?;
        Ok(format!(
            "FieldSpec {{ field_id: {}, value_type: {}, group_id: {group_id}, calculation_id: {calculation_id}, behavior: {behavior} }}",
            rust_string(self.string(object, "field_id", path)?),
            self.render_value_type(self.string(object, "value_type", path)?, path)?,
        ))
    }

    fn render_field_behavior(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<String> {
        self.require_keys(
            object,
            &[
                "coercion",
                "event_normalization",
                "normalization",
                "review_decision",
                "source_refs",
                "state",
            ],
            path,
        )?;
        let event_normalization = object
            .get("event_normalization")
            .map(|value| {
                self.render_field_event_normalization(value, &format!("{path}.event_normalization"))
            })
            .transpose()?
            .unwrap_or_else(|| "&[]".to_owned());
        Ok(format!(
            "FieldBehavior {{ normalization: {}, event_normalization: {event_normalization}, coercion: {} }}",
            self.render_normalization(
                self.required(object, "normalization", path)?,
                &format!("{path}.normalization"),
            )?,
            self.render_coercion(
                self.required(object, "coercion", path)?,
                &format!("{path}.coercion"),
            )?,
        ))
    }

    fn render_field_event_normalization(&self, value: &JsonValue, path: &str) -> Result<String> {
        self.render_json_list(
            self.array_value(value, path)?,
            path,
            |emitter, value, entry_path| {
                let object = emitter.object(value, entry_path)?;
                emitter.require_keys(object, &["normalization", "phase"], entry_path)?;
                let phase = emitter.enum_value(
                    emitter.string(object, "phase", entry_path)?,
                    &[
                        ("input", "ValidationPhase::Input"),
                        ("blur", "ValidationPhase::Blur"),
                        ("change", "ValidationPhase::Change"),
                    ],
                    entry_path,
                    "field-event-normalization.phase",
                )?;
                let normalization = emitter.render_normalization(
                    emitter.required(object, "normalization", entry_path)?,
                    &format!("{entry_path}.normalization"),
                )?;
                Ok(format!(
                    "FieldEventNormalization {{ phase: {phase}, normalization: {normalization} }}"
                ))
            },
        )
    }

    fn render_field_event_program(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &["phase", "profiles", "source_refs", "trigger_field_id"],
            path,
        )?;
        let phase = self.enum_value(
            self.string(object, "phase", path)?,
            &[
                ("input", "ValidationPhase::Input"),
                ("blur", "ValidationPhase::Blur"),
                ("change", "ValidationPhase::Change"),
            ],
            path,
            "field-event-program.phase",
        )?;
        let profiles = self.render_profiled_branches(
            self.required(object, "profiles", path)?,
            &format!("{path}.profiles"),
            |emitter, branch, branch_path| {
                emitter.render_field_event_program_branch(branch, branch_path)
            },
        )?;
        Ok(format!(
            "FieldEventProgramSpec {{ phase: {phase}, trigger_field_id: {}, profiles: {profiles} }}",
            rust_string(self.string(object, "trigger_field_id", path)?),
        ))
    }

    fn render_field_event_program_branch(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<String> {
        self.require_keys(
            object,
            &["review_decision", "source_refs", "state", "steps"],
            path,
        )?;
        let steps = self.render_json_list(
            self.array(object, "steps", path)?,
            &format!("{path}.steps"),
            |emitter, value, step_path| emitter.render_field_event_step(value, step_path),
        )?;
        Ok(format!("FieldEventProgram {{ steps: {steps} }}"))
    }

    fn render_field_event_step(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        match self.string(object, "kind", path)? {
            "rule" => {
                self.require_keys(object, &["kind", "rule_id"], path)?;
                Ok(format!(
                    "FieldEventStep::Rule {{ rule_id: {} }}",
                    rust_string(self.string(object, "rule_id", path)?),
                ))
            }
            "calculation" => {
                self.require_keys(
                    object,
                    &["calculation_id", "kind", "output_ids", "write_mode"],
                    path,
                )?;
                let write_mode = self.enum_value(
                    self.string(object, "write_mode", path)?,
                    &[
                        ("insert", "ScheduledOutputWriteMode::Insert"),
                        ("replace", "ScheduledOutputWriteMode::Replace"),
                    ],
                    path,
                    "field-event-calculation-step.write_mode",
                )?;
                Ok(format!(
                    "FieldEventStep::Calculation {{ calculation_id: {}, output_ids: {}, write_mode: {write_mode} }}",
                    rust_string(self.string(object, "calculation_id", path)?),
                    self.render_json_string_slice(self.array(object, "output_ids", path)?, path)?,
                ))
            }
            kind => Err(self.unsupported(path, "field-event-step", kind)),
        }
    }

    fn render_calculation(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "calculation_id",
                "depends_on",
                "output_ids",
                "phases",
                "profiles",
                "scope",
                "source_refs",
                "trigger_field_ids",
            ],
            path,
        )?;
        self.validate_event_binding_shape(object, path)?;
        let profiles = self.render_profiled_branches(
            self.required(object, "profiles", path)?,
            &format!("{path}.profiles"),
            |emitter, branch, branch_path| emitter.render_calculation_branch(branch, branch_path),
        )?;
        let trigger_field_ids = self.render_optional_trigger_field_ids(object, path)?;
        Ok(format!(
            "CalculationSpec {{ calculation_id: {}, scope: {}, depends_on: {}, phases: {}, trigger_field_ids: {trigger_field_ids}, profiles: {profiles} }}",
            rust_string(self.string(object, "calculation_id", path)?),
            self.render_evaluation_scope(
                self.required(object, "scope", path)?,
                &format!("{path}.scope"),
            )?,
            self.render_json_string_slice(self.array(object, "depends_on", path)?, path)?,
            self.render_phases(self.array(object, "phases", path)?, path)?,
        ))
    }

    fn render_calculation_branch(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<String> {
        self.require_keys(
            object,
            &[
                "condition",
                "outputs",
                "review_decision",
                "source_refs",
                "state",
            ],
            path,
        )?;
        let outputs = self.render_json_list(
            self.array(object, "outputs", path)?,
            &format!("{path}.outputs"),
            |emitter, value, value_path| emitter.render_calculation_output(value, value_path),
        )?;
        Ok(format!(
            "CalculationBranch {{ condition: &{}, outputs: {outputs} }}",
            self.render_predicate(
                self.required(object, "condition", path)?,
                &format!("{path}.condition"),
            )?,
        ))
    }

    fn render_calculation_output(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &["output_id", "rounding", "value", "writeback"],
            path,
        )?;
        let rounding = match self.required(object, "rounding", path)? {
            JsonValue::Null => "None".to_owned(),
            value @ JsonValue::Object(_) => format!(
                "Some(&[{}])",
                self.render_rounding(value, &format!("{path}.rounding"))?
            ),
            JsonValue::Array(values) => {
                if values.is_empty() {
                    return Err(self.error(
                        &format!("{path}.rounding"),
                        "rounding pipeline must contain at least one step",
                    ));
                }
                format!(
                    "Some({})",
                    self.render_json_list(
                        values,
                        &format!("{path}.rounding"),
                        |emitter, value, value_path| { emitter.render_rounding(value, value_path) },
                    )?
                )
            }
            _ => {
                return Err(self.error(
                    &format!("{path}.rounding"),
                    "expected rounding object, nonempty rounding array, or null",
                ));
            }
        };
        let writeback = match object.get("writeback") {
            None | Some(JsonValue::Null) => "None".to_owned(),
            Some(value) => format!(
                "Some({})",
                self.render_calculation_writeback(value, &format!("{path}.writeback"))?
            ),
        };
        Ok(format!(
            "CalculationOutput {{ output_id: {}, value: &{}, rounding: {rounding}, writeback: {writeback} }}",
            rust_string(self.string(object, "output_id", path)?),
            self.render_expression(
                self.required(object, "value", path)?,
                &format!("{path}.value"),
            )?,
        ))
    }

    fn render_calculation_writeback(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &["field", "format", "review_decision", "source_refs"],
            path,
        )?;
        let format_path = format!("{path}.format");
        let format = self.object(self.required(object, "format", path)?, &format_path)?;
        self.require_keys(format, &["kind"], &format_path)?;
        let format = self.enum_value(
            self.string(format, "kind", &format_path)?,
            &[(
                "offline-ebir-format-currency-v1",
                "CalculationWriteFormat::OfflineEbirFormatCurrencyV1",
            )],
            &format_path,
            "calculation-write-format.kind",
        )?;
        Ok(format!(
            "CalculationWriteback {{ field: {}, format: {format} }}",
            self.render_field_ref(
                self.required(object, "field", path)?,
                &format!("{path}.field"),
            )?,
        ))
    }

    fn render_rule(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "field_ids",
                "order",
                "phases",
                "profiles",
                "rule_id",
                "scope",
                "source_refs",
                "trigger_field_ids",
            ],
            path,
        )?;
        let event_rule = self.validate_event_binding_shape(object, path)?;
        let profiles = self.render_profiled_branches(
            self.required(object, "profiles", path)?,
            &format!("{path}.profiles"),
            |emitter, branch, branch_path| {
                emitter.render_rule_branch(branch, branch_path, event_rule)
            },
        )?;
        let trigger_field_ids = self.render_optional_trigger_field_ids(object, path)?;
        Ok(format!(
            "RuleSpec {{ rule_id: {}, scope: {}, order: {}, phases: {}, trigger_field_ids: {trigger_field_ids}, profiles: {profiles} }}",
            rust_string(self.string(object, "rule_id", path)?),
            self.render_evaluation_scope(
                self.required(object, "scope", path)?,
                &format!("{path}.scope"),
            )?,
            self.u32(object, "order", path)?,
            self.render_phases(self.array(object, "phases", path)?, path)?,
        ))
    }

    fn render_rule_branch(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
        event_rule: bool,
    ) -> Result<String> {
        self.require_keys(
            object,
            &[
                "effects",
                "predicate",
                "review_decision",
                "source_refs",
                "state",
            ],
            path,
        )?;
        let effects = self.render_json_list(
            self.array(object, "effects", path)?,
            &format!("{path}.effects"),
            |emitter, value, value_path| emitter.render_effect(value, value_path, event_rule),
        )?;
        Ok(format!(
            "RuleBranch {{ predicate: &{}, effects: {effects} }}",
            self.render_predicate(
                self.required(object, "predicate", path)?,
                &format!("{path}.predicate"),
            )?,
        ))
    }

    fn render_workflow(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        if let Some(state) = object.get("state") {
            let state = self.string_value(state, &format!("{path}.state"))?;
            return match state {
                "documented_only" => {
                    self.require_keys(object, &["source_refs", "state", "summary"], path)?;
                    Ok("Branch::DocumentedOnly".to_owned())
                }
                "unresolved" => {
                    self.require_keys(object, &["reason", "source_refs", "state"], path)?;
                    Ok("Branch::Unresolved".to_owned())
                }
                other => Err(self.unsupported(path, "workflow", other)),
            };
        }
        self.require_keys(object, &["initial_state", "states", "transitions"], path)?;
        let states = self.render_json_list(
            self.array(object, "states", path)?,
            &format!("{path}.states"),
            |emitter, value, value_path| emitter.render_workflow_state(value, value_path),
        )?;
        let transitions = self.render_json_list(
            self.array(object, "transitions", path)?,
            &format!("{path}.transitions"),
            |emitter, value, value_path| emitter.render_workflow_transition(value, value_path),
        )?;
        Ok(format!(
            "Branch::Executable(StaticWorkflowSpec {{ initial_state: {}, states: {states}, transitions: {transitions} }})",
            rust_string(self.string(object, "initial_state", path)?),
        ))
    }

    fn render_workflow_state(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(object, &["source_refs", "state_id", "terminal"], path)?;
        Ok(format!(
            "WorkflowStateSpec {{ state_id: {}, terminal: {} }}",
            rust_string(self.string(object, "state_id", path)?),
            self.boolean(object, "terminal", path)?,
        ))
    }

    fn render_workflow_transition(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "action",
                "evaluation_phase",
                "from_state",
                "profiles",
                "source_refs",
                "to_state",
                "transition_id",
            ],
            path,
        )?;
        let profiles = self.render_profiled_branches(
            self.required(object, "profiles", path)?,
            &format!("{path}.profiles"),
            |emitter, branch, branch_path| {
                emitter.render_workflow_transition_branch(branch, branch_path)
            },
        )?;
        Ok(format!(
            "WorkflowTransitionSpec {{ transition_id: {}, from_state: {}, action: {}, evaluation_phase: {}, to_state: {}, profiles: {profiles} }}",
            rust_string(self.string(object, "transition_id", path)?),
            rust_string(self.string(object, "from_state", path)?),
            self.enum_value(
                self.string(object, "action", path)?,
                &[
                    ("edit", "WorkflowAction::Edit"),
                    ("save", "WorkflowAction::Save"),
                    ("validate", "WorkflowAction::Validate"),
                    ("final-copy", "WorkflowAction::FinalCopy"),
                    ("submit", "WorkflowAction::Submit"),
                    ("print-preview", "WorkflowAction::PrintPreview"),
                ],
                path,
                "workflow.action",
            )?,
            self.enum_value(
                self.string(object, "evaluation_phase", path)?,
                &[
                    ("input", "ValidationPhase::Input"),
                    ("blur", "ValidationPhase::Blur"),
                    ("change", "ValidationPhase::Change"),
                    ("blur-change", "ValidationPhase::BlurChange"),
                    ("page-navigation", "ValidationPhase::PageNavigation"),
                    ("save", "ValidationPhase::Save"),
                    ("draft-preview", "ValidationPhase::DraftPreview"),
                    ("validate", "ValidationPhase::Validate"),
                    ("final-copy", "ValidationPhase::FinalCopy"),
                    ("submit", "ValidationPhase::Submit"),
                ],
                path,
                "workflow.evaluation_phase",
            )?,
            rust_string(self.string(object, "to_state", path)?),
        ))
    }

    fn render_workflow_transition_branch(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<String> {
        self.require_keys(
            object,
            &[
                "effects",
                "guard",
                "review_decision",
                "source_refs",
                "state",
            ],
            path,
        )?;
        let effects = self.render_json_list(
            self.array(object, "effects", path)?,
            &format!("{path}.effects"),
            |emitter, value, value_path| emitter.render_workflow_effect(value, value_path),
        )?;
        Ok(format!(
            "WorkflowTransitionBranch {{ guard: &{}, effects: {effects} }}",
            self.render_predicate(
                self.required(object, "guard", path)?,
                &format!("{path}.guard"),
            )?,
        ))
    }

    fn render_workflow_effect(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        match self.string(object, "kind", path)? {
            "set-workflow-state" => {
                self.require_keys(object, &["kind", "state_id"], path)?;
                Ok(format!(
                    "Effect::SetWorkflowState {{ state_id: {} }}",
                    rust_string(self.string(object, "state_id", path)?),
                ))
            }
            "emit-notification" => {
                self.require_keys(
                    object,
                    &["channel", "kind", "message", "official_message"],
                    path,
                )?;
                let official_message = self.render_optional_string(
                    self.required(object, "official_message", path)?,
                    &format!("{path}.official_message"),
                )?;
                Ok(format!(
                    "Effect::EmitNotification {{ channel: {}, message: {}, official_message: {official_message} }}",
                    self.enum_value(
                        self.string(object, "channel", path)?,
                        &[("alert", "WorkflowNotificationChannel::Alert")],
                        path,
                        "emit-notification.channel",
                    )?,
                    rust_string(self.string(object, "message", path)?),
                ))
            }
            kind => Err(self.unsupported(
                path,
                "workflow-effect",
                &format!("effect `{kind}` is not executable in workflow transitions"),
            )),
        }
    }

    fn render_profiled_branches<F>(
        &self,
        value: &JsonValue,
        path: &str,
        render_executable: F,
    ) -> Result<String>
    where
        F: Fn(&Self, &BTreeMap<String, JsonValue>, &str) -> Result<String>,
    {
        let object = self.object(value, path)?;
        self.require_keys(object, &["filing_safe", "official"], path)?;
        let official_path = format!("{path}.official");
        let filing_safe_path = format!("{path}.filing_safe");
        Ok(format!(
            "Profiled {{ official: {}, filing_safe: {} }}",
            self.render_branch(
                self.required(object, "official", path)?,
                &official_path,
                &render_executable,
            )?,
            self.render_branch(
                self.required(object, "filing_safe", path)?,
                &filing_safe_path,
                &render_executable,
            )?,
        ))
    }

    fn render_branch<F>(
        &self,
        value: &JsonValue,
        path: &str,
        render_executable: &F,
    ) -> Result<String>
    where
        F: Fn(&Self, &BTreeMap<String, JsonValue>, &str) -> Result<String>,
    {
        let object = self.object(value, path)?;
        match self.string(object, "state", path)? {
            "executable" => Ok(format!(
                "Branch::Executable({})",
                render_executable(self, object, path)?
            )),
            "documented_only" => {
                self.require_keys(object, &["source_refs", "state", "summary"], path)?;
                Ok("Branch::DocumentedOnly".to_owned())
            }
            "unresolved" => {
                self.require_keys(object, &["reason", "source_refs", "state"], path)?;
                Ok("Branch::Unresolved".to_owned())
            }
            state => Err(self.unsupported(path, "profile-branch", state)),
        }
    }

    fn render_normalization(&self, value: &JsonValue, path: &str) -> Result<String> {
        self.render_json_list(
            self.array_value(value, path)?,
            path,
            |emitter, value, step_path| {
                let object = emitter.object(value, step_path)?;
                let kind = emitter.string(object, "kind", step_path)?;
                match kind {
                    "trim" => {
                        emitter.require_keys(object, &["kind", "side"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::Trim {{ side: {} }}",
                            emitter.enum_value(
                                emitter.string(object, "side", step_path)?,
                                &[
                                    ("both", "TrimSide::Both"),
                                    ("start", "TrimSide::Start"),
                                    ("end", "TrimSide::End"),
                                ],
                                step_path,
                                "trim.side",
                            )?
                        ))
                    }
                    "change-case" => {
                        emitter.require_keys(object, &["case", "kind"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::ChangeCase {{ case: {} }}",
                            emitter.enum_value(
                                emitter.string(object, "case", step_path)?,
                                &[
                                    ("upper", "LetterCase::Upper"),
                                    ("lower", "LetterCase::Lower"),
                                ],
                                step_path,
                                "change-case.case",
                            )?
                        ))
                    }
                    "replace-literal" => {
                        emitter.require_keys(object, &["from", "kind", "to"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::ReplaceLiteral {{ from: {}, to: {} }}",
                            rust_string(emitter.string(object, "from", step_path)?),
                            rust_string(emitter.string(object, "to", step_path)?),
                        ))
                    }
                    "strip-characters" => {
                        emitter.require_keys(object, &["characters", "kind"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::StripCharacters {{ characters: {} }}",
                            rust_string(emitter.string(object, "characters", step_path)?),
                        ))
                    }
                    "digits-only" => {
                        emitter.require_keys(object, &["kind"], step_path)?;
                        Ok("NormalizationStep::DigitsOnly".to_owned())
                    }
                    "normalize-newlines" => {
                        emitter.require_keys(object, &["kind", "style"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::NormalizeNewlines {{ style: {} }}",
                            emitter.enum_value(
                                emitter.string(object, "style", step_path)?,
                                &[("lf", "NewlineStyle::Lf"), ("crlf", "NewlineStyle::Crlf")],
                                step_path,
                                "normalize-newlines.style",
                            )?
                        ))
                    }
                    "date-format" => {
                        emitter.require_keys(object, &["format", "kind"], step_path)?;
                        Ok(format!(
                            "NormalizationStep::DateFormat {{ format: {} }}",
                            emitter.render_date_format(
                                emitter.string(object, "format", step_path)?,
                                step_path,
                            )?
                        ))
                    }
                    "decimal-format" => {
                        emitter.require_keys(
                            object,
                            &["grouping", "kind", "rounding"],
                            step_path,
                        )?;
                        Ok(format!(
                            "NormalizationStep::DecimalFormat {{ grouping: {}, rounding: {} }}",
                            emitter.enum_value(
                                emitter.string(object, "grouping", step_path)?,
                                &[
                                    ("none", "DecimalGrouping::None"),
                                    ("comma", "DecimalGrouping::Comma"),
                                ],
                                step_path,
                                "decimal-format.grouping",
                            )?,
                            emitter.render_rounding(
                                emitter.required(object, "rounding", step_path)?,
                                &format!("{step_path}.rounding"),
                            )?,
                        ))
                    }
                    "offline-ebir-money-round-v1" => {
                        emitter.require_keys(object, &["kind"], step_path)?;
                        Ok("NormalizationStep::OfflineEbirMoneyRoundV1".to_owned())
                    }
                    "offline-ebir-parse-float-fixed-zero-v1" => {
                        emitter.require_keys(object, &["kind"], step_path)?;
                        Ok("NormalizationStep::OfflineEbirParseFloatFixedZeroV1".to_owned())
                    }
                    _ => Err(emitter.unsupported(step_path, "normalization", kind)),
                }
            },
        )
    }

    fn render_coercion(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        let kind = self.string(object, "kind", path)?;
        match kind {
            "string" => {
                self.require_keys(object, &["kind", "on_empty"], path)?;
                Ok(format!(
                    "Coercion::String {{ on_empty: {} }}",
                    self.enum_value(
                        self.string(object, "on_empty", path)?,
                        &[
                            ("empty-string", "StringEmptyPolicy::EmptyString"),
                            ("null", "StringEmptyPolicy::Null"),
                            ("error", "StringEmptyPolicy::Error"),
                        ],
                        path,
                        "string-coercion.on-empty",
                    )?
                ))
            }
            "decimal" => {
                self.require_keys(
                    object,
                    &["decimal", "grouping", "kind", "on_empty", "on_invalid"],
                    path,
                )?;
                Ok(format!(
                    "Coercion::Decimal {{ decimal: {}, grouping: {}, on_empty: {}, on_invalid: {} }}",
                    self.render_decimal_policy(
                        self.required(object, "decimal", path)?,
                        &format!("{path}.decimal"),
                    )?,
                    self.enum_value(
                        self.string(object, "grouping", path)?,
                        &[
                            ("forbidden", "InputGrouping::Forbidden"),
                            ("comma", "InputGrouping::Comma"),
                        ],
                        path,
                        "decimal-coercion.grouping",
                    )?,
                    self.render_numeric_empty(self.string(object, "on_empty", path)?, path)?,
                    self.render_invalid_value(self.string(object, "on_invalid", path)?, path)?,
                ))
            }
            "integer" => {
                self.require_keys(object, &["kind", "on_empty", "on_invalid"], path)?;
                Ok(format!(
                    "Coercion::Integer {{ on_empty: {}, on_invalid: {} }}",
                    self.render_numeric_empty(self.string(object, "on_empty", path)?, path)?,
                    self.render_invalid_value(self.string(object, "on_invalid", path)?, path)?,
                ))
            }
            "boolean" => {
                self.require_keys(
                    object,
                    &[
                        "false_values",
                        "kind",
                        "on_empty",
                        "on_invalid",
                        "true_values",
                    ],
                    path,
                )?;
                Ok(format!(
                    "Coercion::Boolean {{ true_values: {}, false_values: {}, on_empty: {}, on_invalid: {} }}",
                    self.render_json_string_slice(self.array(object, "true_values", path)?, path)?,
                    self.render_json_string_slice(self.array(object, "false_values", path)?, path)?,
                    self.enum_value(
                        self.string(object, "on_empty", path)?,
                        &[
                            ("null", "BooleanEmptyPolicy::Null"),
                            ("false", "BooleanEmptyPolicy::False"),
                            ("error", "BooleanEmptyPolicy::Error"),
                        ],
                        path,
                        "boolean-coercion.on-empty",
                    )?,
                    self.render_invalid_value(self.string(object, "on_invalid", path)?, path)?,
                ))
            }
            "date" => {
                self.require_keys(
                    object,
                    &["accepted_formats", "kind", "on_empty", "on_invalid"],
                    path,
                )?;
                let formats = self
                    .array(object, "accepted_formats", path)?
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        self.render_date_format(
                            self.string_value(value, &format!("{path}.accepted_formats[{index}]"))?,
                            path,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(format!(
                    "Coercion::Date {{ accepted_formats: &[{}], on_empty: {}, on_invalid: {} }}",
                    formats.join(", "),
                    self.enum_value(
                        self.string(object, "on_empty", path)?,
                        &[
                            ("null", "DateEmptyPolicy::Null"),
                            ("error", "DateEmptyPolicy::Error"),
                        ],
                        path,
                        "date-coercion.on-empty",
                    )?,
                    self.render_invalid_value(self.string(object, "on_invalid", path)?, path)?,
                ))
            }
            _ => Err(self.unsupported(path, "coercion", kind)),
        }
    }

    fn render_decimal_policy(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(
            object,
            &[
                "division_scale",
                "overflow",
                "precision",
                "rounding",
                "scale",
            ],
            path,
        )?;
        let precision = self.u32(object, "precision", path)?;
        let scale = self.u32(object, "scale", path)?;
        if scale > precision {
            return Err(self.error(
                path,
                format!("invalid decimal policy: scale {scale} exceeds precision {precision}"),
            ));
        }
        Ok(format!(
            "DecimalPolicy {{ precision: {}, scale: {}, division_scale: {}, rounding: {}, overflow: {} }}",
            precision,
            scale,
            self.u32(object, "division_scale", path)?,
            self.render_rounding(
                self.required(object, "rounding", path)?,
                &format!("{path}.rounding"),
            )?,
            self.enum_value(
                self.string(object, "overflow", path)?,
                &[
                    ("error", "OverflowPolicy::Error"),
                    ("clamp", "OverflowPolicy::Clamp"),
                ],
                path,
                "decimal-policy.overflow",
            )?,
        ))
    }

    fn render_rounding(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(object, &["mode", "scale"], path)?;
        Ok(format!(
            "Rounding {{ mode: {}, scale: {} }}",
            self.enum_value(
                self.string(object, "mode", path)?,
                &[
                    ("none", "RoundingMode::None"),
                    ("half-up", "RoundingMode::HalfUp"),
                    ("half-even", "RoundingMode::HalfEven"),
                    ("half-ceiling", "RoundingMode::HalfCeiling"),
                    ("toward-zero", "RoundingMode::TowardZero"),
                    ("away-from-zero", "RoundingMode::AwayFromZero"),
                    ("floor", "RoundingMode::Floor"),
                    ("ceiling", "RoundingMode::Ceiling"),
                ],
                path,
                "rounding.mode",
            )?,
            self.u32(object, "scale", path)?,
        ))
    }

    fn render_decimal_division_policy(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(object, &["rounding", "scale"], path)?;
        let scale = self.u32(object, "scale", path)?;
        if scale > 18 {
            return Err(self.error(path, format!("decimal division scale {scale} exceeds 18")));
        }
        Ok(format!(
            "DecimalDivisionPolicy {{ scale: {scale}, rounding: {} }}",
            self.enum_value(
                self.string(object, "rounding", path)?,
                &[
                    ("none", "RoundingMode::None"),
                    ("half-up", "RoundingMode::HalfUp"),
                    ("half-even", "RoundingMode::HalfEven"),
                    ("toward-zero", "RoundingMode::TowardZero"),
                    ("away-from-zero", "RoundingMode::AwayFromZero"),
                    ("floor", "RoundingMode::Floor"),
                    ("ceiling", "RoundingMode::Ceiling"),
                ],
                path,
                "decimal-division-policy.rounding",
            )?,
        ))
    }

    fn render_expression(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        let kind = self.string(object, "kind", path)?;
        match kind {
            "literal" => {
                self.require_keys(object, &["kind", "value"], path)?;
                Ok(format!(
                    "Expression::Literal({})",
                    self.render_typed_value(
                        self.required(object, "value", path)?,
                        &format!("{path}.value"),
                    )?
                ))
            }
            "field" => {
                self.require_keys(object, &["field", "kind", "result_type"], path)?;
                Ok(format!(
                    "Expression::Field {{ result_type: {}, field: {} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.render_field_ref(
                        self.required(object, "field", path)?,
                        &format!("{path}.field"),
                    )?,
                ))
            }
            "derived" => {
                self.require_keys(
                    object,
                    &[
                        "calculation_id",
                        "instance",
                        "kind",
                        "output_id",
                        "result_type",
                    ],
                    path,
                )?;
                Ok(format!(
                    "Expression::Derived {{ result_type: {}, calculation_id: {}, output_id: {}, instance: {} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    rust_string(self.string(object, "calculation_id", path)?),
                    rust_string(self.string(object, "output_id", path)?),
                    self.render_derived_instance_selector_value(
                        self.required(object, "instance", path)?,
                        &format!("{path}.instance"),
                    )?,
                ))
            }
            "context" => {
                self.require_keys(object, &["context_value_id", "kind", "result_type"], path)?;
                Ok(format!(
                    "Expression::Context {{ result_type: {}, context_value_id: {} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    rust_string(self.string(object, "context_value_id", path)?),
                ))
            }
            "unary" => {
                self.require_keys(
                    object,
                    &["kind", "operand", "operator", "result_type"],
                    path,
                )?;
                Ok(format!(
                    "Expression::Unary {{ result_type: {}, operator: {}, operand: &{} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.enum_value(
                        self.string(object, "operator", path)?,
                        &[
                            ("negate", "UnaryOperator::Negate"),
                            ("absolute", "UnaryOperator::Absolute"),
                            ("length", "UnaryOperator::Length"),
                        ],
                        path,
                        "unary.operator",
                    )?,
                    self.render_expression(
                        self.required(object, "operand", path)?,
                        &format!("{path}.operand"),
                    )?,
                ))
            }
            "binary" => {
                self.require_keys(
                    object,
                    &[
                        "division_policy",
                        "kind",
                        "left",
                        "operator",
                        "result_type",
                        "right",
                    ],
                    path,
                )?;
                let result_type = self.string(object, "result_type", path)?;
                let operator = self.string(object, "operator", path)?;
                let division_policy = if operator == "divide" {
                    if result_type != "decimal" {
                        return Err(self.error(path, "binary divide requires decimal result_type"));
                    }
                    format!(
                        "Some({})",
                        self.render_decimal_division_policy(
                            self.required(object, "division_policy", path)?,
                            &format!("{path}.division_policy"),
                        )?
                    )
                } else {
                    if object.contains_key("division_policy") {
                        return Err(self.error(
                            path,
                            format!(
                                "binary operator `{operator}` must not carry `division_policy`"
                            ),
                        ));
                    }
                    "None".to_owned()
                };
                Ok(format!(
                    "Expression::Binary {{ result_type: {}, operator: {}, division_policy: {division_policy}, left: &{}, right: &{} }}",
                    self.render_value_type(result_type, path)?,
                    self.enum_value(
                        operator,
                        &[
                            ("add", "BinaryOperator::Add"),
                            ("subtract", "BinaryOperator::Subtract"),
                            ("multiply", "BinaryOperator::Multiply"),
                            ("divide", "BinaryOperator::Divide"),
                            ("remainder", "BinaryOperator::Remainder"),
                            ("concat", "BinaryOperator::Concat"),
                        ],
                        path,
                        "binary.operator",
                    )?,
                    self.render_expression(
                        self.required(object, "left", path)?,
                        &format!("{path}.left"),
                    )?,
                    self.render_expression(
                        self.required(object, "right", path)?,
                        &format!("{path}.right"),
                    )?,
                ))
            }
            "nary" => {
                self.require_keys(
                    object,
                    &["kind", "operands", "operator", "result_type"],
                    path,
                )?;
                let operands = self.render_json_list(
                    self.array(object, "operands", path)?,
                    &format!("{path}.operands"),
                    |emitter, value, value_path| emitter.render_expression(value, value_path),
                )?;
                Ok(format!(
                    "Expression::Nary {{ result_type: {}, operator: {}, operands: {operands} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.enum_value(
                        self.string(object, "operator", path)?,
                        &[
                            ("sum", "NaryOperator::Sum"),
                            ("minimum", "NaryOperator::Minimum"),
                            ("maximum", "NaryOperator::Maximum"),
                            ("concat", "NaryOperator::Concat"),
                            ("coalesce", "NaryOperator::Coalesce"),
                        ],
                        path,
                        "nary.operator",
                    )?,
                ))
            }
            "conditional" => {
                self.require_keys(
                    object,
                    &[
                        "condition",
                        "kind",
                        "result_type",
                        "when_false",
                        "when_true",
                    ],
                    path,
                )?;
                Ok(format!(
                    "Expression::Conditional {{ result_type: {}, condition: &{}, when_true: &{}, when_false: &{} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.render_predicate(
                        self.required(object, "condition", path)?,
                        &format!("{path}.condition"),
                    )?,
                    self.render_expression(
                        self.required(object, "when_true", path)?,
                        &format!("{path}.when_true"),
                    )?,
                    self.render_expression(
                        self.required(object, "when_false", path)?,
                        &format!("{path}.when_false"),
                    )?,
                ))
            }
            "coerce" => {
                self.require_keys(object, &["coercion", "input", "kind", "result_type"], path)?;
                Ok(format!(
                    "Expression::Coerce {{ result_type: {}, input: &{}, coercion: {} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                    self.render_coercion(
                        self.required(object, "coercion", path)?,
                        &format!("{path}.coercion"),
                    )?,
                ))
            }
            "split-component" => {
                self.require_keys(
                    object,
                    &["delimiter", "index", "input", "kind", "result_type"],
                    path,
                )?;
                let delimiter = self.string(object, "delimiter", path)?;
                if delimiter != "/" {
                    return Err(self.error(path, "split-component delimiter must be `/`"));
                }
                let index = self.u32(object, "index", path)?;
                let result_type = self.string(object, "result_type", path)?;
                if result_type != "string" {
                    return Err(self.error(path, "split-component requires string result_type"));
                }
                Ok(format!(
                    "Expression::SplitComponent {{ result_type: ValueType::String, input: &{}, delimiter: {}, index: {index} }}",
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                    rust_string(delimiter),
                ))
            }
            "javascript-parse-int-radix10" => {
                self.require_keys(object, &["input", "kind", "result_type"], path)?;
                if self.string(object, "result_type", path)? != "integer" {
                    return Err(self.error(
                        path,
                        "javascript-parse-int-radix10 requires integer result_type",
                    ));
                }
                Ok(format!(
                    "Expression::JavaScriptParseIntRadix10 {{ result_type: ValueType::Integer, input: &{} }}",
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                ))
            }
            "javascript-date-local-day" => {
                self.require_keys(
                    object,
                    &["day", "kind", "month_index", "result_type", "year"],
                    path,
                )?;
                if self.string(object, "result_type", path)? != "integer" {
                    return Err(self.error(
                        path,
                        "javascript-date-local-day requires integer result_type",
                    ));
                }
                Ok(format!(
                    "Expression::JavaScriptDateLocalDay {{ result_type: ValueType::Integer, year: &{}, month_index: &{}, day: &{} }}",
                    self.render_expression(
                        self.required(object, "year", path)?,
                        &format!("{path}.year"),
                    )?,
                    self.render_expression(
                        self.required(object, "month_index", path)?,
                        &format!("{path}.month_index"),
                    )?,
                    self.render_expression(
                        self.required(object, "day", path)?,
                        &format!("{path}.day"),
                    )?,
                ))
            }
            "canonical-local-date-day" => {
                self.require_keys(object, &["input", "kind", "result_type"], path)?;
                if self.string(object, "result_type", path)? != "integer" {
                    return Err(self.error(
                        path,
                        "canonical-local-date-day requires integer result_type",
                    ));
                }
                Ok(format!(
                    "Expression::CanonicalLocalDateDay {{ result_type: ValueType::Integer, input: &{} }}",
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                ))
            }
            "group-aggregate" => {
                self.require_keys(
                    object,
                    &["group_id", "kind", "operator", "result_type", "value"],
                    path,
                )?;
                Ok(format!(
                    "Expression::GroupAggregate {{ result_type: {}, operator: {}, group_id: {}, value: &{} }}",
                    self.render_value_type(self.string(object, "result_type", path)?, path)?,
                    self.enum_value(
                        self.string(object, "operator", path)?,
                        &[
                            ("sum", "GroupAggregateOperator::Sum"),
                            ("minimum", "GroupAggregateOperator::Minimum"),
                            ("maximum", "GroupAggregateOperator::Maximum"),
                            ("count", "GroupAggregateOperator::Count"),
                            ("count-present", "GroupAggregateOperator::CountPresent"),
                        ],
                        path,
                        "group-aggregate.operator",
                    )?,
                    rust_string(self.string(object, "group_id", path)?),
                    self.render_expression(
                        self.required(object, "value", path)?,
                        &format!("{path}.value"),
                    )?,
                ))
            }
            _ => Err(self.unsupported(path, "expression", kind)),
        }
    }

    fn render_predicate(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        let kind = self.string(object, "kind", path)?;
        match kind {
            "constant" => {
                self.require_keys(object, &["kind", "value"], path)?;
                Ok(format!(
                    "Predicate::Constant({})",
                    self.boolean(object, "value", path)?
                ))
            }
            "not" => {
                self.require_keys(object, &["kind", "predicate"], path)?;
                Ok(format!(
                    "Predicate::Not(&{})",
                    self.render_predicate(
                        self.required(object, "predicate", path)?,
                        &format!("{path}.predicate"),
                    )?
                ))
            }
            "all" | "any" => {
                self.require_keys(object, &["kind", "predicates"], path)?;
                let predicates = self.render_json_list(
                    self.array(object, "predicates", path)?,
                    &format!("{path}.predicates"),
                    |emitter, value, value_path| emitter.render_predicate(value, value_path),
                )?;
                let variant = if kind == "all" { "All" } else { "Any" };
                Ok(format!("Predicate::{variant}({predicates})"))
            }
            "compare" => {
                self.require_keys(object, &["kind", "left", "operator", "right"], path)?;
                Ok(format!(
                    "Predicate::Compare {{ operator: {}, left: &{}, right: &{} }}",
                    self.enum_value(
                        self.string(object, "operator", path)?,
                        &[
                            ("equal", "CompareOperator::Equal"),
                            ("not-equal", "CompareOperator::NotEqual"),
                            ("less-than", "CompareOperator::LessThan"),
                            ("less-than-or-equal", "CompareOperator::LessThanOrEqual"),
                            ("greater-than", "CompareOperator::GreaterThan"),
                            (
                                "greater-than-or-equal",
                                "CompareOperator::GreaterThanOrEqual",
                            ),
                        ],
                        path,
                        "compare.operator",
                    )?,
                    self.render_expression(
                        self.required(object, "left", path)?,
                        &format!("{path}.left"),
                    )?,
                    self.render_expression(
                        self.required(object, "right", path)?,
                        &format!("{path}.right"),
                    )?,
                ))
            }
            "is-empty" | "is-present" | "is-null" => {
                self.require_keys(object, &["kind", "value"], path)?;
                Ok(format!(
                    "Predicate::Presence {{ operator: {}, value: &{} }}",
                    self.enum_value(
                        kind,
                        &[
                            ("is-empty", "PresenceOperator::IsEmpty"),
                            ("is-present", "PresenceOperator::IsPresent"),
                            ("is-null", "PresenceOperator::IsNull"),
                        ],
                        path,
                        "presence.kind",
                    )?,
                    self.render_expression(
                        self.required(object, "value", path)?,
                        &format!("{path}.value"),
                    )?,
                ))
            }
            "coercion-failed" => {
                self.require_keys(object, &["field", "kind"], path)?;
                Ok(format!(
                    "Predicate::CoercionFailed {{ field: {} }}",
                    self.render_field_ref(
                        self.required(object, "field", path)?,
                        &format!("{path}.field"),
                    )?,
                ))
            }
            "javascript-parse-float" => {
                let operator = self.string(object, "operator", path)?;
                let (operator, operand) = match operator {
                    "is-nan" => {
                        self.require_keys(object, &["input", "kind", "operator"], path)?;
                        ("JavaScriptParseFloatOperator::IsNaN", "None".to_owned())
                    }
                    "strict-equal" | "greater-than" => {
                        self.require_keys(object, &["input", "kind", "operand", "operator"], path)?;
                        let operand_path = format!("{path}.operand");
                        let operand =
                            self.object(self.required(object, "operand", path)?, &operand_path)?;
                        self.require_keys(operand, &["type", "value"], &operand_path)?;
                        if self.string(operand, "type", &operand_path)? != "decimal" {
                            return Err(self.error(
                                &operand_path,
                                "javascript-parse-float operand must be a decimal typed value",
                            ));
                        }
                        let (coefficient, scale) = self.parse_decimal(
                            self.string(operand, "value", &operand_path)?,
                            &format!("{operand_path}.value"),
                        )?;
                        let operator = if operator == "strict-equal" {
                            "JavaScriptParseFloatOperator::StrictEqual"
                        } else {
                            "JavaScriptParseFloatOperator::GreaterThan"
                        };
                        (
                            operator,
                            format!(
                                "Some(DecimalLiteral {{ coefficient: {coefficient}i128, scale: {scale}u32 }})"
                            ),
                        )
                    }
                    _ => {
                        return Err(self.error(
                            path,
                            format!("unsupported javascript-parse-float operator `{operator}`"),
                        ));
                    }
                };
                Ok(format!(
                    "Predicate::JavaScriptParseFloat {{ operator: {operator}, input: &{}, operand: {operand} }}",
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                ))
            }
            "javascript-global-is-nan-logical-or" => {
                self.require_keys(object, &["inputs", "kind"], path)?;
                let inputs = self.array(object, "inputs", path)?;
                if inputs.is_empty() {
                    return Err(self.error(
                        &format!("{path}.inputs"),
                        "javascript-global-is-nan-logical-or requires nonempty inputs",
                    ));
                }
                let inputs = self.render_json_list(
                    inputs,
                    &format!("{path}.inputs"),
                    |emitter, value, value_path| emitter.render_expression(value, value_path),
                )?;
                Ok(format!(
                    "Predicate::JavaScriptGlobalIsNaNLogicalOr {{ inputs: {inputs} }}"
                ))
            }
            "javascript-number-compare" => {
                self.require_keys(object, &["input", "kind", "operand", "operator"], path)?;
                Ok(format!(
                    "Predicate::JavaScriptNumberCompare {{ operator: {}, input: &{}, operand: &{} }}",
                    self.enum_value(
                        self.string(object, "operator", path)?,
                        &[
                            ("less-than", "JavaScriptNumberCompareOperator::LessThan",),
                            (
                                "greater-than",
                                "JavaScriptNumberCompareOperator::GreaterThan",
                            ),
                            (
                                "strict-equal",
                                "JavaScriptNumberCompareOperator::StrictEqual",
                            ),
                        ],
                        path,
                        "javascript-number-compare.operator",
                    )?,
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                    self.render_expression(
                        self.required(object, "operand", path)?,
                        &format!("{path}.operand"),
                    )?,
                ))
            }
            "checksum" => {
                self.require_keys(object, &["algorithm", "input", "kind"], path)?;
                Ok(format!(
                    "Predicate::Checksum {{ algorithm: {}, input: &{} }}",
                    self.enum_value(
                        self.string(object, "algorithm", path)?,
                        &[("offline-ebir-tin-v1", "ChecksumAlgorithm::OfflineEbirTinV1",)],
                        path,
                        "checksum.algorithm",
                    )?,
                    self.render_expression(
                        self.required(object, "input", path)?,
                        &format!("{path}.input"),
                    )?,
                ))
            }
            "matches" => Err(self.unsupported(
                path,
                "predicate.matches",
                "no offline packaged matcher backend exists for the audited regex dialect",
            )),
            "in" => {
                self.require_keys(object, &["candidates", "kind", "value"], path)?;
                let candidates = self.render_json_list(
                    self.array(object, "candidates", path)?,
                    &format!("{path}.candidates"),
                    |emitter, value, value_path| emitter.render_typed_value(value, value_path),
                )?;
                Ok(format!(
                    "Predicate::In {{ value: &{}, candidates: {candidates} }}",
                    self.render_expression(
                        self.required(object, "value", path)?,
                        &format!("{path}.value"),
                    )?,
                ))
            }
            "group-quantifier" => {
                self.require_keys(
                    object,
                    &["group_id", "kind", "predicate", "quantifier"],
                    path,
                )?;
                Ok(format!(
                    "Predicate::GroupQuantifier {{ quantifier: {}, group_id: {}, predicate: &{} }}",
                    self.enum_value(
                        self.string(object, "quantifier", path)?,
                        &[
                            ("any", "GroupQuantifier::Any"),
                            ("all", "GroupQuantifier::All"),
                            ("none", "GroupQuantifier::None"),
                        ],
                        path,
                        "group-quantifier.quantifier",
                    )?,
                    rust_string(self.string(object, "group_id", path)?),
                    self.render_predicate(
                        self.required(object, "predicate", path)?,
                        &format!("{path}.predicate"),
                    )?,
                ))
            }
            _ => Err(self.unsupported(path, "predicate", kind)),
        }
    }

    fn render_effect(&self, value: &JsonValue, path: &str, event_rule: bool) -> Result<String> {
        let object = self.object(value, path)?;
        let kind = self.string(object, "kind", path)?;
        match kind {
            "emit-issue" => {
                self.require_keys(
                    object,
                    &[
                        "assessment",
                        "fields",
                        "kind",
                        "message",
                        "official_message",
                        "severity",
                    ],
                    path,
                )?;
                let official_message = object
                    .get("official_message")
                    .map(|value| self.render_optional_string(value, path))
                    .transpose()?
                    .unwrap_or_else(|| "None".to_owned());
                let fields = self.render_json_list(
                    self.array(object, "fields", path)?,
                    &format!("{path}.fields"),
                    |emitter, value, value_path| emitter.render_field_ref(value, value_path),
                )?;
                Ok(format!(
                    "Effect::EmitIssue {{ severity: {}, message: {}, official_message: {official_message}, assessment: {}, fields: {fields} }}",
                    self.enum_value(
                        self.string(object, "severity", path)?,
                        &[
                            ("advisory", "RuleSeverity::Advisory"),
                            ("blocking", "RuleSeverity::Blocking"),
                        ],
                        path,
                        "emit-issue.severity",
                    )?,
                    rust_string(self.string(object, "message", path)?),
                    self.enum_value(
                        self.string(object, "assessment", path)?,
                        &[
                            ("verified-correct", "RuleAssessment::VerifiedCorrect"),
                            (
                                "official-bug-compatible",
                                "RuleAssessment::OfficialBugCompatible",
                            ),
                            (
                                "incorrect-official-behavior",
                                "RuleAssessment::IncorrectOfficialBehavior",
                            ),
                            ("ambiguous", "RuleAssessment::Ambiguous"),
                            ("unverified", "RuleAssessment::Unverified"),
                            ("obsolete", "RuleAssessment::Obsolete"),
                        ],
                        path,
                        "emit-issue.assessment",
                    )?,
                ))
            }
            "set-raw-field-value" => {
                if !event_rule {
                    return Err(self.unsupported(
                        path,
                        "effect.set-raw-field-value",
                        "raw field assignments are permitted only on exact field-event rules",
                    ));
                }
                self.require_keys(object, &["field", "kind", "value"], path)?;
                Ok(format!(
                    "Effect::SetRawFieldValue {{ field: {}, value: {} }}",
                    self.render_field_ref(
                        self.required(object, "field", path)?,
                        &format!("{path}.field"),
                    )?,
                    self.render_static_raw_value(
                        self.required(object, "value", path)?,
                        &format!("{path}.value"),
                    )?,
                ))
            }
            "set-derived" => Err(self.unsupported(
                path,
                "effect.set-derived",
                "effects run after calculations, so mutating a derived output could leave \
                 dependent generated outputs internally inconsistent",
            )),
            "normalize-field" => Err(self.unsupported(
                path,
                "effect.normalize-field",
                "effects run after calculations, so mutating canonical input could leave \
                 dependent generated outputs internally inconsistent",
            )),
            "set-workflow-state" => Err(self.unsupported(
                path,
                "effect.set-workflow-state",
                "EvaluationResult has no reviewed workflow-state output channel",
            )),
            _ => Err(self.unsupported(path, "effect", kind)),
        }
    }

    fn render_evaluation_scope(&self, value: &JsonValue, path: &str) -> Result<String> {
        let scope = self.object(value, path)?;
        match self.string(scope, "kind", path)? {
            "singleton" => {
                self.require_keys(scope, &["kind"], path)?;
                Ok("EvaluationScope::Singleton".to_owned())
            }
            "each-group" => {
                self.require_keys(scope, &["group_id", "kind"], path)?;
                Ok(format!(
                    "EvaluationScope::EachGroup({})",
                    rust_string(self.string(scope, "group_id", path)?)
                ))
            }
            kind => Err(self.unsupported(path, "evaluation-scope", kind)),
        }
    }

    fn render_derived_instance_selector(&self, selector: &DerivedInstanceSelector) -> String {
        match selector {
            DerivedInstanceSelector::Singleton => "DerivedInstanceSelector::Singleton".to_owned(),
            DerivedInstanceSelector::CurrentGroupInstance => {
                "DerivedInstanceSelector::CurrentGroupInstance".to_owned()
            }
            DerivedInstanceSelector::StableInstanceId { instance_id } => format!(
                "DerivedInstanceSelector::StableInstanceId({})",
                rust_string(instance_id)
            ),
        }
    }

    fn render_derived_instance_selector_value(
        &self,
        value: &JsonValue,
        path: &str,
    ) -> Result<String> {
        let instance = self.object(value, path)?;
        match self.string(instance, "kind", path)? {
            "singleton" => {
                self.require_keys(instance, &["kind"], path)?;
                Ok("DerivedInstanceSelector::Singleton".to_owned())
            }
            "current-group-instance" => {
                self.require_keys(instance, &["kind"], path)?;
                Ok("DerivedInstanceSelector::CurrentGroupInstance".to_owned())
            }
            "stable-instance-id" => {
                self.require_keys(instance, &["instance_id", "kind"], path)?;
                Ok(format!(
                    "DerivedInstanceSelector::StableInstanceId({})",
                    rust_string(self.string(instance, "instance_id", path)?)
                ))
            }
            kind => Err(self.unsupported(path, "derived-instance", kind)),
        }
    }

    fn render_field_ref(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(object, &["field_id", "instance"], path)?;
        let instance_path = format!("{path}.instance");
        let selector = self.render_field_instance_selector(
            self.required(object, "instance", path)?,
            &instance_path,
        )?;
        Ok(format!(
            "FieldRef {{ field_id: {}, instance: {selector} }}",
            rust_string(self.string(object, "field_id", path)?)
        ))
    }

    fn render_static_raw_value(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        match self.string(object, "state", path)? {
            "absent" => {
                self.require_keys(object, &["state"], path)?;
                Ok("StaticRawValue::Absent".to_owned())
            }
            "text" => {
                self.require_keys(object, &["state", "text"], path)?;
                Ok(format!(
                    "StaticRawValue::Text({})",
                    rust_string(self.string(object, "text", path)?)
                ))
            }
            state => Err(self.unsupported(path, "static-raw-value", state)),
        }
    }

    fn render_field_instance_selector(&self, value: &JsonValue, path: &str) -> Result<String> {
        let instance = self.object(value, path)?;
        let selector = match self.string(instance, "kind", path)? {
            "singleton" => {
                self.require_keys(instance, &["kind"], path)?;
                "FieldInstanceSelector::Singleton".to_owned()
            }
            "current-group-instance" => {
                self.require_keys(instance, &["kind"], path)?;
                "FieldInstanceSelector::CurrentGroupInstance".to_owned()
            }
            "stable-instance-id" => {
                self.require_keys(instance, &["instance_id", "kind"], path)?;
                format!(
                    "FieldInstanceSelector::StableInstanceId({})",
                    rust_string(self.string(instance, "instance_id", path)?)
                )
            }
            kind => return Err(self.unsupported(path, "field-instance", kind)),
        };
        Ok(selector)
    }

    fn render_typed_value(&self, value: &JsonValue, path: &str) -> Result<String> {
        let object = self.object(value, path)?;
        self.require_keys(object, &["type", "value"], path)?;
        let payload = self.required(object, "value", path)?;
        match self.string(object, "type", path)? {
            "null" if matches!(payload, JsonValue::Null) => Ok("TypedValue::Null".to_owned()),
            "string" => Ok(format!(
                "TypedValue::String({})",
                rust_string(self.string_value(payload, path)?)
            )),
            "boolean" => Ok(format!(
                "TypedValue::Boolean({})",
                self.boolean_value(payload, path)?
            )),
            "integer" => Ok(format!(
                "TypedValue::Integer({}i128)",
                self.i128_value(payload, path)?
            )),
            "decimal" => {
                let (coefficient, scale) =
                    self.parse_decimal(self.string_value(payload, path)?, path)?;
                Ok(format!(
                    "TypedValue::Decimal(DecimalLiteral {{ coefficient: {coefficient}i128, scale: {scale}u32 }})"
                ))
            }
            "date" => {
                let (year, month, day) =
                    self.parse_date(self.string_value(payload, path)?, path)?;
                Ok(format!(
                    "TypedValue::Date(DateLiteral {{ year: {year}u16, month: {month}u8, day: {day}u8 }})"
                ))
            }
            kind => Err(self.unsupported(path, "typed-value", kind)),
        }
    }

    fn render_value_type(&self, value: &str, path: &str) -> Result<&'static str> {
        self.enum_value(
            value,
            &[
                ("null", "ValueType::Null"),
                ("string", "ValueType::String"),
                ("boolean", "ValueType::Boolean"),
                ("integer", "ValueType::Integer"),
                ("decimal", "ValueType::Decimal"),
                ("date", "ValueType::Date"),
            ],
            path,
            "value-type",
        )
    }

    fn render_date_format(&self, value: &str, path: &str) -> Result<&'static str> {
        self.enum_value(
            value,
            &[
                ("yyyy-mm-dd", "DateFormat::YearMonthDay"),
                ("mm/dd/yyyy", "DateFormat::MonthSlashDaySlashYear"),
                ("mm-dd-yyyy", "DateFormat::MonthDashDayDashYear"),
            ],
            path,
            "date-format",
        )
    }

    fn render_numeric_empty(&self, value: &str, path: &str) -> Result<&'static str> {
        self.enum_value(
            value,
            &[
                ("null", "NumericEmptyPolicy::Null"),
                ("zero", "NumericEmptyPolicy::Zero"),
                ("error", "NumericEmptyPolicy::Error"),
            ],
            path,
            "numeric-empty-policy",
        )
    }

    fn render_invalid_value(&self, value: &str, path: &str) -> Result<&'static str> {
        self.enum_value(
            value,
            &[
                ("error", "InvalidValuePolicy::Error"),
                ("preserve-raw", "InvalidValuePolicy::PreserveRaw"),
            ],
            path,
            "invalid-value-policy",
        )
    }

    fn render_phases(&self, phases: &[JsonValue], path: &str) -> Result<String> {
        let mut rendered = Vec::with_capacity(phases.len());
        for (index, phase) in phases.iter().enumerate() {
            let phase_path = format!("{path}.phases[{index}]");
            rendered.push(self.enum_value(
                self.string_value(phase, &phase_path)?,
                &[
                    ("input", "ValidationPhase::Input"),
                    ("blur", "ValidationPhase::Blur"),
                    ("change", "ValidationPhase::Change"),
                    ("blur-change", "ValidationPhase::BlurChange"),
                    ("page-navigation", "ValidationPhase::PageNavigation"),
                    ("save", "ValidationPhase::Save"),
                    ("draft-preview", "ValidationPhase::DraftPreview"),
                    ("validate", "ValidationPhase::Validate"),
                    ("final-copy", "ValidationPhase::FinalCopy"),
                    ("submit", "ValidationPhase::Submit"),
                ],
                &phase_path,
                "validation-phase",
            )?);
        }
        Ok(format!("&[{}]", rendered.join(", ")))
    }

    fn validate_event_binding_shape(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<bool> {
        let phases = self.array(object, "phases", path)?;
        if phases.is_empty() {
            return Err(self.error(&format!("{path}.phases"), "phases must not be empty"));
        }
        let mut has_event = false;
        for (index, phase) in phases.iter().enumerate() {
            match self.string_value(phase, &format!("{path}.phases[{index}]"))? {
                "input" | "blur" | "change" => has_event = true,
                _ => {}
            }
        }
        match (has_event, object.get("trigger_field_ids")) {
            (true, Some(value)) if value != &JsonValue::Null => {
                if self
                    .array_value(value, &format!("{path}.trigger_field_ids"))?
                    .is_empty()
                {
                    return Err(self.error(
                        &format!("{path}.trigger_field_ids"),
                        "field-event entry requires at least one trigger field",
                    ));
                }
            }
            (true, None | Some(JsonValue::Null)) => {
                return Err(self.error(
                    path,
                    "field-event entry is missing required `trigger_field_ids`",
                ));
            }
            (false, Some(value)) if value != &JsonValue::Null => {
                return Err(self.error(path, "non-event entry must not carry `trigger_field_ids`"));
            }
            (false, None | Some(JsonValue::Null)) => {}
            _ => unreachable!("all event-binding shapes are covered"),
        }
        Ok(has_event)
    }

    fn render_optional_trigger_field_ids(
        &self,
        object: &BTreeMap<String, JsonValue>,
        path: &str,
    ) -> Result<String> {
        object.get("trigger_field_ids").map_or_else(
            || Ok("&[]".to_owned()),
            |value| match value {
                JsonValue::Null => Ok("&[]".to_owned()),
                _ => self.render_json_string_slice(
                    self.array_value(value, &format!("{path}.trigger_field_ids"))?,
                    &format!("{path}.trigger_field_ids"),
                ),
            },
        )
    }

    fn render_string_slice(&self, values: &[String]) -> String {
        format!(
            "&[{}]",
            values
                .iter()
                .map(|value| rust_string(value))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn render_json_string_slice(&self, values: &[JsonValue], path: &str) -> Result<String> {
        let mut rendered = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            rendered.push(rust_string(
                self.string_value(value, &format!("{path}[{index}]"))?,
            ));
        }
        Ok(format!("&[{}]", rendered.join(", ")))
    }

    fn render_json_list<F>(&self, values: &[JsonValue], path: &str, render: F) -> Result<String>
    where
        F: Fn(&Self, &JsonValue, &str) -> Result<String>,
    {
        let mut rendered = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            rendered.push(render(self, value, &format!("{path}[{index}]"))?);
        }
        Ok(format!("&[{}]", rendered.join(", ")))
    }

    fn render_optional_string(&self, value: &JsonValue, path: &str) -> Result<String> {
        match value {
            JsonValue::Null => Ok("None".to_owned()),
            JsonValue::String(value) => Ok(format!("Some({})", rust_string(value))),
            _ => Err(self.error(path, "expected string or null")),
        }
    }

    fn parse_decimal(&self, value: &str, path: &str) -> Result<(i128, u32)> {
        let scale = value
            .split_once('.')
            .map(|(_, fraction)| fraction.len())
            .unwrap_or(0);
        let compact = value.replace('.', "");
        let coefficient = compact.parse::<i128>().map_err(|error| {
            self.error(path, format!("decimal literal is out of range: {error}"))
        })?;
        let scale = u32::try_from(scale)
            .map_err(|_| self.error(path, "decimal scale cannot be represented"))?;
        Ok((coefficient, scale))
    }

    fn parse_date(&self, value: &str, path: &str) -> Result<(u16, u8, u8)> {
        let mut parts = value.split('-');
        let year = parts
            .next()
            .and_then(|part| part.parse::<u16>().ok())
            .ok_or_else(|| self.error(path, "invalid date literal year"))?;
        let month = parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .ok_or_else(|| self.error(path, "invalid date literal month"))?;
        let day = parts
            .next()
            .and_then(|part| part.parse::<u8>().ok())
            .ok_or_else(|| self.error(path, "invalid date literal day"))?;
        if parts.next().is_some() {
            return Err(self.error(path, "invalid date literal"));
        }
        Ok((year, month, day))
    }

    fn enum_value(
        &self,
        value: &str,
        variants: &[(&str, &'static str)],
        path: &str,
        node: &str,
    ) -> Result<&'static str> {
        variants
            .iter()
            .find_map(|(wire, rust)| (*wire == value).then_some(*rust))
            .ok_or_else(|| self.unsupported(path, node, value))
    }

    fn object<'a>(
        &self,
        value: &'a JsonValue,
        path: &str,
    ) -> Result<&'a BTreeMap<String, JsonValue>> {
        value
            .object()
            .ok_or_else(|| self.error(path, "expected object"))
    }

    fn require_keys(
        &self,
        object: &BTreeMap<String, JsonValue>,
        allowed: &[&str],
        path: &str,
    ) -> Result<()> {
        if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(self.error(path, format!("unexpected executable property `{key}`")));
        }
        Ok(())
    }

    fn required<'a>(
        &self,
        object: &'a BTreeMap<String, JsonValue>,
        key: &str,
        path: &str,
    ) -> Result<&'a JsonValue> {
        object
            .get(key)
            .ok_or_else(|| self.error(path, format!("missing required property `{key}`")))
    }

    fn string<'a>(
        &self,
        object: &'a BTreeMap<String, JsonValue>,
        key: &str,
        path: &str,
    ) -> Result<&'a str> {
        self.string_value(self.required(object, key, path)?, &format!("{path}.{key}"))
    }

    fn string_value<'a>(&self, value: &'a JsonValue, path: &str) -> Result<&'a str> {
        value
            .as_str()
            .ok_or_else(|| self.error(path, "expected string"))
    }

    fn boolean(&self, object: &BTreeMap<String, JsonValue>, key: &str, path: &str) -> Result<bool> {
        self.boolean_value(self.required(object, key, path)?, &format!("{path}.{key}"))
    }

    fn boolean_value(&self, value: &JsonValue, path: &str) -> Result<bool> {
        match value {
            JsonValue::Bool(value) => Ok(*value),
            _ => Err(self.error(path, "expected boolean")),
        }
    }

    fn array<'a>(
        &self,
        object: &'a BTreeMap<String, JsonValue>,
        key: &str,
        path: &str,
    ) -> Result<&'a [JsonValue]> {
        self.array_value(self.required(object, key, path)?, &format!("{path}.{key}"))
    }

    fn array_value<'a>(&self, value: &'a JsonValue, path: &str) -> Result<&'a [JsonValue]> {
        match value {
            JsonValue::Array(values) => Ok(values),
            _ => Err(self.error(path, "expected array")),
        }
    }

    fn u32(&self, object: &BTreeMap<String, JsonValue>, key: &str, path: &str) -> Result<u32> {
        let value = self.u64_value(self.required(object, key, path)?, &format!("{path}.{key}"))?;
        u32::try_from(value).map_err(|_| self.error(path, format!("`{key}` exceeds u32")))
    }

    fn usize(&self, object: &BTreeMap<String, JsonValue>, key: &str, path: &str) -> Result<usize> {
        self.usize_value(self.required(object, key, path)?, &format!("{path}.{key}"))
    }

    fn usize_value(&self, value: &JsonValue, path: &str) -> Result<usize> {
        let value = self.u64_value(value, path)?;
        usize::try_from(value).map_err(|_| self.error(path, "integer exceeds usize"))
    }

    fn u64_value(&self, value: &JsonValue, path: &str) -> Result<u64> {
        match value {
            JsonValue::Number(value) => value
                .as_u64()
                .ok_or_else(|| self.error(path, "expected non-negative integer")),
            _ => Err(self.error(path, "expected integer")),
        }
    }

    fn i128_value(&self, value: &JsonValue, path: &str) -> Result<i128> {
        match value {
            JsonValue::Number(value) => {
                if let Some(value) = value.as_i64() {
                    Ok(i128::from(value))
                } else if let Some(value) = value.as_u64() {
                    Ok(i128::from(value))
                } else {
                    Err(self.error(path, "integer is outside the supported exact range"))
                }
            }
            _ => Err(self.error(path, "expected integer")),
        }
    }
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::Emitter;
    use crate::json::JsonValue;
    use crate::model::SerializationValueProjection;
    use serde_json::json;

    #[test]
    fn emitter_preserves_scope_derived_selector_and_aggregate_value() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let scope: JsonValue =
            serde_json::from_value(json!({"kind": "each-group", "group_id": "rows"})).unwrap();
        assert_eq!(
            emitter.render_evaluation_scope(&scope, "$.scope").unwrap(),
            "EvaluationScope::EachGroup(\"rows\")"
        );

        let expression: JsonValue = serde_json::from_value(json!({
            "kind": "group-aggregate",
            "result_type": "decimal",
            "operator": "sum",
            "group_id": "rows",
            "value": {
                "kind": "derived",
                "result_type": "decimal",
                "calculation_id": "row-tax",
                "output_id": "value",
                "instance": {"kind": "current-group-instance"}
            }
        }))
        .unwrap();
        let rendered = emitter
            .render_expression(&expression, "$.expression")
            .unwrap();
        assert!(rendered.contains("Expression::GroupAggregate"));
        assert!(rendered.contains("value: &Expression::Derived"));
        assert!(rendered.contains("DerivedInstanceSelector::CurrentGroupInstance"));

        let projection: SerializationValueProjection = serde_json::from_value(json!({
            "kind": "derived",
            "calculation_id": "row-tax",
            "output_id": "value",
            "instance": {"kind": "stable-instance-id", "instance_id": "row-1"}
        }))
        .unwrap();
        let rendered = emitter
            .render_serialization_value_projection(&projection, "$.projection")
            .unwrap();
        assert!(rendered.contains("DerivedInstanceSelector::StableInstanceId(\"row-1\")"));
    }

    #[test]
    fn emitter_renders_javascript_date_compatibility_pipeline() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let field = json!({
            "kind": "field",
            "result_type": "string",
            "field": {
                "field_id": "return-to",
                "instance": {"kind": "singleton"}
            }
        });
        let split = |index| {
            json!({
                "kind": "split-component",
                "result_type": "string",
                "input": field.clone(),
                "delimiter": "/",
                "index": index
            })
        };
        let parse = |index| {
            json!({
                "kind": "javascript-parse-int-radix10",
                "result_type": "integer",
                "input": split(index)
            })
        };
        let expression: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-date-local-day",
            "result_type": "integer",
            "year": parse(2),
            "month_index": {
                "kind": "binary",
                "result_type": "integer",
                "operator": "subtract",
                "left": parse(0),
                "right": {"kind": "literal", "value": {"type": "integer", "value": 1}}
            },
            "day": parse(1)
        }))
        .unwrap();

        let rendered = emitter
            .render_expression(&expression, "$.expression")
            .unwrap();
        assert!(rendered.contains("Expression::JavaScriptDateLocalDay"));
        assert!(rendered.contains("Expression::JavaScriptParseIntRadix10"));
        assert!(rendered.contains("Expression::SplitComponent"));
        assert!(rendered.contains("delimiter: \"/\""));
    }

    #[test]
    fn emitter_renders_javascript_parse_float_predicates_with_closed_operands() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let input = json!({
            "kind": "field",
            "result_type": "string",
            "field": {
                "field_id": "item-19-amount",
                "instance": {"kind": "singleton"}
            }
        });
        let is_nan: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-parse-float",
            "operator": "is-nan",
            "input": input.clone()
        }))
        .unwrap();
        let greater_than: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-parse-float",
            "operator": "greater-than",
            "input": input,
            "operand": {"type": "decimal", "value": "1000"}
        }))
        .unwrap();

        assert_eq!(
            emitter.render_predicate(&is_nan, "$.predicate").unwrap(),
            "Predicate::JavaScriptParseFloat { operator: JavaScriptParseFloatOperator::IsNaN, input: &Expression::Field { result_type: ValueType::String, field: FieldRef { field_id: \"item-19-amount\", instance: FieldInstanceSelector::Singleton } }, operand: None }"
        );
        let rendered = emitter
            .render_predicate(&greater_than, "$.predicate")
            .unwrap();
        assert!(rendered.contains("JavaScriptParseFloatOperator::GreaterThan"));
        assert!(rendered.contains("coefficient: 1000i128, scale: 0u32"));
    }

    #[test]
    fn emitter_renders_closed_checksum_algorithm() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let predicate: JsonValue = serde_json::from_value(json!({
            "kind": "checksum",
            "algorithm": "offline-ebir-tin-v1",
            "input": {
                "kind": "field",
                "result_type": "string",
                "field": {
                    "field_id": "spouse-tin",
                    "instance": {"kind": "singleton"}
                }
            }
        }))
        .unwrap();

        let rendered = emitter.render_predicate(&predicate, "$.predicate").unwrap();
        assert!(rendered.contains("Predicate::Checksum"));
        assert!(rendered.contains("ChecksumAlgorithm::OfflineEbirTinV1"));
        assert!(rendered.contains("field_id: \"spouse-tin\""));
    }

    #[test]
    fn emitter_preserves_legacy_rounding_shapes_and_orders_pipeline_steps() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let render = |rounding: serde_json::Value| {
            let output: JsonValue = serde_json::from_value(json!({
                "output_id": "rounded",
                "value": {
                    "kind": "literal",
                    "value": {"type": "decimal", "value": "0.499"}
                },
                "rounding": rounding
            }))
            .unwrap();
            emitter.render_calculation_output(&output, "$.output")
        };

        let unrounded = render(json!(null)).unwrap();
        assert!(unrounded.contains("rounding: None"));
        assert!(unrounded.contains("writeback: None"));
        assert!(
            render(json!({"mode": "half-up", "scale": 2}))
                .unwrap()
                .contains("rounding: Some(&[Rounding { mode: RoundingMode::HalfUp, scale: 2 }])")
        );

        let pipeline = render(json!([
            {"mode": "half-up", "scale": 2},
            {"mode": "half-ceiling", "scale": 0}
        ]))
        .unwrap();
        assert!(pipeline.contains(
            "rounding: Some(&[Rounding { mode: RoundingMode::HalfUp, scale: 2 }, Rounding { mode: RoundingMode::HalfCeiling, scale: 0 }])"
        ));

        let error = render(json!([])).expect_err("empty rounding pipeline must fail closed");
        assert!(
            error
                .message()
                .contains("rounding pipeline must contain at least one step")
        );
    }

    #[test]
    fn emitter_renders_exact_field_event_normalization_before_string_coercion() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let behavior: JsonValue = serde_json::from_value(json!({
            "state": "executable",
            "normalization": [],
            "event_normalization": [{
                "phase": "blur",
                "normalization": [{"kind": "offline-ebir-money-round-v1"}]
            }],
            "coercion": {"kind": "string", "on_empty": "null"},
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        }))
        .unwrap();

        let rendered = emitter
            .render_field_behavior(behavior.object().unwrap(), "$.behavior.official")
            .unwrap();
        assert!(rendered.contains("event_normalization: &[FieldEventNormalization"));
        assert!(rendered.contains("phase: ValidationPhase::Blur"));
        assert!(rendered.contains("NormalizationStep::OfflineEbirMoneyRoundV1"));
        assert!(rendered.contains("coercion: Coercion::String"));

        let year_behavior: JsonValue = serde_json::from_value(json!({
            "state": "executable",
            "normalization": [],
            "event_normalization": [{
                "phase": "blur",
                "normalization": [{"kind": "offline-ebir-parse-float-fixed-zero-v1"}]
            }],
            "coercion": {"kind": "string", "on_empty": "null"},
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        }))
        .unwrap();
        let rendered = emitter
            .render_field_behavior(year_behavior.object().unwrap(), "$.behavior.official")
            .unwrap();
        assert!(rendered.contains("NormalizationStep::OfflineEbirParseFloatFixedZeroV1"));
    }

    #[test]
    fn emitter_renders_exact_event_trigger_and_static_raw_assignment_fail_closed() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let branch = json!({
            "state": "executable",
            "predicate": {"kind": "constant", "value": true},
            "effects": [{
                "kind": "set-raw-field-value",
                "field": {
                    "field_id": "amount",
                    "instance": {"kind": "singleton"}
                },
                "value": {"state": "text", "text": "0.00"}
            }],
            "review_decision": {"source_id": "review"},
            "source_refs": [{"source_id": "review"}]
        });
        let rule = |phases: serde_json::Value, triggers: Option<serde_json::Value>| {
            let mut value = json!({
                "rule_id": "amount-reset",
                "scope": {"kind": "singleton"},
                "order": 1,
                "phases": phases,
                "field_ids": ["amount"],
                "profiles": {
                    "official": branch.clone(),
                    "filing_safe": branch.clone()
                },
                "source_refs": [{"source_id": "review"}]
            });
            if let Some(triggers) = triggers {
                value
                    .as_object_mut()
                    .unwrap()
                    .insert("trigger_field_ids".to_owned(), triggers);
            }
            serde_json::from_value::<JsonValue>(value).unwrap()
        };

        let rendered = emitter
            .render_rule(
                &rule(json!(["blur"]), Some(json!(["amount"]))),
                "$.rules[0]",
            )
            .unwrap();
        assert!(rendered.contains("ValidationPhase::Blur"));
        assert!(rendered.contains("trigger_field_ids: &[\"amount\"]"));
        assert!(rendered.contains("Effect::SetRawFieldValue"));
        assert!(rendered.contains("StaticRawValue::Text(\"0.00\")"));

        let error = emitter
            .render_rule(&rule(json!(["validate"]), None), "$.rules[0]")
            .expect_err("raw assignment on a non-event rule must fail");
        assert!(error.message().contains("permitted only"));
    }

    #[test]
    fn emitter_renders_profiled_event_program_writeback_and_field_calculation_owner() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let program: JsonValue = serde_json::from_value(json!({
            "phase": "change",
            "trigger_field_id": "amount",
            "profiles": {
                "official": {
                    "state": "executable",
                    "steps": [
                        {
                            "kind": "calculation",
                            "calculation_id": "amount-calc",
                            "output_ids": ["amount", "tax"],
                            "write_mode": "insert"
                        },
                        {"kind": "rule", "rule_id": "amount-reset"},
                        {
                            "kind": "calculation",
                            "calculation_id": "amount-calc",
                            "output_ids": ["tax"],
                            "write_mode": "replace"
                        }
                    ],
                    "review_decision": {"source_id": "review"},
                    "source_refs": [{"source_id": "review"}]
                },
                "filing_safe": {
                    "state": "unresolved",
                    "reason": "not reviewed",
                    "source_refs": [{"source_id": "review"}]
                }
            },
            "source_refs": [{"source_id": "review"}]
        }))
        .unwrap();
        let rendered = emitter
            .render_field_event_program(&program, "$.field_event_programs[0]")
            .unwrap();
        assert!(rendered.contains("FieldEventProgramSpec"));
        assert!(rendered.contains("phase: ValidationPhase::Change"));
        assert!(rendered.contains("trigger_field_id: \"amount\""));
        assert!(rendered.contains("FieldEventStep::Rule { rule_id: \"amount-reset\" }"));
        assert!(rendered.contains("calculation_id: \"amount-calc\""));
        assert!(rendered.contains("output_ids: &[\"amount\", \"tax\"]"));
        assert!(rendered.contains("ScheduledOutputWriteMode::Insert"));
        assert!(rendered.contains("ScheduledOutputWriteMode::Replace"));

        let output: JsonValue = serde_json::from_value(json!({
            "output_id": "amount",
            "value": {
                "kind": "literal",
                "value": {"type": "decimal", "value": "1.25"}
            },
            "rounding": null,
            "writeback": {
                "field": {
                    "field_id": "amount",
                    "instance": {"kind": "singleton"}
                },
                "format": {"kind": "offline-ebir-format-currency-v1"},
                "review_decision": {"source_id": "review"},
                "source_refs": [{"source_id": "review"}]
            }
        }))
        .unwrap();
        let rendered = emitter
            .render_calculation_output(&output, "$.output")
            .unwrap();
        assert!(rendered.contains("writeback: Some(CalculationWriteback"));
        assert!(rendered.contains("field_id: \"amount\""));
        assert!(rendered.contains("CalculationWriteFormat::OfflineEbirFormatCurrencyV1"));

        let field = |calculation_id: serde_json::Value| {
            serde_json::from_value::<JsonValue>(json!({
                "field_id": "amount",
                "value_type": "string",
                "control_kind": "currency",
                "requiredness": "computed",
                "group_id": null,
                "calculation_id": calculation_id,
                "serialized": [],
                "behavior": {
                    "official": {
                        "state": "unresolved",
                        "reason": "test",
                        "source_refs": [{"source_id": "review"}]
                    },
                    "filing_safe": {
                        "state": "unresolved",
                        "reason": "test",
                        "source_refs": [{"source_id": "review"}]
                    }
                },
                "source_refs": [{"source_id": "review"}]
            }))
            .unwrap()
        };
        assert!(
            emitter
                .render_field(&field(json!("amount-calc")), "$.fields[0]")
                .unwrap()
                .contains("calculation_id: Some(\"amount-calc\")")
        );
        assert!(
            emitter
                .render_field(&field(json!(null)), "$.fields[0]")
                .unwrap()
                .contains("calculation_id: None")
        );
    }

    #[test]
    fn emitter_renders_closed_javascript_number_predicates_and_rejects_empty_or_inputs() {
        let emitter = Emitter {
            rule_set_id: "test-v1-p1",
        };
        let logical_or: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-global-is-nan-logical-or",
            "inputs": [
                {
                    "kind": "literal",
                    "value": {"type": "null", "value": null}
                },
                {
                    "kind": "literal",
                    "value": {"type": "string", "value": "amount"}
                }
            ]
        }))
        .unwrap();
        let rendered = emitter
            .render_predicate(&logical_or, "$.predicate")
            .unwrap();
        assert!(rendered.contains("Predicate::JavaScriptGlobalIsNaNLogicalOr"));
        assert!(rendered.contains("TypedValue::Null"));
        assert!(rendered.contains("TypedValue::String(\"amount\")"));

        let number_compare: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-number-compare",
            "operator": "less-than",
            "input": {
                "kind": "literal",
                "value": {"type": "string", "value": "2024"}
            },
            "operand": {
                "kind": "context",
                "result_type": "integer",
                "context_value_id": "current-calendar-year"
            }
        }))
        .unwrap();
        let rendered = emitter
            .render_predicate(&number_compare, "$.predicate")
            .unwrap();
        assert!(rendered.contains("JavaScriptNumberCompareOperator::LessThan"));
        assert!(rendered.contains("context_value_id: \"current-calendar-year\""));

        let empty: JsonValue = serde_json::from_value(json!({
            "kind": "javascript-global-is-nan-logical-or",
            "inputs": []
        }))
        .unwrap();
        assert!(
            emitter
                .render_predicate(&empty, "$.predicate")
                .expect_err("empty logical-or inputs must fail closed")
                .message()
                .contains("requires nonempty inputs")
        );
    }
}
