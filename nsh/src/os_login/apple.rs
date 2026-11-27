//! Apple half of `os_login`: LocalAuthentication's `.deviceOwnerAuthentication`, in the system's own sheet.
//!
//! PLAN.md §A.3.1, §B.6 and OS-LOGIN-INTERFACE.md "Apple mapping":
//! - One policy only, `.deviceOwnerAuthentication`: Touch ID, Face ID or the device's password or passcode, every one
//!   drawn by the OS. Never a password box of ours.
//! - A fresh `LAContext` for every call, so an earlier success is never reused.
//! - `owner_check` is `None` when `canEvaluatePolicy` refuses: an iPhone or iPad with no passcode, the Simulator.
//!   `prove_owner` then answers `Unavailable` without a sheet.
//! - `prove_owner` blocks the caller's own thread until the reply. LocalAuthentication calls the reply once, on a
//!   queue of its own and never on the calling thread, so the wait cannot hold the reply up.
//!
//! The answer mapping (`map_la_error`) is a plain function over the LAError code, tested on every OS. The OS calls are
//! in `os` below, macOS and iOS only: the targets that link objc2-local-authentication (nsh/Cargo.toml).

use super::OwnerCheckResult;

/// LAError's codes (LocalAuthentication's LAError.h), as plain numbers so the mapping is tested on every OS. The
/// Apple-only test `the_codes_are_local_authentications_own` checks them against the crate's constants.
mod code {
    pub const AUTHENTICATION_FAILED: isize = -1;
    pub const USER_CANCEL: isize = -2;
    pub const USER_FALLBACK: isize = -3;
    pub const SYSTEM_CANCEL: isize = -4;
    pub const PASSCODE_NOT_SET: isize = -5;
    pub const BIOMETRY_NOT_AVAILABLE: isize = -6;
    pub const BIOMETRY_NOT_ENROLLED: isize = -7;
    pub const BIOMETRY_LOCKOUT: isize = -8;
    pub const APP_CANCEL: isize = -9;
    pub const INVALID_CONTEXT: isize = -10;
    pub const NOT_INTERACTIVE: isize = -1004;
}

/// What a failed evaluation means to the caller.
/// - The person cancelled or failed the sheet, or the system or the app dismissed it: `Cancelled`. The caller
///   shows nothing new.
/// - Too many failed tries: `LockedOut`.
/// - Anything else, the passcode gone since `owner_check`, a context that cannot show a sheet: `Unavailable`. The
///   caller falls back to its page's own path.
pub(super) fn map_la_error(error: isize) -> OwnerCheckResult {
    match error {
        code::USER_CANCEL | code::SYSTEM_CANCEL | code::APP_CANCEL | code::AUTHENTICATION_FAILED => {
            OwnerCheckResult::Cancelled
        }
        code::BIOMETRY_LOCKOUT => OwnerCheckResult::LockedOut,
        _ => OwnerCheckResult::Unavailable,
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) use os::{owner_check, prove_owner};

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod os {
    use std::sync::mpsc;

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};

    use super::super::{OwnerCheck, OwnerCheckResult};
    use super::map_la_error;

    /// The only policy this module evaluates.
    const POLICY: LAPolicy = LAPolicy::DeviceOwnerAuthentication;

    pub(in crate::os_login) fn owner_check() -> OwnerCheck {
        // SAFETY: a fresh context, used on this thread only, asked about one of LAPolicy's own values.
        let context = unsafe { LAContext::new() };
        match unsafe { context.canEvaluatePolicy_error(POLICY) } {
            Ok(()) => OwnerCheck::DeviceOwner,
            Err(error) => {
                tracing::warn!(code = error.code(), "os_login: the device owner check is not available");
                OwnerCheck::None
            }
        }
    }

    pub(in crate::os_login) fn prove_owner(reason: &str) -> OwnerCheckResult {
        // SAFETY: as in `owner_check`.
        let context = unsafe { LAContext::new() };
        if let Err(error) = unsafe { context.canEvaluatePolicy_error(POLICY) } {
            tracing::warn!(code = error.code(), "os_login: the device owner check is not available");
            return OwnerCheckResult::Unavailable;
        }

        // Room for the one reply, so sending never waits.
        let (send, answer) = mpsc::sync_channel(1);
        let reply = RcBlock::new(move |verified: Bool, error: *mut NSError| {
            let result = if verified.as_bool() {
                OwnerCheckResult::Verified
            } else {
                // SAFETY: LocalAuthentication passes a valid NSError, or nil, that lives for the reply's call.
                let code = unsafe { error.as_ref() }.map(NSError::code);
                if let Some(code) = code {
                    tracing::warn!(code, "os_login: the device owner sheet did not verify");
                }
                code.map_or(OwnerCheckResult::Unavailable, map_la_error)
            };
            let _ = send.send(result);
        });
        let reason = NSString::from_str(reason);
        // SAFETY: the context, the reason and the reply are alive for the call; LocalAuthentication copies the reply
        // and calls it once. The context stays alive below until the reply has come, as LAContext requires.
        unsafe { context.evaluatePolicy_localizedReason_reply(POLICY, &reason, &reply) };
        let result = answer.recv().unwrap_or(OwnerCheckResult::Unavailable);
        drop(context);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_errors_map_to_the_callers_words() {
        for cancelled in [
            code::USER_CANCEL,
            code::SYSTEM_CANCEL,
            code::APP_CANCEL,
            code::AUTHENTICATION_FAILED,
        ] {
            assert_eq!(map_la_error(cancelled), OwnerCheckResult::Cancelled, "{cancelled}");
        }
        assert_eq!(map_la_error(code::BIOMETRY_LOCKOUT), OwnerCheckResult::LockedOut);
        for unavailable in [
            code::PASSCODE_NOT_SET,
            code::NOT_INTERACTIVE,
            code::BIOMETRY_NOT_AVAILABLE,
            code::BIOMETRY_NOT_ENROLLED,
            code::USER_FALLBACK,
            code::INVALID_CONTEXT,
            0,
            -11,
            -99_999,
        ] {
            assert_eq!(map_la_error(unavailable), OwnerCheckResult::Unavailable, "{unavailable}");
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn the_codes_are_local_authentications_own() {
        use objc2_local_authentication::LAError;
        for (ours, theirs) in [
            (code::AUTHENTICATION_FAILED, LAError::AuthenticationFailed),
            (code::USER_CANCEL, LAError::UserCancel),
            (code::USER_FALLBACK, LAError::UserFallback),
            (code::SYSTEM_CANCEL, LAError::SystemCancel),
            (code::PASSCODE_NOT_SET, LAError::PasscodeNotSet),
            (code::BIOMETRY_NOT_AVAILABLE, LAError::BiometryNotAvailable),
            (code::BIOMETRY_NOT_ENROLLED, LAError::BiometryNotEnrolled),
            (code::BIOMETRY_LOCKOUT, LAError::BiometryLockout),
            (code::APP_CANCEL, LAError::AppCancel),
            (code::INVALID_CONTEXT, LAError::InvalidContext),
            (code::NOT_INTERACTIVE, LAError::NotInteractive),
        ] {
            assert_eq!(ours, theirs.0);
        }
    }

    /// Real OS call, outside the test-environment rule (this module has none of its own), with no prompt. Ignored in
    /// CI; run on a Devin Mac with `cargo test -p <app crate> os_login::apple -- --ignored real_owner_check
    /// --nocapture`. A Mac with an account password answers `DeviceOwner`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    #[ignore = "real OS call; run by hand on a Mac"]
    fn real_owner_check() {
        let check = owner_check();
        println!("owner_check={check:?}");
        #[cfg(target_os = "macos")]
        assert_eq!(check, super::super::OwnerCheck::DeviceOwner, "a Mac with a password set");
    }

    /// Shows the system sheet and waits for a person. Run by hand only, with `-- --ignored real_prove_owner`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    #[ignore = "shows the system sheet; run by hand with a person at the Mac"]
    fn real_prove_owner() {
        println!(
            "prove_owner={:?}",
            prove_owner("move your tax profiles to your Buwiz account")
        );
    }
}
