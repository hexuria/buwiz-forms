//! Backoff for wrong sign-in codes, one count per account.
//!
//! The wait lengths of the painted app's `components::rate_limiter`: after
//! every six wrong codes in a row, a wait of 1 minute, then 5, 15, 30, 1 hour
//! and 24 hours. A right code clears the count. Like the painted app's, the
//! count lives in memory, so quitting the app resets it.
//!
//! The host checks it rather than the script, so every way the UI asks for a
//! code (sign-in, the admin override, the recovery key) waits the same.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui_shell::HostError;

/// The wait after the 6th, 12th, 18th... wrong code in a row; the last one
/// repeats from then on.
const LOCKOUT_WAITS: [Duration; 6] = [
    Duration::from_secs(60),
    Duration::from_secs(300),
    Duration::from_secs(900),
    Duration::from_secs(1800),
    Duration::from_secs(3600),
    Duration::from_secs(86_400),
];

const ATTEMPTS_PER_WAIT: u32 = 6;

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
        if !self.failed.is_multiple_of(ATTEMPTS_PER_WAIT) {
            return;
        }
        let window = (self.failed / ATTEMPTS_PER_WAIT) as usize - 1;
        self.locked_until = Some(now + LOCKOUT_WAITS[window.min(LOCKOUT_WAITS.len() - 1)]);
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

/// One taxpayer's PIN and authenticator code share one count.
pub(crate) fn taxpayer(tin_root: &str) -> String {
    format!("taxpayer:{tin_root}")
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

/// "Too many wrong tries. Try again in 1:00." (minutes:seconds, or hours and
/// minutes for the long waits).
fn lockout_message(wait: Duration) -> String {
    let secs = wait.as_secs().max(1);
    let left = if secs >= 3600 {
        format!("{} h {:02} min", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    };
    format!("Too many wrong tries. Try again in {left}.")
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
