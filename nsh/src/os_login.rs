//! Proving that the device's owner is present, through the OS's own prompt and never a password box of ours.
//!
//! The interface is fixed in features/accounts-and-lock/OS-LOGIN-INTERFACE.md (PLAN.md §A.3.1, §B.6, §E.1–E.3).
//! - The Apple half (`apple.rs`) is ios-submission's.
//! - The Windows half (`windows.rs`) is ms-store-submission's.
//!
//! The module is inert until A3a-2 registers the host verbs `owner_check` and `prove_owner` and calls it.
//!
//! Rules:
//! - Every call blocks. The caller runs it on a fresh thread of its own, as `mac_login` does, so the engine
//!   thread never waits.
//! - In a test environment every call answers `None` / `Unavailable` without asking the OS.
//! - One `info!` line per call.

// The Windows half's pure rules (window choice, answer mapping) are tested on every OS; its OS calls are
// `#[cfg(windows)]` inside it.
#[cfg(any(windows, test))]
mod windows;

/// The app's main window title: nsh/nsh.json `window.title`, the value the kit gives the window, set by the build
/// script (build.rs), which stops the build unless it is a non-empty, one-line string. Never a second copy of the
/// string (PLAN W4). A3a-2 passes it to `prove_owner`, so the sheet belongs to the main window and never to a print
/// preview (titled with the taxpayer).
pub const APP_WINDOW_TITLE: &str = env!("APP_WINDOW_TITLE");

/// What this device can do to prove its owner, checked without prompting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerCheck {
    /// macOS, iOS, iPadOS: LAContext `.deviceOwnerAuthentication` can be evaluated
    /// (Touch ID, Face ID or the device's password/passcode, in the system's own sheet).
    DeviceOwner,
    /// Windows: `UserConsentVerifier::CheckAvailabilityAsync` answered `Available`.
    WindowsHello,
    /// Nothing usable: Linux, Android (until its kit route), Windows without Hello,
    /// an Apple device with no passcode set, or the test environment.
    None,
}

/// The answer to one prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerCheckResult {
    /// The OS says the device's owner is present.
    Verified,
    /// The person cancelled the system sheet. The caller shows nothing new.
    Cancelled,
    /// Too many tries at the OS level (Apple biometry lockout, Hello RetriesExhausted).
    LockedOut,
    /// Not available after all (it changed since `owner_check()`, no window to own the
    /// prompt, a device busy, or any other error). The caller falls back to its page's
    /// own path: support text on the legacy unlock, the email code for Forgot PIN.
    Unavailable,
}

/// What `prove_owner` would use on this device, with no prompt.
///
/// Blocking, and not cheap on Windows: `CheckAvailabilityAsync` loads the credential providers (tens of ms, over a
/// second when cold). The caller runs it on a fresh thread and reads it once per page visit, never per render.
pub fn owner_check() -> OwnerCheck {
    let answer = owner_check_with(crate::door::test_environment(), platform_owner_check);
    tracing::info!(?answer, "os_login owner_check");
    answer
}

/// Asks the OS to prove the device's owner is present, once, with `reason` as the sheet's text. Blocks until the
/// person answers.
///
/// `window_title` is the exact title of the window that owns the sheet: the app's main window, [`APP_WINDOW_TITLE`]. Windows: `RequestVerificationForWindowAsync`, owned by our NSH window with that title, never the print
/// preview (`windows.rs`); no such window, no prompt. Other targets ignore it.
pub fn prove_owner(reason: &str, window_title: &str) -> OwnerCheckResult {
    let answer = prove_owner_with(crate::door::test_environment(), || {
        platform_prove_owner(reason, window_title)
    });
    tracing::info!(?answer, "os_login prove_owner");
    answer
}

/// The test-environment rule, apart from the OS: a test environment never reaches `ask`.
fn owner_check_with(test_environment: bool, ask: impl FnOnce() -> OwnerCheck) -> OwnerCheck {
    if test_environment {
        OwnerCheck::None
    } else {
        ask()
    }
}

fn prove_owner_with(
    test_environment: bool,
    ask: impl FnOnce() -> OwnerCheckResult,
) -> OwnerCheckResult {
    if test_environment {
        OwnerCheckResult::Unavailable
    } else {
        ask()
    }
}

#[cfg(windows)]
fn platform_owner_check() -> OwnerCheck {
    self::windows::owner_check()
}

#[cfg(windows)]
fn platform_prove_owner(reason: &str, window_title: &str) -> OwnerCheckResult {
    self::windows::prove_owner(reason, window_title)
}

// Every other target, until its half lands (Apple: ios-submission's `apple.rs`).
#[cfg(not(windows))]
fn platform_owner_check() -> OwnerCheck {
    OwnerCheck::None
}

#[cfg(not(windows))]
fn platform_prove_owner(_reason: &str, _window_title: &str) -> OwnerCheckResult {
    OwnerCheckResult::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_environment_never_asks_the_os() {
        let mut asked = false;
        assert_eq!(
            owner_check_with(true, || {
                asked = true;
                OwnerCheck::WindowsHello
            }),
            OwnerCheck::None
        );
        assert_eq!(
            prove_owner_with(true, || {
                asked = true;
                OwnerCheckResult::Verified
            }),
            OwnerCheckResult::Unavailable
        );
        assert!(!asked);
    }

    #[test]
    fn outside_a_test_environment_the_os_answer_is_the_answer() {
        assert_eq!(
            owner_check_with(false, || OwnerCheck::WindowsHello),
            OwnerCheck::WindowsHello
        );
        assert_eq!(
            prove_owner_with(false, || OwnerCheckResult::LockedOut),
            OwnerCheckResult::LockedOut
        );
    }

    #[test]
    fn the_page_reads_snake_case_words() {
        let word = |value: serde_json::Value| value.as_str().unwrap_or_default().to_owned();
        assert_eq!(
            word(serde_json::json!(OwnerCheck::WindowsHello)),
            "windows_hello"
        );
        assert_eq!(
            word(serde_json::json!(OwnerCheck::DeviceOwner)),
            "device_owner"
        );
        assert_eq!(word(serde_json::json!(OwnerCheck::None)), "none");
        assert_eq!(
            word(serde_json::json!(OwnerCheckResult::Verified)),
            "verified"
        );
        assert_eq!(
            word(serde_json::json!(OwnerCheckResult::Cancelled)),
            "cancelled"
        );
        assert_eq!(
            word(serde_json::json!(OwnerCheckResult::LockedOut)),
            "locked_out"
        );
        assert_eq!(
            word(serde_json::json!(OwnerCheckResult::Unavailable)),
            "unavailable"
        );
    }

    #[test]
    fn the_main_window_title_comes_from_nsh_json() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../nsh.json")).expect("nsh.json is JSON");
        assert_eq!(
            config
                .pointer("/window/title")
                .and_then(serde_json::Value::as_str),
            Some(APP_WINDOW_TITLE)
        );
        assert!(!APP_WINDOW_TITLE.is_empty());
    }

    #[test]
    fn os_login_has_no_password_input() {
        // The module draws no text field and never takes a password: none of the Windows password-box calls, and
        // not the old crate that made them. Comment lines may name them; code may not.
        let banned = [
            ["Cred", "UI"].concat(),
            ["Logon", "User"].concat(),
            ["robius", "_authentication"].concat(),
            ["Text", "Field"].concat(),
            ["Secure", "Text"].concat(),
            // WinRT's own user name and password prompt, in the same Security_Credentials_UI feature as Hello.
            ["Credential", "Picker"].concat(),
            ["Cred", "Pack"].concat(),
        ];
        for source in [
            include_str!("os_login.rs"),
            include_str!("os_login/windows.rs"),
        ] {
            for line in source
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
            {
                for word in &banned {
                    assert!(
                        !line.contains(word.as_str()),
                        "os_login code must not use {word}: {line}"
                    );
                }
            }
        }
    }
}
