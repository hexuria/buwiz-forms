//! Strict, deterministic offline compiler for the executable validation-rules
//! IR v2 corpus.
//!
//! This crate is a development tool. It is deliberately not a build script and
//! must not be linked into the packaged application.
//!
//! The low-level builder is intentionally not part of the public API; public
//! generation entrypoints always perform their own audit.
//!
//! ```compile_fail
//! use bir_rules_codegen::build_generated_files;
//! ```
//!
//! Audited snapshot internals are not a public construction or mutation API.
//!
//! ```compile_fail
//! use bir_rules_codegen::AuditedSnapshot;
//! ```

#![forbid(unsafe_code)]
#![recursion_limit = "256"]

mod audit;
mod bindings;
mod capture_metadata;
mod check;
mod corpus;
mod coverage;
mod emit;
mod error;
mod evidence;
mod evidence_review_scaffold;
mod evidence_set;
mod files;
mod form_factory;
mod form_integration;
mod generate;
mod hash;
mod json;
mod model;
mod operator_census;
mod path;
mod projections;
mod reconciliation;
mod rollpin;
mod schema;
mod sensitive;
mod status;
mod vault_acquisition;
mod vault_source_discovery;
mod verified_file;

pub use audit::{AuditOptions, AuditReport, SnapshotSummary, audit, discover_default_repo_root};
pub use bindings::{
    BindingsReport, BuildBindingsOptions, DEFAULT_BINDING_INVENTORY_PATH, build_2550q_bindings,
};
pub use capture_metadata::{
    WriteEvidenceVaultCaptureMetadataOptions, WriteEvidenceVaultCaptureMetadataReport,
    write_evidence_vault_capture_metadata,
};
pub use check::{CheckOptions, check};
pub use corpus::{
    CorpusReport, DEFAULT_RULES_DIR, FormResult, V1_SCHEMA_VALIDATOR_ID, ValidateV1Options,
    validate_v1,
};
pub use coverage::{CoverageOptions, CoverageReport, FormCoverage, coverage};
pub use error::{CodegenError, Result};
pub use evidence::{
    DerivedEvidenceFile, DerivedEvidenceKind, EVIDENCE_PACKET_DIGEST_DOMAIN,
    EVIDENCE_PACKET_FORMAT, EVIDENCE_PACKET_MANIFEST, EvidenceAttestation, EvidenceAttestationKind,
    EvidenceCaptureOperatingSystem, EvidenceCaptureProvenance, EvidenceObservation,
    EvidencePacketManifest, EvidenceReview, EvidenceReviewStatus, ImportEvidenceOptions,
    ImportEvidenceReport, RuleSetSourceState, STAGED_FORM_DIGEST_DOMAIN, SourceExcerptLocator,
    StageFormOptions, StageFormReport, UpstreamEvidenceFile, VerifyEvidenceOptions,
    VerifyEvidenceReport, evidence_usage, import_evidence, run_evidence_command, stage_form,
    verify_evidence,
};
pub use evidence_review_scaffold::{
    CandidateCaptureGap, CandidateEvidenceReviewInput, CandidateSourceExcerpt,
    EVIDENCE_REVIEW_SCAFFOLD_REQUEST_FORMAT, EXPECTED_REVIEW_LEDGER_FORM_COUNT,
    EvidenceReviewScaffoldRequest, ScaffoldEvidenceReviewLedgerOptions,
    ScaffoldEvidenceReviewLedgerReport, ScaffoldedFormBinding,
    load_evidence_review_scaffold_request, scaffold_evidence_review_ledger,
};
pub use evidence_set::{
    BuildEvidencePacketOptions, BuildEvidencePacketReport, BuildEvidencePacketSetOptions,
    BuildEvidencePacketSetReport, CheckEvidencePacketSetOptions, CheckEvidencePacketSetReport,
    CheckedPacket, EVIDENCE_PACKET_SET_FORMAT, EVIDENCE_PACKET_SET_MANIFEST,
    EVIDENCE_REVIEW_LEDGER_FORMAT, EVIDENCE_SUMMARY_FORMAT, EVIDENCE_VAULT_CATALOG_FORMAT,
    PACKET_SET_DIGEST_DOMAIN, PACKET_SET_ORDER_DOMAIN, StageEvidencePacketReviewOptions,
    TRACKED_V1_SOURCE_SET_DOMAIN, build_evidence_packet, build_evidence_packet_set,
    check_evidence_packet_set, evidence_set_usage, run_evidence_set_command,
    stage_evidence_packet_review,
};
pub use form_integration::{
    FORM_INTEGRATION_TREE_DIGEST_DOMAIN, FormIntegrationFile, FormIntegrationOptions,
    FormIntegrationReport, IntegrateFormFile, IntegrateFormOptions, IntegrateFormReport,
    PACKET_BACKED_HANDOFF_FORMAT, PROTECTED_2550Q_RULE_SET_ID, integrate_form,
};
pub use generate::{GenerateOptions, GenerationReport, MANIFEST_FORMAT, generate};
pub use json::CANONICALIZATION_ID;
pub use operator_census::{
    OperatorCensusOptions, OperatorCensusReport, OperatorCounts, SnapshotOperatorCensus,
    V1CalculationConstructCensus, V1ValidationConstructCensus, operator_census,
};
pub use path::{DEFAULT_OUTPUT_DIR, DEFAULT_SCHEMA_DIR, DEFAULT_SOURCE_DIR};
pub use projections::{
    ProjectStaticSurfaceOptions, StaticProjectionReport, project_2550q_static_surface,
};
pub use reconciliation::{
    ArtifactReconciliation, FormReconciliation, ReconciliationOptions, ReconciliationReport,
    reconciliation,
};
pub use rollpin::{
    RollAllPinsReport, RollPinOptions, RollPinReport, SourceRepin, roll_all_pins, roll_pin,
};
pub use status::{Criterion, CriterionKind, StatusOptions, StatusReport, status};
pub use vault_acquisition::{
    AcquireEvidenceVaultOptions, AcquireEvidenceVaultReport,
    EVIDENCE_VAULT_CAPTURE_METADATA_FORMAT, EVIDENCE_VAULT_CATALOG_FILE,
    EVIDENCE_VAULT_SOURCE_MAP_FORMAT, EVIDENCE_VAULT_SOURCE_VERIFICATION_DOMAIN,
    EXPECTED_V1_FORM_MANIFEST_COUNT, EvidenceVaultCaptureMetadata, EvidenceVaultCatalog,
    EvidenceVaultCatalogEntry, EvidenceVaultSourceMap, EvidenceVaultSourceMapEntry,
    VaultAcquisitionGap, VaultAssetDisposition, VerifyEvidenceVaultSourceMapOptions,
    VerifyEvidenceVaultSourceMapReport, acquire_evidence_vault, vault_asset_disposition,
    verify_evidence_vault_source_map,
};
pub use vault_source_discovery::{
    DiscoverEvidenceVaultSourcesOptions, DiscoverEvidenceVaultSourcesReport,
    EvidenceVaultRejectedCandidate, EvidenceVaultSourceDiscoveryError,
    EvidenceVaultSourceDiscoveryResult, EvidenceVaultUnresolvedAsset,
    EvidenceVaultUnresolvedReport, discover_evidence_vault_sources,
};
