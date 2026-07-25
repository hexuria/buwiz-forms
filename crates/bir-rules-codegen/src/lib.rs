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
mod check;
mod corpus;
mod emit;
mod error;
mod files;
mod generate;
mod hash;
mod json;
mod model;
mod path;
mod projections;
mod rollpin;
mod schema;
mod status;

pub use audit::{AuditOptions, AuditReport, audit, discover_default_repo_root};
pub use bindings::{
    BindingsReport, BuildBindingsOptions, DEFAULT_BINDING_INVENTORY_PATH, build_2550q_bindings,
};
pub use check::{CheckOptions, check};
pub use corpus::{
    CorpusReport, DEFAULT_RULES_DIR, FormResult, V1_SCHEMA_VALIDATOR_ID, ValidateV1Options,
    validate_v1,
};
pub use error::{CodegenError, Result};
pub use generate::{GenerateOptions, GenerationReport, MANIFEST_FORMAT, generate};
pub use json::CANONICALIZATION_ID;
pub use path::{DEFAULT_OUTPUT_DIR, DEFAULT_SCHEMA_DIR, DEFAULT_SOURCE_DIR};
pub use projections::{
    ProjectStaticSurfaceOptions, StaticProjectionReport, project_2550q_static_surface,
};
pub use rollpin::{RollPinOptions, RollPinReport, SourceRepin, roll_pin};
pub use status::{Criterion, CriterionKind, StatusOptions, StatusReport, status};
