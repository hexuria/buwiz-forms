//! Machine-checkable status of the 2550Q v2 candidate slice.
//!
//! This is the validation-rules analogue of `scripts/wave_status.py`: one
//! command whose exit code answers "is the current slice finished?" without
//! anyone having to read prose and believe it.
//!
//! It reports two kinds of criterion, and both must pass to exit zero:
//!
//! * **Boundary** criteria assert that a production authority is still closed.
//!   These hold today and must never stop holding. A boundary failure means
//!   something opened a filing path and is far more serious than an open slice.
//!   They are checked first and reported first.
//! * **Slice** criteria assert that the declared next task is complete. These
//!   are expected to fail until the work lands.
//!
//! Never relax a criterion to make this command pass.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{CodegenError, Result};
use crate::files::read_bytes;
use crate::json::{JsonValue, parse_strict};
use crate::path::{canonical_repo_root, resolve_existing_under};

const RULE_SET_ID: &str = "2550q-v2024-p7.9.6.0";
const RULE_SET_PATH: &str = "rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json";
const INDEX_PATH: &str = "rules/ir/v2/index.json";
const INVENTORY_PATH: &str =
    "rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json";
const GENERATED_REGISTRY_PATH: &str = "crates/bir-rules/src/generated/registry.rs";
const V1_VALIDATIONS_PATH: &str = "rules/forms/2550q-v2024/validations.json";
const SHADOW_PATH: &str = "crates/bir-core/src/form_rules/shadow.rs";

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
}

impl StatusOptions {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionKind {
    /// A production authority that must stay closed.
    Boundary,
    /// A deliverable of the current slice.
    Slice,
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
        self.criteria
            .iter()
            .filter(|criterion| criterion.kind == CriterionKind::Boundary)
            .all(|criterion| criterion.met)
    }

    pub fn complete(&self) -> bool {
        self.criteria.iter().all(|criterion| criterion.met)
    }

    pub fn open(&self) -> impl Iterator<Item = &Criterion> {
        self.criteria.iter().filter(|criterion| !criterion.met)
    }
}

pub fn status(options: &StatusOptions) -> Result<StatusReport> {
    let repo_root = canonical_repo_root(&options.repo_root)?;
    let rule_set = read_json(&repo_root, RULE_SET_PATH)?;
    let index = read_json(&repo_root, INDEX_PATH)?;
    let inventory = read_json(&repo_root, INVENTORY_PATH)?;
    let registry_source = read_text(&repo_root, GENERATED_REGISTRY_PATH)?;

    let mut criteria = Vec::new();
    check_reviewed_registry_empty(&registry_source, &mut criteria);
    check_review_status(&rule_set, &index, &mut criteria);
    check_filing_safe_unresolved(&rule_set, &mut criteria);
    check_artifacts_closed(&rule_set, &mut criteria);
    check_inventory_value_free(&inventory, &mut criteria);
    check_occurrence_decomposition(&inventory, &rule_set, &mut criteria);
    check_inventory_pins_rule_set(&inventory, &mut criteria);
    check_occurrence_classification(&inventory, &rule_set, &mut criteria);
    check_declared_sources_are_clone_reproducible(&repo_root, &rule_set, &mut criteria);
    check_remaining_plan_deliverables(&repo_root, &mut criteria);
    check_filing_safe_resolved_where_official_is_correct(&repo_root, &rule_set, &mut criteria);
    check_shadow_difference_dimensions(&repo_root, &mut criteria);

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

fn check_reviewed_registry_empty(source: &str, criteria: &mut Vec<Criterion>) {
    let metadata_empty = source
        .contains("pub static REVIEWED_RULE_SET_METADATA: &[GeneratedRuleSetMetadata] = &[];");
    let entries_empty = source.contains("LazyLock::new(|| vec![])");
    push(
        criteria,
        "reviewed-registry-empty",
        CriterionKind::Boundary,
        metadata_empty && entries_empty,
        if metadata_empty && entries_empty {
            "generated reviewed registry is empty".to_owned()
        } else {
            format!("{GENERATED_REGISTRY_PATH} no longer declares an empty reviewed registry")
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
fn check_inventory_pins_rule_set(inventory: &JsonValue, criteria: &mut Vec<Criterion>) {
    let pinned = inventory
        .object()
        .and_then(|inventory| inventory.get("input_sha256"))
        .and_then(JsonValue::object)
        .is_some_and(|inputs| inputs.contains_key("rule_set_field_ids"));
    push(
        criteria,
        "inventory-pins-rule-set",
        CriterionKind::Slice,
        pinned,
        if pinned {
            "input_sha256 pins the rule set's executable field-id surface".to_owned()
        } else {
            "input_sha256 does not pin rule_set_field_ids; the executable/documented-only join is unguarded"
                .to_owned()
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
        CriterionKind::Slice,
        classified == bindings.len() && !bindings.is_empty(),
        format!(
            "{classified}/{} occurrence binding(s) carry a classification",
            bindings.len()
        ),
    );

    // Derived independently of any classification field so this stays a real
    // check while the slice is open.
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
            CriterionKind::Slice,
            false,
            "rule-set.json declares no sources",
        );
        return;
    };

    let mut crlf = Vec::new();
    let mut unreadable = Vec::new();
    for source in sources {
        let Some(relative) = string_at(source, "path") else {
            continue;
        };
        let source_id = string_at(source, "source_id").unwrap_or(relative);
        let corpus_relative = format!("rules/{relative}");
        match resolve_existing_under(repo_root, &corpus_relative, "declared source")
            .and_then(|path| read_bytes(&path))
        {
            Ok(bytes) => {
                if bytes.windows(2).any(|pair| pair == b"\r\n") {
                    crlf.push(source_id.to_owned());
                }
            }
            Err(_) => unreadable.push(source_id.to_owned()),
        }
    }

    let met = crlf.is_empty() && unreadable.is_empty();
    push(
        criteria,
        "declared-sources-clone-reproducible",
        CriterionKind::Slice,
        met,
        if met {
            format!("{} declared source(s), none CRLF", sources.len())
        } else {
            format!(
                "{} of {} declared source(s) are CRLF and cannot reproduce their pinned hash on a clone: {}{}",
                crlf.len(),
                sources.len(),
                crlf.join(", "),
                if unreadable.is_empty() {
                    String::new()
                } else {
                    format!("; unreadable: {}", unreadable.join(", "))
                }
            )
        },
    );
}

/// The remaining deliverables of the approved plan that are machine-checkable.
///
/// These exist because the 2550Q slice criteria alone passed while real plan
/// work was still outstanding, which would have let the objective report done
/// prematurely. A done-condition that is satisfiable before the work is
/// finished is worse than none, so the condition was tightened rather than the
/// work declared complete. Add to this as further deliverables land; never
/// remove one to make the command pass.
fn check_remaining_plan_deliverables(repo_root: &Path, criteria: &mut Vec<Criterion>) {
    const DELIVERABLES: [(&str, &str, &str); 6] = [
        (
            "projectors-ported",
            "crates/bir-rules-codegen/src/projections.rs",
            // Only the static projector is portable. The group projector was
            // proven spent: it asserts 60 total fields, the corpus has 94, so it
            // throws immediately and can never run again. It is archaeology.
            "P0.3 — the idempotent static projector ported (the group projector is a spent one-shot migration)",
        ),
        (
            "occurrence-reconciliation-table",
            "docs/validation-rules/generated/static-occurrence-reconciliation-v796.json",
            "P1.5 — one generated table replacing the four hand-maintained partitions of the 119 static occurrences",
        ),
        (
            "slice-review-document",
            "rules/forms/2550q-v2024/v2-candidate-occurrence-classification-review.md",
            "P1.6 — source-pinned review document for the classification slice",
        ),
        (
            "gpui-validation-summary",
            "crates/bir-desktop/src/components/form_validation/summary.rs",
            "P2.3 — the only one of the four planned GPUI validation files that does not exist",
        ),
        (
            "validation-rules-skill",
            ".codex/skills/ebirforms-validation-rules/SKILL.md",
            "D4 — durable skill coverage for the validation-rules domain",
        ),
        (
            "builder-staging-guard",
            "rules/tools/STAGING.md",
            "A5b — builders write straight into the canonical corpus with no staging root (UPDATING.md:33-36)",
        ),
    ];

    for (id, relative, detail) in DELIVERABLES {
        let present = resolve_existing_under(repo_root, relative, "plan deliverable").is_ok();
        push(
            criteria,
            id,
            CriterionKind::Slice,
            present,
            if present {
                format!("{relative} present")
            } else {
                format!("{detail} — missing {relative}")
            },
        );
    }
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
            CriterionKind::Slice,
            false,
            format!("cannot read {V1_VALIDATIONS_PATH}"),
        );
        return;
    };
    let mut verified: BTreeMap<&str, ()> = BTreeMap::new();
    if let Some(JsonValue::Array(rules)) = validations.object().and_then(|d| d.get("rules")) {
        for rule in rules {
            if string_at(rule, "assessment") == Some("verified-correct") {
                if let Some(id) = string_at(rule, "rule_id") {
                    verified.insert(id, ());
                }
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
        CriterionKind::Slice,
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
        .map(|source| source.contains("ShadowDifference"))
        .unwrap_or(false);
    push(
        criteria,
        "shadow-difference-dimensions",
        CriterionKind::Slice,
        present,
        if present {
            "shadow.rs reports evaluation differences".to_owned()
        } else {
            "shadow.rs records outcomes but reports no differences to compare".to_owned()
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

fn string_at<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value
        .object()
        .and_then(|object| object.get(key))
        .and_then(JsonValue::as_str)
}

fn read_json(repo_root: &Path, relative: &str) -> Result<JsonValue> {
    let path = resolve_existing_under(repo_root, relative, relative)?;
    let bytes = read_bytes(&path)?;
    parse_strict(&bytes, &path)
}

fn read_text(repo_root: &Path, relative: &str) -> Result<String> {
    let path = resolve_existing_under(repo_root, relative, relative)?;
    let bytes = read_bytes(&path)?;
    String::from_utf8(bytes)
        .map_err(|source| CodegenError::with_source(format!("{relative} is not UTF-8"), source))
}

#[cfg(test)]
mod tests {
    use super::{CriterionKind, StatusOptions, status};

    fn landed_status() -> super::StatusReport {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        status(&StatusOptions::new(root)).expect("read landed status")
    }

    /// The whole point of the command: while a production authority is closed,
    /// it stays closed. A failure here is a released filing path, not an
    /// unfinished slice.
    #[test]
    fn landed_boundaries_are_all_held() {
        let report = landed_status();
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
    }

    /// Replaced `slice_criteria_are_still_open`, which asserted at least one
    /// criterion was outstanding. That guard existed so completion had to be
    /// acknowledged deliberately rather than drifted into; it fired when the
    /// last criterion was met and has now done its job.
    ///
    /// The condition can no longer be satisfied by *removing* criteria: every
    /// id below must be present. Deleting one to make `status` pass fails here.
    #[test]
    fn every_declared_criterion_is_still_evaluated() {
        const REQUIRED: [&str; 19] = [
            "artifacts-documented-only",
            "builder-staging-guard",
            "filing-safe-mirrors-verified-official",
            "shadow-difference-dimensions",
            "builder-staging-guard",
            "filing-safe-mirrors-verified-official",
            "shadow-difference-dimensions",
            "credentials-not-field-authority",
            "declared-sources-clone-reproducible",
            "documented-only-projection-count",
            "filing-safe-unresolved",
            "gpui-validation-summary",
            "inventory-pins-rule-set",
            "inventory-value-free",
            "occurrence-classification-complete",
            "occurrence-decomposition",
            "occurrence-reconciliation-table",
            "projectors-ported",
            "review-status-candidate",
            "reviewed-registry-empty",
            "slice-review-document",
            "validation-rules-skill",
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
