//! Exponential backoff rate limiter for authentication attempts.
//!
//! Used by lock screen, admin auth overlay, and profile PIN gate.
//! State is in-memory only — restarting the app resets the limiter.

use std::time::{Duration, Instant};

/// Cooldown durations for each 6-attempt window.
const LOCKOUT_TIERS: &[Duration] = &[
    Duration::from_secs(0),     // attempts 1–6: no lockout
    Duration::from_secs(60),    // attempts 7–12: 1 minute
    Duration::from_secs(300),   // attempts 13–18: 5 minutes
    Duration::from_secs(900),   // attempts 19–24: 15 minutes
    Duration::from_secs(1800),  // attempts 25–30: 30 minutes
    Duration::from_secs(3600),  // attempts 31–36: 1 hour
    Duration::from_secs(86400), // attempts 37+: 24 hours
];

const ATTEMPTS_PER_TIER: u32 = 6;

pub struct RateLimiter {
    failed_attempts: u32,
    locked_until: Option<Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            failed_attempts: 0,
            locked_until: None,
        }
    }

    /// Record a failed authentication attempt. Applies exponential lockout
    /// after every `ATTEMPTS_PER_TIER` consecutive failures.
    pub fn record_failure(&mut self) {
        self.failed_attempts += 1;

        // Which tier are we in? (0-indexed)
        let tier = (self.failed_attempts.saturating_sub(1) / ATTEMPTS_PER_TIER) as usize;
        let cooldown = LOCKOUT_TIERS
            .get(tier)
            .copied()
            .unwrap_or(*LOCKOUT_TIERS.last().unwrap());

        if !cooldown.is_zero() && self.failed_attempts.is_multiple_of(ATTEMPTS_PER_TIER) {
            self.locked_until = Some(Instant::now() + cooldown);
        }
    }

    /// Reset all state on successful authentication.
    pub fn reset(&mut self) {
        self.failed_attempts = 0;
        self.locked_until = None;
    }

    /// Whether the limiter is currently in a lockout period.
    pub fn is_locked(&self) -> bool {
        self.locked_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    /// Seconds remaining in the current lockout, or `None` if not locked.
    pub fn remaining_lockout_secs(&self) -> Option<u64> {
        self.locked_until.and_then(|until| {
            let now = Instant::now();
            if now < until {
                Some((until - now).as_secs())
            } else {
                None
            }
        })
    }

    /// Total failed attempts since last reset.
    #[allow(dead_code)]
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// Human-readable lockout message, e.g. "Try again in 4:32".
    pub fn lockout_message(&self) -> Option<String> {
        self.remaining_lockout_secs().map(|secs| {
            if secs >= 3600 {
                let h = secs / 3600;
                let m = (secs % 3600) / 60;
                format!("Too many attempts. Try again in {}h {}m", h, m)
            } else if secs >= 60 {
                let m = secs / 60;
                let s = secs % 60;
                format!("Too many attempts. Try again in {}:{:02}", m, s)
            } else {
                format!("Too many attempts. Try again in {}s", secs)
            }
        })
    }
}
