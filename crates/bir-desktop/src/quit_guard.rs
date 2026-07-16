#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApplicationQuitDecision {
    Quit,
    StayOpenForUnsavedCompliance,
}

pub(crate) fn application_quit_decision(
    has_unsaved_compliance_changes: bool,
) -> ApplicationQuitDecision {
    if has_unsaved_compliance_changes {
        ApplicationQuitDecision::StayOpenForUnsavedCompliance
    } else {
        ApplicationQuitDecision::Quit
    }
}
