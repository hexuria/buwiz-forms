//! Inert 2550Q diagnostic surface after the old per-form view was deleted.
//!
//! The validation-rules application freeze still inspects this module so
//! 2550Q cannot gain trusted-evaluator or queue authority. The painted editor
//! is [`super::form_inventory_view`]. Queue stays behind
//! `can_queue_for_submission` (2550Q is false).

#![allow(dead_code)]

use bir_core::form_rules::{
    Form2550QLiveValidationFacade, Form2550QRepoDiagnosticSetupOutcome,
    Form2550QRepoLiveValidationState,
};
use bir_core::forms::form_2550q::Form2550QDraft;
use gpui::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Form2550QDiagnosticState {
    NotRun,
    Unavailable(String),
    Incomplete(String),
}

pub enum Form2550QV2Event {
    PushNotification(String, String, String),
}

pub struct Form2550QQueueGuard {
    status_message: Option<String>,
}

impl EventEmitter<Form2550QV2Event> for Form2550QQueueGuard {}

impl Form2550QQueueGuard {
    fn mark_submitted(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.status_message = Some(
            "2550Qv2024 has reviewed editable-save evidence only. Electronic queue/submission is not certified."
                .to_string(),
        );
        cx.emit(Form2550QV2Event::PushNotification(
            "warning".to_string(),
            "Manual / External Filing".to_string(),
            "This 2550Q draft cannot be queued or submitted by the app.".to_string(),
        ));
        cx.notify();
    }
}

fn run_inert_repo_diagnostic(draft: &mut Form2550QDraft) -> Form2550QDiagnosticState {
    match Form2550QLiveValidationFacade::setup_repo_default_diagnostic() {
        Form2550QRepoDiagnosticSetupOutcome::Unavailable { reason } => {
            Form2550QDiagnosticState::Unavailable(format!(
                "Validation did not run: {reason}. This is not a valid or filing-ready result."
            ))
        }
        Form2550QRepoDiagnosticSetupOutcome::ExactRegistrationAvailable(diagnostic) => {
            match diagnostic.evaluate(draft) {
                Form2550QRepoLiveValidationState::Unavailable { reason, .. } => {
                    Form2550QDiagnosticState::Unavailable(format!(
                        "Validation did not run: {reason}. This is not a valid or filing-ready result."
                    ))
                }
                Form2550QRepoLiveValidationState::IncompleteCapture { gaps, .. } => {
                    Form2550QDiagnosticState::Incomplete(format!(
                        "Validation is incomplete at input revision: {} required raw capture gap(s) remain. No validity or filing-readiness conclusion was produced.",
                        gaps.len()
                    ))
                }
            }
        }
    }
}

pub(crate) fn diagnostic_state_from_setup(
    setup: &Form2550QRepoDiagnosticSetupOutcome,
) -> Form2550QDiagnosticState {
    match setup {
        Form2550QRepoDiagnosticSetupOutcome::ExactRegistrationAvailable(_) => {
            Form2550QDiagnosticState::NotRun
        }
        Form2550QRepoDiagnosticSetupOutcome::Unavailable { reason } => {
            Form2550QDiagnosticState::Unavailable(format!(
                "{reason}. No candidate validator was evaluated, and this does not mean the draft is valid or filing-ready."
            ))
        }
    }
}
