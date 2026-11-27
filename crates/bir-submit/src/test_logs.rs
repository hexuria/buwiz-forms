//! Log capture for tests that holds up under parallel test threads.
//!
//! Capturing with a thread-local `tracing::subscriber::with_default` is
//! flaky: tracing caches each callsite's interest and the max level
//! globally, computed over whichever dispatchers are alive at that moment.
//! With parallel tests creating and dropping thread-local dispatchers (and
//! other tests logging with none at all), a callsite can be cached "never"
//! for a thread that is capturing, and its event is silently dropped.
//!
//! The cure is one process-wide subscriber, installed once, whose filter
//! decides per event: it says "sometimes" for every callsite, so nothing can
//! be cached away, and lets an event through only on a thread with an active
//! capture and at that capture's level or more severe. Each event is written
//! into the capturing thread's own buffer, so parallel tests never see each
//! other's lines.
//!
//! This module is kept identical in bir-core and bir-submit (bir-submit
//! cannot depend on bir-core); change both together.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::OnceLock;

use tracing::level_filters::LevelFilter;
use tracing::subscriber::{DefaultGuard, Interest};
use tracing::{Level, Metadata};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, Filter, SubscriberExt};

thread_local! {
    static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

struct Capture {
    max: Level,
    text: Vec<u8>,
}

/// Everything `body` logs on this thread at `max` or more severe, as the
/// app's fmt subscriber writes it without time or colour, e.g.
/// ` INFO some_crate::module: message key=value`.
pub(crate) fn capture_logs(max: Level, body: impl FnOnce()) -> String {
    let capture = start_capture(max);
    body();
    capture.finish()
}

/// Start capturing this thread's logs at `max` or more severe until the
/// returned guard is finished or dropped. For async tests on a current-thread
/// runtime, where every poll happens on this thread.
pub(crate) fn start_capture(max: Level) -> LogCapture {
    let fallback = if installed() {
        None
    } else {
        // Something else in this test binary owns the global subscriber, so
        // fall back to a thread-local one. That is exposed to the stale
        // interest cache described above; rebuilding it once the default is
        // in place narrows the window but cannot close it.
        Some(tracing::subscriber::set_default(subscriber()))
    };
    // Also repairs a callsite whose registration raced the global install
    // and so was cached against the dispatchers that existed before it.
    tracing::callsite::rebuild_interest_cache();
    let previous = CAPTURE.with(|slot| {
        slot.replace(Some(Capture {
            max,
            text: Vec::new(),
        }))
    });
    LogCapture {
        previous: Some(previous),
        _fallback: fallback,
        _thread_bound: PhantomData,
    }
}

/// An active capture on this thread; see [`start_capture`].
pub(crate) struct LogCapture {
    previous: Option<Option<Capture>>,
    _fallback: Option<DefaultGuard>,
    _thread_bound: PhantomData<*const ()>,
}

impl LogCapture {
    /// Stop capturing and return the text.
    pub(crate) fn finish(mut self) -> String {
        let capture = self.restore();
        String::from_utf8(capture.map(|capture| capture.text).unwrap_or_default()).unwrap()
    }

    fn restore(&mut self) -> Option<Capture> {
        let previous = self.previous.take()?;
        CAPTURE.with(|slot| slot.replace(previous))
    }
}

impl Drop for LogCapture {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Whether the process-wide capturing subscriber is the global default.
fn installed() -> bool {
    static INSTALLED: OnceLock<bool> = OnceLock::new();
    *INSTALLED.get_or_init(|| tracing::subscriber::set_global_default(subscriber()).is_ok())
}

fn subscriber() -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(|| ToCapture)
            .with_ansi(false)
            .without_time()
            .with_filter(Capturing),
    )
}

/// Lets an event through only on a thread with an active capture, at that
/// capture's level or more severe.
struct Capturing;

impl<S> Filter<S> for Capturing {
    fn enabled(&self, metadata: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        CAPTURE
            .try_with(|slot| {
                slot.borrow()
                    .as_ref()
                    .is_some_and(|capture| *metadata.level() <= capture.max)
            })
            .unwrap_or(false)
    }

    fn callsite_enabled(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(LevelFilter::TRACE)
    }
}

/// Appends to the current thread's capture; fmt writes on the emitting thread.
struct ToCapture;

impl std::io::Write for ToCapture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let _ = CAPTURE.try_with(|slot| {
            if let Some(capture) = slot.borrow_mut().as_mut() {
                capture.text.extend_from_slice(bytes);
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
