//! Pure validation state shared by future form views.
//!
//! This module intentionally contains no GPUI elements, form-specific rules,
//! persistence, or network behavior.

mod field_focus;
mod state;

pub use field_focus::{DuplicateSemanticFieldTarget, SemanticFieldTargets};
pub use state::{
    EvaluationAcceptance, EvaluatorUnavailable, EvaluatorUnavailableKind, FormValidationState,
    IncompleteEvaluationSnapshot, PendingEvaluation, RevisionAdvanceError,
};
