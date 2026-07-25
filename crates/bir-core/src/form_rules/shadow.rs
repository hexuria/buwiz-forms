use bir_rules::{
    ContextFingerprint, EvaluationError, EvaluationRequest, EvaluationResult, FormRevisionKey,
    InputRevision, RegistryError, ValidationContext,
};

/// Metadata needed to correlate shadow observations and discard stale output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationStamp {
    rule_set: FormRevisionKey,
    context: ValidationContext,
    input_revision: InputRevision,
    context_fingerprint: ContextFingerprint,
}

impl EvaluationStamp {
    pub(crate) fn from_request(request: &EvaluationRequest) -> Self {
        Self {
            rule_set: request.rule_set().clone(),
            context: request.context(),
            input_revision: request.input_revision(),
            context_fingerprint: request.context_fingerprint(),
        }
    }

    pub fn rule_set(&self) -> &FormRevisionKey {
        &self.rule_set
    }

    pub const fn context(&self) -> ValidationContext {
        self.context
    }

    pub const fn input_revision(&self) -> InputRevision {
        self.input_revision
    }

    pub const fn context_fingerprint(&self) -> ContextFingerprint {
        self.context_fingerprint
    }
}

/// Observational result that carries failure without becoming authorization.
///
/// Consuming this value must not change UI validity, capabilities, final-copy
/// state, or queue behavior. Trusted callers use
/// [`super::FormRuleEvaluator::evaluate_trusted`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowEvaluationOutcome {
    Evaluated {
        stamp: EvaluationStamp,
        result: EvaluationResult,
    },
    RegistryFailure {
        stamp: EvaluationStamp,
        error: RegistryError,
    },
    EvaluationFailure {
        stamp: EvaluationStamp,
        error: EvaluationError,
    },
}

impl ShadowEvaluationOutcome {
    pub const fn stamp(&self) -> &EvaluationStamp {
        match self {
            Self::Evaluated { stamp, .. }
            | Self::RegistryFailure { stamp, .. }
            | Self::EvaluationFailure { stamp, .. } => stamp,
        }
    }
}
