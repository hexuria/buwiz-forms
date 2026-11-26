//! Backoff for wrong sign-in codes, one count per account.
//!
//! The wait lengths of the retired UI host's `components::rate_limiter`: after
//! every six wrong codes in a row, a wait of 1 minute, then 5, 15, 30, 1 hour
//! and 24 hours. A right code clears the count.
//!
//! A client's count (one taxpayer's PIN and authenticator, [`guarded_client`])
//! is kept in `device.db` (accounts-and-lock PLAN §B.2,
//! `bir_core::owner_lock`), so quitting the app does not reset it. The hash runs,
//! then its result is committed, then the page is answered. The admin's and the recovery key's counts still
//! live in memory, like the original limiter's, until the legacy lock goes
//! (A3a-2, B3).
//!
//! The host checks it rather than the script, so every way the UI asks for a
//! code (sign-in, the admin override, the recovery key) waits the same.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use bir_core::owner_lock::{self, Attempt, AttemptKey, GuardError, lockout_message};
use gpui_shell::HostError;

#[derive(Default)]
struct Limiter {
    failed: u32,
    locked_until: Option<Instant>,
}

impl Limiter {
    fn remaining(&self, now: Instant) -> Option<Duration> {
        self.locked_until
            .filter(|until| now < *until)
            .map(|until| until - now)
    }

    fn record_failure(&mut self, now: Instant) {
        self.failed += 1;
        // The one schedule, shared with the persisted limit.
        if let Some(wait) = owner_lock::wait_after(i64::from(self.failed)) {
            self.locked_until = Some(now + Duration::from_secs(wait as u64));
        }
    }
}

thread_local! {
    static LIMITERS: RefCell<HashMap<String, Limiter>> = RefCell::new(HashMap::new());
}

/// The admin's PIN and authenticator code share one count.
pub(crate) const ADMIN: &str = "admin";
/// The recovery key keeps its own count, so wrong admin PINs cannot lock out
/// the way back in.
pub(crate) const RECOVERY_KEY: &str = "recovery-key";

/// One taxpayer's PIN and authenticator code share one count (the in-memory
/// key, used only when `device.db` cannot be written: [`guarded_client`]).
fn taxpayer(tin_root: &str) -> String {
    format!("taxpayer:{tin_root}")
}

/// Runs `check` for the client `tin_root` under its persisted count in
/// `device.db`, for the room open on this thread (or the legacy file's
/// clients while none is), in the order hash, commit, answer (PLAN §B.2, L5):
/// a running wait refuses; then the check; then its result is committed (a
/// match clears the count, a miss counts, and the miss that starts a wait says
/// so); only then the answer. A `check` that fails commits nothing.
///
/// Where `device.db` cannot be opened before the check (missing while rooms are
/// on disk, unreadable), the count falls back to this process's memory, as
/// before B1: no weaker than today, and the client is not locked out by a
/// broken store. A result that cannot be committed after the check gives no
/// answer.
pub(crate) fn guarded_client(
    tin_root: &str,
    check: impl FnOnce() -> Result<bool, HostError>,
) -> Result<bool, HostError> {
    let store = bir_core::rooms::device_store_here();
    let key = AttemptKey::client(bir_core::db::open_room_id().as_ref(), tin_root);
    let mut check = Some(check);
    let attempt = owner_lock::guarded(&store, &key, &bir_core::boot_clock::read(), || {
        check.take().map_or(Ok(false), |check| check())
    });
    match attempt {
        Ok(attempt) => answer(attempt),
        Err(GuardError::Check(error)) => Err(error),
        Err(GuardError::Store(error)) => {
            tracing::warn!(
                "client PIN count kept in memory: the device store is not usable: {error}"
            );
            match check.take() {
                Some(check) => guarded(&taxpayer(tin_root), check),
                None => Err(HostError::new(error.to_string())),
            }
        }
    }
}

/// The page's answer to a guarded try: a wait (running, or started by this
/// miss) is an error that says how long.
pub(crate) fn answer(attempt: Attempt) -> Result<bool, HostError> {
    match attempt {
        Attempt::Matched => Ok(true),
        Attempt::Missed { wait: None } => Ok(false),
        Attempt::Missed { wait: Some(wait) } | Attempt::Waiting(wait) => {
            Err(HostError::new(lockout_message(wait)))
        }
    }
}

/// Runs `check` for `account` unless it is waiting out a lockout. A match
/// clears the count; a miss counts, and the miss that starts a wait says so.
/// A `check` that fails (the database is unreachable) does not count.
pub(crate) fn guarded(
    account: &str,
    check: impl FnOnce() -> Result<bool, HostError>,
) -> Result<bool, HostError> {
    guarded_at(account, Instant::now(), check)
}

fn guarded_at(
    account: &str,
    now: Instant,
    check: impl FnOnce() -> Result<bool, HostError>,
) -> Result<bool, HostError> {
    let waiting = LIMITERS.with(|all| {
        all.borrow()
            .get(account)
            .and_then(|limiter| limiter.remaining(now))
    });
    if let Some(wait) = waiting {
        return Err(HostError::new(lockout_message(wait)));
    }
    let matched = check()?;
    LIMITERS.with(|all| {
        let mut all = all.borrow_mut();
        if matched {
            all.remove(account);
            return Ok(true);
        }
        let limiter = all.entry(account.to_owned()).or_default();
        limiter.record_failure(now);
        match limiter.remaining(now) {
            Some(wait) => Err(HostError::new(lockout_message(wait))),
            None => Ok(false),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrong() -> Result<bool, HostError> {
        Ok(false)
    }
    fn right() -> Result<bool, HostError> {
        Ok(true)
    }

    #[test]
    fn six_wrong_codes_start_a_one_minute_wait() {
        let start = Instant::now();
        for _ in 0..5 {
            assert!(!guarded_at("a", start, wrong).unwrap());
        }
        let sixth = guarded_at("a", start, wrong).unwrap_err().to_string();
        assert!(sixth.contains("Try again in 1:00."), "{sixth}");

        // Even the right code waits, and the wait does not count as a try.
        let during = guarded_at("a", start + Duration::from_secs(30), right).unwrap_err();
        assert!(
            during.to_string().contains("Try again in 0:30."),
            "{during}"
        );

        let after = start + Duration::from_secs(61);
        assert!(guarded_at("a", after, right).unwrap());
        assert!(
            !guarded_at("a", after, wrong).unwrap(),
            "a match clears the count"
        );
    }

    #[test]
    fn waits_grow_per_window_and_accounts_count_apart() {
        let mut now = Instant::now();
        for _ in 0..6 {
            let _ = guarded_at("b", now, wrong);
        }
        now += Duration::from_secs(61);
        for _ in 0..5 {
            assert!(!guarded_at("b", now, wrong).unwrap());
        }
        let twelfth = guarded_at("b", now, wrong).unwrap_err().to_string();
        assert!(twelfth.contains("Try again in 5:00."), "{twelfth}");

        assert!(
            !guarded_at("c", now, wrong).unwrap(),
            "another account is not locked"
        );
    }

    #[test]
    fn a_failed_check_does_not_count() {
        let now = Instant::now();
        for _ in 0..10 {
            assert!(guarded_at("d", now, || Err(HostError::new("locked"))).is_err());
        }
        assert!(!guarded_at("d", now, wrong).unwrap());
    }

    #[test]
    fn long_waits_read_in_hours() {
        assert_eq!(
            lockout_message(Duration::from_secs(86_400)),
            "Too many wrong tries. Try again in 24 h 00 min."
        );
    }
}
