use serde::Deserialize;

use crate::json::JsonValue;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexDocument {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_version: String,
    pub snapshots: Vec<IndexSnapshot>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexSnapshot {
    pub rule_set_id: String,
    pub form_code: String,
    pub form_revision: String,
    pub official_package_version: String,
    pub source_set_sha256: Option<String>,
    pub path: String,
    pub review_status: ReviewStatus,
    pub profile_states: ProfileStates,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Research inventory only; never emitted.
    Skeleton,
    /// Digest-pinned, fixture-executable review input; emitted only for tests.
    Candidate,
    /// Human-reviewed packaged runtime behavior.
    Reviewed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BranchState {
    Executable,
    DocumentedOnly,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileStates {
    pub official: BranchState,
    pub filing_safe: BranchState,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetDocument {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_version: String,
    pub identity: RuleSetIdentity,
    pub review_status: ReviewStatus,
    pub profile_status: ProfileStatus,
    pub evaluation_policy: EvaluationPolicy,
    pub sources: Vec<Source>,
    pub legacy_v1: LegacyV1,
    pub context_values: Vec<JsonValue>,
    pub field_groups: Vec<JsonValue>,
    pub fields: Vec<JsonValue>,
    pub evaluation_order: Vec<String>,
    pub calculations: Vec<JsonValue>,
    pub rules: Vec<JsonValue>,
    pub workflow: JsonValue,
    pub serialization: SerializationContract,
    pub fixtures: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetIdentity {
    pub rule_set_id: String,
    pub form_code: String,
    pub form_revision: String,
    pub official_package_version: String,
    pub source_set_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileStatus {
    pub official: ProfileStatusBranch,
    pub filing_safe: ProfileStatusBranch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileStatusBranch {
    Executable {
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
    DocumentedOnly {
        summary: String,
        source_refs: Vec<SourceRef>,
    },
    Unresolved {
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

impl ProfileStatusBranch {
    pub fn state(&self) -> BranchState {
        match self {
            Self::Executable { .. } => BranchState::Executable,
            Self::DocumentedOnly { .. } => BranchState::DocumentedOnly,
            Self::Unresolved { .. } => BranchState::Unresolved,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationPolicy {
    pub official: EvaluationPolicyBranch,
    pub filing_safe: EvaluationPolicyBranch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EffectEvaluationMode {
    ApplyAll,
    StopEffectsAfterFirstBlockingIssue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EvaluationScope {
    Singleton,
    EachGroup { group_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DerivedInstanceSelector {
    Singleton,
    CurrentGroupInstance,
    StableInstanceId { instance_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvaluationPolicyBranch {
    Executable {
        effect_mode: EffectEvaluationMode,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
    DocumentedOnly {
        summary: String,
        source_refs: Vec<SourceRef>,
    },
    Unresolved {
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

impl EvaluationPolicyBranch {
    pub fn state(&self) -> BranchState {
        match self {
            Self::Executable { .. } => BranchState::Executable,
            Self::DocumentedOnly { .. } => BranchState::DocumentedOnly,
            Self::Unresolved { .. } => BranchState::Unresolved,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub source_id: String,
    pub locator: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub source_id: String,
    pub kind: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializationContract {
    pub contract_version: String,
    pub artifacts: Vec<SerializationArtifact>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializationArtifact {
    pub artifact_id: String,
    pub target: SerializationArtifactTarget,
    pub variant_id: String,
    pub official: SerializationArtifactBranch,
    pub filing_safe: SerializationArtifactBranch,
    pub source_refs: Vec<SourceRef>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationArtifactTarget {
    EditableSave,
    FinalizedSave,
    EncryptedFinalCopy,
    SubmissionPayload,
    HistoricalImportCompatibility,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SerializationArtifactBranch {
    Executable {
        nodes: Vec<SerializationNode>,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
    DocumentedOnly {
        summary: String,
        source_refs: Vec<SourceRef>,
    },
    Unresolved {
        reason: String,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationNode {
    PseudoXmlField {
        ordinal: u32,
        key_projection: SerializationKeyProjection,
        occurrence_projection: SerializationOccurrenceProjection,
        value_projection: SerializationValueProjection,
        semantic_format: SerializationSemanticFormat,
        body_codec: SerializationBodyCodec,
        presence: SerializationPresence,
        source_refs: Vec<SourceRef>,
    },
    MetadataElement {
        ordinal: u32,
        exact_tag: String,
        value_projection: SerializationValueProjection,
        semantic_format: SerializationSemanticFormat,
        body_codec: SerializationBodyCodec,
        presence: SerializationPresence,
        source_refs: Vec<SourceRef>,
    },
    ReviewedLiteral {
        ordinal: u32,
        exact_bytes: Vec<u8>,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
    DynamicGroup {
        ordinal: u32,
        group_id: String,
        instance_order: SerializationGroupInstanceOrder,
        min_occurs: usize,
        max_occurs: Option<usize>,
        nodes: Vec<SerializationNode>,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
}

impl SerializationNode {
    pub fn ordinal(&self) -> u32 {
        match self {
            Self::PseudoXmlField { ordinal, .. }
            | Self::MetadataElement { ordinal, .. }
            | Self::ReviewedLiteral { ordinal, .. }
            | Self::DynamicGroup { ordinal, .. } => *ordinal,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationGroupInstanceOrder {
    StableInstanceIdAscending,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationKeyProjection {
    Exact {
        key: String,
    },
    GroupIndexed {
        group_id: String,
        index_base: u32,
        index_step: u32,
        padding: u32,
        prefix: String,
        suffix: String,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationOccurrenceProjection {
    Fixed {
        occurrence: u32,
    },
    GroupIndexed {
        group_id: String,
        index_base: u32,
        index_step: u32,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationValueProjection {
    Field {
        field: JsonValue,
    },
    Derived {
        calculation_id: String,
        output_id: String,
        instance: DerivedInstanceSelector,
    },
    Context {
        context_value_id: String,
    },
    Constant {
        value: JsonValue,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
    Default {
        value: JsonValue,
        review_decision: SourceRef,
        source_refs: Vec<SourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerializationSemanticFormat {
    pub absent: SerializationAbsentPolicy,
    pub blank: SerializationBlankPolicy,
    pub present: SerializationPresentFormat,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationAbsentPolicy {
    Reject,
    OmitOccurrence,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationBlankPolicy {
    Reject,
    EmitEmptyBody,
    OmitOccurrence,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationPresentFormat {
    Text,
    Boolean {
        true_text: String,
        false_text: String,
    },
    Base10Integer,
    Decimal {
        scale: u32,
        rounding: SerializationRoundingMode,
        grouping: SerializationGrouping,
        decimal_separator: SerializationDecimalSeparator,
        negative: SerializationNegativeRepresentation,
    },
    Date {
        pattern: SerializationDatePattern,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationRoundingMode {
    None,
    HalfUp,
    HalfEven,
    TowardZero,
    AwayFromZero,
    Floor,
    Ceiling,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationGrouping {
    None,
    Comma,
    Period,
    Space,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationDecimalSeparator {
    Period,
    Comma,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SerializationNegativeRepresentation {
    LeadingMinus,
    TrailingMinus,
    Parentheses,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum SerializationDatePattern {
    #[serde(rename = "yyyy-mm-dd-hyphen")]
    YyyyMmDdHyphen,
    #[serde(rename = "yyyy-mm-dd-slash")]
    YyyyMmDdSlash,
    #[serde(rename = "mm-dd-yyyy-slash")]
    MmDdYyyySlash,
    #[serde(rename = "dd-mm-yyyy-slash")]
    DdMmYyyySlash,
    #[serde(rename = "yyyymmdd")]
    YyyyMmDdCompact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub enum SerializationBodyCodec {
    #[serde(rename = "raw-literal")]
    RawLiteral,
    #[serde(rename = "legacy-javascript-escape")]
    LegacyJavascriptEscape,
    #[serde(rename = "utf8-percent-rfc3986-unreserved")]
    Utf8PercentRfc3986Unreserved,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SerializationPresence {
    Always,
    When { predicate: JsonValue },
    Omitted,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyV1 {
    pub form_id: String,
    pub schema_version: String,
    pub mappings: Vec<LegacyMapping>,
    pub declared_counts: LegacyCounts,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyMapping {
    pub artifact: LegacyArtifact,
    pub source_id: String,
    pub record_count: u64,
    pub target_sections: Vec<String>,
    pub state: BranchState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum LegacyArtifact {
    Manifest,
    Fields,
    Validations,
    Calculations,
    Workflow,
}

impl LegacyArtifact {
    pub fn expected_source_kind(&self) -> &'static str {
        match self {
            Self::Manifest => "legacy-v1-manifest",
            Self::Fields => "legacy-v1-fields",
            Self::Validations => "legacy-v1-validations",
            Self::Calculations => "legacy-v1-calculations",
            Self::Workflow => "legacy-v1-workflow",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::Fields => "fields",
            Self::Validations => "validations",
            Self::Calculations => "calculations",
            Self::Workflow => "workflow",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyCounts {
    pub typed_fields: u64,
    pub concrete_union_fields: u64,
    /// Number of legacy unbounded-family descriptors represented by unique
    /// member fields in unbounded v2 field groups, not the number of groups.
    #[serde(rename = "field_groups")]
    pub unbounded_family_members: u64,
    pub validation_rules: u64,
    pub calculations: u64,
    pub workflow_states: u64,
    pub workflow_transitions: u64,
    pub negative_fixtures: u64,
    pub confirmed_official_bugs: u64,
    pub unverified_gaps: u64,
}
