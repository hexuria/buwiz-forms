//! Slow waits for, and long holds of, the process database lock.
//!
//! The desktop app runs every host call on its UI thread and the submission
//! worker shares the same `Mutex<Database>`, so a long hold anywhere is a
//! frozen window. Nobody has caught which hold that is; these warnings name
//! the call site of every wait or hold past [`SLOW`], so the next freeze
//! names its cause in the log. Nothing is logged below the threshold.

use std::ops::{Deref, DerefMut};
use std::panic::Location;
use std::sync::{LockResult, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Past this a user feels the window stop.
pub const SLOW: Duration = Duration::from_millis(250);

/// A [`MutexGuard`] that warns on drop when it was held past [`SLOW`].
///
/// Listed in this crate's `clippy.toml` so `await_holding_invalid_type`
/// refuses one held across an `.await`, as `await_holding_lock` does for
/// the bare guard it wraps.
pub struct TimedGuard<'a, T: ?Sized> {
    guard: MutexGuard<'a, T>,
    acquired: Instant,
    site: &'static Location<'static>,
}

/// Lock `mutex`, warning when the wait or, on drop, the hold passes
/// [`SLOW`]. The warning names the caller's file and line.
#[track_caller]
pub fn lock<T: ?Sized>(mutex: &Mutex<T>) -> LockResult<TimedGuard<'_, T>> {
    let site = Location::caller();
    let start = Instant::now();
    let locked = mutex.lock();
    let acquired = Instant::now();
    warn_if_slow(site, acquired - start, "waited for the database lock");
    let timed = |guard| TimedGuard {
        guard,
        acquired,
        site,
    };
    match locked {
        Ok(guard) => Ok(timed(guard)),
        Err(poisoned) => Err(PoisonError::new(timed(poisoned.into_inner()))),
    }
}

/// Run `read`, warning when it takes past [`SLOW`]: for work that takes no
/// lock but still runs on the UI thread, such as a read-only connection.
#[track_caller]
pub fn time<R>(read: impl FnOnce() -> R) -> R {
    let site = Location::caller();
    let start = Instant::now();
    let result = read();
    warn_if_slow(site, start.elapsed(), "database read was slow");
    result
}

fn warn_if_slow(site: &Location<'_>, took: Duration, what: &str) {
    if took > SLOW {
        tracing::warn!(
            site = %site,
            millis = u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
            "{what}"
        );
    }
}

impl<T: ?Sized> Deref for TimedGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T: ?Sized> DerefMut for TimedGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T: ?Sized> Drop for TimedGuard<'_, T> {
    fn drop(&mut self) {
        warn_if_slow(self.site, self.acquired.elapsed(), "held the database lock");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};

    /// Every warning emitted on this thread while `body` runs, as text.
    fn captured(body: impl FnOnce()) -> String {
        #[derive(Clone, Default)]
        struct Buffer(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Buffer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let buffer = Buffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, body);
        String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn a_long_hold_is_logged_with_its_call_site() {
        let mutex = Mutex::new(0);
        let log = captured(|| {
            let _guard = lock(&mutex).unwrap();
            std::thread::sleep(SLOW + Duration::from_millis(50));
        });
        assert!(log.contains("held the database lock"), "{log}");
        assert!(log.contains("lock_timing.rs"), "names the caller: {log}");
        assert!(!log.contains("waited"), "{log}");
    }

    #[test]
    fn a_long_wait_is_logged_with_its_call_site() {
        let mutex = Arc::new(Mutex::new(0));
        let (held_tx, held_rx) = mpsc::channel();
        let holder = {
            let mutex = mutex.clone();
            std::thread::spawn(move || {
                let _guard = mutex.lock().unwrap();
                held_tx.send(()).unwrap();
                std::thread::sleep(SLOW + Duration::from_millis(50));
            })
        };
        held_rx.recv().unwrap();
        let log = captured(|| drop(lock(&mutex).unwrap()));
        holder.join().unwrap();
        assert!(log.contains("waited for the database lock"), "{log}");
        assert!(log.contains("lock_timing.rs"), "names the caller: {log}");
    }

    #[test]
    fn a_short_lock_logs_nothing() {
        let mutex = Mutex::new(0);
        let log = captured(|| {
            *lock(&mutex).unwrap() += 1;
            time(|| ());
        });
        assert_eq!(log, "");
    }

    #[test]
    fn a_slow_read_is_logged_with_its_call_site() {
        let log = captured(|| time(|| std::thread::sleep(SLOW + Duration::from_millis(50))));
        assert!(log.contains("database read was slow"), "{log}");
        assert!(log.contains("lock_timing.rs"), "names the caller: {log}");
    }
}
