#[path = "../src/quit_guard.rs"]
mod quit_guard;

use quit_guard::{ApplicationQuitDecision, application_quit_decision};

#[test]
fn application_quit_is_allowed_when_compliance_editor_is_clean() {
    assert_eq!(
        application_quit_decision(false),
        ApplicationQuitDecision::Quit
    );
}

#[test]
fn application_quit_stays_open_when_compliance_editor_is_dirty() {
    assert_eq!(
        application_quit_decision(true),
        ApplicationQuitDecision::StayOpenForUnsavedCompliance
    );
}
