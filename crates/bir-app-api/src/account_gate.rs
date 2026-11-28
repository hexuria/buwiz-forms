//! Whether this process may read the filing database.
//!
//! The welcome screen is not the room lock. Host verbs that touch taxpayer
//! data call [`ensure_unlocked`], which requires a signed-in account, an open
//! room and the host's own unlock of that room. Verbs of the lock screen call
//! [`ensure_signed_in`], the account gate as it was before the room lock. A
//! debug test bypass exists so `cargo test` and the demo launcher can open a
//! scratch database; release builds compile that check out.

use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_shell::HostError;

static UNLOCKED: AtomicBool = AtomicBool::new(false);
static ROOM_UNLOCKED: AtomicBool = AtomicBool::new(false);
static CLIENT_SCOPE: Mutex<Option<String>> = Mutex::new(None);
static CLIENT_SCOPE_ROOM: Mutex<Option<std::sync::Weak<std::sync::Mutex<bir_core::db::Database>>>> =
    Mutex::new(None);

thread_local! {
    static FORCE_LOCKED: Cell<bool> = const { Cell::new(false) };
}

pub const LOCKED_MESSAGE: &str = "Sign in to Buwiz before using eBIRForms.";

/// Both flags are required. Either one alone does nothing, so a test database
/// (`EBIR_TEST_ENV=1`) does not silently skip the account gate.
#[cfg(any(debug_assertions, test))]
pub fn bypass_from_flags(test_env: bool, auth_bypass: bool) -> bool {
    test_env && auth_bypass
}

/// Exactly `1`. Deliberately stricter than `bir_core::db::test_environment`
/// (`1`, `true`, `yes`, `on`), which every other "is this a test run" reader
/// calls: this one guards the sign-in bypass, so it is not loosened to that list.
#[cfg(debug_assertions)]
fn env_flag(name: &str) -> bool {
    bir_core::config::var(name).ok().as_deref() == Some("1")
}

#[cfg(debug_assertions)]
pub fn auth_bypass_enabled() -> bool {
    bypass_from_flags(env_flag("EBIR_TEST_ENV"), env_flag("BIR_AUTH_BYPASS"))
}

#[cfg(not(debug_assertions))]
pub fn auth_bypass_enabled() -> bool {
    false
}

/// Whether the account is signed in. The submission worker uses this meaning:
/// it keeps sending a queued filing while the room is locked.
pub fn is_unlocked() -> bool {
    if FORCE_LOCKED.with(Cell::get) {
        return false;
    }
    auth_bypass_enabled() || UNLOCKED.load(Ordering::SeqCst)
}

/// What sign-in and sign-out set, without the test bypass or the force-locked flag that
/// [`is_unlocked`] adds. The machine's invariants read this: they are about what the session
/// wrote, never about the bypass outside it.
pub(crate) fn unlocked_flag() -> bool {
    UNLOCKED.load(Ordering::SeqCst)
}

pub fn set_unlocked(unlocked: bool) {
    UNLOCKED.store(unlocked, Ordering::SeqCst);
    if !unlocked {
        clear_room_unlocked();
        // Signing out leaves the gate. Filing menus must not survive into it.
        crate::hooks::hooks().set_shell_chrome_open(false);
    }
    // The worker may already be running from an earlier sign-in in this
    // process. Pausing it stops the next filing tick without the one-way
    // shutdown flag, so a later sign-in can file again.
    let paused = !is_unlocked();
    bir_core::background_cron::set_submission_paused(paused);
}

/// Whether the host's own checks unlocked the open room in this process.
pub fn room_unlocked() -> bool {
    ROOM_UNLOCKED.load(Ordering::SeqCst)
}

pub fn set_room_unlocked(unlocked: bool) {
    ROOM_UNLOCKED.store(unlocked, Ordering::SeqCst);
}

pub fn clear_room_unlocked() {
    ROOM_UNLOCKED.store(false, Ordering::SeqCst);
    clear_client_scope();
}

/// The taxpayer whose own PIN or authenticator opened a client session.
pub fn client_scope() -> Option<String> {
    CLIENT_SCOPE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn set_client_scope(
    tin_root: String,
    room: &std::sync::Arc<std::sync::Mutex<bir_core::db::Database>>,
) {
    *CLIENT_SCOPE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(tin_root);
    *CLIENT_SCOPE_ROOM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(std::sync::Arc::downgrade(room));
}

/// The taxpayer scope only while the room it was checked against is still the open handle.
pub(crate) fn client_scope_for_room(
    room: &std::sync::Arc<std::sync::Mutex<bir_core::db::Database>>,
) -> Option<String> {
    let matches = CLIENT_SCOPE_ROOM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .and_then(std::sync::Weak::upgrade)
        .is_some_and(|scoped| std::sync::Arc::ptr_eq(&scoped, room));
    matches.then(client_scope).flatten()
}

pub fn clear_client_scope() {
    *CLIENT_SCOPE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    *CLIENT_SCOPE_ROOM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

/// The account gate as it was before the room lock: signed in, with no filing-data or room check.
pub fn ensure_signed_in() -> Result<(), HostError> {
    if is_unlocked() {
        Ok(())
    } else {
        Err(HostError::new(LOCKED_MESSAGE))
    }
}

/// The one filing-data checkpoint: signed in, a room open and this process's
/// host-checked room unlock. A room with no lock configured is opened by the
/// host here, so first launch, the store-review account and a legacy file with
/// no access mode keep working.
pub fn ensure_unlocked() -> Result<(), HostError> {
    ensure_signed_in()?;
    if !crate::bir_module::room_is_open() {
        return Err(HostError::new(LOCKED_MESSAGE));
    }
    if !room_unlocked() && crate::bir_module::open_room_has_no_lock_configured()? {
        set_room_unlocked(true);
    }
    let owner_pin_open = crate::session::snapshot()
        .room
        .and_then(|(_, _, room, _)| room)
        .or_else(bir_core::db::open_room_id)
        .is_some_and(|room| crate::owner_lock_module::is_unlocked(&room));
    if room_unlocked() || owner_pin_open {
        Ok(())
    } else {
        Err(HostError::new(LOCKED_MESSAGE))
    }
}

/// Refuse data verbs on this thread even when the process bypass is set.
/// Other tests keep the bypass; cargo runs them in parallel.
#[cfg(any(test, feature = "test-support"))]
pub fn force_locked<R>(body: impl FnOnce() -> R) -> R {
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
            assert!(ensure_signed_in().is_err());
        });
        assert!(is_unlocked());
    }
}
