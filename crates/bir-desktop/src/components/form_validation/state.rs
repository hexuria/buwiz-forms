use bir_core::{
    BehaviorProfile, ContextFingerprint, ContextValueSnapshot, EvaluationResult, FormRevisionKey,
    InputRevision, RuleSeverity, RuleViolation, ShadowEvaluationOutcome, ValidationContext,
    ValidationPhase, WorkflowTransitionResult,
};
use std::{error::Error, fmt};

/// Exact token for one in-flight evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEvaluation {
    rule_set: FormRevisionKey,
    context: ValidationContext,
    input_revision: InputRevision,
    context_fingerprint: ContextFingerprint,
}

impl PendingEvaluation {
    pub fn rule_set(&self) -> &FormRevisionKey {
        &self.rule_set
    }

    pub const fn context(&self) -> ValidationContext {
        self.context
    }

    pub const fn phase(&self) -> ValidationPhase {
        self.context.phase()
    }

    pub const fn profile(&self) -> BehaviorProfile {
        self.context.profile()
    }

    pub const fn input_revision(&self) -> InputRevision {
        self.input_revision
    }

    pub const fn context_fingerprint(&self) -> ContextFingerprint {
        self.context_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluatorUnavailableKind {
    Registry,
    Evaluation,
}

/// Explicit UI state for a rule set that could not be resolved or evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatorUnavailable {
    kind: EvaluatorUnavailableKind,
    detail: String,
}

impl EvaluatorUnavailable {
    pub fn new(kind: EvaluatorUnavailableKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub const fn kind(&self) -> EvaluatorUnavailableKind {
        self.kind
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Incomplete diagnostic output stamped with the exact in-flight token that
/// produced it.
///
/// This is visible UI state only. It contains no report, trusted evaluation,
/// serialization artifact, or filing authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteEvaluationSnapshot {
    stamp: PendingEvaluation,
    detail: String,
}

impl IncompleteEvaluationSnapshot {
    pub fn new(stamp: &PendingEvaluation, detail: impl Into<String>) -> Self {
        Self {
            stamp: stamp.clone(),
            detail: detail.into(),
        }
    }

    pub fn stamp(&self) -> &PendingEvaluation {
        &self.stamp
    }

    pub fn rule_set(&self) -> &FormRevisionKey {
        self.stamp.rule_set()
    }

    pub const fn context(&self) -> ValidationContext {
        self.stamp.context()
    }

    pub const fn input_revision(&self) -> InputRevision {
        self.stamp.input_revision()
    }

    pub const fn context_fingerprint(&self) -> ContextFingerprint {
        self.stamp.context_fingerprint()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationAcceptance {
    AcceptedReport,
    AcceptedWorkflowTransition,
    AcceptedUnavailable,
    AcceptedIncomplete,
    IgnoredStale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionAdvanceError;

impl fmt::Display for RevisionAdvanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("input revision counter is exhausted")
    }
}

impl Error for RevisionAdvanceError {}

/// Pure controller state for one exact form/rule-set identity.
#[derive(Debug, Clone)]
pub struct FormValidationState {
    rule_set: FormRevisionKey,
    input_revision: InputRevision,
    context_fingerprint: ContextFingerprint,
    pending: Option<PendingEvaluation>,
    result: Option<EvaluationResult>,
    workflow_transition: Option<WorkflowTransitionResult>,
    evaluator_unavailable: Option<EvaluatorUnavailable>,
    incomplete: Option<IncompleteEvaluationSnapshot>,
}

impl FormValidationState {
    /// Start controller state from the validated external context values.
    ///
    /// The caller supplies facts, never a detached fingerprint assertion.
    pub fn new(rule_set: FormRevisionKey, context_values: &ContextValueSnapshot) -> Self {
        Self {
            rule_set,
            input_revision: InputRevision::default(),
            context_fingerprint: context_values.fingerprint(),
            pending: None,
            result: None,
            workflow_transition: None,
            evaluator_unavailable: None,
            incomplete: None,
        }
    }

    pub fn rule_set(&self) -> &FormRevisionKey {
        &self.rule_set
    }

    pub const fn input_revision(&self) -> InputRevision {
        self.input_revision
    }

    pub const fn context_fingerprint(&self) -> ContextFingerprint {
        self.context_fingerprint
    }

    pub fn pending(&self) -> Option<&PendingEvaluation> {
        self.pending.as_ref()
    }

    pub fn pending_phase(&self) -> Option<ValidationPhase> {
        self.pending.as_ref().map(PendingEvaluation::phase)
    }

    pub fn pending_profile(&self) -> Option<BehaviorProfile> {
        self.pending.as_ref().map(PendingEvaluation::profile)
    }

    pub fn result(&self) -> Option<&EvaluationResult> {
        self.result.as_ref()
    }

    pub fn workflow_transition(&self) -> Option<&WorkflowTransitionResult> {
        self.workflow_transition.as_ref()
    }

    pub fn evaluator_unavailable(&self) -> Option<&EvaluatorUnavailable> {
        self.evaluator_unavailable.as_ref()
    }

    pub fn incomplete(&self) -> Option<&IncompleteEvaluationSnapshot> {
        self.incomplete.as_ref()
    }

    /// Advance the raw snapshot generation and invalidate all older output.
    pub fn raw_edited(&mut self) -> Result<InputRevision, RevisionAdvanceError> {
        let Some(next) = self.input_revision.get().checked_add(1) else {
            self.clear_evaluation_state();
            return Err(RevisionAdvanceError);
        };
        self.input_revision = InputRevision::new(next);
        self.clear_evaluation_state();
        Ok(self.input_revision)
    }

    /// Install new validated external context and invalidate prior output.
    ///
    /// This does not increment `InputRevision`: stale context is rejected by
    /// the snapshot-derived fingerprint.
    pub fn context_changed(&mut self, context_values: &ContextValueSnapshot) -> bool {
        let context_fingerprint = context_values.fingerprint();
        if self.context_fingerprint == context_fingerprint {
            return false;
        }
        self.context_fingerprint = context_fingerprint;
        self.clear_evaluation_state();
        true
    }

    /// Begin an ordinary UI evaluation using the filing-safe profile.
    pub fn begin_filing_safe_evaluation(&mut self, phase: ValidationPhase) -> PendingEvaluation {
        self.begin_with_profile(phase, BehaviorProfile::FilingSafe)
    }

    /// Begin an OfficialCompatibility observation for shadow diagnostics only.
    ///
    /// This deliberately separate API must not be used to authorize a Final
    /// Copy, export, queue entry, or submission.
    pub fn begin_official_compatibility_shadow_diagnostic(
        &mut self,
        phase: ValidationPhase,
    ) -> PendingEvaluation {
        self.begin_with_profile(phase, BehaviorProfile::OfficialCompatibility)
    }

    fn begin_with_profile(
        &mut self,
        phase: ValidationPhase,
        profile: BehaviorProfile,
    ) -> PendingEvaluation {
        let pending = PendingEvaluation {
            rule_set: self.rule_set.clone(),
            context: ValidationContext::new(phase, profile),
            input_revision: self.input_revision,
            context_fingerprint: self.context_fingerprint,
        };
        self.pending = Some(pending.clone());
        self.result = None;
        self.workflow_transition = None;
        self.evaluator_unavailable = None;
        self.incomplete = None;
        pending
    }

    /// Accept a direct evaluator result only when it matches the current
    /// in-flight token exactly.
    pub fn accept_result(&mut self, result: EvaluationResult) -> EvaluationAcceptance {
        if !self.matches_pending(
            result.rule_set(),
            result.context(),
            result.input_revision(),
            result.context_fingerprint(),
        ) {
            return EvaluationAcceptance::IgnoredStale;
        }

        self.pending = None;
        self.evaluator_unavailable = None;
        self.incomplete = None;
        self.result = Some(result);
        self.workflow_transition = None;
        EvaluationAcceptance::AcceptedReport
    }

    /// Install a separately evaluated workflow transition only when it binds
    /// to the exact visible validation result. This does not mutate GPUI
    /// controls; the view applies the returned semantic state explicitly.
    pub fn accept_workflow_transition(
        &mut self,
        transition: WorkflowTransitionResult,
    ) -> EvaluationAcceptance {
        let Some(result) = self.result.as_ref() else {
            return EvaluationAcceptance::IgnoredStale;
        };
        if transition.rule_set() != &self.rule_set
            || transition.rule_set() != result.rule_set()
            || transition.context() != result.context()
            || !result.is_valid()
            || transition.input_revision() != self.input_revision
            || transition.input_revision() != result.input_revision()
            || transition.context_fingerprint() != self.context_fingerprint
            || transition.context_fingerprint() != result.context_fingerprint()
        {
            return EvaluationAcceptance::IgnoredStale;
        }
        self.workflow_transition = Some(transition);
        EvaluationAcceptance::AcceptedWorkflowTransition
    }

    /// Apply an inert core shadow observation. Failure is visible state, never
    /// authorization or a capability/queue mutation.
    pub fn accept_shadow(&mut self, outcome: ShadowEvaluationOutcome) -> EvaluationAcceptance {
        match outcome {
            ShadowEvaluationOutcome::Evaluated { stamp, result } => {
                if !self.matches_pending(
                    stamp.rule_set(),
                    stamp.context(),
                    stamp.input_revision(),
                    stamp.context_fingerprint(),
                ) || stamp.rule_set() != result.rule_set()
                    || stamp.context() != result.context()
                    || stamp.input_revision() != result.input_revision()
                    || stamp.context_fingerprint() != result.context_fingerprint()
                {
                    return EvaluationAcceptance::IgnoredStale;
                }
                self.accept_result(result)
            }
            ShadowEvaluationOutcome::RegistryFailure { stamp, error } => self
                .accept_unavailable_stamp(
                    stamp.rule_set(),
                    stamp.context(),
                    stamp.input_revision(),
                    stamp.context_fingerprint(),
                    EvaluatorUnavailable::new(
                        EvaluatorUnavailableKind::Registry,
                        error.to_string(),
                    ),
                ),
            ShadowEvaluationOutcome::EvaluationFailure { stamp, error } => self
                .accept_unavailable_stamp(
                    stamp.rule_set(),
                    stamp.context(),
                    stamp.input_revision(),
                    stamp.context_fingerprint(),
                    EvaluatorUnavailable::new(
                        EvaluatorUnavailableKind::Evaluation,
                        error.to_string(),
                    ),
                ),
        }
    }

    /// Record an unavailable evaluator from another pure controller layer,
    /// guarded by the same exact pending token.
    pub fn accept_unavailable(
        &mut self,
        pending: &PendingEvaluation,
        unavailable: EvaluatorUnavailable,
    ) -> EvaluationAcceptance {
        self.accept_unavailable_stamp(
            pending.rule_set(),
            pending.context(),
            pending.input_revision(),
            pending.context_fingerprint(),
            unavailable,
        )
    }

    /// Install incomplete diagnostic state only when its embedded stamp still
    /// matches the exact current in-flight token.
    pub fn accept_incomplete(
        &mut self,
        incomplete: IncompleteEvaluationSnapshot,
    ) -> EvaluationAcceptance {
        if !self.matches_pending(
            incomplete.rule_set(),
            incomplete.context(),
            incomplete.input_revision(),
            incomplete.context_fingerprint(),
        ) {
            return EvaluationAcceptance::IgnoredStale;
        }

        self.pending = None;
        self.result = None;
        self.workflow_transition = None;
        self.evaluator_unavailable = None;
        self.incomplete = Some(incomplete);
        EvaluationAcceptance::AcceptedIncomplete
    }

    pub fn blocking_issues(&self) -> Vec<&RuleViolation> {
        self.issues_with_severity(RuleSeverity::Blocking)
    }

    pub fn advisory_issues(&self) -> Vec<&RuleViolation> {
        self.issues_with_severity(RuleSeverity::Advisory)
    }

    pub fn blocking_count(&self) -> usize {
        self.blocking_issues().len()
    }

    pub fn advisory_count(&self) -> usize {
        self.advisory_issues().len()
    }

    fn issues_with_severity(&self, severity: RuleSeverity) -> Vec<&RuleViolation> {
        self.result
            .iter()
            .flat_map(|result| result.report().violations())
            .filter(|issue| issue.severity() == severity)
            .collect()
    }

    fn accept_unavailable_stamp(
        &mut self,
        rule_set: &FormRevisionKey,
        context: ValidationContext,
        input_revision: InputRevision,
        context_fingerprint: ContextFingerprint,
        unavailable: EvaluatorUnavailable,
    ) -> EvaluationAcceptance {
        if !self.matches_pending(rule_set, context, input_revision, context_fingerprint) {
            return EvaluationAcceptance::IgnoredStale;
        }

        self.pending = None;
        self.result = None;
        self.workflow_transition = None;
        self.incomplete = None;
        self.evaluator_unavailable = Some(unavailable);
        EvaluationAcceptance::AcceptedUnavailable
    }

    fn matches_pending(
        &self,
        rule_set: &FormRevisionKey,
        context: ValidationContext,
        input_revision: InputRevision,
        context_fingerprint: ContextFingerprint,
    ) -> bool {
        self.rule_set == *rule_set
            && self.input_revision == input_revision
            && self.context_fingerprint == context_fingerprint
            && self.pending.as_ref().is_some_and(|pending| {
                pending.rule_set == *rule_set
                    && pending.context == context
                    && pending.input_revision == input_revision
                    && pending.context_fingerprint == context_fingerprint
            })
    }

    fn clear_evaluation_state(&mut self) {
        self.pending = None;
        self.result = None;
        self.evaluator_unavailable = None;
        self.incomplete = None;
        // An accepted transition describes the workflow state of a *specific*
        // evaluated snapshot. Leaving it behind after a raw edit or a context
        // change lets the view keep showing `validated` for input that has
        // since been altered.
        self.workflow_transition = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const RULE_SET_ID: &str = "test-v1-p1";
    const SOURCE_DIGEST: &str = "0404040404040404040404040404040404040404040404040404040404040404";

    fn rule_set() -> FormRevisionKey {
        serde_json::from_value(json!({
            "rule_set_id": RULE_SET_ID,
            "form_code": "TEST",
            "form_revision": "v1",
            "official_package_version": "p1",
            "source_set_sha256": SOURCE_DIGEST,
        }))
        .unwrap()
    }

    fn context_snapshot(profile_version: &str) -> ContextValueSnapshot {
        serde_json::from_value(json!({
            "values": [{
                "id": "profile-version",
                "value": {
                    "type": "text",
                    "value": profile_version,
                },
            }],
        }))
        .unwrap()
    }

    fn result(
        input_revision: u64,
        phase: ValidationPhase,
        profile: BehaviorProfile,
        context_values: &ContextValueSnapshot,
    ) -> EvaluationResult {
        let phase = serde_json::to_value(phase).unwrap();
        let profile = serde_json::to_value(profile).unwrap();
        let context_fingerprint = context_values.fingerprint();
        serde_json::from_value(json!({
            "report": {
                "rule_set": {
                    "rule_set_id": RULE_SET_ID,
                    "form_code": "TEST",
                    "form_revision": "v1",
                    "official_package_version": "p1",
                    "source_set_sha256": SOURCE_DIGEST,
                },
                "context": {
                    "phase": phase,
                    "profile": profile,
                },
                "input_revision": input_revision,
                "context_fingerprint": context_fingerprint,
                "expected_rules": [
                    {"execution": {"rule_id": "blocking-rule", "instance": null}, "order": 10},
                    {"execution": {"rule_id": "advisory-rule", "instance": null}, "order": 20}
                ],
                "evaluated_rules": [
                    {"rule_id": "blocking-rule", "instance": null},
                    {"rule_id": "advisory-rule", "instance": null}
                ],
                "violations": [
                    {
                        "execution": {"rule_id": "blocking-rule", "instance": null},
                        "phase": phase,
                        "order": {"rule_order": 10, "occurrence": 0},
                        "fields": [],
                        "official_message": null,
                        "message": "Blocking",
                        "assessment": "verified-correct",
                        "severity": "blocking",
                        "profile": profile
                    },
                    {
                        "execution": {"rule_id": "advisory-rule", "instance": null},
                        "phase": phase,
                        "order": {"rule_order": 20, "occurrence": 0},
                        "fields": [],
                        "official_message": null,
                        "message": "Advisory",
                        "assessment": "verified-correct",
                        "severity": "advisory",
                        "profile": profile
                    }
                ]
            },
            "canonical_inputs": [],
            "expected_outputs": [],
            "derived_outputs": []
        }))
        .unwrap()
    }

    fn valid_result(
        input_revision: u64,
        context_values: &ContextValueSnapshot,
    ) -> EvaluationResult {
        serde_json::from_value(json!({
            "report": {
                "rule_set": {
                    "rule_set_id": RULE_SET_ID,
                    "form_code": "TEST",
                    "form_revision": "v1",
                    "official_package_version": "p1",
                    "source_set_sha256": SOURCE_DIGEST,
                },
                "context": {
                    "phase": "validate",
                    "profile": "official",
                },
                "input_revision": input_revision,
                "context_fingerprint": context_values.fingerprint(),
                "expected_rules": [],
                "evaluated_rules": [],
                "violations": [],
            },
            "canonical_inputs": [],
            "expected_outputs": [],
            "derived_outputs": [],
        }))
        .unwrap()
    }

    fn workflow_result(
        input_revision: u64,
        context_values: &ContextValueSnapshot,
    ) -> WorkflowTransitionResult {
        serde_json::from_value(json!({
            "rule_set": {
                "rule_set_id": RULE_SET_ID,
                "form_code": "TEST",
                "form_revision": "v1",
                "official_package_version": "p1",
                "source_set_sha256": SOURCE_DIGEST,
            },
            "context": {
                "phase": "validate",
                "profile": "official"
            },
            "input_revision": input_revision,
            "context_fingerprint": context_values.fingerprint(),
            "transition_id": "validate-success",
            "from_state": "edit",
            "action": "validate",
            "to_state": "validated",
            "notifications": [{
                "channel": "alert",
                "message": "Validation successful.",
                "official_message": "Validation successful.",
            }],
        }))
        .unwrap()
    }

    fn edit_workflow_result(
        input_revision: u64,
        context_values: &ContextValueSnapshot,
    ) -> WorkflowTransitionResult {
        serde_json::from_value(json!({
            "rule_set": {
                "rule_set_id": RULE_SET_ID,
                "form_code": "TEST",
                "form_revision": "v1",
                "official_package_version": "p1",
                "source_set_sha256": SOURCE_DIGEST,
            },
            "context": {
                "phase": "validate",
                "profile": "official"
            },
            "input_revision": input_revision,
            "context_fingerprint": context_values.fingerprint(),
            "transition_id": "edit-after-validation",
            "from_state": "validated",
            "action": "edit",
            "to_state": "edit",
            "notifications": [{
                "channel": "alert",
                "message": "You can now modify your entries.",
                "official_message": "You can now modify your entries.",
            }],
        }))
        .unwrap()
    }

    fn state() -> FormValidationState {
        FormValidationState::new(rule_set(), &context_snapshot("p-1"))
    }

    #[test]
    fn raw_edit_advances_revision_and_ignores_older_result() {
        let mut state = state();
        let context_values = context_snapshot("p-1");
        state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        let old = result(
            0,
            ValidationPhase::Validate,
            BehaviorProfile::FilingSafe,
            &context_values,
        );

        assert_eq!(state.raw_edited().unwrap(), InputRevision::new(1));
        assert_eq!(state.accept_result(old), EvaluationAcceptance::IgnoredStale);
        assert!(state.result().is_none());
    }

    #[test]
    fn matching_report_installs_ordered_blocking_and_advisory_summaries() {
        let mut state = state();
        let context_values = context_snapshot("p-1");
        state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        assert_eq!(state.pending_phase(), Some(ValidationPhase::Validate));
        assert_eq!(state.pending_profile(), Some(BehaviorProfile::FilingSafe));

        assert_eq!(
            state.accept_result(result(
                0,
                ValidationPhase::Validate,
                BehaviorProfile::FilingSafe,
                &context_values,
            )),
            EvaluationAcceptance::AcceptedReport
        );
        assert_eq!(state.blocking_count(), 1);
        assert_eq!(state.advisory_count(), 1);
        assert_eq!(state.blocking_issues()[0].message(), "Blocking");
        assert_eq!(state.advisory_issues()[0].message(), "Advisory");
    }

    #[test]
    fn workflow_transition_requires_exact_visible_valid_result_and_clears_on_edit() {
        let mut state = state();
        let context_values = context_snapshot("p-1");
        state.begin_official_compatibility_shadow_diagnostic(ValidationPhase::Validate);
        assert_eq!(
            state.accept_result(result(
                0,
                ValidationPhase::Validate,
                BehaviorProfile::OfficialCompatibility,
                &context_values,
            )),
            EvaluationAcceptance::AcceptedReport
        );
        assert_eq!(
            state.accept_workflow_transition(workflow_result(0, &context_values)),
            EvaluationAcceptance::IgnoredStale
        );
        state.begin_official_compatibility_shadow_diagnostic(ValidationPhase::Validate);
        assert_eq!(
            state.accept_result(valid_result(0, &context_values)),
            EvaluationAcceptance::AcceptedReport
        );
        assert_eq!(
            state.accept_workflow_transition(workflow_result(0, &context_values)),
            EvaluationAcceptance::AcceptedWorkflowTransition
        );
        assert_eq!(
            state
                .workflow_transition()
                .map(|transition| transition.to_state().as_str()),
            Some("validated")
        );
        assert_eq!(
            state.accept_workflow_transition(edit_workflow_result(0, &context_values)),
            EvaluationAcceptance::AcceptedWorkflowTransition
        );
        assert_eq!(
            state
                .workflow_transition()
                .map(|transition| transition.to_state().as_str()),
            Some("edit")
        );

        let stale = workflow_result(0, &context_values);
        state.raw_edited().unwrap();
        assert!(state.workflow_transition().is_none());
        assert_eq!(
            state.accept_workflow_transition(stale),
            EvaluationAcceptance::IgnoredStale
        );
    }

    #[test]
    fn wrong_phase_profile_or_context_is_stale() {
        let mut state = state();
        let context_values = context_snapshot("p-1");
        state.begin_filing_safe_evaluation(ValidationPhase::Validate);

        assert_eq!(
            state.accept_result(result(
                0,
                ValidationPhase::DraftPreview,
                BehaviorProfile::FilingSafe,
                &context_values,
            )),
            EvaluationAcceptance::IgnoredStale
        );
        assert_eq!(
            state.accept_result(result(
                0,
                ValidationPhase::Validate,
                BehaviorProfile::OfficialCompatibility,
                &context_values,
            )),
            EvaluationAcceptance::IgnoredStale
        );
        assert!(!state.context_changed(&context_values));
        assert!(state.context_changed(&context_snapshot("p-2")));
        assert_eq!(
            state.accept_result(result(
                0,
                ValidationPhase::Validate,
                BehaviorProfile::FilingSafe,
                &context_values,
            )),
            EvaluationAcceptance::IgnoredStale
        );
    }

    #[test]
    fn evaluator_unavailable_is_explicit_and_stale_failure_is_ignored() {
        let mut state = state();
        let pending = state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        assert_eq!(
            state.accept_unavailable(
                &pending,
                EvaluatorUnavailable::new(
                    EvaluatorUnavailableKind::Registry,
                    "exact ruleset unavailable",
                ),
            ),
            EvaluationAcceptance::AcceptedUnavailable
        );
        assert_eq!(
            state
                .evaluator_unavailable()
                .map(EvaluatorUnavailable::kind),
            Some(EvaluatorUnavailableKind::Registry)
        );

        state.raw_edited().unwrap();
        assert_eq!(
            state.accept_unavailable(
                &pending,
                EvaluatorUnavailable::new(EvaluatorUnavailableKind::Evaluation, "late failure",),
            ),
            EvaluationAcceptance::IgnoredStale
        );
        assert!(state.evaluator_unavailable().is_none());
    }

    #[test]
    fn incomplete_snapshot_is_accepted_only_for_the_exact_pending_stamp() {
        let mut state = state();
        let pending = state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        let incomplete =
            IncompleteEvaluationSnapshot::new(&pending, "two raw controls are not captured");

        assert_eq!(
            state.accept_incomplete(incomplete),
            EvaluationAcceptance::AcceptedIncomplete
        );
        let accepted = state.incomplete().expect("incomplete state is visible");
        assert_eq!(accepted.stamp(), &pending);
        assert_eq!(accepted.detail(), "two raw controls are not captured");
        assert!(state.pending().is_none());
        assert!(state.result().is_none());
        assert!(state.evaluator_unavailable().is_none());

        state.raw_edited().unwrap();
        assert!(
            state.incomplete().is_none(),
            "an edit must clear an incomplete observation"
        );
    }

    #[test]
    fn context_change_clears_incomplete_and_rejects_its_stale_snapshot() {
        let mut state = state();
        let pending = state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        let incomplete =
            IncompleteEvaluationSnapshot::new(&pending, "capture is not reviewed complete");
        let stale = incomplete.clone();

        assert_eq!(
            state.accept_incomplete(incomplete),
            EvaluationAcceptance::AcceptedIncomplete
        );
        assert!(state.context_changed(&context_snapshot("p-2")));
        assert!(state.incomplete().is_none());
        assert_eq!(
            state.accept_incomplete(stale),
            EvaluationAcceptance::IgnoredStale
        );
        assert!(state.incomplete().is_none());
    }

    #[test]
    fn input_revision_overflow_invalidates_every_pending_or_visible_outcome() {
        let mut state = state();
        state.input_revision = InputRevision::new(u64::MAX);
        let pending = state.begin_filing_safe_evaluation(ValidationPhase::Validate);
        state.result = Some(result(
            u64::MAX,
            ValidationPhase::Validate,
            BehaviorProfile::FilingSafe,
            &context_snapshot("p-1"),
        ));
        state.evaluator_unavailable = Some(EvaluatorUnavailable::new(
            EvaluatorUnavailableKind::Registry,
            "stale unavailable state",
        ));
        state.incomplete = Some(IncompleteEvaluationSnapshot::new(
            &pending,
            "stale incomplete state",
        ));

        assert_eq!(state.raw_edited(), Err(RevisionAdvanceError));
        assert_eq!(state.input_revision(), InputRevision::new(u64::MAX));
        assert!(state.pending().is_none());
        assert!(state.result().is_none());
        assert!(state.evaluator_unavailable().is_none());
        assert!(state.incomplete().is_none());
    }

    #[test]
    fn official_compatibility_start_is_explicitly_shadow_diagnostic() {
        let mut state = state();

        let pending =
            state.begin_official_compatibility_shadow_diagnostic(ValidationPhase::DraftPreview);

        assert_eq!(pending.phase(), ValidationPhase::DraftPreview);
        assert_eq!(pending.profile(), BehaviorProfile::OfficialCompatibility);
        assert_eq!(
            pending.context_fingerprint(),
            context_snapshot("p-1").fingerprint()
        );
    }
}
