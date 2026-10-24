//! Whether this process may read the filing database.
//!
//! The welcome screen is not the lock. Host verbs that touch taxpayer data
//! call [`ensure_unlocked`] and refuse while the Buwiz account is signed out.
//! A debug test bypass exists so `cargo test` and the demo launcher can open
//! a scratch database; release builds compile that check out.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_shell::HostError;

static UNLOCKED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static FORCE_LOCKED: Cell<bool> = const { Cell::new(false) };
}

pub const LOCKED_MESSAGE: &str = "Sign in to Buwiz before using this computer.";

/// Both flags are required. Either one alone does nothing, so a test database
/// (`EBIR_TEST_ENV=1`) does not silently skip the account gate.
#[cfg(any(debug_assertions, test))]
pub fn bypass_from_flags(test_env: bool, auth_bypass: bool) -> bool {
    test_env && auth_bypass
}

#[cfg(debug_assertions)]
fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().as_deref() == Some("1")
}

#[cfg(debug_assertions)]
pub fn auth_bypass_enabled() -> bool {
    bypass_from_flags(env_flag("EBIR_TEST_ENV"), env_flag("BIR_AUTH_BYPASS"))
}

#[cfg(not(debug_assertions))]
pub fn auth_bypass_enabled() -> bool {
    false
}

pub fn is_unlocked() -> bool {
    if FORCE_LOCKED.with(Cell::get) {
        return false;
    }
    auth_bypass_enabled() || UNLOCKED.load(Ordering::SeqCst)
}

pub fn set_unlocked(unlocked: bool) {
    UNLOCKED.store(unlocked, Ordering::SeqCst);
    // Signing out leaves the gate. Filing menus must not survive into it.
    if !unlocked {
        crate::app_menu::set_shell_chrome_open(false);
    }
    // The worker may already be running from an earlier sign-in in this
    // process. Pausing it stops the next filing tick without the one-way
    // shutdown flag, so a later sign-in can file again.
    let paused = !is_unlocked();
    bir_core::background_cron::set_submission_paused(paused);
}

pub fn ensure_unlocked() -> Result<(), HostError> {
    if is_unlocked() {
        Ok(())
    } else {
        Err(HostError::new(LOCKED_MESSAGE))
    }
}

/// Refuse data verbs on this thread even when the process bypass is set.
/// Other tests keep the bypass; cargo runs them in parallel.
#[cfg(test)]
pub(crate) fn force_locked<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCE_LOCKED.with(|cell| cell.set(self.0));
        }
    }
    let previous = FORCE_LOCKED.with(Cell::get);
    FORCE_LOCKED.with(|cell| cell.set(true));
    let _restore = Restore(previous);
    body()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_needs_both_flags() {
        assert!(!bypass_from_flags(true, false));
        assert!(!bypass_from_flags(false, true));
        assert!(!bypass_from_flags(false, false));
        assert!(bypass_from_flags(true, true));
    }

    /// Release compiles `auth_bypass_enabled` to false. Debug clippy does not
    /// build this test; `cargo test --release` does.
    #[cfg(not(debug_assertions))]
    #[test]
    fn release_builds_ignore_the_bypass() {
        assert!(!auth_bypass_enabled());
    }

    #[test]
    fn force_locked_hides_an_unlocked_process() {
        set_unlocked(true);
        assert!(is_unlocked());
        force_locked(|| {
            assert!(!is_unlocked());
            assert!(ensure_unlocked().is_err());
        });
        assert!(is_unlocked());
    }
}
