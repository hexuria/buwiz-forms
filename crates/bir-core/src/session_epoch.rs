//! The session epoch, mirrored here so bir-core code — the cron, the OAuth server, the account
//! session — can compare a captured epoch without taking the app's session lock (the design page,
//! §1). A captured epoch means "I act for the session that existed when I started"; a mismatch
//! means that session is gone and the action must not happen.
//!
//! Written only by the app's session, under its lock. bir-core never writes it.

use std::sync::atomic::{AtomicU64, Ordering};

static CURRENT: AtomicU64 = AtomicU64::new(0);

/// The live epoch. Compared, never written, outside the session.
pub fn current() -> u64 {
    CURRENT.load(Ordering::SeqCst)
}

/// The session's one writer, called under its lock with the epoch already bumped.
pub fn set(epoch: u64) {
    CURRENT.store(epoch, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_mirror_reads_back_what_was_set() {
        let before = super::current();
        super::set(before + 41);
        assert_eq!(super::current(), before + 41);
        super::set(before);
    }
}
