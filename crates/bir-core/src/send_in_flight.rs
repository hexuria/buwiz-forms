//! "A send is in flight": while the app itself carries a return to the BIR
//! (the Buwiz relay pipes it through this device), a phone must not lock
//! its screen or freeze the app, or the upload stalls. The embedding app
//! registers one hook ([`set_hook`]) that keeps the phone awake and asks
//! the system for time; the send holds a [`SendInFlight`] guard.
//!
//! Sends may overlap: the hook hears `true` once, at the first begin, and
//! `false` once, when the last guard goes. A guard ends on drop, so an
//! early return, a timeout or a panic still ends it. With no hook
//! registered (the desktops) nothing happens.

use std::sync::{Mutex, OnceLock, PoisonError};

/// The hook: `true` when the first send begins, `false` when the last ends.
/// It runs under the signal's lock, so begins and ends reach it in order;
/// it must only hand the change on (to the main thread), never block on a
/// send or begin one.
pub type Hook = fn(bool);

/// The count of sends in flight and the hook told of its edges.
pub struct Signal {
    hook: OnceLock<Hook>,
    count: Mutex<usize>,
}

impl Signal {
    pub const fn new() -> Self {
        Self {
            hook: OnceLock::new(),
            count: Mutex::new(0),
        }
    }

    /// Registers the hook, once. `false` when one was already registered.
    pub fn set_hook(&self, hook: Hook) -> bool {
        self.hook.set(hook).is_ok()
    }

    /// A send begins; it ends when the guard is dropped.
    pub fn begin(&'static self) -> SendInFlight {
        self.step(true);
        SendInFlight { signal: self }
    }

    /// How many sends are in flight.
    pub fn count(&self) -> usize {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn step(&self, begin: bool) {
        // A hook that panicked once must not stop later sends from ending.
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        let edge = if begin {
            *count += 1;
            *count == 1
        } else {
            *count = count.saturating_sub(1);
            *count == 0
        };
        if edge && let Some(hook) = self.hook.get() {
            hook(begin);
        }
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

/// One send in flight. Ends on drop.
#[must_use = "the send ends when the guard is dropped"]
pub struct SendInFlight {
    signal: &'static Signal,
}

impl Drop for SendInFlight {
    fn drop(&mut self) {
        self.signal.step(false);
    }
}

static SENDS: Signal = Signal::new();

/// Registers the process's hook (the app does, at startup, on phones).
pub fn set_hook(hook: Hook) -> bool {
    SENDS.set_hook(hook)
}

/// A send of the process begins.
pub fn begin() -> SendInFlight {
    SENDS.begin()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test has its own signal and its own record, so tests running at once do not mix.
    macro_rules! recording {
        ($signal:ident, $seen:ident) => {
            static $seen: Mutex<Vec<bool>> = Mutex::new(Vec::new());
            static $signal: Signal = Signal::new();
            fn hook(on: bool) {
                $seen.lock().unwrap().push(on);
            }
            assert!($signal.set_hook(hook));
            assert!(!$signal.set_hook(hook), "one hook only");
        };
    }

    #[test]
    fn one_send_tells_the_hook_on_then_off() {
        recording!(SIGNAL, SEEN);
        let guard = SIGNAL.begin();
        assert_eq!(*SEEN.lock().unwrap(), vec![true]);
        assert_eq!(SIGNAL.count(), 1);
        drop(guard);
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false]);
        assert_eq!(SIGNAL.count(), 0);
    }

    #[test]
    fn nested_sends_tell_the_hook_once_each_way() {
        recording!(SIGNAL, SEEN);
        {
            let _outer = SIGNAL.begin();
            {
                let _inner = SIGNAL.begin();
                assert_eq!(SIGNAL.count(), 2);
            }
            assert_eq!(*SEEN.lock().unwrap(), vec![true]);
        }
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false]);
    }

    #[test]
    fn overlapping_sends_end_when_the_last_one_ends() {
        recording!(SIGNAL, SEEN);
        let first = SIGNAL.begin();
        let second = SIGNAL.begin();
        drop(first);
        assert_eq!(*SEEN.lock().unwrap(), vec![true]);
        let third = SIGNAL.begin();
        drop(second);
        drop(third);
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false]);
        // A later send starts a new span.
        drop(SIGNAL.begin());
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false, true, false]);
    }

    #[test]
    fn sends_on_many_threads_still_make_one_span() {
        recording!(SIGNAL, SEEN);
        let outer = SIGNAL.begin();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..100 {
                        drop(SIGNAL.begin());
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        drop(outer);
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false]);
    }

    #[test]
    fn a_send_that_panics_still_ends() {
        recording!(SIGNAL, SEEN);
        let result = std::panic::catch_unwind(|| {
            let _guard = SIGNAL.begin();
            panic!("the send failed");
        });
        assert!(result.is_err());
        assert_eq!(*SEEN.lock().unwrap(), vec![true, false]);
        assert_eq!(SIGNAL.count(), 0);
    }

    #[test]
    fn without_a_hook_a_send_changes_nothing_but_the_count() {
        static SIGNAL: Signal = Signal::new();
        let guard = SIGNAL.begin();
        assert_eq!(SIGNAL.count(), 1);
        drop(guard);
        assert_eq!(SIGNAL.count(), 0);
    }
}
