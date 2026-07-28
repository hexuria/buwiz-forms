//! Machine-checkable status of the validation-rules library, anchored by the
//! 2550Q v2 candidate while aggregate coverage expands to all 43 forms.
//!
//! This is the validation-rules analogue of `scripts/wave_status.py`: one
//! command whose exit code answers "is the active library objective finished?"
//! without anyone having to read prose and believe it.
//!
//! It reports three kinds of criterion:
//!
//! * **Boundary** criteria assert that a production authority is still closed.
//!   These hold today and must never stop holding. A boundary failure means
//!   something opened a filing path and is far more serious than unfinished
//!   library work.
//!   They are checked first and reported first.
//! * **Active library** criteria assert that the current implementation
//!   objective is complete.
//! * **Deferred promotion** criteria require policy or evidence that is outside
//!   the library objective. They are always reported, but block only when the
//!   caller explicitly requires promotion readiness.
//!
//! Never relax a criterion to make this command pass.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

/// Hoisted out of the per-capability-record loop: the pattern is constant, so
/// recompiling it for every record is pure waste.
static CAPABILITIES_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bcapabilities\s*:\s*FormCapabilities\s*\{")
        .expect("capabilities block regex is valid")
});
use serde::Serialize;
use syn::{Attribute, Item, Meta, UseTree};

use crate::coverage::{CoverageOptions, CoverageReport, coverage};
use crate::error::{CodegenError, Result};
use crate::evidence_set::{
    CheckEvidencePacketSetOptions, CheckEvidencePacketSetReport, check_evidence_packet_set,
};
use crate::files::read_tracked_bytes;
use crate::hash::sha256_hex;
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, is_symlink_or_reparse_point, resolve_existing_under};
use crate::reconciliation::{ReconciliationOptions, ReconciliationReport, reconciliation};

const RULE_SET_ID: &str = "2550q-v2024-p7.9.6.0";
const RULE_SET_PATH: &str = "rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json";
const INDEX_PATH: &str = "rules/ir/v2/index.json";
const INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json";
const CORE_2550Q_PATH: &str = "crates/bir-core/src/form_rules/form_2550q.rs";
const GENERATED_MOD_PATH: &str = "crates/bir-rules/src/generated/mod.rs";
const GENERATED_REGISTRY_PATH: &str = "crates/bir-rules/src/generated/registry.rs";
const GENERATED_MANIFEST_PATH: &str = "crates/bir-rules/src/generated/manifest.json";
const PAYLOAD_PATH: &str = "crates/bir-core/src/form_rules/payload.rs";
const CORE_FORM_2550Q_PATH: &str = "crates/bir-core/src/forms/form_2550q.rs";
const CORE_CAPABILITIES_PATH: &str = "crates/bir-core/src/forms/support_level.rs";
const DESKTOP_FORM_2550Q_PATH: &str = "crates/bir-desktop/src/views/form_2550q_view.rs";
const V1_VALIDATIONS_PATH: &str = "rules/forms/2550q-v2024/validations.json";
const SHADOW_PATH: &str = "crates/bir-core/src/form_rules/shadow.rs";
const REVIEWED_EVIDENCE_PACKET_SET_PATH: &str = "evidence/validation-rules/packets/v1";

/// Files whose structure is itself part of the application freeze, even when
/// they contain none of the general authority tokens below.
const FROZEN_PRODUCTION_STRUCTURE_PATHS: [&str; 6] = [
    CORE_2550Q_PATH,
    CORE_FORM_2550Q_PATH,
    DESKTOP_FORM_2550Q_PATH,
    "crates/bir-core/src/background_cron.rs",
    "crates/bir-core/src/official_import.rs",
    "crates/bir-core/src/transport.rs",
];

/// Every cross-file application-freeze regex contains at least one of these
/// literal identifiers. The prefilter may admit comments and strings because
/// `production_rust_code` removes those later; it must never reject a possible
/// production match.
const FROZEN_AUTHORITY_TOKENS: [&str; 21] = [
    "reviewed_rule_set_entries",
    "candidate_rule_set_entries",
    "CANDIDATE_RULE_SET_ENTRIES",
    "CANDIDATE_RULE_SET_METADATA",
    "FormRevisionKey",
    "FormRuleEvaluator",
    "evaluate_trusted",
    "evaluate_shadow",
    "materialize_checked",
    "preflight_active_form_submission",
    "create_form_final_copy",
    "CheckedFinalCopyPayload",
    "Form2550QLiveValidationEvaluator",
    "EvaluationRequest",
    "TrustedEvaluation",
    "evaluate_official_diagnostic",
    "evaluate_filing_safe_trusted",
    "CheckedSerializationArtifact",
    "SerializationArtifactTarget",
    "SubmissionPreflightArtifact",
    "setup_repo_default_diagnostic",
];

/// The published occurrence decomposition. Every one of these is quoted as fact
/// in `handoff.md` and the 2550Q review documents.
const PLAINTEXT_OCCURRENCES: usize = 160;
const ENCRYPTED_OCCURRENCES: usize = 159;
const STATIC_OCCURRENCES: usize = 119;
const GROUP_OCCURRENCES: usize = 40;
const GENERATED_OCCURRENCES: usize = 1;
const EXECUTABLE_SINGLETONS: usize = 66;
const REPEATED_FAMILY_DESCRIPTORS: usize = 28;
const FIELD_GROUPS: usize = 7;
const DOCUMENTED_ONLY_PROJECTIONS: usize = 53;
const DERIVED_OR_ALIAS_PROJECTIONS: usize = 44;
const WORKFLOW_OR_CREDENTIAL_PROJECTIONS: usize = 9;
const EXPECTED_FORM_COUNT: usize = 43;
const EXPECTED_VALIDATION_RECORDS: usize = 2_007;
const EXPECTED_CALCULATION_RECORDS: usize = 623;

/// Controls that must never become ordinary field authority in a tax draft.
const CREDENTIAL_KEYS: [&str; 4] = [
    "ebirOnlineConfirmUsername",
    "ebirOnlineSecret",
    "ebirOnlineUsername",
    "txtEmail",
];

#[derive(Clone, Debug)]
pub struct StatusOptions {
    pub repo_root: PathBuf,
    pub boundaries_only: bool,
}

impl StatusOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            boundaries_only: false,
        }
    }

    pub fn boundaries_only(mut self) -> Self {
        self.boundaries_only = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionKind {
    /// A production authority that must stay closed.
    Boundary,
    /// A deliverable of the current library objective.
    ActiveLibrary,
    /// Promotion work requiring an explicit policy/evidence decision.
    DeferredPromotion,
}

#[derive(Clone, Debug, Serialize)]
pub struct Criterion {
    pub id: &'static str,
    pub kind: CriterionKind,
    pub met: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatusReport {
    pub rule_set_id: &'static str,
    pub criteria: Vec<Criterion>,
}

impl StatusReport {
    pub fn boundaries_held(&self) -> bool {
        self.kind_complete(CriterionKind::Boundary)
    }

    pub fn active_library_complete(&self) -> bool {
        self.kind_complete(CriterionKind::ActiveLibrary)
    }

    pub fn deferred_promotion_complete(&self) -> bool {
        self.kind_complete(CriterionKind::DeferredPromotion)
    }

    /// The default completion condition: production remains closed and the
    /// active library objective is complete. Deferred promotion work is still
    /// reported, but does not make the default status fail.
    pub fn complete(&self) -> bool {
        self.complete_for(false)
    }

    /// Completion under the caller's requested policy.
    pub fn complete_for(&self, require_promotion: bool) -> bool {
        self.boundaries_held()
            && self.active_library_complete()
            && (!require_promotion || self.deferred_promotion_complete())
    }

    fn kind_complete(&self, kind: CriterionKind) -> bool {
        self.criteria
            .iter()
            .filter(|criterion| criterion.kind == kind)
            .all(|criterion| criterion.met)
    }

    /// Every unmet criterion, including deferred promotion work.
    pub fn open(&self) -> impl Iterator<Item = &Criterion> {
        self.criteria.iter().filter(|criterion| !criterion.met)
    }

    /// Unmet criteria that block under the caller's requested policy.
    pub fn blocking_open(&self, require_promotion: bool) -> impl Iterator<Item = &Criterion> {
        self.criteria.iter().filter(move |criterion| {
            !criterion.met
                && match criterion.kind {
                    CriterionKind::Boundary | CriterionKind::ActiveLibrary => true,
                    CriterionKind::DeferredPromotion => require_promotion,
                }
        })
    }
}

pub fn status(options: &StatusOptions) -> Result<StatusReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let rule_set = read_json(&repo_root, RULE_SET_PATH)?;
    let index = read_json(&repo_root, INDEX_PATH)?;
    let inventory = read_json(&repo_root, INVENTORY_PATH)?;

    let mut criteria = Vec::new();
    check_production_guards(&repo_root, &mut criteria);
    check_all_snapshots_unpromoted(&repo_root, &index, &mut criteria);
    check_all_generated_candidates_test_only(&repo_root, &mut criteria);
    check_application_integration_frozen(&repo_root, &mut criteria);
    check_review_status(&rule_set, &index, &mut criteria);
    check_filing_safe_unresolved(&rule_set, &mut criteria);
    check_artifacts_closed(&rule_set, &mut criteria);
    check_inventory_value_free(&inventory, &mut criteria);
    check_occurrence_decomposition(&inventory, &rule_set, &mut criteria);
    check_occurrence_classification(&inventory, &rule_set, &mut criteria);
    if options.boundaries_only {
        criteria.retain(|criterion| criterion.kind == CriterionKind::Boundary);
    } else {
        let coverage = coverage(&CoverageOptions::new(&repo_root));
        let reconciliation = reconciliation(&ReconciliationOptions::tracked_checkout(&repo_root));
        check_aggregate_library_gates(
            &repo_root,
            coverage.as_ref(),
            reconciliation.as_ref(),
            &mut criteria,
        );
        check_reviewed_evidence_packet_set(&repo_root, &mut criteria);
        check_inventory_pins_rule_set(&inventory, &rule_set, &mut criteria);
        check_declared_sources_are_clone_reproducible(&repo_root, &rule_set, &mut criteria);
        check_remaining_plan_deliverables(&repo_root, &mut criteria);
        check_filing_safe_resolved_where_official_is_correct(&repo_root, &rule_set, &mut criteria);
        check_shadow_difference_dimensions(&repo_root, &mut criteria);
    }
    criteria.sort_by_key(|criterion| match criterion.kind {
        CriterionKind::Boundary => 0,
        CriterionKind::ActiveLibrary => 1,
        CriterionKind::DeferredPromotion => 2,
    });

    Ok(StatusReport {
        rule_set_id: RULE_SET_ID,
        criteria,
    })
}

fn push(
    criteria: &mut Vec<Criterion>,
    id: &'static str,
    kind: CriterionKind,
    met: bool,
    detail: impl Into<String>,
) {
    criteria.push(Criterion {
        id,
        kind,
        met,
        detail: detail.into(),
    });
}

fn check_reviewed_evidence_packet_set(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    let packet_root = repo_root.join(REVIEWED_EVIDENCE_PACKET_SET_PATH);
    let result = check_evidence_packet_set(&CheckEvidencePacketSetOptions::tracked_checkout(
        repo_root,
        packet_root,
    ));
    record_reviewed_evidence_packet_set(result, criteria);
}

fn record_reviewed_evidence_packet_set(
    result: Result<CheckEvidencePacketSetReport>,
    criteria: &mut Vec<Criterion>,
) {
    match result {
        Ok(report) => {
            let met = report.packet_count == EXPECTED_FORM_COUNT;
            push(
                criteria,
                "reviewed-evidence-packet-set",
                CriterionKind::ActiveLibrary,
                met,
                if met {
                    format!(
                        "{REVIEWED_EVIDENCE_PACKET_SET_PATH} contains the exact ordered set of {EXPECTED_FORM_COUNT} reviewed v1 evidence packets; upstream vault bytes were not required"
                    )
                } else {
                    format!(
                        "{REVIEWED_EVIDENCE_PACKET_SET_PATH} contains {} checked packet(s), expected exactly {EXPECTED_FORM_COUNT}",
                        report.packet_count
                    )
                },
            );
        }
        Err(error) => push(
            criteria,
            "reviewed-evidence-packet-set",
            CriterionKind::ActiveLibrary,
            false,
            format!("{REVIEWED_EVIDENCE_PACKET_SET_PATH} is missing or invalid: {error}"),
        ),
    }
}

/// The five independent switches that keep the only v2 candidate out of every
/// production filing path. These are source-level boundaries on purpose: the
/// candidate is compiled only in tests, so a production runtime test cannot
/// exercise the code whose continued absence it needs to prove.
fn check_production_guards(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    check_source_guard(
        repo_root,
        CORE_2550Q_PATH,
        criteria,
        "core-default-designation-none",
        |source| {
            source_matches(
                source,
                r"(?ms)^[ \t]*fn[ \t]+reviewed_repo_default_designation[ \t]*\([ \t]*\)[ \t]*->[ \t]*Option[ \t]*<[ \t]*FormRevisionKey[ \t]*>[ \t]*\{[ \t\r\n]*None[ \t\r\n]*\}",
            )
        },
        "reviewed_repo_default_designation() has the exact inert `None` body",
        "reviewed_repo_default_designation() is missing or no longer has the exact inert `None` body",
    );

    check_source_guard(
        repo_root,
        GENERATED_MOD_PATH,
        criteria,
        "generated-candidate-test-only",
        |source| {
            source_matches(
                source,
                r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*mod[ \t]+form_2550q_v2024_04_01_p7_9_6_0[ \t]*;",
            )
        },
        "generated 2550Q candidate module is directly gated by #[cfg(test)]",
        "generated 2550Q candidate module is missing or is not directly gated by #[cfg(test)]",
    );

    check_source_guard(
        repo_root,
        CORE_2550Q_PATH,
        criteria,
        "core-candidate-evaluator-test-only",
        |source| {
            let struct_is_test_only = source_matches(
                source,
                r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*struct[ \t]+Form2550QLiveValidationEvaluator[ \t]*<'registry>",
            );
            let impl_is_test_only = source_matches(
                source,
                r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*impl[ \t]*<'registry>[ \t]+Form2550QLiveValidationEvaluator[ \t]*<'registry>",
            );
            struct_is_test_only && impl_is_test_only
        },
        "core 2550Q candidate evaluator and implementation are directly gated by #[cfg(test)]",
        "core 2550Q candidate evaluator or its implementation is no longer directly gated by #[cfg(test)]",
    );

    match read_text(repo_root, GENERATED_REGISTRY_PATH) {
        Ok(source) => check_reviewed_registry_empty(&source, criteria),
        Err(error) => push(
            criteria,
            "reviewed-registry-empty",
            CriterionKind::Boundary,
            false,
            format!("cannot read {GENERATED_REGISTRY_PATH}: {error}"),
        ),
    }

    check_source_guard(
        repo_root,
        PAYLOAD_PATH,
        criteria,
        "payload-constructor-closed",
        |source| {
            source_matches(
                source,
                r"(?ms)^[ \t]*pub\(crate\)[ \t]+fn[ \t]+try_new[ \t]*\([^)]*\)[ \t\r\n]*->[ \t\r\n]*Result[ \t]*<[ \t]*Self[ \t]*,[ \t]*CheckedFinalCopyPayloadError[ \t]*>[ \t]*\{[ \t\r\n]*Err[ \t]*\([ \t]*CheckedFinalCopyPayloadError::MissingSerializationContract[ \t]*\)[ \t\r\n]*\}",
            )
        },
        "CheckedFinalCopyPayload::try_new always returns Err(MissingSerializationContract)",
        "CheckedFinalCopyPayload::try_new is missing or no longer has the exact Err(MissingSerializationContract) body",
    );
}

fn check_all_snapshots_unpromoted(
    repo_root: &Path,
    index: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let snapshots = index
        .object()
        .and_then(|index| index.get("snapshots"))
        .and_then(array);
    let Some(snapshots) = snapshots else {
        for id in [
            "all-snapshots-unpromoted",
            "all-filing-safe-profiles-unresolved",
        ] {
            push(
                criteria,
                id,
                CriterionKind::Boundary,
                false,
                "v2 index snapshots array is missing",
            );
        }
        return;
    };

    let mut promotion_failures = Vec::new();
    let mut filing_safe_failures = Vec::new();
    for (ordinal, snapshot) in snapshots.iter().enumerate() {
        let rule_set_id = string_at(snapshot, "rule_set_id")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("<snapshot-{ordinal}>"));
        let indexed_review = string_at(snapshot, "review_status");
        if !matches!(indexed_review, Some("skeleton" | "candidate")) {
            promotion_failures.push(format!(
                "{rule_set_id}: index review_status={}",
                indexed_review.unwrap_or("<missing>")
            ));
        }
        let indexed_filing_safe = snapshot
            .object()
            .and_then(|snapshot| snapshot.get("profile_states"))
            .and_then(|states| states.object())
            .and_then(|states| states.get("filing_safe"))
            .and_then(JsonValue::as_str);
        if indexed_filing_safe != Some("unresolved") {
            filing_safe_failures.push(format!(
                "{rule_set_id}: index filing_safe={}",
                indexed_filing_safe.unwrap_or("<missing>")
            ));
        }

        let Some(relative) = string_at(snapshot, "path") else {
            promotion_failures.push(format!("{rule_set_id}: index path is missing"));
            filing_safe_failures.push(format!("{rule_set_id}: index path is missing"));
            continue;
        };
        let source_path = format!("rules/ir/v2/{relative}");
        match read_json(repo_root, &source_path) {
            Ok(rule_set) => {
                let declared_id = rule_set
                    .object()
                    .and_then(|rule_set| rule_set.get("identity"))
                    .and_then(|identity| string_at(identity, "rule_set_id"));
                let declared_review = string_at(&rule_set, "review_status");
                if declared_id != Some(rule_set_id.as_str()) || declared_review != indexed_review {
                    promotion_failures.push(format!(
                        "{rule_set_id}: rule-set identity/review_status is {}/{}",
                        declared_id.unwrap_or("<missing>"),
                        declared_review.unwrap_or("<missing>")
                    ));
                }
                let declared_filing_safe = rule_set
                    .object()
                    .and_then(|rule_set| rule_set.get("profile_status"))
                    .and_then(|states| states.object())
                    .and_then(|states| states.get("filing_safe"))
                    .and_then(|branch| string_at(branch, "state"));
                if declared_filing_safe != Some("unresolved") {
                    filing_safe_failures.push(format!(
                        "{rule_set_id}: rule-set filing_safe={}",
                        declared_filing_safe.unwrap_or("<missing>")
                    ));
                }
            }
            Err(error) => {
                promotion_failures
                    .push(format!("{rule_set_id}: cannot read {source_path}: {error}"));
                filing_safe_failures
                    .push(format!("{rule_set_id}: cannot read {source_path}: {error}"));
            }
        }
    }
    if snapshots.is_empty() {
        promotion_failures.push("v2 index has no snapshots".to_owned());
        filing_safe_failures.push("v2 index has no snapshots".to_owned());
    }

    push(
        criteria,
        "all-snapshots-unpromoted",
        CriterionKind::Boundary,
        promotion_failures.is_empty(),
        if promotion_failures.is_empty() {
            format!(
                "all {} indexed snapshot(s) remain unpromoted (skeleton/candidate) and match their rule-set documents",
                snapshots.len()
            )
        } else {
            promotion_failures.join("; ")
        },
    );
    push(
        criteria,
        "all-filing-safe-profiles-unresolved",
        CriterionKind::Boundary,
        filing_safe_failures.is_empty(),
        if filing_safe_failures.is_empty() {
            format!(
                "all {} indexed snapshot(s) keep filing_safe unresolved",
                snapshots.len()
            )
        } else {
            filing_safe_failures.join("; ")
        },
    );
}

fn check_all_generated_candidates_test_only(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    let manifest = read_json(repo_root, GENERATED_MANIFEST_PATH);
    let generated_mod = read_text(repo_root, GENERATED_MOD_PATH);
    let registry = read_text(repo_root, GENERATED_REGISTRY_PATH);
    let (manifest, generated_mod, registry) = match (manifest, generated_mod, registry) {
        (Ok(manifest), Ok(generated_mod), Ok(registry)) => (manifest, generated_mod, registry),
        (manifest, generated_mod, registry) => {
            let mut failures = Vec::new();
            if let Err(error) = manifest {
                failures.push(format!("manifest: {error}"));
            }
            if let Err(error) = generated_mod {
                failures.push(format!("mod.rs: {error}"));
            }
            if let Err(error) = registry {
                failures.push(format!("registry.rs: {error}"));
            }
            push(
                criteria,
                "all-generated-candidates-test-only",
                CriterionKind::Boundary,
                false,
                failures.join("; "),
            );
            return;
        }
    };

    let mut failures = Vec::new();
    let candidate_modules: Vec<&str> = manifest
        .object()
        .and_then(|manifest| manifest.get("snapshots"))
        .and_then(array)
        .into_iter()
        .flatten()
        .filter(|snapshot| string_at(snapshot, "review_status") == Some("candidate"))
        .filter_map(|snapshot| string_at(snapshot, "generated_module"))
        .collect();
    for module in &candidate_modules {
        let pattern = format!(
            r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*mod[ \t]+{}[ \t]*;",
            regex::escape(module)
        );
        if !source_matches(&generated_mod, &pattern) {
            failures.push(format!("{module} is not directly gated by #[cfg(test)]"));
        }
    }
    for (symbol, pattern) in [
        (
            "CANDIDATE_RULE_SET_METADATA",
            r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*pub[ \t]+static[ \t]+CANDIDATE_RULE_SET_METADATA",
        ),
        (
            "CANDIDATE_RULE_SET_ENTRIES",
            r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*static[ \t]+CANDIDATE_RULE_SET_ENTRIES",
        ),
        (
            "candidate_rule_set_entries",
            r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*pub[ \t]+fn[ \t]+candidate_rule_set_entries",
        ),
    ] {
        if registry.contains(symbol) && !source_matches(&registry, pattern) {
            failures.push(format!(
                "{symbol} exists without a direct #[cfg(test)] gate"
            ));
        }
    }
    match (syn::parse_file(&generated_mod), syn::parse_file(&registry)) {
        (Ok(generated_file), Ok(registry_file)) => {
            failures.extend(generated_candidate_ast_failures(
                &generated_file,
                &registry_file,
                &candidate_modules,
            ));
        }
        (generated_file, registry_file) => {
            if let Err(error) = generated_file {
                failures.push(format!("mod.rs is not parseable Rust: {error}"));
            }
            if let Err(error) = registry_file {
                failures.push(format!("registry.rs is not parseable Rust: {error}"));
            }
        }
    }
    push(
        criteria,
        "all-generated-candidates-test-only",
        CriterionKind::Boundary,
        failures.is_empty(),
        if failures.is_empty() {
            format!(
                "all {} generated candidate module(s), and any candidate catalogs, are directly test-only",
                candidate_modules.len()
            )
        } else {
            failures.join("; ")
        },
    );
}

/// Freeze the application-facing side of the library objective.
///
/// The five direct switches above protect the known 2550Q path. This guard is
/// intentionally wider: it inventories semantic production call sites across
/// `bir-core` and `bir-desktop`, while ignoring comments, string literals, and
/// items directly gated by `#[cfg(test)]`. Existing generic scaffolding remains
/// permitted, but a new provider designation, evaluator caller, serialization
/// authority, Final Copy caller, or 2550Q queue/release authority breaches a
/// Boundary criterion instead of quietly becoming "library work".
fn check_application_integration_frozen(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    let sources = match FrozenApplicationSources::read(repo_root) {
        Ok(sources) => sources,
        Err(error) => {
            push(
                criteria,
                "application-integration-frozen",
                CriterionKind::Boundary,
                false,
                error,
            );
            return;
        }
    };
    let violations = application_freeze_violations(&sources);
    push(
        criteria,
        "application-integration-frozen",
        CriterionKind::Boundary,
        violations.is_empty(),
        if violations.is_empty() {
            "application integration remains frozen: only the reviewed diagnostic/scaffolding inventory is present, with no production filing authority"
                .to_owned()
        } else {
            format!(
                "{} application-freeze violation(s): {}",
                violations.len(),
                violations.join("; ")
            )
        },
    );
}

#[derive(Clone, Debug)]
struct FrozenApplicationSources {
    files: BTreeMap<String, String>,
}

impl FrozenApplicationSources {
    fn read(repo_root: &Path) -> std::result::Result<Self, String> {
        let repo_root = canonical_repo_root(repo_root)
            .map_err(|error| format!("cannot canonicalize frozen repository root: {error}"))?;
        let mut files = BTreeMap::new();
        for relative_root in ["crates/bir-core/src", "crates/bir-desktop/src"] {
            let root = resolve_existing_under(&repo_root, relative_root, relative_root).map_err(
                |error| format!("cannot resolve frozen source root {relative_root}: {error}"),
            )?;
            collect_rust_sources(&repo_root, &root, &mut files)?;
        }
        Ok(Self { files })
    }

    fn raw(&self, relative: &str) -> std::result::Result<&str, String> {
        self.files
            .get(relative)
            .map(String::as_str)
            .ok_or_else(|| format!("frozen source `{relative}` is missing"))
    }

    fn production(&self) -> std::result::Result<BTreeMap<String, String>, String> {
        self.files
            .iter()
            .filter(|(path, source)| {
                frozen_production_structure_path(path)
                    || contains_frozen_authority_token(source.as_bytes())
            })
            .map(|(path, source)| {
                production_rust_code(source)
                    .map(|source| (path.clone(), source))
                    .map_err(|error| format!("cannot inspect production Rust in {path}: {error}"))
            })
            .collect()
    }
}

fn collect_rust_sources(
    repo_root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, String>,
) -> std::result::Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            format!(
                "cannot read frozen directory {}: {error}",
                directory.display()
            )
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "cannot enumerate frozen directory {}: {error}",
                directory.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect frozen path {}: {error}", path.display()))?;
        if is_symlink_or_reparse_point(&metadata) {
            return Err(format!(
                "frozen application source must not be reached through a symlink or reparse point: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_rust_sources(repo_root, &path, files)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let relative = path
            .strip_prefix(repo_root)
            .map_err(|_| {
                format!(
                    "frozen source {} escaped repository root {}",
                    path.display(),
                    repo_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = read_tracked_bytes(&path)
            .map_err(|error| format!("cannot read frozen source {relative}: {error}"))?;
        let relevant = frozen_source_path(&relative) || contains_frozen_authority_token(&bytes);
        if relevant {
            let source = String::from_utf8(bytes)
                .map_err(|error| format!("frozen source {relative} is not UTF-8: {error}"))?;
            if files.insert(relative.clone(), source).is_some() {
                return Err(format!("duplicate frozen source path `{relative}`"));
            }
        } else {
            // Preserve the previous fail-closed UTF-8 validation without
            // allocating or retaining source that no freeze check can inspect.
            std::str::from_utf8(&bytes)
                .map_err(|error| format!("frozen source {relative} is not UTF-8: {error}"))?;
        }
    }
    Ok(())
}

fn frozen_source_path(relative: &str) -> bool {
    relative == CORE_CAPABILITIES_PATH || frozen_production_structure_path(relative)
}

fn frozen_production_structure_path(relative: &str) -> bool {
    FROZEN_PRODUCTION_STRUCTURE_PATHS.contains(&relative)
}

fn contains_frozen_authority_token(bytes: &[u8]) -> bool {
    use std::sync::OnceLock;

    static TOKENS: OnceLock<regex::bytes::RegexSet> = OnceLock::new();
    TOKENS
        .get_or_init(|| {
            regex::bytes::RegexSet::new(FROZEN_AUTHORITY_TOKENS)
                .expect("application-freeze authority tokens are valid byte regexes")
        })
        .is_match(bytes)
}

fn application_freeze_violations(sources: &FrozenApplicationSources) -> Vec<String> {
    let mut violations = Vec::new();
    let production = match sources.production() {
        Ok(production) => production,
        Err(error) => return vec![error],
    };

    check_occurrence_inventory(
        &production,
        r"\bbir_rules\s*::\s*generated\s*::\s*reviewed_rule_set_entries\s*\(",
        &[
            (CORE_2550Q_PATH, 2),
            ("crates/bir-core/src/form_rules/submission_preflight.rs", 1),
        ],
        "reviewed generated-registry consumers",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\b(?:candidate_rule_set_entries|CANDIDATE_RULE_SET_ENTRIES|CANDIDATE_RULE_SET_METADATA)\b",
        &[],
        "candidate-provider production consumers",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\bFormRevisionKey\s*::\s*new\s*\(",
        &[],
        "production exact-identity constructors",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\bFormRuleEvaluator\s*::\s*new\s*\(",
        &[("crates/bir-core/src/form_rules/submission_preflight.rs", 1)],
        "production rule evaluator constructors",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\.\s*evaluate_trusted\s*\(",
        &[("crates/bir-core/src/form_rules/submission_preflight.rs", 1)],
        "trusted-evaluation production calls",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\.\s*evaluate_shadow\s*\(",
        &[],
        "shadow-evaluation production calls",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\.\s*materialize_checked\s*\(",
        &[("crates/bir-core/src/form_rules/submission_preflight.rs", 1)],
        "checked-serialization production calls",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\bpreflight_active_form_submission\b",
        &[
            ("crates/bir-core/src/form_rules/mod.rs", 1),
            ("crates/bir-core/src/form_rules/submission_preflight.rs", 1),
        ],
        "submission-preflight production surfaces",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\.\s*create_form_final_copy\s*\(",
        &[],
        "Final Copy production callers",
        &mut violations,
    );
    check_occurrence_inventory(
        &production,
        r"\bCheckedFinalCopyPayload\s*::\s*try_new\s*\(",
        &[],
        "checked Final Copy payload production callers",
        &mut violations,
    );

    check_desktop_evaluator_surface(&production, &mut violations);
    check_closed_2550q_core_surface(&production, &mut violations);
    check_capability_matrix(sources, &mut violations);
    check_closed_2550q_actions(&production, &mut violations);
    check_2550q_transport_references(&production, &mut violations);

    violations
}

fn check_occurrence_inventory(
    sources: &BTreeMap<String, String>,
    pattern: &str,
    expected: &[(&str, usize)],
    label: &str,
    violations: &mut Vec<String>,
) {
    let regex = Regex::new(pattern).expect("application-freeze inventory regex is valid");
    let actual: BTreeMap<String, usize> = sources
        .iter()
        .filter_map(|(path, source)| {
            let count = regex.find_iter(source).count();
            (count != 0).then(|| (path.clone(), count))
        })
        .collect();
    let expected: BTreeMap<String, usize> = expected
        .iter()
        .map(|(path, count)| ((*path).to_owned(), *count))
        .collect();
    if actual != expected {
        violations.push(format!(
            "{label} changed (expected {expected:?}, found {actual:?})"
        ));
    }
}

fn check_desktop_evaluator_surface(
    production: &BTreeMap<String, String>,
    violations: &mut Vec<String>,
) {
    let desktop: BTreeMap<String, String> = production
        .iter()
        .filter(|(path, _)| path.starts_with("crates/bir-desktop/src/"))
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect();
    check_occurrence_inventory(
        &desktop,
        r"\b(?:FormRuleEvaluator|Form2550QLiveValidationEvaluator|EvaluationRequest|TrustedEvaluation|evaluate_trusted|evaluate_shadow|evaluate_official_diagnostic|evaluate_filing_safe_trusted|reviewed_rule_set_entries|candidate_rule_set_entries|CheckedSerializationArtifact|CheckedFinalCopyPayload|preflight_active_form_submission|create_form_final_copy|SerializationArtifactTarget|SubmissionPreflightArtifact)\b",
        &[],
        "desktop trusted-evaluator/serialization API references",
        violations,
    );
    check_occurrence_inventory(
        &desktop,
        r"\bForm2550QLiveValidationFacade\s*::\s*setup_repo_default_diagnostic\s*\(",
        &[(DESKTOP_FORM_2550Q_PATH, 1)],
        "desktop repository-diagnostic setup calls",
        violations,
    );

    let Some(view) = production.get(DESKTOP_FORM_2550Q_PATH) else {
        violations.push(format!("{DESKTOP_FORM_2550Q_PATH} is missing"));
        return;
    };
    let evaluate_calls = Regex::new(r"\.\s*evaluate\s*\(")
        .expect("diagnostic evaluate regex is valid")
        .find_iter(view)
        .count();
    if evaluate_calls != 1 {
        violations.push(format!(
            "{DESKTOP_FORM_2550Q_PATH} must retain exactly one inert diagnostic evaluate call; found {evaluate_calls}"
        ));
    }
    match enum_variants(view, r"\benum\s+Form2550QDiagnosticState\b") {
        Ok(variants)
            if variants
                == ["NotRun", "Unavailable", "Incomplete"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>() => {}
        Ok(variants) => violations.push(format!(
            "desktop 2550Q diagnostic state gained authority: {variants:?}"
        )),
        Err(error) => violations.push(error),
    }
}

fn check_closed_2550q_core_surface(
    production: &BTreeMap<String, String>,
    violations: &mut Vec<String>,
) {
    let Some(source) = production.get(CORE_2550Q_PATH) else {
        violations.push(format!("{CORE_2550Q_PATH} is missing"));
        return;
    };
    match enum_variants(source, r"\bpub\s+enum\s+Form2550QRepoLiveValidationState\b") {
        Ok(variants)
            if variants
                == ["Unavailable", "IncompleteCapture"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>() => {}
        Ok(variants) => violations.push(format!(
            "core 2550Q repository diagnostic state gained authority: {variants:?}"
        )),
        Err(error) => violations.push(error),
    }
    match enum_variants(
        source,
        r"\bpub\s+enum\s+Form2550QRepoDiagnosticSetupOutcome\b",
    ) {
        Ok(variants)
            if variants
                == ["ExactRegistrationAvailable", "Unavailable"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>() => {}
        Ok(variants) => violations.push(format!(
            "core 2550Q diagnostic setup state changed: {variants:?}"
        )),
        Err(error) => violations.push(error),
    }
    check_impl_method_inventory(
        source,
        r"\bimpl\s+Form2550QLiveValidationFacade\b",
        &["setup_repo_default_diagnostic"],
        "Form2550QLiveValidationFacade",
        violations,
    );
    check_impl_method_inventory(
        source,
        r"\bimpl\s+Form2550QRepoDiagnostic\b",
        &["evaluate"],
        "Form2550QRepoDiagnostic",
        violations,
    );
}

fn check_impl_method_inventory(
    source: &str,
    pattern: &str,
    expected: &[&str],
    label: &str,
    violations: &mut Vec<String>,
) {
    match braced_body_after_pattern(source, pattern) {
        Ok(body) => {
            let methods = top_level_function_names(body);
            let expected: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
            if methods != expected {
                violations.push(format!(
                    "{label} production method inventory changed (expected {expected:?}, found {methods:?})"
                ));
            }
        }
        Err(error) => violations.push(error),
    }
}

fn check_capability_matrix(sources: &FrozenApplicationSources, violations: &mut Vec<String>) {
    const EXPECTED: [(&str, &str, &str, u16, bool); 10] = [
        ("2551Q", "2018", "2551Qv2018", 1023, false),
        ("1601C", "2018", "1601Cv2018", 1023, false),
        ("0619E", "2018", "0619Ev2018", 1007, false),
        ("0619F", "2018", "0619Fv2018", 1007, false),
        ("0605", "1999", "0605v1999", 1007, false),
        ("1701Q", "2018", "1701Qv2018", 1007, false),
        ("2550Q", "2024", "2550Qv2024", 1007, false),
        ("1701", "2018", "1701v2018", 1007, false),
        ("1702RT", "2018C", "1702RTv2018C", 1007, false),
        ("1702MX", "2018C", "1702MXv2018C", 1007, false),
    ];
    let source = match sources.raw(CORE_CAPABILITIES_PATH) {
        Ok(source) => source,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match capability_matrix(source) {
        Ok(actual) => {
            let expected: Vec<CapabilityFact> = EXPECTED
                .iter()
                .map(
                    |(code, revision, form_id, bits, release_ready)| CapabilityFact {
                        code: (*code).to_owned(),
                        revision: (*revision).to_owned(),
                        form_id: (*form_id).to_owned(),
                        capability_bits: *bits,
                        release_ready: *release_ready,
                    },
                )
                .collect();
            if actual != expected {
                violations.push(format!(
                    "form capability/release authority changed (expected {expected:?}, found {actual:?})"
                ));
            }
        }
        Err(error) => violations.push(error),
    }
}

fn check_closed_2550q_actions(production: &BTreeMap<String, String>, violations: &mut Vec<String>) {
    let Some(core) = production.get(CORE_FORM_2550Q_PATH) else {
        violations.push(format!("{CORE_FORM_2550Q_PATH} is missing"));
        return;
    };
    if !source_matches(
        core,
        r"(?m)^[ \t]*pub[ \t]+const[ \t]+QUEUE_SUBMISSION_SUPPORTED[ \t]*:[ \t]*bool[ \t]*=[ \t]*false[ \t]*;",
    ) {
        violations
            .push("2550Q QUEUE_SUBMISSION_SUPPORTED is missing or no longer false".to_owned());
    }
    match braced_body_after_pattern(
        core,
        r"\bpub\s+fn\s+transition_to_queued\s*\([^)]*\)\s*->\s*Result",
    ) {
        Ok(body) if compact_rust(body) == "Err(vec![(.to_string(),.to_string(),)])" => {}
        Ok(body) => violations.push(format!(
            "2550Q transition_to_queued is no longer the inert Err-only body: {}",
            compact_rust(body)
        )),
        Err(error) => violations.push(error),
    }

    let Some(desktop) = production.get(DESKTOP_FORM_2550Q_PATH) else {
        violations.push(format!("{DESKTOP_FORM_2550Q_PATH} is missing"));
        return;
    };
    match braced_body_after_pattern(desktop, r"\bfn\s+mark_submitted\s*\([^)]*\)\s*") {
        Ok(body)
            if compact_rust(body)
                == "self.status_message=Some(.to_string(),);cx.emit(Form2550QV2Event::PushNotification(.to_string(),.to_string(),.to_string(),));cx.notify();" =>
            {}
        Ok(body) => violations.push(format!(
            "desktop 2550Q mark_submitted is no longer notification-only: {}",
            compact_rust(body)
        )),
        Err(error) => violations.push(error),
    }
}

fn check_2550q_transport_references(
    production: &BTreeMap<String, String>,
    violations: &mut Vec<String>,
) {
    let authority_paths = [
        "crates/bir-core/src/background_cron.rs",
        "crates/bir-core/src/official_import.rs",
        "crates/bir-core/src/transport.rs",
    ];
    for path in authority_paths {
        let Some(source) = production.get(path) else {
            violations.push(format!("{path} is missing"));
            continue;
        };
        if source_matches(source, r"\b(?:Form2550QDraft|Form2550Q|form_2550q)\b") {
            violations.push(format!(
                "{path} gained a production 2550Q queue/transport/submission reference"
            ));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapabilityFact {
    code: String,
    revision: String,
    form_id: String,
    capability_bits: u16,
    release_ready: bool,
}

fn capability_matrix(source: &str) -> std::result::Result<Vec<CapabilityFact>, String> {
    const FIELDS: [&str; 15] = [
        "typed_model",
        "xml_round_trip",
        "formula_evidence",
        "persistence",
        "queue_submission",
        "editor",
        "render_contract",
        "html_component",
        "html_spec",
        "pagination",
        "visual_parity",
        "native_preview",
        "native_print",
        "pdf_export",
        "packaged_offline",
    ];
    let code = mask_rust_non_code(source)?;
    let scaffold =
        braced_body_after_pattern(&code, r"\bconst\s+SCAFFOLD\s*:\s*FormCapabilities\b")?;
    let scaffold_values = capability_values(scaffold, None, &FIELDS)?;

    let registry_pattern = Regex::new(
        r"\bpub\s+const\s+FORM_CAPABILITY_REGISTRY\s*:\s*&\s*\[\s*FormCapabilityRecord\s*\]\s*=\s*&\s*\[",
    )
    .expect("capability registry regex is valid");
    let registry_match = registry_pattern
        .find(&code)
        .ok_or_else(|| "FORM_CAPABILITY_REGISTRY initializer is missing".to_owned())?;
    let open = code[registry_match.start()..registry_match.end()]
        .rfind('[')
        .map(|offset| registry_match.start() + offset)
        .ok_or_else(|| "FORM_CAPABILITY_REGISTRY opening bracket is missing".to_owned())?;
    let close = matching_delimiter(&code, open, b'[', b']')?;
    let registry_code = &code[open + 1..close];
    let registry_raw = &source[open + 1..close];
    let record_regex =
        Regex::new(r"\bFormCapabilityRecord\s*\{").expect("capability record regex is valid");
    let string_field = |block: &str, field: &str| -> std::result::Result<String, String> {
        let regex = Regex::new(&format!(r#"\b{}\s*:\s*"([^"]+)""#, regex::escape(field)))
            .expect("capability string-field regex is valid");
        let captures = regex
            .captures(block)
            .ok_or_else(|| format!("capability record is missing `{field}`"))?;
        Ok(captures[1].to_owned())
    };
    let release_regex =
        Regex::new(r"\brelease_ready\s*:\s*(true|false)").expect("release-ready regex is valid");

    let mut facts = Vec::new();
    let mut cursor = 0usize;
    while let Some(record_match) = record_regex.find_at(registry_code, cursor) {
        let record_open = registry_code[record_match.start()..record_match.end()]
            .rfind('{')
            .map(|offset| record_match.start() + offset)
            .ok_or_else(|| "capability record opening brace is missing".to_owned())?;
        let record_close = matching_delimiter(registry_code, record_open, b'{', b'}')?;
        let raw_block = &registry_raw[record_match.start()..=record_close];
        let code_block = &registry_code[record_match.start()..=record_close];
        let capabilities_match = CAPABILITIES_PATTERN
            .find(code_block)
            .ok_or_else(|| "capability record lacks FormCapabilities initializer".to_owned())?;
        let capabilities_open = code_block[capabilities_match.start()..capabilities_match.end()]
            .rfind('{')
            .map(|offset| capabilities_match.start() + offset)
            .ok_or_else(|| "FormCapabilities opening brace is missing".to_owned())?;
        let capabilities_close = matching_delimiter(code_block, capabilities_open, b'{', b'}')?;
        let capabilities_body = &code_block[capabilities_open + 1..capabilities_close];
        let inherits_scaffold = source_matches(capabilities_body, r"\.\.\s*SCAFFOLD\b");
        let values = capability_values(
            capabilities_body,
            inherits_scaffold.then_some(&scaffold_values),
            &FIELDS,
        )?;
        let capability_bits = values
            .iter()
            .enumerate()
            .fold(0u16, |bits, (index, value)| {
                bits | ((if *value { 1u16 } else { 0u16 }) << index)
            });
        let release = release_regex
            .captures(raw_block)
            .ok_or_else(|| "capability record is missing release_ready".to_owned())?;
        facts.push(CapabilityFact {
            code: string_field(raw_block, "code")?,
            revision: string_field(raw_block, "revision")?,
            form_id: string_field(raw_block, "form_id")?,
            capability_bits,
            release_ready: &release[1] == "true",
        });
        cursor = record_close + 1;
    }
    if facts.is_empty() {
        return Err("FORM_CAPABILITY_REGISTRY contains no records".to_owned());
    }
    Ok(facts)
}

fn capability_values(
    body: &str,
    inherited: Option<&[bool; 15]>,
    fields: &[&str; 15],
) -> std::result::Result<[bool; 15], String> {
    let mut values = inherited.copied().unwrap_or([false; 15]);
    let mut seen = BTreeSet::new();
    let assignment = Regex::new(r"\b([a-z_]+)\s*:\s*(true|false)")
        .expect("capability assignment regex is valid");
    for captures in assignment.captures_iter(body) {
        let name = &captures[1];
        let Some(index) = fields.iter().position(|field| *field == name) else {
            return Err(format!("unknown FormCapabilities boolean field `{name}`"));
        };
        if !seen.insert(name.to_owned()) {
            return Err(format!("duplicate FormCapabilities field `{name}`"));
        }
        values[index] = &captures[2] == "true";
    }
    if inherited.is_none() && seen.len() != fields.len() {
        let missing: Vec<&str> = fields
            .iter()
            .copied()
            .filter(|field| !seen.contains(*field))
            .collect();
        return Err(format!(
            "FormCapabilities initializer is incomplete; missing {missing:?}"
        ));
    }
    Ok(values)
}

fn production_rust_code(source: &str) -> std::result::Result<String, String> {
    let code = mask_rust_non_code(source)?;
    let cfg_test =
        Regex::new(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]").expect("cfg(test) regex is valid");
    let mut cfg_test_items = Vec::new();
    let mut cursor = 0usize;
    while let Some(attribute) = cfg_test.find_at(&code, cursor) {
        let end = cfg_test_item_end(&code, attribute.end())?;
        cfg_test_items.push((attribute.start(), end));
        cursor = end;
    }

    let mut code = code.into_bytes();
    for (start, end) in cfg_test_items {
        mask_non_newline(&mut code[start..end]);
    }
    String::from_utf8(code).map_err(|error| format!("masked Rust was not UTF-8: {error}"))
}

fn cfg_test_item_end(source: &str, start: usize) -> std::result::Result<usize, String> {
    let bytes = source.as_bytes();
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => parentheses += 1,
            b')' => {
                parentheses = parentheses
                    .checked_sub(1)
                    .ok_or_else(|| "unbalanced parenthesis after #[cfg(test)]".to_owned())?;
            }
            b'[' => brackets += 1,
            b']' => {
                brackets = brackets
                    .checked_sub(1)
                    .ok_or_else(|| "unbalanced bracket after #[cfg(test)]".to_owned())?;
            }
            b';' if parentheses == 0 && brackets == 0 => return Ok(index + 1),
            b'{' if parentheses == 0 && brackets == 0 => {
                return matching_delimiter(source, index, b'{', b'}').map(|end| end + 1);
            }
            _ => {}
        }
        index += 1;
    }
    Err("#[cfg(test)] is not followed by a complete Rust item".to_owned())
}

/// Replace comments and literal contents with spaces while retaining byte
/// offsets, line breaks, punctuation, identifiers, and delimiters.
fn mask_rust_non_code(source: &str) -> std::result::Result<String, String> {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            let start = index;
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            mask_non_newline(&mut masked[start..index]);
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let start = index;
            index += 2;
            let mut depth = 1usize;
            while index < bytes.len() && depth != 0 {
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            if depth != 0 {
                return Err("unterminated Rust block comment".to_owned());
            }
            mask_non_newline(&mut masked[start..index]);
            continue;
        }
        if let Some((prefix_end, hashes)) = raw_string_start(bytes, index) {
            let start = index;
            index = prefix_end;
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'"'
                    && (0..hashes).all(|offset| bytes.get(index + 1 + offset) == Some(&b'#'))
                {
                    index += 1 + hashes;
                    closed = true;
                    break;
                }
                index += 1;
            }
            if !closed {
                return Err("unterminated Rust raw string".to_owned());
            }
            mask_non_newline(&mut masked[start..index]);
            continue;
        }
        if bytes[index] == b'"' {
            let start = index;
            index += 1;
            let mut closed = false;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    index += 1;
                    closed = true;
                    break;
                } else {
                    index += 1;
                }
            }
            if !closed {
                return Err("unterminated Rust string".to_owned());
            }
            mask_non_newline(&mut masked[start..index]);
            continue;
        }
        if bytes[index] == b'\''
            && let Some(end) = rust_char_literal_end(bytes, index)
        {
            mask_non_newline(&mut masked[index..end]);
            index = end;
            continue;
        }
        index += 1;
    }
    String::from_utf8(masked).map_err(|error| format!("masked Rust was not UTF-8: {error}"))
}

fn mask_non_newline(bytes: &mut [u8]) {
    for byte in bytes {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

fn raw_string_start(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut index = start;
    if bytes.get(index) == Some(&b'b') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'r') {
        return None;
    }
    index += 1;
    let hash_start = index;
    while bytes.get(index) == Some(&b'#') {
        index += 1;
    }
    (bytes.get(index) == Some(&b'"')).then_some((index + 1, index - hash_start))
}

fn rust_char_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let first = *bytes.get(start + 1)?;
    if first != b'\\' {
        return (bytes.get(start + 2) == Some(&b'\'')).then_some(start + 3);
    }
    let escape = *bytes.get(start + 2)?;
    let closing = match escape {
        b'x' => start + 5,
        b'u' if bytes.get(start + 3) == Some(&b'{') => {
            let end = bytes[start + 4..]
                .iter()
                .position(|byte| *byte == b'}')
                .map(|offset| start + 4 + offset)?;
            end + 1
        }
        _ => start + 3,
    };
    (bytes.get(closing) == Some(&b'\'')).then_some(closing + 1)
}

fn matching_delimiter(
    source: &str,
    open: usize,
    opening: u8,
    closing: u8,
) -> std::result::Result<usize, String> {
    let bytes = source.as_bytes();
    if bytes.get(open) != Some(&opening) {
        return Err(format!(
            "expected `{}` delimiter at byte {open}",
            char::from(opening)
        ));
    }
    let mut depth = 0usize;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        if *byte == opening {
            depth += 1;
        } else if *byte == closing {
            depth -= 1;
            if depth == 0 {
                return Ok(open + offset);
            }
        }
    }
    Err(format!(
        "unclosed `{}` delimiter at byte {open}",
        char::from(opening)
    ))
}

fn braced_body_after_pattern<'a>(
    source: &'a str,
    pattern: &str,
) -> std::result::Result<&'a str, String> {
    let regex = Regex::new(pattern).expect("braced-body regex is valid");
    let matched = regex
        .find(source)
        .ok_or_else(|| format!("required semantic source pattern `{pattern}` is missing"))?;
    let open = source[matched.end()..]
        .find('{')
        .map(|offset| matched.end() + offset)
        .ok_or_else(|| format!("source pattern `{pattern}` has no body"))?;
    let close = matching_delimiter(source, open, b'{', b'}')?;
    Ok(&source[open + 1..close])
}

fn top_level_function_names(body: &str) -> Vec<String> {
    let regex =
        Regex::new(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)").expect("function-name regex is valid");
    let mut depth = 0isize;
    let mut depth_at = vec![0isize; body.len() + 1];
    for (index, byte) in body.bytes().enumerate() {
        depth_at[index] = depth;
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
    }
    regex
        .captures_iter(body)
        .filter(|captures| depth_at[captures.get(0).expect("whole match").start()] == 0)
        .map(|captures| captures[1].to_owned())
        .collect()
}

fn enum_variants(source: &str, pattern: &str) -> std::result::Result<Vec<String>, String> {
    let body = braced_body_after_pattern(source, pattern)?;
    let identifier =
        Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\b").expect("enum identifier regex is valid");
    let mut variants = Vec::new();
    let mut braces = 0isize;
    let mut parentheses = 0isize;
    let mut brackets = 0isize;
    let mut depth_at = vec![(0isize, 0isize, 0isize); body.len() + 1];
    for (index, byte) in body.bytes().enumerate() {
        depth_at[index] = (braces, parentheses, brackets);
        match byte {
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b'(' => parentheses += 1,
            b')' => parentheses -= 1,
            b'[' => brackets += 1,
            b']' => brackets -= 1,
            _ => {}
        }
    }
    for captures in identifier.captures_iter(body) {
        let whole = captures.get(0).expect("whole enum identifier");
        if depth_at[whole.start()] != (0, 0, 0) {
            continue;
        }
        let next = body[whole.end()..]
            .bytes()
            .find(|byte| !byte.is_ascii_whitespace());
        if matches!(next, Some(b'{' | b'(' | b',' | b'=')) {
            variants.push(captures[1].to_owned());
        }
    }
    Ok(variants)
}

fn compact_rust(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn check_source_guard(
    repo_root: &Path,
    relative: &'static str,
    criteria: &mut Vec<Criterion>,
    id: &'static str,
    predicate: impl FnOnce(&str) -> bool,
    met_detail: &'static str,
    unmet_detail: &'static str,
) {
    match read_text(repo_root, relative) {
        Ok(source) => {
            let met = predicate(&source);
            push(
                criteria,
                id,
                CriterionKind::Boundary,
                met,
                if met {
                    met_detail.to_owned()
                } else {
                    format!("{unmet_detail} in {relative}")
                },
            );
        }
        Err(error) => push(
            criteria,
            id,
            CriterionKind::Boundary,
            false,
            format!("cannot read {relative}: {error}"),
        ),
    }
}

fn source_matches(source: &str, pattern: &str) -> bool {
    Regex::new(pattern)
        .expect("status source guard regex is valid")
        .is_match(source)
}

fn check_aggregate_library_gates(
    repo_root: &Path,
    coverage: std::result::Result<&CoverageReport, &CodegenError>,
    reconciliation: std::result::Result<&ReconciliationReport, &CodegenError>,
    criteria: &mut Vec<Criterion>,
) {
    let coverage = match coverage {
        Ok(coverage) => {
            let snapshot_coverage_met = coverage.form_count == EXPECTED_FORM_COUNT
                && coverage.forms_with_v2_snapshot == EXPECTED_FORM_COUNT;
            push(
                criteria,
                "v2-snapshot-coverage",
                CriterionKind::ActiveLibrary,
                snapshot_coverage_met,
                format!(
                    "{}/{} form(s) have a v2 snapshot; expected {EXPECTED_FORM_COUNT}/{EXPECTED_FORM_COUNT}",
                    coverage.forms_with_v2_snapshot, coverage.form_count
                ),
            );
            check_candidate_generated_catalog(repo_root, coverage, criteria);
            Some(coverage)
        }
        Err(error) => {
            for id in [
                "v2-snapshot-coverage",
                "candidate-generated-catalog-coverage",
            ] {
                push(
                    criteria,
                    id,
                    CriterionKind::ActiveLibrary,
                    false,
                    format!("cannot measure aggregate coverage: {error}"),
                );
            }
            None
        }
    };

    let reconciliation = match reconciliation {
        Ok(reconciliation) => reconciliation,
        Err(error) => {
            for id in [
                "record-reconciliation-complete",
                "validations-represented",
                "calculations-represented",
            ] {
                push(
                    criteria,
                    id,
                    CriterionKind::ActiveLibrary,
                    false,
                    format!("cannot reconcile aggregate library records: {error}"),
                );
            }
            return;
        }
    };

    let required_artifacts = ["fields", "validations", "calculations", "workflow"];
    let mut missing_artifact_surfaces = Vec::new();
    for form in &reconciliation.forms {
        for artifact in required_artifacts {
            if !form
                .artifacts
                .iter()
                .any(|candidate| candidate.artifact == artifact)
            {
                missing_artifact_surfaces.push(format!("{}:{artifact}", form.form_id));
            }
        }
    }
    let reconciliation_met = reconciliation.forms_with_v2_snapshot == EXPECTED_FORM_COUNT
        && reconciliation.complete_forms == EXPECTED_FORM_COUNT
        && reconciliation.unclassified_records == 0
        && reconciliation.unresolved_records == 0
        && missing_artifact_surfaces.is_empty();
    push(
        criteria,
        "record-reconciliation-complete",
        CriterionKind::ActiveLibrary,
        reconciliation_met,
        if missing_artifact_surfaces.is_empty() {
            format!(
                "{}/{} form(s) complete; {} unclassified and {} unresolved legacy record(s)",
                reconciliation.complete_forms,
                reconciliation.forms_with_v2_snapshot,
                reconciliation.unclassified_records,
                reconciliation.unresolved_records
            )
        } else {
            format!(
                "{}/{} form(s) complete; {} unclassified and {} unresolved legacy record(s); missing artifact surfaces: {}",
                reconciliation.complete_forms,
                reconciliation.forms_with_v2_snapshot,
                reconciliation.unclassified_records,
                reconciliation.unresolved_records,
                missing_artifact_surfaces.join(", ")
            )
        },
    );

    check_represented_record_total(
        reconciliation,
        criteria,
        "validations-represented",
        "validations",
        coverage.map(|coverage| coverage.v1_rules),
        EXPECTED_VALIDATION_RECORDS,
    );
    check_represented_record_total(
        reconciliation,
        criteria,
        "calculations-represented",
        "calculations",
        coverage.map(|coverage| coverage.v1_calculations),
        EXPECTED_CALCULATION_RECORDS,
    );
}

fn check_represented_record_total(
    reconciliation: &ReconciliationReport,
    criteria: &mut Vec<Criterion>,
    id: &'static str,
    artifact_name: &str,
    coverage_v1_total: Option<usize>,
    expected_total: usize,
) {
    let (legacy, represented, non_runtime, unresolved, unclassified) =
        reconciliation_artifact_totals(reconciliation, artifact_name);
    let accounted = represented.checked_add(non_runtime);
    let met = represented_record_total_met(
        coverage_v1_total,
        expected_total,
        legacy,
        represented,
        non_runtime,
        unresolved,
        unclassified,
    );
    push(
        criteria,
        id,
        CriterionKind::ActiveLibrary,
        met,
        format!(
            "{}/{} {artifact_name} accounted; executable/represented={represented}, intentionally-non-runtime={non_runtime}, legacy={legacy}, unresolved={unresolved}, unclassified={unclassified}, v1-corpus={}",
            accounted.map_or("<overflow>".to_owned(), |total| total.to_string()),
            expected_total,
            coverage_v1_total.map_or("<unavailable>".to_owned(), |total| total.to_string())
        ),
    );
}

fn represented_record_total_met(
    coverage_v1_total: Option<usize>,
    expected_total: usize,
    legacy: usize,
    represented: usize,
    non_runtime: usize,
    unresolved: usize,
    unclassified: usize,
) -> bool {
    coverage_v1_total == Some(expected_total)
        && legacy == expected_total
        && represented.checked_add(non_runtime) == Some(expected_total)
        && unresolved == 0
        && unclassified == 0
}

fn reconciliation_artifact_totals(
    report: &ReconciliationReport,
    artifact_name: &str,
) -> (usize, usize, usize, usize, usize) {
    report
        .forms
        .iter()
        .flat_map(|form| form.artifacts.iter())
        .filter(|artifact| artifact.artifact == artifact_name)
        .fold(
            (0, 0, 0, 0, 0),
            |(legacy, represented, non_runtime, unresolved, unclassified), artifact| {
                (
                    legacy + artifact.legacy_records,
                    represented + artifact.represented_records,
                    non_runtime + artifact.intentionally_non_runtime_records,
                    unresolved + artifact.unresolved_records,
                    unclassified + artifact.unclassified_records,
                )
            },
        )
}

fn check_candidate_generated_catalog(
    repo_root: &Path,
    coverage: &CoverageReport,
    criteria: &mut Vec<Criterion>,
) {
    let manifest = match read_json(repo_root, GENERATED_MANIFEST_PATH) {
        Ok(manifest) => manifest,
        Err(error) => {
            push(
                criteria,
                "candidate-generated-catalog-coverage",
                CriterionKind::ActiveLibrary,
                false,
                format!("cannot read {GENERATED_MANIFEST_PATH}: {error}"),
            );
            return;
        }
    };
    let registry = match read_text(repo_root, GENERATED_REGISTRY_PATH) {
        Ok(registry) => registry,
        Err(error) => {
            push(
                criteria,
                "candidate-generated-catalog-coverage",
                CriterionKind::ActiveLibrary,
                false,
                format!("cannot read {GENERATED_REGISTRY_PATH}: {error}"),
            );
            return;
        }
    };
    let generated_mod = match read_text(repo_root, GENERATED_MOD_PATH) {
        Ok(generated_mod) => generated_mod,
        Err(error) => {
            push(
                criteria,
                "candidate-generated-catalog-coverage",
                CriterionKind::ActiveLibrary,
                false,
                format!("cannot read {GENERATED_MOD_PATH}: {error}"),
            );
            return;
        }
    };

    let candidate_modules: Vec<&str> = manifest
        .object()
        .and_then(|manifest| manifest.get("snapshots"))
        .and_then(array)
        .into_iter()
        .flatten()
        .filter(|snapshot| string_at(snapshot, "review_status") == Some("candidate"))
        .filter_map(|snapshot| string_at(snapshot, "generated_module"))
        .collect();
    let unique_modules: BTreeSet<&str> = candidate_modules.iter().copied().collect();
    let metadata_catalog_test_only = source_matches(
        &registry,
        r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*pub[ \t]+static[ \t]+CANDIDATE_RULE_SET_METADATA",
    );
    let provider_catalog_test_only = source_matches(
        &registry,
        r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*static[ \t]+CANDIDATE_RULE_SET_ENTRIES",
    ) && source_matches(
        &registry,
        r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*pub[ \t]+fn[ \t]+candidate_rule_set_entries",
    );
    let every_module_cataloged = unique_modules.iter().all(|module| {
        let module_pattern = format!(
            r"(?ms)^[ \t]*#[ \t]*\[[ \t]*cfg[ \t]*\([ \t]*test[ \t]*\)[ \t]*\][ \t\r\n]+[ \t]*mod[ \t]+{}[ \t]*;",
            regex::escape(module)
        );
        let metadata_pattern = format!(
            r"(?m)^[ \t]*rule_set_id:[ \t]*super::{}::RULE_SET_ID[ \t]*,",
            regex::escape(module)
        );
        let provider_pattern = format!(
            r"(?m)^[ \t]*RuleSetRegistryEntry::new[ \t]*\([ \t]*&\*super::{}::COMPILED_RULE_SET[ \t]*\)[ \t]*,",
            regex::escape(module)
        );
        source_matches(&generated_mod, &module_pattern)
            && source_matches(&registry, &metadata_pattern)
            && source_matches(&registry, &provider_pattern)
    });
    let met = coverage.form_count == EXPECTED_FORM_COUNT
        && unique_modules.len() == EXPECTED_FORM_COUNT
        && unique_modules.len() == candidate_modules.len()
        && metadata_catalog_test_only
        && provider_catalog_test_only
        && every_module_cataloged;
    push(
        criteria,
        "candidate-generated-catalog-coverage",
        CriterionKind::ActiveLibrary,
        met,
        format!(
            "{}/{} candidate module(s) have unique generated manifest identities; metadata-catalog-test-only={metadata_catalog_test_only}, provider-catalog-test-only={provider_catalog_test_only}, every-module-cataloged={every_module_cataloged}",
            unique_modules.len(),
            coverage.form_count
        ),
    );
}

fn generated_candidate_ast_failures(
    generated_file: &syn::File,
    registry_file: &syn::File,
    candidate_modules: &[&str],
) -> Vec<String> {
    let expected_modules: BTreeSet<&str> = candidate_modules.iter().copied().collect();
    let mut module_counts = BTreeMap::<String, usize>::new();
    let mut reviewed_use_count = 0_usize;
    let mut candidate_use_count = 0_usize;
    let mut failures = Vec::new();

    for item in &generated_file.items {
        match item {
            Item::Mod(module) => {
                let name = module.ident.to_string();
                *module_counts.entry(name.clone()).or_default() += 1;
                if name == "registry" {
                    if !module.attrs.is_empty()
                        || module.content.is_some()
                        || module.semi.is_none()
                        || !matches!(module.vis, syn::Visibility::Inherited)
                    {
                        failures.push(
                            "the generated registry must be exactly the private, attribute-free external declaration `mod registry;`"
                                .to_owned(),
                        );
                    }
                } else if expected_modules.contains(name.as_str()) {
                    if !has_exact_cfg_test(&module.attrs)
                        || module.attrs.len() != 1
                        || module.content.is_some()
                        || module.semi.is_none()
                        || !matches!(module.vis, syn::Visibility::Inherited)
                    {
                        failures.push(format!(
                            "candidate module `{name}` is not an external private module guarded only by #[cfg(test)]"
                        ));
                    }
                } else {
                    failures.push(format!(
                        "unexpected generated module `{name}` could bypass the candidate manifest"
                    ));
                }
            }
            Item::Use(item_use) => {
                let paths = use_tree_paths(&item_use.tree);
                let reviewed = [
                    "GeneratedRuleSetMetadata",
                    "REVIEWED_RULE_SET_METADATA",
                    "reviewed_rule_set_entries",
                ]
                .into_iter()
                .map(|name| vec!["registry".to_owned(), name.to_owned()])
                .collect::<BTreeSet<_>>();
                let candidates = ["CANDIDATE_RULE_SET_METADATA", "candidate_rule_set_entries"]
                    .into_iter()
                    .map(|name| vec!["registry".to_owned(), name.to_owned()])
                    .collect::<BTreeSet<_>>();
                if !matches!(item_use.vis, syn::Visibility::Public(_)) {
                    failures.push("generated mod.rs contains a non-public use item".to_owned());
                } else if paths == reviewed {
                    reviewed_use_count += 1;
                    if has_cfg_attribute(&item_use.attrs) {
                        failures
                            .push("reviewed registry re-exports must be unconditional".to_owned());
                    }
                } else if paths == candidates {
                    candidate_use_count += 1;
                    if !has_exact_cfg_test(&item_use.attrs) {
                        failures.push(
                            "candidate registry re-exports are not guarded by exactly #[cfg(test)]"
                                .to_owned(),
                        );
                    }
                } else {
                    failures.push(format!(
                        "unexpected generated mod.rs re-export set: {}",
                        render_use_paths(&paths)
                    ));
                }
            }
            other => failures.push(format!(
                "unexpected top-level item `{}` in generated mod.rs",
                rust_item_kind(other)
            )),
        }
    }

    if module_counts.get("registry").copied() != Some(1) {
        failures.push("generated mod.rs must declare exactly one registry module".to_owned());
    }
    for module in &expected_modules {
        if module_counts.get(*module).copied() != Some(1) {
            failures.push(format!(
                "candidate module `{module}` must have exactly one declaration"
            ));
        }
    }
    if reviewed_use_count != 1 {
        failures.push("generated mod.rs must contain exactly one reviewed re-export".to_owned());
    }
    if candidate_use_count != 1 {
        failures.push(
            "generated mod.rs must contain exactly one test-only candidate re-export".to_owned(),
        );
    }
    failures.extend(registry_boundary_ast_failures(registry_file));
    failures
}

fn registry_boundary_ast_failures(file: &syn::File) -> Vec<String> {
    const REVIEWED_METADATA: &str = "REVIEWED_RULE_SET_METADATA";
    const REVIEWED_ENTRIES: &str = "REVIEWED_RULE_SET_ENTRIES";
    const REVIEWED_ACCESSOR: &str = "reviewed_rule_set_entries";
    const CANDIDATE_METADATA: &str = "CANDIDATE_RULE_SET_METADATA";
    const CANDIDATE_ENTRIES: &str = "CANDIDATE_RULE_SET_ENTRIES";
    const CANDIDATE_ACCESSOR: &str = "candidate_rule_set_entries";

    let mut item_counts = BTreeMap::<String, usize>::new();
    let mut use_counts = BTreeMap::<String, usize>::new();
    let mut failures = Vec::new();
    for item in &file.items {
        match item {
            Item::Use(item_use) => {
                let paths = use_tree_paths(&item_use.tree);
                if has_cfg_attribute(&item_use.attrs)
                    || !matches!(item_use.vis, syn::Visibility::Inherited)
                    || paths.len() != 1
                {
                    failures.push(
                        "registry imports must be private, unconditional, single-symbol imports"
                            .to_owned(),
                    );
                    continue;
                }
                let path = paths.into_iter().next().expect("one import path");
                let (name, expected_path) = match path.last().map(String::as_str) {
                    Some("RuleSetRegistryEntry") => (
                        "RuleSetRegistryEntry",
                        vec!["crate".to_owned(), "RuleSetRegistryEntry".to_owned()],
                    ),
                    Some("LazyLock") => (
                        "LazyLock",
                        vec!["std".to_owned(), "sync".to_owned(), "LazyLock".to_owned()],
                    ),
                    _ => {
                        failures.push(format!("unexpected registry import `{}`", path.join("::")));
                        continue;
                    }
                };
                if path != expected_path {
                    failures.push(format!(
                        "registry import `{name}` comes from `{}` instead of `{}`",
                        path.join("::"),
                        expected_path.join("::")
                    ));
                }
                *use_counts.entry(name.to_owned()).or_default() += 1;
            }
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                if name != "GeneratedRuleSetMetadata"
                    || has_cfg_attribute(&item_struct.attrs)
                    || !matches!(item_struct.vis, syn::Visibility::Public(_))
                {
                    failures.push(format!(
                        "unexpected or conditional registry struct `{name}`"
                    ));
                }
                *item_counts.entry(name).or_default() += 1;
            }
            Item::Static(item_static) => {
                let name = item_static.ident.to_string();
                let reviewed = matches!(name.as_str(), REVIEWED_METADATA | REVIEWED_ENTRIES);
                let candidate = matches!(name.as_str(), CANDIDATE_METADATA | CANDIDATE_ENTRIES);
                if reviewed {
                    if has_cfg_attribute(&item_static.attrs) {
                        failures.push(format!(
                            "reviewed registry static `{name}` must be unconditional"
                        ));
                    }
                    if !reviewed_static_semantics_are_closed(item_static) {
                        failures.push(format!(
                            "reviewed registry static `{name}` does not have the exact closed type and initializer"
                        ));
                    }
                } else if candidate {
                    if !has_exact_cfg_test(&item_static.attrs) {
                        failures.push(format!(
                            "candidate registry static `{name}` is not guarded by exactly #[cfg(test)]"
                        ));
                    }
                } else {
                    failures.push(format!(
                        "unexpected registry static `{name}` could provide a decoy production catalog"
                    ));
                }
                *item_counts.entry(name).or_default() += 1;
            }
            Item::Fn(item_fn) => {
                let name = item_fn.sig.ident.to_string();
                if name == REVIEWED_ACCESSOR {
                    if has_cfg_attribute(&item_fn.attrs) {
                        failures
                            .push("reviewed registry accessor must be unconditional".to_owned());
                    }
                    if !reviewed_accessor_semantics_are_closed(item_fn) {
                        failures.push(
                            "reviewed registry accessor does not have the exact closed signature and body"
                                .to_owned(),
                        );
                    }
                } else if name == CANDIDATE_ACCESSOR {
                    if !has_exact_cfg_test(&item_fn.attrs) {
                        failures.push(
                            "candidate registry accessor is not guarded by exactly #[cfg(test)]"
                                .to_owned(),
                        );
                    }
                } else {
                    failures.push(format!(
                        "unexpected registry function `{name}` could expose a production provider"
                    ));
                }
                *item_counts.entry(name).or_default() += 1;
            }
            other => failures.push(format!(
                "unexpected top-level item `{}` in generated registry",
                rust_item_kind(other)
            )),
        }
    }

    for name in ["RuleSetRegistryEntry", "LazyLock"] {
        if use_counts.get(name).copied() != Some(1) {
            failures.push(format!(
                "generated registry must import `{name}` exactly once"
            ));
        }
    }
    for name in [
        "GeneratedRuleSetMetadata",
        REVIEWED_METADATA,
        REVIEWED_ENTRIES,
        REVIEWED_ACCESSOR,
        CANDIDATE_METADATA,
        CANDIDATE_ENTRIES,
        CANDIDATE_ACCESSOR,
    ] {
        if item_counts.get(name).copied() != Some(1) {
            failures.push(format!(
                "generated registry must declare `{name}` exactly once"
            ));
        }
    }
    failures
}

fn has_cfg_attribute(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg"))
}

fn has_exact_cfg_test(attributes: &[Attribute]) -> bool {
    let cfg_attributes = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .collect::<Vec<_>>();
    cfg_attributes.len() == 1
        && cfg_attributes[0]
            .parse_args::<Meta>()
            .is_ok_and(|meta| matches!(meta, Meta::Path(path) if path.is_ident("test")))
}

fn use_tree_paths(tree: &UseTree) -> BTreeSet<Vec<String>> {
    let mut paths = BTreeSet::new();
    collect_use_tree_paths(tree, &mut Vec::new(), &mut paths);
    paths
}

fn collect_use_tree_paths(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    paths: &mut BTreeSet<Vec<String>>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree_paths(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let mut path = prefix.clone();
            path.push(name.ident.to_string());
            paths.insert(path);
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            path.push(rename.ident.to_string());
            path.push(format!("as {}", rename.rename));
            paths.insert(path);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree_paths(item, prefix, paths);
            }
        }
        UseTree::Glob(_) => {
            let mut path = prefix.clone();
            path.push("*".to_owned());
            paths.insert(path);
        }
    }
}

fn render_use_paths(paths: &BTreeSet<Vec<String>>) -> String {
    paths
        .iter()
        .map(|path| path.join("::"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn reviewed_static_semantics_are_closed(item: &syn::ItemStatic) -> bool {
    if !matches!(item.mutability, syn::StaticMutability::None) {
        return false;
    }
    match item.ident.to_string().as_str() {
        "REVIEWED_RULE_SET_METADATA" => {
            matches!(item.vis, syn::Visibility::Public(_))
                && type_is_reference_slice(&item.ty, "GeneratedRuleSetMetadata", None)
                && expression_is_empty_reference_array(&item.expr)
        }
        "REVIEWED_RULE_SET_ENTRIES" => {
            matches!(item.vis, syn::Visibility::Inherited)
                && type_is_lazy_lock_vec(&item.ty, "RuleSetRegistryEntry")
                && expression_is_empty_lazy_lock_vec(&item.expr)
        }
        _ => false,
    }
}

fn reviewed_accessor_semantics_are_closed(item: &syn::ItemFn) -> bool {
    let signature = &item.sig;
    if !matches!(item.vis, syn::Visibility::Public(_))
        || signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || !signature.inputs.is_empty()
        || signature.variadic.is_some()
    {
        return false;
    }
    let syn::ReturnType::Type(_, output) = &signature.output else {
        return false;
    };
    if !type_is_reference_slice(output, "RuleSetRegistryEntry", Some("static")) {
        return false;
    }
    let [syn::Stmt::Expr(expression, None)] = item.block.stmts.as_slice() else {
        return false;
    };
    let syn::Expr::MethodCall(call) = expression else {
        return false;
    };
    call.method == "as_slice"
        && call.turbofish.is_none()
        && call.args.is_empty()
        && expression_is_path(&call.receiver, &["REVIEWED_RULE_SET_ENTRIES"])
}

fn expression_is_empty_reference_array(expression: &syn::Expr) -> bool {
    let syn::Expr::Reference(reference) = expression else {
        return false;
    };
    if reference.mutability.is_some() {
        return false;
    }
    matches!(
        reference.expr.as_ref(),
        syn::Expr::Array(array) if array.elems.is_empty()
    )
}

fn expression_is_empty_lazy_lock_vec(expression: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = expression else {
        return false;
    };
    if !expression_is_path(&call.func, &["LazyLock", "new"]) || call.args.len() != 1 {
        return false;
    }
    let syn::Expr::Closure(closure) = &call.args[0] else {
        return false;
    };
    if !closure.inputs.is_empty() {
        return false;
    }
    let syn::Expr::Macro(vector) = closure.body.as_ref() else {
        return false;
    };
    path_is_exact(&vector.mac.path, &["vec"]) && vector.mac.tokens.is_empty()
}

fn expression_is_path(expression: &syn::Expr, expected: &[&str]) -> bool {
    matches!(
        expression,
        syn::Expr::Path(path) if path.qself.is_none() && path_is_exact(&path.path, expected)
    )
}

fn type_is_reference_slice(ty: &syn::Type, element_name: &str, lifetime: Option<&str>) -> bool {
    let syn::Type::Reference(reference) = ty else {
        return false;
    };
    if reference.mutability.is_some()
        || reference
            .lifetime
            .as_ref()
            .map(|value| value.ident.to_string())
            .as_deref()
            != lifetime
    {
        return false;
    }
    let syn::Type::Slice(slice) = reference.elem.as_ref() else {
        return false;
    };
    type_is_plain_path(&slice.elem, &[element_name])
}

fn type_is_lazy_lock_vec(ty: &syn::Type, element_name: &str) -> bool {
    let syn::Type::Path(lazy_lock) = ty else {
        return false;
    };
    if lazy_lock.qself.is_some()
        || lazy_lock.path.leading_colon.is_some()
        || lazy_lock.path.segments.len() != 1
    {
        return false;
    }
    let segment = &lazy_lock.path.segments[0];
    if segment.ident != "LazyLock" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    if arguments.args.len() != 1 {
        return false;
    }
    let Some(syn::GenericArgument::Type(vector)) = arguments.args.first() else {
        return false;
    };
    let syn::Type::Path(vector) = vector else {
        return false;
    };
    if vector.qself.is_some()
        || vector.path.leading_colon.is_some()
        || vector.path.segments.len() != 1
    {
        return false;
    }
    let vector_segment = &vector.path.segments[0];
    if vector_segment.ident != "Vec" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &vector_segment.arguments else {
        return false;
    };
    if arguments.args.len() != 1 {
        return false;
    }
    let Some(syn::GenericArgument::Type(element)) = arguments.args.first() else {
        return false;
    };
    type_is_plain_path(element, &[element_name])
}

fn type_is_plain_path(ty: &syn::Type, expected: &[&str]) -> bool {
    matches!(
        ty,
        syn::Type::Path(path)
            if path.qself.is_none() && path_is_exact(&path.path, expected)
    )
}

fn path_is_exact(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(segment.arguments, syn::PathArguments::None)
            })
}

fn rust_item_kind(item: &Item) -> &'static str {
    match item {
        Item::Const(_) => "const",
        Item::Enum(_) => "enum",
        Item::ExternCrate(_) => "extern-crate",
        Item::Fn(_) => "fn",
        Item::ForeignMod(_) => "foreign-mod",
        Item::Impl(_) => "impl",
        Item::Macro(_) => "macro",
        Item::Mod(_) => "mod",
        Item::Static(_) => "static",
        Item::Struct(_) => "struct",
        Item::Trait(_) => "trait",
        Item::TraitAlias(_) => "trait-alias",
        Item::Type(_) => "type",
        Item::Union(_) => "union",
        Item::Use(_) => "use",
        Item::Verbatim(_) => "verbatim",
        _ => "unknown",
    }
}

fn check_reviewed_registry_empty(source: &str, criteria: &mut Vec<Criterion>) {
    let metadata_empty = source_matches(
        source,
        r"(?ms)^[ \t]*pub[ \t]+static[ \t]+REVIEWED_RULE_SET_METADATA[ \t]*:[ \t]*&[ \t]*\[[ \t]*GeneratedRuleSetMetadata[ \t]*\][ \t]*=[ \t]*&[ \t]*\[[ \t]*\][ \t]*;",
    );
    let entries_empty = source_matches(
        source,
        r"(?ms)^[ \t]*static[ \t]+REVIEWED_RULE_SET_ENTRIES[ \t]*:[ \t]*LazyLock[ \t]*<[ \t]*Vec[ \t]*<[ \t]*RuleSetRegistryEntry[ \t]*>[ \t]*>[ \t]*=[ \t]*LazyLock::new[ \t]*\([ \t]*\|\|[ \t]*vec![ \t]*\[[ \t]*\][ \t]*\)[ \t]*;",
    );
    let accessor_is_pinned = source_matches(
        source,
        r"(?ms)^[ \t]*pub[ \t]+fn[ \t]+reviewed_rule_set_entries[ \t]*\([ \t]*\)[ \t]*->[ \t]*&'static[ \t]*\[[ \t]*RuleSetRegistryEntry[ \t]*\][ \t]*\{[ \t\r\n]*REVIEWED_RULE_SET_ENTRIES\.as_slice[ \t]*\([ \t]*\)[ \t\r\n]*\}",
    );
    let ast_failures = match syn::parse_file(source) {
        Ok(file) => registry_boundary_ast_failures(&file),
        Err(error) => vec![format!("registry.rs is not parseable Rust: {error}")],
    };
    let met = metadata_empty && entries_empty && accessor_is_pinned && ast_failures.is_empty();
    push(
        criteria,
        "reviewed-registry-empty",
        CriterionKind::Boundary,
        met,
        if met {
            "generated reviewed metadata and provider registry are empty, and the accessor returns that empty registry"
                .to_owned()
        } else {
            if ast_failures.is_empty() {
                format!(
                    "{GENERATED_REGISTRY_PATH} no longer declares and returns the exact empty reviewed registry"
                )
            } else {
                format!(
                    "{GENERATED_REGISTRY_PATH} boundary AST rejected: {}",
                    ast_failures.join("; ")
                )
            }
        },
    );
}

fn check_review_status(rule_set: &JsonValue, index: &JsonValue, criteria: &mut Vec<Criterion>) {
    let declared = string_at(rule_set, "review_status");
    let indexed = index
        .object()
        .and_then(|index| index.get("snapshots"))
        .and_then(array)
        .and_then(|snapshots| {
            snapshots
                .iter()
                .find(|snapshot| string_at(snapshot, "rule_set_id") == Some(RULE_SET_ID))
        })
        .and_then(|snapshot| string_at(snapshot, "review_status"));
    let met = declared == Some("candidate") && indexed == Some("candidate");
    push(
        criteria,
        "review-status-candidate",
        CriterionKind::Boundary,
        met,
        format!(
            "rule-set={} index={}",
            declared.unwrap_or("<missing>"),
            indexed.unwrap_or("<missing>")
        ),
    );
}

fn check_filing_safe_unresolved(rule_set: &JsonValue, criteria: &mut Vec<Criterion>) {
    let state = rule_set
        .object()
        .and_then(|rule_set| rule_set.get("profile_status"))
        .and_then(|status| status.object())
        .and_then(|status| status.get("filing_safe"))
        .and_then(|branch| string_at(branch, "state"));
    push(
        criteria,
        "filing-safe-unresolved",
        CriterionKind::Boundary,
        state == Some("unresolved"),
        format!("filing_safe.state={}", state.unwrap_or("<missing>")),
    );
}

fn check_artifacts_closed(rule_set: &JsonValue, criteria: &mut Vec<Criterion>) {
    let artifacts = rule_set
        .object()
        .and_then(|rule_set| rule_set.get("serialization"))
        .and_then(|serialization| serialization.object())
        .and_then(|serialization| serialization.get("artifacts"))
        .and_then(array);

    let Some(artifacts) = artifacts else {
        push(
            criteria,
            "artifacts-documented-only",
            CriterionKind::Boundary,
            false,
            "serialization.artifacts is missing",
        );
        return;
    };

    let mut failures = Vec::new();
    for artifact in artifacts {
        let id = string_at(artifact, "artifact_id").unwrap_or("<unnamed>");
        for branch_name in ["official", "filing_safe"] {
            let Some(branch) = artifact
                .object()
                .and_then(|artifact| artifact.get(branch_name))
                .and_then(JsonValue::object)
            else {
                failures.push(format!("{id}.{branch_name} is missing"));
                continue;
            };
            if branch.contains_key("nodes") {
                failures.push(format!("{id}.{branch_name} has an executable node list"));
            }
            let state = branch.get("state").and_then(JsonValue::as_str);
            let expected = if branch_name == "official" {
                "documented_only"
            } else {
                "unresolved"
            };
            if state != Some(expected) {
                failures.push(format!(
                    "{id}.{branch_name}.state={} (expected {expected})",
                    state.unwrap_or("<missing>")
                ));
            }
        }
    }

    push(
        criteria,
        "artifacts-documented-only",
        CriterionKind::Boundary,
        failures.is_empty() && artifacts.len() == 3,
        if failures.is_empty() {
            format!("{} artifact(s), all node-less", artifacts.len())
        } else {
            failures.join("; ")
        },
    );
}

fn check_inventory_value_free(inventory: &JsonValue, criteria: &mut Vec<Criterion>) {
    let values_emitted = inventory
        .object()
        .and_then(|inventory| inventory.get("values_emitted"));
    let status = string_at(inventory, "status");
    let met =
        matches!(values_emitted, Some(JsonValue::Bool(false))) && status == Some("documented-only");
    push(
        criteria,
        "inventory-value-free",
        CriterionKind::Boundary,
        met,
        format!(
            "values_emitted={} status={}",
            match values_emitted {
                Some(JsonValue::Bool(value)) => value.to_string(),
                _ => "<missing>".to_owned(),
            },
            status.unwrap_or("<missing>")
        ),
    );
}

fn check_occurrence_decomposition(
    inventory: &JsonValue,
    rule_set: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let empty = Vec::new();
    let bindings = occurrence_bindings(inventory);
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for binding in bindings.unwrap_or(&empty) {
        let kind = binding
            .object()
            .and_then(|binding| binding.get("source_projection"))
            .and_then(|projection| string_at(projection, "kind"))
            .unwrap_or("<missing>");
        *kinds.entry(kind).or_default() += 1;
    }

    let fields = rule_set
        .object()
        .and_then(|set| set.get("fields"))
        .and_then(array);
    let groups = rule_set
        .object()
        .and_then(|set| set.get("field_groups"))
        .and_then(array);
    let singletons = fields.map(|fields| {
        fields
            .iter()
            .filter(|field| {
                field
                    .object()
                    .and_then(|field| field.get("group_id"))
                    .is_none_or(|group| matches!(group, JsonValue::Null))
            })
            .count()
    });
    let grouped = fields.map(|fields| fields.len() - singletons.unwrap_or(0));

    let observed = inventory
        .object()
        .and_then(|inventory| inventory.get("observed_contract"));
    let encrypted = observed
        .and_then(|observed| observed.object())
        .and_then(|observed| observed.get("encrypted_pseudo_div_occurrence_count"))
        .and_then(integer);

    let mut failures = Vec::new();
    let mut expect = |label: &str, actual: Option<usize>, expected: usize| {
        if actual != Some(expected) {
            failures.push(format!(
                "{label}={} (expected {expected})",
                actual.map_or("<missing>".to_owned(), |value| value.to_string())
            ));
        }
    };
    expect(
        "plaintext_occurrences",
        bindings.map(Vec::len),
        PLAINTEXT_OCCURRENCES,
    );
    expect("encrypted_occurrences", encrypted, ENCRYPTED_OCCURRENCES);
    expect(
        "raw-static-control",
        kinds.get("raw-static-control").copied(),
        STATIC_OCCURRENCES,
    );
    expect(
        "raw-group-field",
        kinds.get("raw-group-field").copied(),
        GROUP_OCCURRENCES,
    );
    expect(
        "generated-local-date",
        kinds.get("generated-local-date").copied(),
        GENERATED_OCCURRENCES,
    );
    expect("executable_singletons", singletons, EXECUTABLE_SINGLETONS);
    expect(
        "repeated_family_descriptors",
        grouped,
        REPEATED_FAMILY_DESCRIPTORS,
    );
    expect("field_groups", groups.map(Vec::len), FIELD_GROUPS);

    push(
        criteria,
        "occurrence-decomposition",
        CriterionKind::Boundary,
        failures.is_empty(),
        if failures.is_empty() {
            format!(
                "{PLAINTEXT_OCCURRENCES}/{ENCRYPTED_OCCURRENCES} occurrences, \
                 {STATIC_OCCURRENCES}+{GROUP_OCCURRENCES}+{GENERATED_OCCURRENCES} projections, \
                 {EXECUTABLE_SINGLETONS}+{REPEATED_FAMILY_DESCRIPTORS} fields in {FIELD_GROUPS} groups"
            )
        } else {
            failures.join("; ")
        },
    );
}

/// The executable/documented-only split is a join between the inventory and the
/// rule set's field surface, and that join must be pinned or "the 53" can drift
/// with nothing noticing.
///
/// It cannot be pinned with `sha256(rule-set.json)`. That is circular: the
/// inventory is itself a **declared source** of the rule set, so pinning the
/// rule set's file hash into the inventory changes the inventory, which changes
/// the hash the rule set must declare for it, which changes the rule set, which
/// invalidates the pin. The loop never settles.
///
/// The inventory therefore pins `rule_set_field_ids` — a digest over the sorted
/// executable field-id list. That is the part of the rule set this inventory
/// actually depends on, it changes exactly when the field surface drifts, and
/// it is stable across `source_set_sha256` rolls.
fn check_inventory_pins_rule_set(
    inventory: &JsonValue,
    rule_set: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let declared = inventory
        .object()
        .and_then(|inventory| inventory.get("input_sha256"))
        .and_then(JsonValue::object)
        .and_then(|inputs| inputs.get("rule_set_field_ids"))
        .and_then(JsonValue::as_str);

    let mut field_ids = BTreeSet::new();
    let mut malformed = Vec::new();
    match rule_set
        .object()
        .and_then(|rule_set| rule_set.get("fields"))
        .and_then(array)
    {
        Some(fields) => {
            for (index, field) in fields.iter().enumerate() {
                match string_at(field, "field_id") {
                    Some(field_id) if field_ids.insert(field_id) => {}
                    Some(field_id) => malformed.push(format!("duplicate field_id `{field_id}`")),
                    None => malformed.push(format!("fields[{index}] has no field_id")),
                }
            }
        }
        None => malformed.push("rule-set fields array is missing".to_owned()),
    }

    let expected = (!field_ids.is_empty() && malformed.is_empty()).then(|| {
        let joined = field_ids.iter().copied().collect::<Vec<_>>().join("\n");
        sha256_hex(joined.as_bytes())
    });
    let met = expected.as_deref() == declared && malformed.is_empty();
    push(
        criteria,
        "inventory-pins-rule-set",
        CriterionKind::ActiveLibrary,
        met,
        if met {
            format!(
                "input_sha256.rule_set_field_ids matches the {}-field executable surface ({})",
                field_ids.len(),
                declared.expect("met requires a declared pin")
            )
        } else if !malformed.is_empty() {
            malformed.join("; ")
        } else {
            format!(
                "input_sha256.rule_set_field_ids={} does not match the current executable field surface digest {}",
                declared.unwrap_or("<missing>"),
                expected.as_deref().unwrap_or("<unavailable>")
            )
        },
    );
}

/// The declared next task: every occurrence carries an explicit classification,
/// so "the 53" stops being a set difference nobody can reproduce.
fn check_occurrence_classification(
    inventory: &JsonValue,
    rule_set: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let empty = Vec::new();
    let bindings = occurrence_bindings(inventory).unwrap_or(&empty);
    let classified = bindings
        .iter()
        .filter(|binding| {
            binding
                .object()
                .is_some_and(|binding| binding.contains_key("classification"))
        })
        .count();

    push(
        criteria,
        "occurrence-classification-complete",
        CriterionKind::ActiveLibrary,
        classified == bindings.len() && !bindings.is_empty(),
        format!(
            "{classified}/{} occurrence binding(s) carry a classification",
            bindings.len()
        ),
    );

    // Derived independently of any classification field so this stays a real
    // check while the library objective is open.
    let executable: Vec<&str> = rule_set
        .object()
        .and_then(|set| set.get("fields"))
        .and_then(array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(|field| string_at(field, "field_id"))
                .collect()
        })
        .unwrap_or_default();

    let documented_only: Vec<&str> = bindings
        .iter()
        .filter(|binding| {
            binding
                .object()
                .and_then(|binding| binding.get("source_projection"))
                .and_then(|projection| string_at(projection, "kind"))
                == Some("raw-static-control")
        })
        .filter_map(|binding| string_at(binding, "candidate_v2_field_id"))
        .filter(|field_id| !executable.contains(field_id))
        .collect();

    push(
        criteria,
        "documented-only-projection-count",
        CriterionKind::Boundary,
        documented_only.len() == DOCUMENTED_ONLY_PROJECTIONS,
        format!(
            "{} documented-only projection(s) (expected {DOCUMENTED_ONLY_PROJECTIONS} = \
             {DERIVED_OR_ALIAS_PROJECTIONS} derived/alias + {WORKFLOW_OR_CREDENTIAL_PROJECTIONS} workflow/credential)",
            documented_only.len()
        ),
    );

    let exposed: Vec<&str> = CREDENTIAL_KEYS
        .iter()
        .copied()
        .filter(|key| executable.contains(key))
        .collect();
    push(
        criteria,
        "credentials-not-field-authority",
        CriterionKind::Boundary,
        exposed.is_empty(),
        if exposed.is_empty() {
            format!(
                "{} credential control(s) absent from the executable field set",
                CREDENTIAL_KEYS.len()
            )
        } else {
            format!("credentials exposed as fields: {}", exposed.join(", "))
        },
    );
}

/// Every entry in `rule-set.json`'s `sources[]` is re-hashed from disk by the
/// audit and rejected on mismatch. `.gitattributes` is `* text=auto eol=lf`, so
/// a CRLF source file has a pinned hash that matches **only** the working tree
/// that produced it: normalize on checkout and the hash cannot reproduce.
///
/// This was not theoretical. Materializing `rules/` from the git index and
/// running the audit against it failed on the first CRLF source:
///
/// ```text
/// error: source `v1-manifest` hash mismatch for `forms/2550q-v2024/manifest.json`
/// ```
///
/// So `audit`, `check` and `rules:ci` fail on every clone, on every OS, until
/// the CRLF sources are normalized and their hashes re-pinned. That re-pin
/// changes `rule-set.json`, which changes `source_set_sha256`, which is a
/// 123-file atomic digest roll — so this must be fixed with the `roll-pin`
/// command, never by hand.
fn check_declared_sources_are_clone_reproducible(
    repo_root: &Path,
    rule_set: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let Some(sources) = rule_set
        .object()
        .and_then(|rule_set| rule_set.get("sources"))
        .and_then(array)
    else {
        push(
            criteria,
            "declared-sources-clone-reproducible",
            CriterionKind::ActiveLibrary,
            false,
            "rule-set.json declares no sources",
        );
        return;
    };

    let mut crlf = Vec::new();
    let mut unreadable = Vec::new();
    let mut unpinned = Vec::new();
    let mut hash_mismatches = Vec::new();
    for source in sources {
        let Some(relative) = string_at(source, "path") else {
            unpinned.push("<missing-path>".to_owned());
            continue;
        };
        let source_id = string_at(source, "source_id").unwrap_or(relative);
        let declared_sha256 = string_at(source, "sha256");
        let corpus_relative = format!("rules/{relative}");
        match resolve_existing_under(repo_root, &corpus_relative, "declared source")
            .and_then(|path| read_tracked_bytes(&path))
        {
            Ok(bytes) => {
                if bytes.windows(2).any(|pair| pair == b"\r\n") {
                    crlf.push(source_id.to_owned());
                }
                match declared_sha256 {
                    Some(expected) => {
                        let actual = sha256_hex(&bytes);
                        if actual != expected {
                            hash_mismatches.push(format!(
                                "{source_id} (declared {expected}, actual {actual})"
                            ));
                        }
                    }
                    None => unpinned.push(source_id.to_owned()),
                }
            }
            Err(_) => unreadable.push(source_id.to_owned()),
        }
    }

    let met = crlf.is_empty()
        && unreadable.is_empty()
        && unpinned.is_empty()
        && hash_mismatches.is_empty();
    push(
        criteria,
        "declared-sources-clone-reproducible",
        CriterionKind::ActiveLibrary,
        met,
        if met {
            format!(
                "{} declared source(s), all LF and matching their source SHA-256 pins",
                sources.len()
            )
        } else {
            let mut failures = Vec::new();
            if !crlf.is_empty() {
                failures.push(format!("CRLF and clone-unstable: {}", crlf.join(", ")));
            }
            if !unpinned.is_empty() {
                failures.push(format!(
                    "missing source SHA-256 pin: {}",
                    unpinned.join(", ")
                ));
            }
            if !hash_mismatches.is_empty() {
                failures.push(format!(
                    "source SHA-256 mismatch: {}",
                    hash_mismatches.join(", ")
                ));
            }
            if !unreadable.is_empty() {
                failures.push(format!("unreadable: {}", unreadable.join(", ")));
            }
            failures.join("; ")
        },
    );
}

/// The remaining deliverables of the approved plan that are machine-checkable.
///
/// These exist because the earlier narrow criteria passed while real plan work
/// was still outstanding, which would have let the objective report done
/// prematurely. A done-condition that is satisfiable before the work is
/// finished is worse than none, so the condition was tightened rather than the
/// work declared complete. Add to this as further deliverables land; never
/// remove one to make the command pass.
fn check_remaining_plan_deliverables(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    const PROJECTIONS_PATH: &str = "crates/bir-rules-codegen/src/projections.rs";
    check_text_deliverable(
        repo_root,
        criteria,
        "projectors-ported",
        PROJECTIONS_PATH,
        |source| {
            source_matches(
                source,
                r"(?m)^[ \t]*pub[ \t]+fn[ \t]+project_2550q_static_surface[ \t]*\(",
            ) && source_matches(
                source,
                r"(?m)^[ \t]*fn[ \t]+static_projection_reproduces_the_tracked_corpus[ \t]*\(",
            )
        },
        "idempotent static projector and its tracked-corpus reproduction test are present",
        "P0.3 requires the static projector and its tracked-corpus reproduction test",
    );

    check_occurrence_reconciliation_table(repo_root, criteria);

    check_text_deliverable(
        repo_root,
        criteria,
        "slice-review-document",
        "rules/forms/2550q-v2024/v2-candidate-occurrence-classification-review.md",
        slice_review_document_complete,
        "classification review records all 160 occurrences, the field-surface pin, and the non-authority decision",
        "P1.6 review is missing its classification, source-pin, or non-authority assertions",
    );

    check_text_deliverable(
        repo_root,
        criteria,
        "gpui-validation-summary",
        "crates/bir-desktop/src/components/form_validation/summary.rs",
        |source| {
            source_matches(
                source,
                r"(?m)^[ \t]*pub[ \t]+struct[ \t]+ValidationSummary[ \t]*\{",
            ) && source_matches(
                source,
                r"(?m)^[ \t]*pub[ \t]+fn[ \t]+from_violations[ \t]*\(",
            ) && source_matches(
                source,
                r"(?m)^[ \t]*pub[ \t]+fn[ \t]+first_blocking[ \t]*\(",
            )
        },
        "validation summary has the report projection and first-blocking focus seam",
        "P2.3 requires a semantic validation summary, not only a file marker",
    );

    check_text_deliverable(
        repo_root,
        criteria,
        "validation-rules-skill",
        "rules/agent-boundaries/SKILL.md",
        |source| {
            source.contains("name: ebirforms-validation-rules")
                && source.contains("Read [boundaries](references/boundaries.md)")
                && source.contains("Only form 2550Q has a v2 rule set")
                && source.contains("Never weaken a validator")
        },
        "validation-rules skill carries routing, boundary, and fail-closed instructions",
        "D4 requires durable validation-rules routing and boundary instructions",
    );

    check_text_deliverable(
        repo_root,
        criteria,
        "builder-staging-guard",
        PROJECTIONS_PATH,
        |source| {
            source_matches(
                source,
                r"(?m)^[ \t]*pub[ \t]+staging_root[ \t]*:[ \t]*Option<String>[ \t]*,",
            ) && source.contains("if destination.exists()")
                && source.contains("refusing to overwrite a previous projection")
        },
        "projector exposes a staging root and refuses to overwrite an existing staged target",
        "A5b requires a staging root and a fail-if-target-exists implementation",
    );
}

fn slice_review_document_complete(source: &str) -> bool {
    [
        r"(?s)\bEach\s+of\s+the\s+160\s+occurrences\s+now\s+carries\s+exactly\s+one\s+classification\b",
        r"(?s)\binput_sha256\.rule_set_field_ids\b",
        r"(?s)\breviewed\s+runtime\s+registry\s+remains\s+empty\b",
        r"(?s)\bThis\s+review\s+confers\s+no\s+authority\b",
    ]
    .into_iter()
    .all(|pattern| source_matches(source, pattern))
}

fn check_text_deliverable(
    repo_root: &Path,
    criteria: &mut Vec<Criterion>,
    id: &'static str,
    relative: &'static str,
    predicate: impl FnOnce(&str) -> bool,
    met_detail: &'static str,
    unmet_detail: &'static str,
) {
    match read_text(repo_root, relative) {
        Ok(source) => {
            let met = predicate(&source);
            push(
                criteria,
                id,
                CriterionKind::ActiveLibrary,
                met,
                if met {
                    met_detail.to_owned()
                } else {
                    format!("{unmet_detail}: {relative}")
                },
            );
        }
        Err(error) => push(
            criteria,
            id,
            CriterionKind::ActiveLibrary,
            false,
            format!("{unmet_detail}: cannot read {relative}: {error}"),
        ),
    }
}

fn check_occurrence_reconciliation_table(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    const PATH: &str =
        "schemas/validation-rules/generated/static-occurrence-reconciliation-v796.json";
    let document = match read_json(repo_root, PATH) {
        Ok(document) => document,
        Err(error) => {
            push(
                criteria,
                "occurrence-reconciliation-table",
                CriterionKind::ActiveLibrary,
                false,
                format!("cannot read {PATH}: {error}"),
            );
            return;
        }
    };

    let identity_matches = string_at(&document, "form_id") == Some("2550q-v2024")
        && string_at(&document, "package_version") == Some("7.9.6.0")
        && string_at(&document, "generated_from")
            == Some("serialization-binding-inventory-v796.json occurrence classifications")
        && integer_at(&document, "occurrence_total") == Some(PLAINTEXT_OCCURRENCES)
        && matches!(
            document
                .object()
                .and_then(|document| document.get("values_emitted")),
            Some(JsonValue::Bool(false))
        );
    let expected: [(&str, &[(&str, usize)], usize); 4] = [
        (
            "occurrence-projection-kind",
            &[
                ("generated-local-date", 1),
                ("raw-group-field", 40),
                ("raw-static-control", 119),
            ],
            160,
        ),
        (
            "static-executable-vs-documented-only",
            &[("documented-only", 53), ("executable-singleton", 66)],
            119,
        ),
        (
            "documented-only-detail",
            &[
                ("credential", 4),
                ("derived-or-alias", 44),
                ("workflow-or-ui", 5),
            ],
            53,
        ),
        (
            "classification",
            &[
                ("documented-only-credential", 4),
                ("documented-only-derived-or-alias", 44),
                ("documented-only-workflow-or-ui", 5),
                ("executable-group-field", 40),
                ("executable-singleton", 66),
                ("generated-context-metadata", 1),
            ],
            160,
        ),
    ];

    let partitions = document
        .object()
        .and_then(|document| document.get("partitions"))
        .and_then(array);
    let partitions_match = partitions.is_some_and(|partitions| {
        partitions.len() == expected.len()
            && expected.iter().all(|(name, terms, total)| {
                partitions.iter().any(|partition| {
                    string_at(partition, "partition") == Some(*name)
                        && integer_at(partition, "total") == Some(*total)
                        && terms_match(partition, terms)
                })
            })
    });
    let met = identity_matches && partitions_match;
    push(
        criteria,
        "occurrence-reconciliation-table",
        CriterionKind::ActiveLibrary,
        met,
        if met {
            "reconciliation table semantically matches all four published occurrence partitions"
                .to_owned()
        } else {
            format!(
                "{PATH} exists but its identity, value-free marker, or published partitions drifted"
            )
        },
    );
}

fn terms_match(partition: &JsonValue, expected: &[(&str, usize)]) -> bool {
    let Some(terms) = partition
        .object()
        .and_then(|partition| partition.get("terms"))
        .and_then(JsonValue::object)
    else {
        return false;
    };
    terms.len() == expected.len()
        && expected
            .iter()
            .all(|(name, count)| terms.get(*name).and_then(integer) == Some(*count))
}

/// Promotion needs zero `"state": "unresolved"` anywhere in the rule set. Of the
/// 135 present, most carry no judgement: 94 field branches and every rule whose
/// v1 assessment is `verified-correct` can only mean "filing-safe behaves as
/// official does", because official was reviewed and found correct.
///
/// This criterion covers exactly that mechanical part. It deliberately does
/// **not** cover the rules assessed `incorrect-official-behavior` or
/// `official-bug-compatible` — those are real decisions about what safer
/// behaviour should be, and mirroring official for them would silently inherit
/// a defect into the filing-safe profile.
fn check_filing_safe_resolved_where_official_is_correct(
    repo_root: &Path,
    rule_set: &JsonValue,
    criteria: &mut Vec<Criterion>,
) {
    let Ok(validations) = read_json(repo_root, V1_VALIDATIONS_PATH) else {
        push(
            criteria,
            "filing-safe-mirrors-verified-official",
            CriterionKind::DeferredPromotion,
            false,
            format!("cannot read {V1_VALIDATIONS_PATH}"),
        );
        return;
    };
    let mut verified: BTreeMap<&str, ()> = BTreeMap::new();
    if let Some(JsonValue::Array(rules)) = validations.object().and_then(|d| d.get("rules")) {
        for rule in rules {
            if string_at(rule, "assessment") == Some("verified-correct")
                && let Some(id) = string_at(rule, "rule_id")
            {
                verified.insert(id, ());
            }
        }
    }

    let unresolved_state = |branch: Option<&JsonValue>| {
        branch.and_then(|b| string_at(b, "state")) == Some("unresolved")
    };

    let mut open_fields = 0usize;
    if let Some(JsonValue::Array(fields)) = rule_set.object().and_then(|r| r.get("fields")) {
        for field in fields {
            let branch = field
                .object()
                .and_then(|f| f.get("behavior"))
                .and_then(|b| b.object())
                .and_then(|b| b.get("filing_safe"));
            if unresolved_state(branch) {
                open_fields += 1;
            }
        }
    }

    let mut open_rules = Vec::new();
    if let Some(JsonValue::Array(rules)) = rule_set.object().and_then(|r| r.get("rules")) {
        for rule in rules {
            let Some(id) = string_at(rule, "rule_id") else {
                continue;
            };
            if !verified.contains_key(id) {
                continue;
            }
            let branch = rule
                .object()
                .and_then(|r| r.get("profiles"))
                .and_then(|p| p.object())
                .and_then(|p| p.get("filing_safe"));
            if unresolved_state(branch) {
                open_rules.push(id.to_owned());
            }
        }
    }

    let met = open_fields == 0 && open_rules.is_empty();
    push(
        criteria,
        "filing-safe-mirrors-verified-official",
        CriterionKind::DeferredPromotion,
        met,
        if met {
            "every field and every verified-correct rule has a resolved filing-safe branch"
                .to_owned()
        } else {
            format!(
                "{open_fields} field(s) and {} verified-correct rule(s) still unresolved on filing_safe",
                open_rules.len()
            )
        },
    );
}

/// `shadow.rs` holds only `EvaluationStamp` and `ShadowEvaluationOutcome`. The
/// four difference dimensions the plan calls for are what make a shadow run
/// useful: without them a shadow evaluation produces a result nobody compares.
fn check_shadow_difference_dimensions(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    let present = read_text(repo_root, SHADOW_PATH)
        .map(|source| {
            source_matches(
                &source,
                r"(?m)^[ \t]*pub[ \t]+enum[ \t]+ShadowDifferenceKind[ \t]*\{",
            ) && source_matches(
                &source,
                r"(?m)^[ \t]*pub[ \t]+fn[ \t]+has_behavioural_difference[ \t]*\(",
            )
        })
        .unwrap_or(false);
    push(
        criteria,
        "shadow-difference-dimensions",
        CriterionKind::ActiveLibrary,
        present,
        if present {
            "shadow.rs declares difference dimensions and a behavioural-difference classifier"
                .to_owned()
        } else {
            "shadow.rs lacks semantic difference dimensions or the behavioural-difference classifier"
                .to_owned()
        },
    );
}

fn occurrence_bindings(inventory: &JsonValue) -> Option<&Vec<JsonValue>> {
    inventory
        .object()
        .and_then(|inventory| inventory.get("occurrence_bindings"))
        .and_then(array)
}

fn array(value: &JsonValue) -> Option<&Vec<JsonValue>> {
    match value {
        JsonValue::Array(values) => Some(values),
        _ => None,
    }
}

fn integer(value: &JsonValue) -> Option<usize> {
    match value {
        JsonValue::Number(number) => number.as_u64().map(|value| value as usize),
        _ => None,
    }
}

fn integer_at(value: &JsonValue, key: &str) -> Option<usize> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(integer)
}

fn string_at<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(JsonValue::as_str)
}

fn read_json(repo_root: &Path, relative: &str) -> Result<JsonValue> {
    let path = resolve_existing_under(repo_root, relative, relative)?;
    let bytes = read_tracked_bytes(&path)?;
    parse_strict(&bytes, &path)
}

fn read_text(repo_root: &Path, relative: &str) -> Result<String> {
    let path = resolve_existing_under(repo_root, relative, relative)?;
    let bytes = read_tracked_bytes(&path)?;
    String::from_utf8(bytes)
        .map_err(|source| CodegenError::with_source(format!("{relative} is not UTF-8"), source))
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::{
        CORE_CAPABILITIES_PATH, CORE_FORM_2550Q_PATH, Criterion, CriterionKind,
        DESKTOP_FORM_2550Q_PATH, FrozenApplicationSources, GENERATED_REGISTRY_PATH, StatusOptions,
        StatusReport, application_freeze_violations, check_application_integration_frozen,
        check_reviewed_registry_empty, generated_candidate_ast_failures, read_text,
        record_reviewed_evidence_packet_set, represented_record_total_met,
        slice_review_document_complete, status,
    };
    use crate::error::CodegenError;
    use crate::evidence_set::CheckEvidencePacketSetReport;

    fn repo_root() -> std::path::PathBuf {
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("canonical repository root")
    }

    fn landed_status() -> super::StatusReport {
        static LANDED_STATUS: OnceLock<super::StatusReport> = OnceLock::new();
        LANDED_STATUS
            .get_or_init(|| status(&StatusOptions::new(repo_root())).expect("read landed status"))
            .clone()
    }

    fn landed_frozen_sources() -> FrozenApplicationSources {
        static LANDED_SOURCES: OnceLock<FrozenApplicationSources> = OnceLock::new();
        LANDED_SOURCES
            .get_or_init(|| {
                FrozenApplicationSources::read(&repo_root())
                    .expect("read frozen application sources")
            })
            .clone()
    }

    fn append_source(sources: &mut FrozenApplicationSources, path: &str, addition: &str) {
        sources
            .files
            .get_mut(path)
            .unwrap_or_else(|| panic!("missing frozen source `{path}`"))
            .push_str(addition);
    }

    /// The whole point of the command: while a production authority is closed,
    /// it stays closed. A failure here is a released filing path, not an
    /// unfinished library objective.
    #[test]
    fn landed_boundaries_are_all_held() {
        let report = status(&StatusOptions::new(repo_root()).boundaries_only())
            .expect("read landed boundary status");
        let breached: Vec<&str> = report
            .criteria
            .iter()
            .filter(|criterion| criterion.kind == CriterionKind::Boundary && !criterion.met)
            .map(|criterion| criterion.id)
            .collect();
        assert!(
            breached.is_empty(),
            "production boundaries breached: {breached:?}"
        );
        assert!(
            report
                .criteria
                .iter()
                .all(|criterion| criterion.kind == CriterionKind::Boundary),
            "boundary-only status must not evaluate active or deferred criteria"
        );
    }

    #[test]
    fn all_five_production_switches_are_direct_boundary_criteria() {
        let report = landed_status();
        for id in [
            "core-default-designation-none",
            "generated-candidate-test-only",
            "core-candidate-evaluator-test-only",
            "reviewed-registry-empty",
            "payload-constructor-closed",
        ] {
            let criterion = report
                .criteria
                .iter()
                .find(|criterion| criterion.id == id)
                .unwrap_or_else(|| panic!("missing production switch criterion `{id}`"));
            assert_eq!(criterion.kind, CriterionKind::Boundary, "{id}");
            assert!(criterion.met, "{id}: {}", criterion.detail);
        }
    }

    #[test]
    fn application_freeze_is_a_held_boundary_criterion() {
        let mut criteria = Vec::new();
        check_application_integration_frozen(&repo_root(), &mut criteria);
        let [criterion] = criteria.as_slice() else {
            panic!("application freeze must produce exactly one criterion: {criteria:?}");
        };
        assert_eq!(criterion.id, "application-integration-frozen");
        assert_eq!(criterion.kind, CriterionKind::Boundary);
        assert!(criterion.met, "{}", criterion.detail);
    }

    #[test]
    fn application_freeze_rejects_new_production_authority_surfaces() {
        let mut mutated = landed_frozen_sources();
        assert!(
            application_freeze_violations(&mutated).is_empty(),
            "landed source inventory must be the permitted baseline"
        );
        let mutations = [
            (
                "crates/bir-core/src/lib.rs",
                "\nfn new_designation() { let _ = FormRevisionKey::new(todo!()); }\n",
                "production exact-identity constructors",
            ),
            (
                DESKTOP_FORM_2550Q_PATH,
                "\nfn run_trusted(evaluator: &FormRuleEvaluator, request: &EvaluationRequest) { let _ = evaluator.evaluate_trusted(request); }\n",
                "desktop trusted-evaluator/serialization API references",
            ),
            (
                DESKTOP_FORM_2550Q_PATH,
                "\nfn materialize_payload() { let _ = CheckedFinalCopyPayload::try_new(todo!(), vec![], String::new()); }\n",
                "desktop trusted-evaluator/serialization API references",
            ),
            (
                "crates/bir-core/src/db/form_rule_state.rs",
                "\nfn authorize_final_copy(database: &Database) { let _ = database.create_form_final_copy(todo!()); }\n",
                "Final Copy production callers",
            ),
            (
                "crates/bir-core/src/transport.rs",
                "\nfn submit_2550q(draft: Form2550QDraft) { let _ = draft; }\n",
                "production 2550Q queue/transport/submission reference",
            ),
        ];
        for (path, addition, _) in mutations {
            append_source(&mut mutated, path, addition);
        }
        let violations = application_freeze_violations(&mutated);
        for (path, _, expected_detail) in mutations {
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(expected_detail)),
                "{path} mutation was not rejected as {expected_detail}: {violations:?}"
            );
        }
    }

    #[test]
    fn application_freeze_rejects_capability_and_queue_authority() {
        let mut mutated = landed_frozen_sources();
        let source = mutated
            .files
            .get_mut(CORE_CAPABILITIES_PATH)
            .expect("capability registry source");
        *source = source.replacen("queue_submission: false", "queue_submission: true", 1);

        let source = mutated
            .files
            .get_mut(CORE_FORM_2550Q_PATH)
            .expect("2550Q form source");
        *source = source.replacen(
            "QUEUE_SUBMISSION_SUPPORTED: bool = false",
            "QUEUE_SUBMISSION_SUPPORTED: bool = true",
            1,
        );
        let violations = application_freeze_violations(&mutated);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("capability/release authority")),
            "queue capability mutation was not rejected: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("QUEUE_SUBMISSION_SUPPORTED")),
            "queue authorization mutation was not rejected: {violations:?}"
        );
    }

    #[test]
    fn application_freeze_ignores_direct_cfg_test_authority_exercises() {
        let mut sources = landed_frozen_sources();
        append_source(
            &mut sources,
            DESKTOP_FORM_2550Q_PATH,
            "\n#[cfg(test)]\nfn test_only_authority_exercise() {\n    let evaluator = FormRuleEvaluator::new(todo!());\n    let _ = evaluator.evaluate_trusted(todo!());\n    let _ = CheckedFinalCopyPayload::try_new(todo!());\n}\n",
        );
        assert_eq!(
            application_freeze_violations(&sources),
            Vec::<String>::new(),
            "directly cfg(test)-gated exercises are not production authority"
        );
    }

    #[test]
    fn reviewed_registry_population_still_fails_closed() {
        let source =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        let populated = source.replacen("= &[];", "= &[UNREVIEWED_ENTRY];", 1);
        assert_ne!(populated, source, "test mutation must populate metadata");
        let mut criteria = Vec::new();
        check_reviewed_registry_empty(&populated, &mut criteria);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].id, "reviewed-registry-empty");
        assert!(!criteria[0].met);
    }

    #[test]
    fn reviewed_registry_rejects_test_decoys_with_active_counterparts() {
        let source =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        let mut decoy = source.replacen(
            "pub static REVIEWED_RULE_SET_METADATA",
            "#[cfg(test)]\npub static REVIEWED_RULE_SET_METADATA",
            1,
        );
        assert_ne!(decoy, source, "test mutation must gate the empty decoy");
        decoy.push_str(
            "\n#[cfg(not(test))]\n\
             pub static REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = \
             &[UNREVIEWED_ENTRY];\n",
        );

        let mut criteria = Vec::new();
        check_reviewed_registry_empty(&decoy, &mut criteria);
        assert_eq!(criteria.len(), 1);
        assert!(!criteria[0].met, "active counterpart must breach boundary");
        assert!(
            criteria[0].detail.contains("boundary AST rejected"),
            "{}",
            criteria[0].detail
        );
    }

    #[test]
    fn reviewed_registry_rejects_comment_decoys_for_live_nonempty_semantics() {
        let source =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        assert!(
            !source.contains("*/"),
            "landed registry must be safe to embed in the adversarial block comment"
        );
        let live = source
            .replacen("= &[];", "= &[UNREVIEWED_ENTRY];", 1)
            .replacen(
                "LazyLock::new(|| vec![])",
                "LazyLock::new(|| vec![UNREVIEWED_PROVIDER])",
                1,
            )
            .replacen(
                "REVIEWED_RULE_SET_ENTRIES.as_slice()",
                "CANDIDATE_RULE_SET_ENTRIES.as_slice()",
                1,
            );
        assert_ne!(live, source, "test mutation must change live semantics");
        let attacked = format!("/*\n{source}\n*/\n{live}");

        let mut criteria = Vec::new();
        check_reviewed_registry_empty(&attacked, &mut criteria);
        assert_eq!(criteria.len(), 1);
        assert!(
            !criteria[0].met,
            "commented regex decoys must not mask live nonempty semantics"
        );
        assert!(
            criteria[0].detail.contains("boundary AST rejected"),
            "{}",
            criteria[0].detail
        );
    }

    #[test]
    fn generated_candidate_modules_reject_active_decoy_declarations() {
        let generated = read_text(&repo_root(), super::GENERATED_MOD_PATH)
            .expect("read generated module catalog");
        let registry =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        let module = "form_2550q_v2024_04_01_p7_9_6_0";
        let mutated = format!("{generated}\n#[cfg(not(test))]\nmod {module};\n");
        let failures = generated_candidate_ast_failures(
            &syn::parse_file(&mutated).expect("parse mutated generated mod"),
            &syn::parse_file(&registry).expect("parse generated registry"),
            &[module],
        );
        assert!(
            failures.iter().any(|failure| {
                failure.contains("exactly #[cfg(test)]")
                    || failure.contains("exactly one declaration")
            }),
            "active candidate decoy was not rejected: {failures:?}"
        );
    }

    #[test]
    fn generated_registry_reexports_are_bound_to_the_registry_module_path() {
        let generated = read_text(&repo_root(), super::GENERATED_MOD_PATH)
            .expect("read generated module catalog");
        let registry =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        let mutated = generated.replacen("pub use registry::{", "pub use crate::{", 1);
        assert_ne!(mutated, generated, "test mutation must switch use source");
        let failures = generated_candidate_ast_failures(
            &syn::parse_file(&mutated).expect("parse mutated generated mod"),
            &syn::parse_file(&registry).expect("parse generated registry"),
            &["form_2550q_v2024_04_01_p7_9_6_0"],
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("re-export set")),
            "crate-root re-export bypass was not rejected: {failures:?}"
        );
    }

    #[test]
    fn generated_registry_module_is_bound_to_the_checked_external_file() {
        let generated = read_text(&repo_root(), super::GENERATED_MOD_PATH)
            .expect("read generated module catalog");
        let registry =
            read_text(&repo_root(), GENERATED_REGISTRY_PATH).expect("read generated registry");
        let mutated = generated.replacen(
            "mod registry;",
            "#[path = \"active_registry.rs\"]\nmod registry;",
            1,
        );
        assert_ne!(mutated, generated, "test mutation must redirect module");
        let failures = generated_candidate_ast_failures(
            &syn::parse_file(&mutated).expect("parse redirected generated mod"),
            &syn::parse_file(&registry).expect("parse checked registry"),
            &["form_2550q_v2024_04_01_p7_9_6_0"],
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("attribute-free external declaration")),
            "redirected registry module was not rejected: {failures:?}"
        );
    }

    #[test]
    fn represented_and_intentionally_non_runtime_records_jointly_account_for_legacy() {
        assert!(represented_record_total_met(
            Some(100),
            100,
            100,
            73,
            27,
            0,
            0,
        ));
        assert!(
            !represented_record_total_met(Some(100), 100, 100, 100, 1, 0, 0),
            "represented and non-runtime records may not double-count the legacy total"
        );
        assert!(
            !represented_record_total_met(Some(100), 100, 100, 73, 27, 1, 0),
            "unresolved records remain blocking even when totals add up"
        );
    }

    #[test]
    fn slice_review_assertions_are_whitespace_tolerant() {
        assert!(slice_review_document_complete(
            "Each of the 160 occurrences now carries exactly one classification.\n\
             The pin is input_sha256.rule_set_field_ids.\n\
             The reviewed\nruntime registry remains empty.\n\
             This review confers no authority."
        ));
    }

    #[test]
    fn landed_aggregate_library_gates_remain_open() {
        let report = landed_status();
        let open: std::collections::BTreeSet<&str> = report
            .criteria
            .iter()
            .filter(|criterion| criterion.kind == CriterionKind::ActiveLibrary && !criterion.met)
            .map(|criterion| criterion.id)
            .collect();
        for id in [
            "v2-snapshot-coverage",
            "record-reconciliation-complete",
            "validations-represented",
            "calculations-represented",
            "candidate-generated-catalog-coverage",
        ] {
            assert!(
                open.contains(id),
                "aggregate library gate `{id}` is not open"
            );
        }
        assert!(!report.complete(), "the 43-form library is not complete");
    }

    #[test]
    fn landed_reviewed_evidence_packet_set_gate_is_met_for_the_exact_set() {
        let report = landed_status();
        let criterion = report
            .criteria
            .iter()
            .find(|criterion| criterion.id == "reviewed-evidence-packet-set")
            .expect("reviewed evidence packet set criterion");
        assert_eq!(criterion.kind, CriterionKind::ActiveLibrary);
        assert!(criterion.met, "{}", criterion.detail);
        assert!(
            criterion
                .detail
                .contains("contains the exact ordered set of 43 reviewed v1 evidence packets"),
            "{}",
            criterion.detail
        );
    }

    #[test]
    fn reviewed_evidence_packet_set_failures_and_wrong_count_leave_gate_open() {
        for failure in [
            "missing packet set",
            "invalid packet set",
            "candidate packet",
            "wrong packet order",
        ] {
            let mut criteria = Vec::new();
            record_reviewed_evidence_packet_set(Err(CodegenError::new(failure)), &mut criteria);
            let [criterion] = criteria.as_slice() else {
                panic!("failure must produce exactly one criterion: {criteria:?}");
            };
            assert_eq!(criterion.kind, CriterionKind::ActiveLibrary);
            assert!(!criterion.met, "{failure}");
            assert!(criterion.detail.contains(failure), "{}", criterion.detail);
        }

        let mut criteria = Vec::new();
        record_reviewed_evidence_packet_set(
            Ok(CheckEvidencePacketSetReport {
                packet_count: 42,
                packet_set_digest_sha256: String::new(),
                rules_index_sha256: String::new(),
                full_upstream_verified: false,
                packets: Vec::new(),
            }),
            &mut criteria,
        );
        let [criterion] = criteria.as_slice() else {
            panic!("wrong count must produce exactly one criterion: {criteria:?}");
        };
        assert_eq!(criterion.kind, CriterionKind::ActiveLibrary);
        assert!(!criterion.met);
        assert!(criterion.detail.contains("expected exactly 43"));
    }

    #[test]
    fn boundary_only_status_does_not_run_reviewed_evidence_packet_set_gate() {
        let report = status(&StatusOptions::new(repo_root()).boundaries_only())
            .expect("read landed boundary status");
        assert!(
            report
                .criteria
                .iter()
                .all(|criterion| criterion.id != "reviewed-evidence-packet-set")
        );
    }

    #[test]
    fn landed_2550q_library_slice_remains_complete() {
        let aggregate = [
            "v2-snapshot-coverage",
            "record-reconciliation-complete",
            "validations-represented",
            "calculations-represented",
            "candidate-generated-catalog-coverage",
            "reviewed-evidence-packet-set",
        ];
        let report = landed_status();
        let unexpectedly_open: Vec<&str> = report
            .criteria
            .iter()
            .filter(|criterion| {
                criterion.kind == CriterionKind::ActiveLibrary
                    && !criterion.met
                    && !aggregate.contains(&criterion.id)
            })
            .map(|criterion| criterion.id)
            .collect();
        assert!(
            unexpectedly_open.is_empty(),
            "completed 2550Q library criteria regressed: {unexpectedly_open:?}"
        );
    }

    #[test]
    fn filing_safe_policy_is_deferred_without_becoming_invisible() {
        let report = landed_status();
        let deferred: Vec<&str> = report
            .criteria
            .iter()
            .filter(|criterion| criterion.kind == CriterionKind::DeferredPromotion)
            .map(|criterion| criterion.id)
            .collect();
        assert_eq!(
            deferred,
            ["filing-safe-mirrors-verified-official"],
            "promotion policy must stay reported under its existing criterion id"
        );
        assert!(
            report
                .open()
                .any(|criterion| { criterion.id == "filing-safe-mirrors-verified-official" }),
            "the unresolved promotion policy must remain visible"
        );
    }

    #[test]
    fn default_completion_ignores_only_deferred_promotion() {
        let report = StatusReport {
            rule_set_id: "test",
            criteria: vec![
                criterion("boundary", CriterionKind::Boundary, true),
                criterion("library", CriterionKind::ActiveLibrary, true),
                criterion("promotion", CriterionKind::DeferredPromotion, false),
            ],
        };

        assert!(report.complete());
        assert!(report.complete_for(false));
        assert!(!report.complete_for(true));
        assert_eq!(report.blocking_open(false).count(), 0);
        assert_eq!(
            report
                .blocking_open(true)
                .map(|criterion| criterion.id)
                .collect::<Vec<_>>(),
            ["promotion"]
        );
    }

    #[test]
    fn active_library_or_boundary_failure_always_blocks() {
        for kind in [CriterionKind::Boundary, CriterionKind::ActiveLibrary] {
            let report = StatusReport {
                rule_set_id: "test",
                criteria: vec![
                    criterion("required", kind, false),
                    criterion("promotion", CriterionKind::DeferredPromotion, true),
                ],
            };
            assert!(!report.complete());
            assert!(!report.complete_for(true));
            assert_eq!(
                report
                    .blocking_open(false)
                    .map(|criterion| criterion.id)
                    .collect::<Vec<_>>(),
                ["required"]
            );
        }
    }

    fn criterion(id: &'static str, kind: CriterionKind, met: bool) -> Criterion {
        Criterion {
            id,
            kind,
            met,
            detail: String::new(),
        }
    }

    /// The condition cannot be satisfied by removing criteria: every id below
    /// must be present. Deleting one to make `status` pass fails here.
    ///
    #[test]
    fn every_declared_criterion_is_still_evaluated() {
        const REQUIRED: [&str; 33] = [
            "all-filing-safe-profiles-unresolved",
            "all-generated-candidates-test-only",
            "all-snapshots-unpromoted",
            "application-integration-frozen",
            "artifacts-documented-only",
            "builder-staging-guard",
            "calculations-represented",
            "candidate-generated-catalog-coverage",
            "core-candidate-evaluator-test-only",
            "core-default-designation-none",
            "filing-safe-mirrors-verified-official",
            "shadow-difference-dimensions",
            "credentials-not-field-authority",
            "declared-sources-clone-reproducible",
            "documented-only-projection-count",
            "filing-safe-unresolved",
            "generated-candidate-test-only",
            "gpui-validation-summary",
            "inventory-pins-rule-set",
            "inventory-value-free",
            "occurrence-classification-complete",
            "occurrence-decomposition",
            "occurrence-reconciliation-table",
            "payload-constructor-closed",
            "projectors-ported",
            "review-status-candidate",
            "reviewed-evidence-packet-set",
            "reviewed-registry-empty",
            "record-reconciliation-complete",
            "slice-review-document",
            "v2-snapshot-coverage",
            "validation-rules-skill",
            "validations-represented",
        ];
        let report = landed_status();
        let present: std::collections::BTreeSet<&str> = report
            .criteria
            .iter()
            .map(|criterion| criterion.id)
            .collect();
        for id in REQUIRED {
            assert!(present.contains(id), "criterion `{id}` was removed");
        }
        assert_eq!(present.len(), REQUIRED.len(), "criterion set changed size");
    }
}
