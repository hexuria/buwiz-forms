//! The OS sign-in prompt for the NSH app: Touch ID or the account password on macOS, Windows Hello on Windows.
//! It is the last way back in for an admin who lost the PIN and the recovery key (host function `mac_login`,
//! the "Use my Mac login instead" button of the sign-in screen).
//!
//! The host function runs the prompt on its own `mac-login` thread and answers the TSX promise from there
//! (crates/bir-app-api/src/bir_module.rs `mac_login`), so the engine thread never waits on it; blocking here
//! is fine. Same crate and version as the old host (robius-authentication 0.1.1, crates/bir-shell/src/bir_host.rs).
//!
//! A test environment (`EBIR_TEST_ENV`: every test, shots and dev recipe) answers false without asking the OS,
//! so a headless run never puts a system prompt on the screen or waits for one.

/// Asks the OS to confirm the person at the keyboard; true only when the OS said yes.
pub(crate) fn os_authenticate(reason: &str) -> bool {
    answer(crate::door::test_environment(), reason, prompt)
}

/// The rule, apart from the OS: a test environment never reaches `ask`.
fn answer(test_environment: bool, reason: &str, ask: impl FnOnce(&str) -> bool) -> bool {
    if test_environment {
        tracing::info!("OS sign-in prompt not shown: a test environment never asks the OS");
        return false;
    }
    ask(reason)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn prompt(reason: &str) -> bool {
    use robius_authentication::{AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText};
    let Some(policy) = PolicyBuilder::new()
        .biometrics(Some(BiometricStrength::Strong))
        .password(true)
        .watch(true)
        .build()
    else {
        tracing::warn!("OS sign-in policy unavailable");
        return false;
    };
    let text = Text {
        android: AndroidText {
            title: "Admin sign-in",
            subtitle: None,
            description: None,
        },
        apple: reason,
        windows: WindowsText::new_truncated("Admin sign-in", reason),
    };
    match Context::new(()).blocking_authenticate(text, &policy) {
        Ok(()) => true,
        Err(error) => {
            tracing::info!(?error, "OS sign-in refused");
            false
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn prompt(_reason: &str) -> bool {
    tracing::warn!("this platform has no OS sign-in prompt");
    false
}

#[cfg(test)]
mod tests {
    use super::answer;

    #[test]
    fn a_test_environment_answers_false_without_asking_the_os() {
        let mut asked = false;
        assert!(!answer(true, "reset the eBIRForms admin PIN", |_| {
            asked = true;
            true
        }));
        assert!(!asked);
    }

    #[test]
    fn outside_a_test_environment_the_os_answer_is_the_answer() {
        assert!(answer(false, "reset", |reason| reason == "reset"));
        assert!(!answer(false, "reset", |_| false));
    }
}
