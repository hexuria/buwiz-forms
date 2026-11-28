//! The `bir` host module: the JavaScript-facing bridge into bir-core.
//!
//! Everything here crosses the boundary as `HostValue` — JSON-shaped records,
//! never Rust types. Functions are synchronous (main thread, no engine
//! re-entry); slow work (SFTP, exports) gets an `async_function` in a later
//! slice. The database opens lazily through `open_live_database` so
//! `BIR_DATABASE_PATH` keeps working for tests across both shell hosts.

use std::cell::RefCell;

use bir_core::db::{self, Database};
use bir_core::forms::FORM_REGISTRY;
use bir_core::forms::dry_run::is_dry_run;
use bir_core::forms::queueable::QueueableForm as _;
use gpui_shell::{HostError, HostModule, HostObject, HostResult, HostValue};

use crate::auth_limiter;

thread_local! {
    static DATABASE: RefCell<Option<std::sync::Arc<std::sync::Mutex<Database>>>> =
        const { RefCell::new(None) };
    /// The handle holds an in-memory placeholder because [`reset_database`]
    /// let go of the live file; the next call reopens the file into it.
    static RELEASED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The read-only connection behind [`with_reader`], opened with the
    /// writer and dropped with it. Only this thread reads through it, so it
    /// needs no mutex: that is what keeps its reads off the worker's lock.
    static READER: RefCell<Option<std::rc::Rc<Database>>> = const { RefCell::new(None) };
}

/// The shared handle behind [`with_database`], exposed for host functions
/// whose work must *release* the lock mid-call — the inbox poller re-locks
/// internally during an IMAP fetch, so `check_inbox_now` clones this Arc,
/// drops its guard, then polls.
///
/// The handle is never replaced afterwards (a database import reopens the
/// file *inside* it). [`open_database_at_launch`] starts the submission
/// worker on it once the account gate is open, so the worker sees the same
/// database as the page.
///
/// Open or reuse this engine's database. Does not check the account gate and
/// does not start the submission worker, except to finish a launch that found
/// the database held ([`open_database_at_launch`]) once the gate is open.
/// Cloud-account settings use this while the filing UI is still locked.
fn open_database_slot() -> Result<std::sync::Arc<std::sync::Mutex<Database>>, HostError> {
    let mut opened = false;
    let handle = DATABASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_ref() {
            None => {
                let (mut database, _path, _recovered) =
                    db::open_live_database().map_err(|error| HostError::new(error.to_string()))?;
                install_reader(&mut database);
                let handle = std::sync::Arc::new(std::sync::Mutex::new(database));
                // Listed before anything else can use it. (The open itself took the owner lock
                // earlier: `owner_lock_released` reads true until here.)
                LIVE_HANDLES
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(std::sync::Arc::downgrade(&handle));
                *slot = Some(handle);
                opened = true;
            }
            Some(handle) if RELEASED.get() => {
                let (mut database, _path, _recovered) =
                    db::open_live_database().map_err(|error| HostError::new(error.to_string()))?;
                install_reader(&mut database);
                *bir_core::lock_timing::lock(handle)
                    .map_err(|_| HostError::new("database lock poisoned"))? = database;
                RELEASED.set(false);
            }
            Some(_) => {}
        }
        Ok::<_, HostError>(slot.as_ref().expect("database opened").clone())
    })?;
    if opened {
        // A new handle: its own end, its own owner lock.
        ENGINE_SHUT_DOWN.with(|done| done.set(false));
        if LAUNCH_PENDING.with(std::cell::Cell::get) && crate::account_gate::is_unlocked() {
            launch_on(handle.clone());
        }
    }
    Ok(handle)
}

/// Open the read-only connection for `database`, just opened, in place of
/// any earlier one. One that fails to open is logged, and reads wait on the
/// writer's lock as they did before there was a reader.
fn install_reader(database: &mut Database) {
    let reader = database.open_reader().unwrap_or_else(|error| {
        tracing::error!(
            %error,
            "read-only connection not opened; reads wait on the submission worker's lock"
        );
        None
    });
    READER.with(|slot| *slot.borrow_mut() = reader.map(std::rc::Rc::new));
}

fn drop_reader() {
    READER.with(|slot| slot.borrow_mut().take());
}

/// The shared handle behind [`with_database`]. Refuses while the Buwiz
/// account gate is locked. The first unlocked open is what
/// [`open_database_at_launch`] uses to start the submission worker.
pub fn database_handle() -> Result<std::sync::Arc<std::sync::Mutex<Database>>, HostError> {
    crate::account_gate::ensure_unlocked()?;
    open_database_slot()
}

/// The submission worker that runs now: at most one per process, owned by
/// the engine (the thread with the database slot) that started it. Its
/// engine's end ([`shutdown_for_quit`]) stops it and empties the slot, so the
/// next engine of the same process (an Android relaunch, an iOS scene that
/// reconnects) starts one of its own.
static SUBMISSION_WORKER: std::sync::Mutex<Option<SubmissionWorker>> = std::sync::Mutex::new(None);
/// Every database handle an engine of this process opened; one that is still
/// alive holds the owner lock ([`owner_lock_released`]).
static LIVE_HANDLES: std::sync::Mutex<Vec<std::sync::Weak<std::sync::Mutex<Database>>>> =
    std::sync::Mutex::new(Vec::new());
static SUBMISSION_WORKER_REFUSED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static NEXT_ENGINE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// How long an engine's end waits for its worker to stop before it drops
/// what is left (the same bound a desktop quit always had).
const WORKER_STOP_BOUND: std::time::Duration = std::time::Duration::from_secs(5);
/// What dropping the worker's runtime may take once the bound is spent: tasks
/// are dropped, not waited for; this only covers a task that is inside a
/// blocking call at that moment.
const WORKER_DROP_GRACE: std::time::Duration = std::time::Duration::from_millis(250);
/// How long an engine's end waits for a holder outside the worker's runtime
/// (a blocking task that was mid-call when the runtime went) to let go of the
/// database before it falls back to "the owner lock releases when the process
/// exits".
const RELEASE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

thread_local! {
    /// This engine's number (0 until asked). No destructor, so the engine's
    /// end can read it while the thread's other locals are going.
    static ENGINE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// [`shutdown_for_quit`] already ran for the handle this engine opened
    /// last. Cleared when the engine opens a new handle.
    static ENGINE_SHUT_DOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The launch found the database held (an earlier engine or another
    /// copy still had it): the next open on this engine does what the launch
    /// could not, so "Try again" ends with a running worker.
    static LAUNCH_PENDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn this_engine() -> u64 {
    ENGINE.with(|engine| {
        if engine.get() == 0 {
            engine.set(NEXT_ENGINE.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        }
        engine.get()
    })
}

/// One running worker: its loop and every task it spawned live on `runtime`,
/// so shutting the runtime down drops the clones of the engine's database
/// handle they hold. Not every clone: work inside a blocking call at that
/// moment (the calendar sync's blocking task, an inbox poll) keeps its clone
/// until the call returns. Both check the worker's stop between items, the
/// calendar sync holds only a `Weak` between calendars, and an inbox read or
/// write gives up after 30 s; [`release_database`] waits a short while for them.
struct SubmissionWorker {
    engine: u64,
    stop: bir_core::background_cron::CronStop,
    runtime: tokio::runtime::Runtime,
    /// Answers (or disconnects) once the worker loop has returned.
    finished: std::sync::mpsc::Receiver<()>,
}

impl SubmissionWorker {
    /// The loop returned by itself (a [`bir_core::background_cron::request_shutdown`]).
    fn loop_ended(&self) -> bool {
        !matches!(
            self.finished.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        )
    }

    /// Ask the loop to stop, wait for it up to `WORKER_STOP_BOUND`, then shut
    /// the runtime down without waiting for its tasks: a send still in flight
    /// is abandoned (its claim and "upload started" are already recorded, so
    /// it is settled later and never sent again blind). Returns whether the
    /// runtime is gone, its tasks (and their database handles) with it.
    fn stop_and_drop(self) -> bool {
        let deadline = std::time::Instant::now() + WORKER_STOP_BOUND;
        self.stop.stop();
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
            self.finished.recv_timeout(WORKER_STOP_BOUND)
        {
            tracing::warn!(
                "submission worker did not stop within 5s; dropping it (a send in flight is abandoned)"
            );
        }
        let grace = deadline
            .saturating_duration_since(std::time::Instant::now())
            .max(WORKER_DROP_GRACE);
        // On a thread of its own: a runtime may not be shut down from inside
        // another runtime, and the engine's end can be inside one.
        let runtime = std::sync::Arc::new(std::sync::Mutex::new(Some(self.runtime)));
        let for_dropper = runtime.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let dropper = std::thread::Builder::new()
            .name("bir-submission-worker-drop".into())
            .spawn(move || {
                let taken = for_dropper
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                if let Some(runtime) = taken {
                    runtime.shutdown_timeout(grace);
                }
                let _ = tx.send(());
            });
        match dropper {
            Ok(_) => rx.recv_timeout(grace + WORKER_DROP_GRACE).is_ok(),
            Err(error) => {
                // Not dropped here: this thread may be inside a runtime, where
                // dropping one panics. Its tasks are left as they are.
                let taken = runtime
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                if let Some(runtime) = taken {
                    std::mem::forget(runtime);
                }
                tracing::warn!(
                    "submission worker not dropped: no thread to drop it on ({error}); its runtime is left running"
                );
                false
            }
        }
    }
}

/// The host refused sending for this process (the NSH door's send guard):
/// from now on no route starts the submission worker (the launch, a sign-in's
/// "session opened", anything that reaches [`open_database_at_launch`]). One
/// way: nothing clears it. A worker already running is not stopped; the host
/// sets this before anything opens the database.
pub fn refuse_submission_worker() {
    SUBMISSION_WORKER_REFUSED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Whether [`refuse_submission_worker`] was called in this process.
pub fn submission_worker_refused() -> bool {
    SUBMISSION_WORKER_REFUSED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Start `bir_core::background_cron` — queued submissions, their retries,
/// confirmation-inbox polls and scheduled jobs — on a Tokio runtime of its
/// own, owned by this engine. Nothing happens while a worker runs already
/// (this engine's, or another engine's that has not ended). It shares `db`,
/// the one owner-locked connection, rather than opening a second.
/// `BIR_SFTP_DRY_RUN` is read by the worker's transport, unchanged.
fn start_submission_worker(db: std::sync::Arc<std::sync::Mutex<Database>>) {
    if SUBMISSION_WORKER_REFUSED.load(std::sync::atomic::Ordering::SeqCst) {
        tracing::warn!("submission worker not started: the host refused sending in this process");
        return;
    }
    let ended = {
        let mut slot = SUBMISSION_WORKER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(live) = slot.as_ref().filter(|worker| !worker.loop_ended()) {
            tracing::info!(
                engine = live.engine,
                "submission worker not started: one runs already (this engine's, or one whose engine has not ended)"
            );
            return;
        }
        slot.take()
    };
    // Out of the slot and its lock before it is stopped.
    if let Some(ended) = ended {
        ended.stop_and_drop();
    }
    // 16 MiB stacks: the relay client's send future (crates/bir-submit) overflows a default stack in a debug
    // build. The worker's loop and its tasks run on the runtime's threads.
    const WORKER_STACK: usize = 16 * 1024 * 1024;
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("bir-submission-worker")
        .thread_stack_size(WORKER_STACK)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!("submission worker not started: no Tokio runtime: {error}");
            return;
        }
    };
    let mut slot = SUBMISSION_WORKER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if slot.is_some() {
        // Another engine started one while this one built its runtime.
        tracing::info!("submission worker not started: another engine started one meanwhile");
        runtime.shutdown_background();
        return;
    }
    let stop = bir_core::background_cron::CronStop::new();
    let (done, finished) = std::sync::mpsc::channel();
    let cron = bir_core::background_cron::run_cron_jobs(db, stop.clone());
    runtime.spawn(async move {
        cron.await;
        let _ = done.send(());
    });
    *slot = Some(SubmissionWorker {
        engine: this_engine(),
        stop,
        runtime,
        finished,
    });
}

/// What an end found in the worker slot.
enum TakenWorker {
    None,
    /// Out of the slot, its loop already told to stop.
    Taken(SubmissionWorker),
    /// Another engine's, left running: the owner's number.
    Others(u64),
}

/// Take the submission worker out of its slot and tell its loop to stop:
/// `engine`'s (an engine's end), or with `None` whichever runs (a quit or an
/// update's restart ends the process's sending, whoever started it). Another
/// engine's worker stays. Does not log (see [`shutdown_at_engine_end`]).
fn take_worker(engine: Option<u64>) -> TakenWorker {
    let mut slot = SUBMISSION_WORKER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match (slot.as_ref(), engine) {
        (None, _) => TakenWorker::None,
        (Some(other), Some(engine)) if other.engine != engine => TakenWorker::Others(other.engine),
        _ => match slot.take() {
            Some(worker) => {
                worker.stop.stop();
                TakenWorker::Taken(worker)
            }
            None => TakenWorker::None,
        },
    }
}

/// Wait for a taken worker to stop and drop it (see [`SubmissionWorker::stop_and_drop`]).
fn finish_worker(taken: TakenWorker, ended: u64) {
    match taken {
        TakenWorker::None => {}
        TakenWorker::Others(owner) => tracing::info!(
            owner,
            ended,
            "an engine without a submission worker of its own ended; the running one ends with its own engine"
        ),
        TakenWorker::Taken(worker) => {
            if !worker.stop_and_drop() {
                tracing::warn!(
                    "submission worker not dropped in time; what it holds goes when it ends"
                );
            }
        }
    }
}

/// Whether the submission worker that runs now is this engine's (the thread
/// that asks). For the host's own checks and tests.
pub fn submission_worker_runs_for_this_engine() -> bool {
    let engine = this_engine();
    SUBMISSION_WORKER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .is_some_and(|worker| worker.engine == engine && !worker.loop_ended())
}

/// Stop the submission worker, whichever engine started it, and release this
/// engine's database (and with it the owner lock).
///
/// Quit (the menu, Cmd/Ctrl+Q, `process.exit`) and an update install call
/// this; the end of an engine thread calls [`shutdown_at_engine_end`]. The
/// worker is asked to stop and given 5 s; what is left of it is then dropped,
/// a send in flight with it, and this thread checkpoints and closes. A second
/// call for the same handle does nothing. Safe when the database was never
/// opened.
///
/// An update install calls it before starting the new process, because the
/// owner lock is a process-wide flock and the replacement cannot open the
/// database until this process has dropped it.
pub fn shutdown_for_quit() {
    if ENGINE_SHUT_DOWN.with(|done| done.replace(true)) {
        return;
    }
    LAUNCH_PENDING.with(|pending| pending.set(false));
    finish_worker(take_worker(None), this_engine());
    release_taken(take_database_slot());
}

/// [`shutdown_for_quit`] at an engine thread's end (the NSH door's guard,
/// which runs while the thread's locals are being destroyed). Only taking
/// this engine's slots happens on this thread; stopping the worker and
/// closing the database run on a thread of their own, waited for. They log,
/// and a log line reads thread locals of the logger's own (the fmt layer's
/// buffer) that this thread may have destroyed already: a log line here
/// panics, the panic hook's own log line panics again, and the process aborts.
pub fn shutdown_at_engine_end() {
    // No destructors on these three, so they are still there.
    if ENGINE_SHUT_DOWN.with(|done| done.replace(true)) {
        return;
    }
    LAUNCH_PENDING.with(|pending| pending.set(false));
    let engine = this_engine();
    // Taken and told to stop here, before anything can fail: a loop told to
    // stop ends at its next check, and its tasks end with their sends.
    let worker = take_worker(Some(engine));
    let slot = take_database_slot();
    let work = std::sync::Arc::new(std::sync::Mutex::new(Some((worker, slot))));
    let for_thread = work.clone();
    let ended = std::thread::Builder::new()
        .name("bir-engine-end".into())
        .spawn(move || {
            let taken = for_thread
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            if let Some((worker, slot)) = taken {
                finish_worker(worker, engine);
                release_taken(slot);
            }
        });
    match ended {
        Ok(thread) => {
            let _ = thread.join();
        }
        Err(_) => {
            // No thread to finish on, and nothing can log here. The handle is
            // dropped (the owner lock goes with it when it was the last
            // holder). The worker's runtime is not dropped on this thread, which
            // may not drop one: it is left to its stopped loop and its tasks.
            let taken = work
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            if let Some((worker, slot)) = taken {
                if let TakenWorker::Taken(worker) = worker {
                    std::mem::forget(worker.runtime);
                }
                drop(slot);
            }
        }
    }
}

/// Make this thread's database slots before its end guard (the NSH door
/// calls this as an engine boots). Thread locals go in reverse order of their
/// first use, so slots made first are still there when the guard runs; one
/// first used after it (a launch that found the database held, opened by Try
/// again) would be gone already, and the guard could not close it.
pub fn prepare_engine() {
    DATABASE.with(|_| ());
    READER.with(|_| ());
}

/// What [`take_database_slot`] found in this thread's database slot.
enum TakenSlot {
    /// Destroyed before the engine's end ran (its handle dropped with it).
    Gone,
    Empty,
    Held(std::sync::Arc<std::sync::Mutex<Database>>),
}

/// Empty this thread's database slots, without logging (see
/// [`shutdown_at_engine_end`]). The reader is dropped here: it is this
/// thread's alone.
fn take_database_slot() -> TakenSlot {
    // `try_with`: at an engine thread's end a slot may already be gone, which
    // is not a reason to stop here.
    let _ = READER.try_with(|slot| slot.borrow_mut().take());
    match DATABASE.try_with(|slot| slot.borrow_mut().take()) {
        Err(_) => TakenSlot::Gone,
        Ok(None) => TakenSlot::Empty,
        Ok(Some(handle)) => {
            RELEASED.set(false);
            TakenSlot::Held(handle)
        }
    }
}

fn release_taken(slot: TakenSlot) {
    match slot {
        TakenSlot::Gone => {
            tracing::warn!(
                "database slot already gone at the engine's end; its handle went with it"
            );
        }
        TakenSlot::Empty => {}
        TakenSlot::Held(handle) => close_handle(handle),
    }
}

/// Close `handle` once this is its last holder, waiting up to `RELEASE_WAIT`
/// for any other to let go; past that, checkpoint and leave it to the last
/// holder (the owner lock goes when it does, at the latest when the process
/// exits).
fn close_handle(mut handle: std::sync::Arc<std::sync::Mutex<Database>>) {
    let deadline = std::time::Instant::now() + RELEASE_WAIT;
    loop {
        match std::sync::Arc::try_unwrap(handle) {
            Ok(mutex) => {
                let db = match mutex.into_inner() {
                    Ok(db) => db,
                    Err(poisoned) => poisoned.into_inner(),
                };
                // `close` drops the owner lock on both success and a failed
                // checkpoint: the fields were moved out, so the guard drops
                // when this function returns.
                if let Err(error) = db.close() {
                    tracing::warn!("database close failed: {error}");
                }
                return;
            }
            Err(shared) if std::time::Instant::now() < deadline => {
                handle = shared;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(shared) => {
                if let Ok(db) = shared.try_lock()
                    && let Err(error) = db.checkpoint()
                {
                    tracing::warn!("database checkpoint failed: {error}");
                }
                tracing::warn!(
                    "database still in use at the engine's end; the owner lock goes when the work holding it ends (a calendar sync after the Google call in flight, an inbox poll after the message in flight), or with the process"
                );
                return;
            }
        }
    }
}

/// The stored global-show chord, if a key is saved: `device_settings` first,
/// then the database this engine already has open (accounts-and-lock O1).
/// Does not open the database: a missing handle means "not set" there.
#[cfg(not(feature = "mas_build"))]
pub fn stored_global_hotkey() -> Option<String> {
    device_setting(GLOBAL_HOTKEY_SETTING).filter(|value| !value.is_empty())
}

/// The stored global-show chord's settings key.
const GLOBAL_HOTKEY_SETTING: &str = "global_hotkey_key";

/// A device-wide setting (accounts-and-lock PLAN §A.2, O1): the
/// `device_settings` value first; until one is there, the same key in the
/// database this engine has open (the legacy file before rooms). Never opens
/// the database. `Some("")` is a value.
pub fn device_setting(key: &str) -> Option<String> {
    if let Some(value) = bir_core::rooms::device_setting(key) {
        return Some(value);
    }
    let handle = DATABASE.with(|slot| slot.borrow().clone())?;
    let db = bir_core::lock_timing::lock(&handle).ok()?;
    db.get_setting(key).ok().flatten()
}

/// Write a device-wide setting: to `device_settings` from A1 on. Refused
/// while the account gate is locked, as the database write it replaces was.
/// A device store that cannot be used (lost, unreadable, newer) never makes
/// the write fail (A2, #423's review item 5): it then goes to the same key in
/// the open database, which every reader falls back to (O1).
fn set_device_setting(key: &str, value: &str) -> Result<(), HostError> {
    crate::account_gate::ensure_unlocked()?;
    match bir_core::rooms::set_device_setting(key, value) {
        Ok(()) => Ok(()),
        Err(error) if error.is_unusable() => with_database(|db| {
            bir_core::rooms::set_device_setting_or(key, value, Some(db))
                .map(|_| ())
                .map_err(|error| HostError::new(error.to_string()))
        }),
        Err(error) => Err(HostError::new(error.to_string())),
    }
}

/// Whether no database handle of this process is open any more: true once
/// every engine's handle has been closed or its last holder has let go.
/// Informational only, with no caller in the app (an update's relaunch does
/// not wait for it); the tests use it. It is not exact while an open is under
/// way: `open_live_database` takes the owner lock (and runs the migrations)
/// before the new handle is listed, so for that time it reads true although
/// the lock is held.
pub fn owner_lock_released() -> bool {
    let mut live = LIVE_HANDLES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    live.retain(|handle| handle.strong_count() > 0);
    live.is_empty()
}

/// Open the database when the shell launches; `host::run` calls this.
///
/// Also adopts the legacy log file and prunes to the user's retention, then
/// re-checks that every six hours (a thread that keeps no hold on the
/// database). When the database is held (an earlier engine of this process
/// still closing it, or another copy), the next open on this engine finishes
/// the launch.
pub fn open_database_at_launch() {
    if !crate::account_gate::is_unlocked() {
        return;
    }
    LAUNCH_PENDING.with(|pending| pending.set(false));
    match open_database_slot() {
        Ok(handle) => launch_on(handle),
        Err(error) => {
            LAUNCH_PENDING.with(|pending| pending.set(true));
            tracing::warn!("database not opened at launch: {}", error.message());
        }
    }
}

/// The launch's work on the opened database: the submission worker and the log retention.
fn launch_on(handle: std::sync::Arc<std::sync::Mutex<Database>>) {
    LAUNCH_PENDING.with(|pending| pending.set(false));
    start_submission_worker(handle.clone());
    let days = bir_core::lock_timing::lock(&handle)
        .ok()
        .map(|guard| bir_core::log_files::retention_days(&guard));
    if let Some(days) = days {
        // The round below reads the device store; until it has a value, this one.
        bir_core::log_files::remember_retention(days);
        let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), days);
    }
    // Not in test processes: the thread sleeps six hours between rounds. It
    // holds no database handle (accounts-and-lock I5).
    #[cfg(not(test))]
    bir_core::log_files::spawn_retention();
}

/// Whether a submission worker runs in this process.
#[cfg(test)]
pub(crate) fn submission_worker_started() -> bool {
    SUBMISSION_WORKER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .is_some_and(|worker| !worker.loop_ended())
}

#[cfg(test)]
thread_local! {
    static SUBMISSION_WORKER_WAKES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// How many times this thread's host calls woke the submission worker.
#[cfg(test)]
pub(crate) fn submission_worker_wakes() -> u32 {
    SUBMISSION_WORKER_WAKES.with(std::cell::Cell::get)
}

/// Tell the submission worker the queue changed, so a queued return goes out
/// now rather than on the worker's next one-minute tick (the shell form screens
/// and the agent host do the same after Queue and Cancel queue).
pub fn wake_submission_worker() {
    #[cfg(test)]
    SUBMISSION_WORKER_WAKES.with(|wakes| wakes.set(wakes.get() + 1));
    bir_core::background_cron::wake();
}

/// One connection for the process — the same owner-lock discipline as painted
/// `bir` (a second app instance cannot take it while we hold it).
///
/// The submission worker shares this connection's lock, so a call here can
/// wait on it; a wait or hold past 250 ms is logged with the caller's line.
/// Reads that need not wait go through [`with_reader`].
#[track_caller]
pub(crate) fn with_database<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    crate::account_gate::ensure_unlocked()?;
    with_account_database(f)
}

/// Database access for the Buwiz account itself: the server URL and nothing
/// that names a taxpayer. Allowed while the filing UI is locked.
#[track_caller]
pub(crate) fn with_account_database<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    let handle = open_database_slot()?;
    let guard = bir_core::lock_timing::lock(&handle)
        .map_err(|_| HostError::new("database lock poisoned"))?;
    f(&guard)
}

/// A read on the read-only connection, so it does not wait for the
/// submission worker: WAL lets it read the last committed state while the
/// worker holds the writer. `f` runs in one read transaction, so it sees
/// one state across its queries, and after this thread's own committed
/// writes. Only for functions that read: the connection is `query_only`,
/// and a write sent to it fails rather than falling back to the writer.
///
/// With no reader, `f` runs on the writer under its lock, as every read did
/// before: an in-memory database (the tests' `use_ephemeral_database`) has
/// no file for a second connection, and a reader that failed to open was
/// logged by [`install_reader`].
#[track_caller]
pub(crate) fn with_reader<R>(
    f: impl FnOnce(&Database) -> Result<R, HostError>,
) -> Result<R, HostError> {
    crate::account_gate::ensure_unlocked()?;
    // Opens the database, or reopens it after an import, with its reader.
    let handle = open_database_slot()?;
    match READER.with(|slot| slot.borrow().clone()) {
        Some(reader) => {
            bir_core::lock_timing::time(|| reader.read_snapshot(f)).map_err(db_error)?
        }
        None => {
            let guard = bir_core::lock_timing::lock(&handle)
                .map_err(|_| HostError::new("database lock poisoned"))?;
            f(&guard)
        }
    }
}

/// Tests install a fresh ephemeral database in this thread's slot, so each
/// test (one thread each) runs against its own scratch data and never opens
/// the live database through `BIR_DATABASE_PATH`.
#[cfg(test)]
pub(crate) fn use_ephemeral_database() {
    // Its device settings in memory too: never a device.db in a real folder.
    bir_core::rooms::use_ephemeral_device_store();
    drop_reader();
    DATABASE.with(|slot| {
        *slot.borrow_mut() = Some(std::sync::Arc::new(std::sync::Mutex::new(
            Database::open_ephemeral().expect("ephemeral database opens"),
        )));
    });
    RELEASED.set(false);
}

/// `serde_json::Value` → `HostValue`. bir-core serializes cleanly; the
/// boundary keeps JSON's six shapes exactly.
pub fn json(value: serde_json::Value) -> HostValue {
    match value {
        serde_json::Value::Null => HostValue::Null,
        serde_json::Value::Bool(v) => HostValue::Bool(v),
        serde_json::Value::Number(v) => HostValue::Number(v.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(v) => HostValue::Str(v),
        serde_json::Value::Array(v) => HostValue::Array(v.into_iter().map(json).collect()),
        serde_json::Value::Object(v) => HostValue::Object(
            v.into_iter()
                .map(|(key, value)| (key, json(value)))
                .collect(),
        ),
    }
}

pub(crate) fn serialize<T: serde::Serialize>(value: &T) -> Result<HostValue, HostError> {
    serde_json::to_value(value)
        .map(json)
        .map_err(|error| HostError::new(error.to_string()))
}

/// `HostValue` → `serde_json::Value`: the inverse of `json`, for functions
/// that take structured arguments (a whole profile record, a draft).
pub fn to_json(value: &HostValue) -> serde_json::Value {
    match value {
        HostValue::Null => serde_json::Value::Null,
        HostValue::Bool(v) => serde_json::Value::Bool(*v),
        // Script numbers arrive as f64, but typed drafts declare ints — an
        // integral double must round-trip as a JSON integer or serde rejects
        // `0.0` for `u16` fields.
        HostValue::Number(v) => {
            if v.fract() == 0.0 && *v >= i64::MIN as f64 && *v <= i64::MAX as f64 {
                serde_json::json!(*v as i64)
            } else {
                serde_json::json!(v)
            }
        }
        HostValue::Str(v) => serde_json::Value::String(v.clone()),
        HostValue::Array(v) => serde_json::Value::Array(v.iter().map(to_json).collect()),
        HostValue::Object(v) => serde_json::Value::Object(
            v.iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect(),
        ),
    }
}

/// Hotkey, log retention, and the paths Settings shows. Device-wide settings:
/// `device_settings` first, then the database if it is open; neither answers
/// the defaults. This must not fail the boot read, and opens no database.
fn computer_snapshot() -> (Option<String>, u32) {
    let hotkey = device_setting(GLOBAL_HOTKEY_SETTING).filter(|value| !value.trim().is_empty());
    let retention = bir_core::log_files::parse_retention(
        device_setting(bir_core::log_files::RETENTION_SETTING).as_deref(),
    );
    (hotkey, retention)
}

/// Chords the Settings picker offers. The same combo language as
/// [`crate::hotkey_combo::is_registrable`]: explicit modifiers, not a bare key.
fn hotkey_presets() -> Vec<HostValue> {
    let (stored_prefix, label_prefix) = if cfg!(target_os = "macos") {
        ("control+alt", "Ctrl+Option")
    } else {
        ("super+shift", "Super+Shift")
    };
    ["E", "B", "H", "F12"]
        .into_iter()
        .map(|key| {
            HostValue::from(
                HostObject::new()
                    .field("value", format!("{stored_prefix}+{key}"))
                    .field("label", format!("{label_prefix}+{key}")),
            )
        })
        .collect()
}

fn system_info() -> HostResult {
    let (hotkey, retention) = computer_snapshot();
    let choices = bir_core::log_files::RETENTION_CHOICES
        .iter()
        .map(|days| HostValue::from(i64::from(*days)))
        .collect::<Vec<_>>();
    Ok(HostObject::new()
        .field("os", std::env::consts::OS)
        .field("arch", std::env::consts::ARCH)
        .field("app_version", env!("CARGO_PKG_VERSION"))
        .field(
            "database_path",
            db::app_database_path().display().to_string(),
        )
        .field(
            "data_dir",
            bir_core::platform::data_dir().display().to_string(),
        )
        .field(
            "logs_dir",
            bir_core::log_files::log_dir().display().to_string(),
        )
        .field(
            "global_hotkey",
            hotkey.map_or(HostValue::Null, HostValue::Str),
        )
        .field("hotkey_presets", HostValue::Array(hotkey_presets()))
        .field("log_retention_days", i64::from(retention))
        .field("log_retention_choices", HostValue::Array(choices))
        .field("runtime", "gpui-shell")
        .into())
}

/// Store the global show chord the tray already polls, or clear it.
/// An unknown key is refused and the stored chord is left as it was.
fn set_global_hotkey(arguments: &gpui_shell::HostArguments) -> HostResult {
    let combo = arguments.string(0)?.trim().to_string();
    if !combo.is_empty() && !crate::hooks::hooks().global_hotkey_registrable(&combo) {
        return Err(HostError::new(crate::hotkey_combo::NOT_REGISTRABLE));
    }
    set_device_setting(GLOBAL_HOTKEY_SETTING, &combo)?;
    Ok(HostValue::Null)
}

/// How many days of `ebirforms.*.log` to keep. Out of range is refused.
fn set_log_retention(arguments: &gpui_shell::HostArguments) -> HostResult {
    let days = arguments.integer(0)?;
    if !(i64::from(bir_core::log_files::MIN_RETENTION_DAYS)
        ..=i64::from(bir_core::log_files::MAX_RETENTION_DAYS))
        .contains(&days)
    {
        return Err(HostError::new(format!(
            "Log retention must be between {} and {} days.",
            bir_core::log_files::MIN_RETENTION_DAYS,
            bir_core::log_files::MAX_RETENTION_DAYS
        )));
    }
    set_device_setting(bir_core::log_files::RETENTION_SETTING, &days.to_string())?;
    let days = bir_core::log_files::parse_retention(
        device_setting(bir_core::log_files::RETENTION_SETTING).as_deref(),
    );
    bir_core::log_files::remember_retention(days);
    let _ = bir_core::log_files::maintain(&bir_core::log_files::log_dir(), days);
    Ok(HostValue::Null)
}

fn update_status() -> HostResult {
    let status = crate::hooks::hooks().update_status();
    let version = match status.version {
        Some(version) => HostValue::from(version),
        None => HostValue::Null,
    };
    let message = match status.message {
        Some(message) => HostValue::from(message),
        None => HostValue::Null,
    };
    Ok(HostObject::new()
        .field("phase", status.phase)
        .field("version", version)
        .field("message", message)
        .field("auto_download", status.auto_download)
        .field("current_version", status.current_version)
        .into())
}

fn install_update() -> HostResult {
    crate::hooks::hooks().install_update();
    Ok(HostValue::Null)
}

fn dismiss_update() -> HostResult {
    crate::hooks::hooks().dismiss_update();
    Ok(HostValue::Null)
}

fn check_for_updates() -> HostResult {
    crate::hooks::hooks().check_for_updates();
    Ok(HostValue::Null)
}

fn set_auto_update_download(arguments: &gpui_shell::HostArguments) -> HostResult {
    let enabled = arguments.boolean(0)?;
    set_device_setting(
        crate::hooks::AUTO_DOWNLOAD_SETTING,
        if enabled { "1" } else { "0" },
    )?;
    crate::hooks::hooks().set_auto_update_download(enabled);
    Ok(HostValue::Null)
}

fn list_profiles() -> HostResult {
    with_database(|db| {
        let profiles = db
            .list_profiles()
            .map_err(|error| HostError::new(error.to_string()))?;
        let mut rows = Vec::with_capacity(profiles.len());
        for profile in &profiles {
            let tin = profile.tin.full();
            rows.push(
                HostObject::new()
                    .field("id", profile.id.unwrap_or_default())
                    .field("tin", tin)
                    .field("full_name", profile.full_name.as_str())
                    .field("rdo_code", profile.rdo_code.as_str())
                    .field("email", profile.email.as_str())
                    .field("taxpayer_type", format!("{:?}", profile.taxpayer_type))
                    .field("is_vat_registered", profile.is_vat_registered)
                    .field("is_archived", profile.is_archived)
                    .field("default_form_type", profile.default_form_type.as_str())
                    .field(
                        "applicable_forms",
                        profile
                            .applicable_forms()
                            .into_iter()
                            .map(HostValue::Str)
                            .collect::<Vec<_>>(),
                    ),
            );
        }
        Ok(HostValue::Array(
            rows.into_iter().map(HostValue::from).collect(),
        ))
    })
}

fn forms_catalog() -> HostResult {
    let mut rows = Vec::with_capacity(FORM_REGISTRY.len());
    for form in FORM_REGISTRY {
        rows.push(HostValue::from(
            HostObject::new()
                .field("code", form.code)
                .field("title", form.title)
                .field("category", form.category)
                .field("frequency", format!("{:?}", form.frequency))
                .field("requires_employees", form.requires_employees)
                .field("is_deprecated", form.is_deprecated),
        ));
    }
    Ok(HostValue::Array(rows))
}

/// The only settings the page may use through `get_setting`, `set_setting`
/// and `delete_setting`: its theme toggle's (`THEME_PREF_KEY`,
/// `nsh/shell/theme.ts`). Every other setting is host state, such as the
/// access mode, the admin's TIN root, PIN hash and authenticator secret, the
/// recovery key's hash, and the welcome's keyed hashes of the accounts
/// welcomed on this device (`bir_core::welcome`, accounts-and-lock W3: the
/// page asks `welcome_first_time` and never reads which accounts use this
/// device); host code reads and writes those through `Database` or the
/// device store directly.
const PAGE_SETTING_KEYS: [&str; 1] = ["theme_preference"];

/// The key a page setting verb names, when it is in [`PAGE_SETTING_KEYS`].
/// Any other key is refused before the database is opened, with an error
/// that names the key and nothing stored under it.
fn page_setting_key(arguments: &gpui_shell::HostArguments) -> Result<String, HostError> {
    let key = arguments.string(0)?;
    if PAGE_SETTING_KEYS.contains(&key) {
        Ok(key.to_owned())
    } else {
        Err(HostError::new(format!(
            "setting `{key}` is not available to the page"
        )))
    }
}

/// The page's settings are device-wide (the theme, accounts-and-lock §A.2):
/// `device_settings` first, then the same key in the open database (O1).
fn get_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    if let Some(value) = bir_core::rooms::device_setting(&key) {
        crate::account_gate::ensure_unlocked()?;
        return Ok(HostValue::from(value));
    }
    with_database(|db| {
        db.get_setting(&key)
            .map(HostValue::from)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

/// Written to `device_settings` (accounts-and-lock A1).
fn set_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    let value = arguments.string(1)?.to_owned();
    set_device_setting(&key, &value)?;
    Ok(HostValue::Null)
}

// ---------- S2: auth + profile management ----------

fn get_profile_by_tin(
    db: &Database,
    tin: &str,
) -> Result<bir_core::profile::TaxpayerProfile, HostError> {
    db.get_profile_by_tin(tin)
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new(format!("no taxpayer profile with TIN {tin}")))
}

/// A profile as the page receives it: the record without the credential
/// keys (removed, not sent as null), built by bir-core's
/// `TaxpayerProfile::record_without_credentials`, which keeps the values out
/// of the profile while the record is built.
///
/// A profile's five credentials (bir-core's `PROFILE_CREDENTIAL_KEYS`) are
/// host state: the page can neither read them nor set or clear them through
/// the profile verbs. Each keeps its own host writer instead: the inbox
/// verbs (`set_inbox_app_password`, the Google grant's persist and
/// disconnect) for the app password and the tokens; the authenticator verbs
/// (`set_taxpayer_totp`, `set_profile_totp`, `set_admin_totp`) for a TOTP
/// secret; and the PIN verbs (`set_taxpayer_pin`, `set_profile_pin`,
/// `set_admin_pin`, `setup_helpdesk`, `change_to_helpdesk`) for a PIN. The
/// authenticator and PIN verbs store theirs on the taxpayer (the help desk
/// admin's in settings), never on a profile. A TOTP secret or PIN hash still
/// on a profile row moves onto its taxpayer at the next launch when the
/// taxpayer has none (bir-core's `migrate_v22_taxpayer_credentials`), so one
/// the page could set would become the taxpayer's.
fn redacted_profile(mut profile: bir_core::profile::TaxpayerProfile) -> HostResult {
    profile
        .record_without_credentials()
        .map(json)
        .map_err(db_error)
}

/// `save_profile` never stores a client's credentials, as
/// [`keep_stored_trigger_flag`] never stores a client's trigger fields: all
/// five are taken from the stored row the save writes, or cleared when there
/// is none. So a page round trip of a redacted profile keeps a stored
/// password or PIN hash, and no TOTP secret or PIN hash from the page
/// reaches a row the next launch would move onto the taxpayer. The row is
/// found the way [`Database::save_profile`] finds it: the one with the
/// client's `id` (a TIN change re-keys that row), else the one with the
/// client's TIN.
fn keep_stored_credentials(
    db: &Database,
    profile: &mut bir_core::profile::TaxpayerProfile,
) -> Result<(), HostError> {
    let by_tin = db
        .get_profile_by_tin(&profile.tin.full())
        .map_err(db_error)?;
    let mut stored = match profile.id {
        // bir-core looks rows up by TIN only, so the list is read just when
        // the id is not the row with this TIN (a TIN change).
        Some(id) if !by_tin.as_ref().is_some_and(|row| row.id == Some(id)) => db
            .list_profiles()
            .map_err(db_error)?
            .into_iter()
            .find(|row| row.id == Some(id))
            .or(by_tin),
        _ => by_tin,
    };
    let credentials = stored
        .as_mut()
        .map(bir_core::profile::TaxpayerProfile::take_credentials)
        .unwrap_or_default();
    profile.put_credentials(credentials);
    Ok(())
}

/// The stored profile for `tin`, without its credentials.
fn profile_detail(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| redacted_profile(get_profile_by_tin(db, tin)?))
}

/// Save the page's profile. Its credentials are settled from the stored row
/// ([`keep_stored_credentials`]); the answer is the saved profile without
/// them.
fn save_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let mut profile: bir_core::profile::TaxpayerProfile =
        serde_json::from_value(to_json(arguments.value(0)?))
            .map_err(|error| HostError::new(format!("profile payload: {error}")))?;
    with_database(|db| {
        keep_stored_credentials(db, &mut profile)?;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(redacted_profile)
    })
}

fn set_profile_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_last_admin_branch(db, &tin, "archived", |profile| profile.is_archived)?;
        }
        let mut profile = get_profile_by_tin(db, &tin)?;
        profile.is_archived = archived;
        db.save_profile(profile)
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(redacted_profile)
    })
}

fn delete_profile(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?;
    with_database(|db| {
        guard_last_admin_branch(db, tin, "deleted", |_| false)?;
        db.delete_profile(tin)
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
    })
}

// ---------- sign-in: one lock per taxpayer, admin by access mode ----------
//
// A PIN and an authenticator belong to the taxpayer (9-digit TIN root) and
// are stored once, on the `taxpayers` row; every branch shares them. The
// admin account depends on the access mode chosen at first launch:
// - `solo`: the admin IS the owner's taxpayer (`admin_tin_root`), so the
//   admin PIN/authenticator are that taxpayer's.
// - `helpdesk`: the admin is the reserved root `000000000`, not a taxpayer;
//   its PIN/authenticator are the `app_lock_pin_hash` / `app_totp_secret`
//   settings, and it must always keep at least one of them.
// Recovery ladder: authenticator -> recovery key -> Mac login.

const ACCESS_MODE: &str = "access_mode";
const ADMIN_TIN_ROOT: &str = "admin_tin_root";
const ADMIN_PIN_HASH: &str = "app_lock_pin_hash";
const ADMIN_TOTP_SECRET: &str = "app_totp_secret";
const RECOVERY_KEY_HASH: &str = "admin_recovery_key_hash";

const WRONG_CODE: &str =
    "That code does not match. Type the 6-digit code your authenticator app shows now.";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccessMode {
    /// The owner's own taxpayer is the admin.
    Solo(String),
    Helpdesk,
}

fn db_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

fn setting(db: &Database, key: &str) -> Result<Option<String>, HostError> {
    db.get_setting(key).map_err(db_error)
}

/// The chosen access mode; `None` on a fresh install not yet set up.
fn access_mode(db: &Database) -> Result<Option<AccessMode>, HostError> {
    Ok(match setting(db, ACCESS_MODE)?.as_deref() {
        Some("solo") => setting(db, ADMIN_TIN_ROOT)?.map(AccessMode::Solo),
        Some("helpdesk") => Some(AccessMode::Helpdesk),
        _ => None,
    })
}

fn require_access_mode(db: &Database) -> Result<AccessMode, HostError> {
    access_mode(db)?.ok_or_else(|| HostError::new("This app is not set up yet."))
}

/// The TIN root of the solo owner, whose taxpayer must never disappear.
fn solo_admin_root(db: &Database) -> Result<Option<String>, HostError> {
    Ok(match access_mode(db)? {
        Some(AccessMode::Solo(root)) => Some(root),
        _ => None,
    })
}

fn tin_root_argument(raw: &str) -> Result<String, HostError> {
    bir_core::tax_profile::parse_tin_root(raw).map_err(db_error)
}

/// A PIN argument: `null` clears, otherwise exactly 4 digits.
fn pin_argument(value: &HostValue) -> Result<Option<String>, HostError> {
    match value {
        HostValue::Null => Ok(None),
        HostValue::Str(pin) if pin.len() == 4 && pin.bytes().all(|b| b.is_ascii_digit()) => {
            Ok(Some(pin.clone()))
        }
        _ => Err(HostError::new("A PIN is 4 digits.")),
    }
}

/// An authenticator argument pair: `null` clears; a secret needs a current
/// code from the app, checked against the new secret, so a mistyped
/// enrolment cannot lock anyone out.
fn totp_argument(
    arguments: &gpui_shell::HostArguments,
    secret_index: usize,
) -> Result<Option<String>, HostError> {
    let secret = arguments.value(secret_index)?.as_str().map(str::to_owned);
    let token = arguments
        .get(secret_index + 1)
        .and_then(HostValue::as_str)
        .unwrap_or_default();
    if let Some(secret) = &secret
        && !bir_core::crypto::validate_totp(secret, token)
    {
        return Err(HostError::new(WRONG_CODE));
    }
    Ok(secret)
}

fn taxpayer_credentials(
    db: &Database,
    tin_root: &str,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    db.taxpayer_credentials(tin_root).map_err(db_error)
}

/// The admin's PIN hash and authenticator secret under `mode`.
fn admin_credentials(
    db: &Database,
    mode: &AccessMode,
) -> Result<bir_core::tax_profile::TaxpayerCredentials, HostError> {
    match mode {
        AccessMode::Solo(root) => taxpayer_credentials(db, root),
        AccessMode::Helpdesk => Ok(bir_core::tax_profile::TaxpayerCredentials {
            pin_hash: setting(db, ADMIN_PIN_HASH)?,
            totp_secret: setting(db, ADMIN_TOTP_SECRET)?,
        }),
    }
}

/// Where a PIN hash lives in the room file, for its rehash to Argon2id.
#[derive(Debug, Clone)]
enum PinStore {
    /// A client PIN: `taxpayers.pin_hash` of this TIN root (or a branch copy, cleared by the rehash).
    Taxpayer(String),
    /// A PIN kept in a setting: the help-desk admin's `app_lock_pin_hash`.
    Setting(&'static str),
}

/// A rehash to Argon2id waiting to start: `matched` is the stored value it replaces, only while it is still there.
/// Started after the try's result is committed (N4), or right after an old-form write (N3).
struct Rehash {
    store: PinStore,
    matched: String,
    pin: bir_core::pin::Zeroizing<String>,
}

/// A PIN of the room file (`store`) against `pin`, in either form. A match against the old SHA-256 value (or a
/// cheaper cost) gives the rehash to start once the try is committed ([`Rehash::start`]); the answer never waits
/// on it.
fn stored_pin_check(store: PinStore, stored: Option<&str>, pin: &str) -> (bool, Option<Rehash>) {
    let Some(stored) = stored else {
        return (false, None);
    };
    match bir_core::pin::check(stored, pin) {
        Ok(bir_core::pin::Check::Match { upgrade }) => (
            true,
            upgrade.then(|| Rehash {
                store,
                matched: stored.to_owned(),
                pin: bir_core::pin::Zeroizing::new(pin.to_owned()),
            }),
        ),
        Ok(bir_core::pin::Check::Miss) => (false, None),
        Err(error) => {
            tracing::warn!("a PIN is stored in no form this version reads: {error}");
            (false, None)
        }
    }
}

/// A client PIN (`taxpayers.pin_hash` of `tin_root`, or a branch copy) against `pin`; see [`stored_pin_check`].
fn client_pin_check(tin_root: &str, stored: Option<&str>, pin: &str) -> (bool, Option<Rehash>) {
    stored_pin_check(PinStore::Taxpayer(tin_root.to_owned()), stored, pin)
}

/// Start `rehash` if the try it came from matched: only after its result was committed (N4).
fn start_rehash_after(matched: bool, rehash: Option<Rehash>) {
    if let (true, Some(rehash)) = (matched, rehash) {
        rehash.start();
    }
}

#[cfg(test)]
thread_local! {
    /// The last rehash [`Rehash::start`] started on this thread, for a test to wait on.
    static LAST_REHASH: RefCell<Option<std::thread::JoinHandle<()>>> = const { RefCell::new(None) };
}

/// Wait for the rehash this thread started last, if any.
#[cfg(test)]
fn join_last_rehash() {
    if let Some(rehash) = LAST_REHASH.with(|slot| slot.borrow_mut().take()) {
        rehash.join().expect("the rehash thread ends");
    }
}

impl Rehash {
    /// A PIN just written in the old form by one of the page's synchronous verbs (N3): rehashed to Argon2id at once.
    fn written(store: PinStore, old_form: &str, pin: &str) -> Self {
        Self {
            store,
            matched: old_form.to_owned(),
            pin: bir_core::pin::Zeroizing::new(pin.to_owned()),
        }
    }

    /// The rehash (PLAN §B.1), on a thread of its own: hash the PIN with Argon2id, then store it in place of
    /// `matched` (for a client, clearing the branch copies), unless the PIN changed meanwhile. A failure is logged;
    /// the old value keeps working and its next match tries again.
    fn start(self) {
        let handle = match open_database_slot() {
            Ok(handle) => handle,
            Err(error) => {
                tracing::warn!("PIN not rehashed: {error}");
                return;
            }
        };
        let spawned = std::thread::Builder::new()
            .name("pin-rehash".into())
            .spawn(move || {
                let phc = match bir_core::pin::hash(&self.pin) {
                    Ok(phc) => phc,
                    Err(error) => return tracing::warn!("PIN not rehashed: {error}"),
                };
                let Ok(db) = bir_core::lock_timing::lock(&handle) else {
                    return tracing::warn!("PIN not rehashed: database lock poisoned");
                };
                let written = match &self.store {
                    PinStore::Taxpayer(root) => db
                        .upgrade_client_pin_hash(root, &self.matched, &phc)
                        .map_err(|error| error.to_string()),
                    PinStore::Setting(key) => db
                        .replace_setting_if(key, &self.matched, &phc)
                        .map_err(|error| error.to_string()),
                };
                if let Err(error) = written {
                    tracing::warn!("PIN not rehashed: {error}");
                }
            });
        match spawned {
            #[cfg(test)]
            Ok(rehash) => LAST_REHASH.with(|slot| *slot.borrow_mut() = Some(rehash)),
            #[cfg(not(test))]
            Ok(_) => {}
            Err(error) => tracing::warn!("PIN not rehashed: {error}"),
        }
    }
}

/// The admin's PIN under `mode`, and the rehash its match calls for. Solo: the owner taxpayer's PIN, a client PIN
/// store. Help desk: the settings PIN.
fn admin_pin_check(
    db: &Database,
    mode: &AccessMode,
    pin: &str,
) -> Result<(bool, Option<Rehash>), HostError> {
    let credentials = admin_credentials(db, mode)?;
    let store = match mode {
        AccessMode::Solo(root) => PinStore::Taxpayer(root.clone()),
        AccessMode::Helpdesk => PinStore::Setting(ADMIN_PIN_HASH),
    };
    Ok(stored_pin_check(
        store,
        credentials.pin_hash.as_deref(),
        pin,
    ))
}

fn totp_matches(secret: Option<&str>, token: &str) -> bool {
    secret.is_some_and(|secret| bir_core::crypto::validate_totp(secret, token))
}

fn generate_totp_secret(arguments: &gpui_shell::HostArguments) -> HostResult {
    let account = arguments.string(0)?;
    let (secret, qr_path) = bir_core::crypto::generate_totp_secret(account);
    Ok(HostObject::new()
        .field("secret", secret)
        .field("qr_path", qr_path.display().to_string())
        .into())
}

/// Deleted from `device_settings` and from the open database, so the
/// fallback cannot bring an old value back.
fn delete_setting(arguments: &gpui_shell::HostArguments) -> HostResult {
    let key = page_setting_key(arguments)?;
    with_database(|db| {
        bir_core::rooms::delete_device_setting(&key)
            .map_err(|error| HostError::new(error.to_string()))?;
        db.delete_setting(&key)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

/// What the lock screen and first-launch setup read once per render.
fn auth_state() -> HostResult {
    with_database(|db| {
        let mode = access_mode(db)?;
        let credentials = match &mode {
            Some(mode) => admin_credentials(db, mode)?,
            None => Default::default(),
        };
        let (mode_name, admin_root) = match &mode {
            Some(AccessMode::Solo(root)) => {
                (HostValue::from("solo"), HostValue::from(root.as_str()))
            }
            Some(AccessMode::Helpdesk) => (
                HostValue::from("helpdesk"),
                HostValue::from(bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT),
            ),
            None => (HostValue::Null, HostValue::Null),
        };
        Ok(HostObject::new()
            .field("mode", mode_name)
            .field("admin_root", admin_root)
            .field("has_admin_pin", credentials.pin_hash.is_some())
            .field("has_admin_totp", credentials.totp_secret.is_some())
            .field(
                "has_recovery_key",
                setting(db, RECOVERY_KEY_HASH)?.is_some(),
            )
            .field("mac_login_available", mac_login_available())
            .into())
    })
}

fn taxpayer_auth(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        let credentials = taxpayer_credentials(db, &tin_root)?;
        Ok(HostObject::new()
            .field("has_pin", credentials.pin_hash.is_some())
            .field("has_totp", credentials.totp_secret.is_some())
            .into())
    })
}

/// Set (`pin`) or clear (`null`) a taxpayer's PIN.
fn set_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| write_taxpayer_pin(db, &tin_root, pin.as_deref()))
}

/// The PIN is stored in the old form so the verb answers at once, then rehashed to Argon2id on its own thread
/// straight after (N3): the page calls this synchronously, and the engine never waits on the slow hash.
fn write_taxpayer_pin(db: &Database, tin_root: &str, pin: Option<&str>) -> HostResult {
    let hash = pin.map(bir_core::crypto::hash_pin);
    db.set_taxpayer_pin_hash(tin_root, hash.as_deref())
        .map_err(db_error)?;
    if let (Some(pin), Some(hash)) = (pin, &hash) {
        Rehash::written(PinStore::Taxpayer(tin_root.to_owned()), hash, pin).start();
    }
    Ok(HostValue::Null)
}

/// The help-desk admin PIN in its setting: the old form at once, then Argon2id on its own thread (N3), as
/// [`write_taxpayer_pin`].
fn write_admin_setting_pin(db: &Database, pin: &str) -> Result<(), HostError> {
    let hash = bir_core::crypto::hash_pin(pin);
    db.set_setting(ADMIN_PIN_HASH, &hash).map_err(db_error)?;
    Rehash::written(PinStore::Setting(ADMIN_PIN_HASH), &hash, pin).start();
    Ok(())
}

/// Set (with a current code) or clear (`null`) a taxpayer's authenticator.
fn set_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        db.set_taxpayer_totp_secret(&tin_root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_taxpayer_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let pin = arguments.string(1)?;
    let mut rehash = None;
    let ok = auth_limiter::guarded_client(&tin_root, || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            let (matched, upgrade) =
                client_pin_check(&tin_root, credentials.pin_hash.as_deref(), pin);
            rehash = upgrade;
            Ok(matched)
        })
    })?;
    start_rehash_after(ok, rehash);
    Ok(HostValue::Bool(ok))
}

fn verify_taxpayer_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin_root = tin_root_argument(arguments.string(0)?)?;
    let token = arguments.string(1)?;
    let ok = auth_limiter::guarded_client(&tin_root, || {
        with_database(|db| {
            let credentials = taxpayer_credentials(db, &tin_root)?;
            Ok(totp_matches(credentials.totp_secret.as_deref(), token))
        })
    })?;
    Ok(HostValue::Bool(ok))
}

/// Legacy per-branch entry point: a branch TIN's PIN is its taxpayer's.
fn set_profile_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let pin = pin_argument(arguments.value(1)?)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        write_taxpayer_pin(db, &root, pin.as_deref())
    })
}

/// Legacy per-branch entry point: a branch TIN's authenticator is its
/// taxpayer's. Setting needs a current code; `null` clears.
fn set_profile_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let secret = totp_argument(arguments, 1)?;
    with_database(|db| {
        let root = get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string();
        db.set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn verify_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = arguments.string(0)?;
    let mut rehash = None;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => {
                    let (matched, upgrade) = admin_pin_check(db, &mode, pin)?;
                    rehash = upgrade;
                    matched
                }
                None => false,
            })
        })
    })?;
    start_rehash_after(ok, rehash);
    Ok(HostValue::Bool(ok))
}

fn verify_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let token = arguments.string(0)?;
    let ok = auth_limiter::guarded(auth_limiter::ADMIN, || {
        with_database(|db| {
            Ok(match access_mode(db)? {
                Some(mode) => {
                    totp_matches(admin_credentials(db, &mode)?.totp_secret.as_deref(), token)
                }
                None => false,
            })
        })
    })?;
    Ok(HostValue::Bool(ok))
}

const KEEP_ONE_WAY_IN: &str = "The admin must keep at least one way to sign in. Set up the other one (PIN or authenticator) before removing this one.";

/// Set or clear the admin PIN. Solo: the owner taxpayer's PIN (clearing is
/// allowed). Help desk: the settings PIN, refused to clear without an
/// authenticator. Only the hash is stored; the script never sees it.
fn set_admin_pin(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = pin_argument(arguments.value(0)?)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => write_taxpayer_pin(db, &root, pin.as_deref()),
        AccessMode::Helpdesk => match pin {
            Some(pin) => write_admin_setting_pin(db, &pin).map(|_| HostValue::Null),
            None => {
                if setting(db, ADMIN_TOTP_SECRET)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_PIN_HASH)
                    .map(|_| HostValue::Null)
                    .map_err(db_error)
            }
        },
    })
}

/// Set (with a current code) or clear the admin authenticator, mode-aware
/// like [`set_admin_pin`].
fn set_admin_totp(arguments: &gpui_shell::HostArguments) -> HostResult {
    let secret = totp_argument(arguments, 0)?;
    with_database(|db| match require_access_mode(db)? {
        AccessMode::Solo(root) => db
            .set_taxpayer_totp_secret(&root, secret.as_deref())
            .map(|_| HostValue::Null)
            .map_err(db_error),
        AccessMode::Helpdesk => match secret {
            Some(secret) => db.set_setting(ADMIN_TOTP_SECRET, &secret),
            None => {
                if setting(db, ADMIN_PIN_HASH)?.is_none() {
                    return Err(HostError::new(KEEP_ONE_WAY_IN));
                }
                db.delete_setting(ADMIN_TOTP_SECRET)
            }
        }
        .map(|_| HostValue::Null)
        .map_err(db_error),
    })
}

const ALREADY_SET_UP: &str = "This app is already set up.";

/// First launch, "Just me or my business": the taxpayer the script just
/// created is the owner and the admin.
fn setup_solo(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        bir_core::tax_profile::reject_reserved_tin_root(&root).map_err(db_error)?;
        if db.get_taxpayer(&root).map_err(db_error)?.is_none() {
            return Err(HostError::new(format!("No taxpayer with TIN {root}")));
        }
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        Ok(HostValue::Null)
    })
}

/// First launch, "I help other taxpayers": admin PIN plus a recovery key.
fn setup_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin =
        pin_argument(arguments.value(0)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        if setting(db, ACCESS_MODE)?.is_some() {
            return Err(HostError::new(ALREADY_SET_UP));
        }
        write_admin_setting_pin(db, &pin)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

// ---------- change mode / owner (admin only, after first launch) ----------
//
// Settings › Computer setup. Each change re-checks the current admin's PIN
// or authenticator code here, never trusting the script, refuses a change
// that would leave nobody able to sign in as admin, and replaces the
// recovery key so the previous admin's key stops working.

const NOT_THE_ADMIN: &str =
    "That PIN or code is not the admin's. Type the current admin PIN or authenticator code.";

/// The current admin's PIN (`pin`) or authenticator code (`code`), checked
/// against the stored credentials of `mode`. Wrong tries count against the
/// admin's sign-in backoff, so this is no way around it.
fn confirm_admin(
    db: &Database,
    mode: &AccessMode,
    pin: Option<&str>,
    code: Option<&str>,
) -> Result<(), HostError> {
    let mut rehash = None;
    let matched = auth_limiter::guarded(auth_limiter::ADMIN, || {
        let credentials = admin_credentials(db, mode)?;
        let by_pin = match pin {
            Some(pin) => {
                let (matched, upgrade) = admin_pin_check(db, mode, pin)?;
                rehash = upgrade;
                matched
            }
            None => false,
        };
        let by_code =
            code.is_some_and(|code| totp_matches(credentials.totp_secret.as_deref(), code));
        Ok(by_pin || by_code)
    })?;
    start_rehash_after(matched, rehash);
    if matched {
        Ok(())
    } else {
        Err(HostError::new(NOT_THE_ADMIN))
    }
}

/// `null` or a non-empty string.
fn optional_text(arguments: &gpui_shell::HostArguments, index: usize) -> Option<String> {
    arguments
        .get(index)
        .and_then(HostValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// A taxpayer that can become the owner: exists, not the reserved root,
/// has a branch that is not archived, and has a PIN or authenticator so
/// someone can sign in as admin afterwards.
fn check_new_owner(db: &Database, root: &str) -> Result<(), HostError> {
    bir_core::tax_profile::reject_reserved_tin_root(root).map_err(db_error)?;
    let Some(taxpayer) = db.get_taxpayer(root).map_err(db_error)? else {
        return Err(HostError::new(format!("No taxpayer with TIN {root}")));
    };
    let mut active = false;
    for branch in &taxpayer.branches {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !profile.is_archived
        {
            active = true;
            break;
        }
    }
    if !active {
        return Err(HostError::new(
            "This taxpayer is archived. Unarchive it before making it the owner.",
        ));
    }
    let credentials = taxpayer_credentials(db, root)?;
    if credentials.pin_hash.is_none() && credentials.totp_secret.is_none() {
        return Err(HostError::new(
            "This taxpayer has no PIN or authenticator yet. Set one for them first, so the new owner can sign in as admin.",
        ));
    }
    Ok(())
}

/// Make `tin_root` the owner (solo mode), from solo or from help desk.
/// Arguments: `tin_root`, then the current admin's `pin` and `code` (either
/// may be `null`). Help desk's admin PIN and authenticator are removed.
/// Answers a new recovery key.
fn change_owner(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let pin = optional_text(arguments, 1);
    let code = optional_text(arguments, 2);
    with_database(|db| {
        let mode = require_access_mode(db)?;
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        if mode == AccessMode::Solo(root.clone()) {
            return Err(HostError::new("This taxpayer is already the owner."));
        }
        check_new_owner(db, &root)?;
        // Owner first, then the mode: a failure in between leaves a working
        // setup either way.
        db.set_setting(ADMIN_TIN_ROOT, &root).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "solo").map_err(db_error)?;
        db.delete_setting(ADMIN_PIN_HASH).map_err(db_error)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// Switch from solo to help desk. Arguments: the current admin's `pin` and
/// `code` (either may be `null`), then the new admin PIN. The former owner
/// stays an ordinary taxpayer with their own PIN. Answers a new recovery
/// key.
fn change_to_helpdesk(arguments: &gpui_shell::HostArguments) -> HostResult {
    let pin = optional_text(arguments, 0);
    let code = optional_text(arguments, 1);
    let new_pin =
        pin_argument(arguments.value(2)?)?.ok_or_else(|| HostError::new("A PIN is 4 digits."))?;
    with_database(|db| {
        let mode = require_access_mode(db)?;
        if mode == AccessMode::Helpdesk {
            return Err(HostError::new(
                "This computer is already set up for a help desk.",
            ));
        }
        confirm_admin(db, &mode, pin.as_deref(), code.as_deref())?;
        // The admin PIN first, then the mode: a failure in between leaves a
        // working solo setup.
        write_admin_setting_pin(db, &new_pin)?;
        db.delete_setting(ADMIN_TOTP_SECRET).map_err(db_error)?;
        db.set_setting(ACCESS_MODE, "helpdesk").map_err(db_error)?;
        db.delete_setting(ADMIN_TIN_ROOT).map_err(db_error)?;
        store_new_recovery_key(db)
    })
}

/// A fresh recovery key replaces any earlier one; only its hash is kept.
fn store_new_recovery_key(db: &Database) -> HostResult {
    let key = bir_core::crypto::generate_recovery_key();
    db.set_setting(
        RECOVERY_KEY_HASH,
        &bir_core::crypto::hash_recovery_key(&key),
    )
    .map_err(db_error)?;
    Ok(HostObject::new().field("recovery_key", key).into())
}

fn create_recovery_key() -> HostResult {
    with_database(|db| {
        require_access_mode(db)?;
        store_new_recovery_key(db)
    })
}

/// Single use: a match answers true and forgets the key. Case, dashes and
/// spaces do not matter.
fn use_recovery_key(arguments: &gpui_shell::HostArguments) -> HostResult {
    let typed = bir_core::crypto::hash_recovery_key(arguments.string(0)?);
    let matches = auth_limiter::guarded(auth_limiter::RECOVERY_KEY, || {
        with_database(|db| {
            let matches = setting(db, RECOVERY_KEY_HASH)?.is_some_and(|stored| stored == typed);
            if matches {
                db.delete_setting(RECOVERY_KEY_HASH).map_err(db_error)?;
            }
            Ok(matches)
        })
    })?;
    Ok(HostValue::Bool(matches))
}

fn mac_login_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// Ask the operating system to confirm the signed-in user (Touch ID or the
/// account password on macOS, Windows Hello elsewhere). The prompt runs on
/// its own thread, so the UI thread never waits; other platforms answer
/// false.
fn mac_login(
    arguments: &gpui_shell::HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    let reason = arguments
        .get(0)
        .and_then(HostValue::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("sign in as the admin")
        .to_owned();
    let (sender, receiver) = futures::channel::oneshot::channel::<bool>();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    std::thread::Builder::new()
        .name("mac-login".into())
        .spawn(move || {
            let _ = sender.send(os_authenticate(&reason));
        })
        .map_err(db_error)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = reason;
        let _ = sender.send(false);
    }
    Ok(async move { Ok(HostValue::Bool(receiver.await.unwrap_or(false))) })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn os_authenticate(reason: &str) -> bool {
    // The prompt (robius-authentication) is the host's: see `Hooks`.
    crate::hooks::hooks().os_authenticate(reason)
}

/// Refuse to archive or delete the solo owner (the admin) or the reserved
/// help desk root.
fn guard_admin_taxpayer(db: &Database, root: &str, action: &str) -> Result<(), HostError> {
    if root == bir_core::tax_profile::RESERVED_ADMIN_TIN_ROOT
        || solo_admin_root(db)?.as_deref() == Some(root)
    {
        return Err(HostError::new(format!(
            "This taxpayer is the admin account and cannot be {action}."
        )));
    }
    Ok(())
}

/// Archiving or deleting one branch must not take the solo owner's last
/// usable branch with it: refuse when every other branch of the admin
/// taxpayer is already `gone`.
fn guard_last_admin_branch(
    db: &Database,
    tin: &str,
    action: &str,
    gone: impl Fn(&bir_core::profile::TaxpayerProfile) -> bool,
) -> Result<(), HostError> {
    let Some(root) = solo_admin_root(db)? else {
        return Ok(());
    };
    if tin.len() < 9 || tin[..9] != root {
        return Ok(());
    }
    let Some(taxpayer) = db.get_taxpayer(&root).map_err(db_error)? else {
        return Ok(());
    };
    for branch in taxpayer.branches.iter().filter(|branch| branch.tin != tin) {
        if let Some(profile) = db.get_profile_by_tin(&branch.tin).map_err(db_error)?
            && !gone(&profile)
        {
            return Ok(());
        }
    }
    Err(HostError::new(format!(
        "This is the admin taxpayer's last branch and cannot be {action}."
    )))
}

fn set_taxpayer_archived(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    let archived = arguments.boolean(1)?;
    with_database(|db| {
        if archived {
            guard_admin_taxpayer(db, &root, "archived")?;
        }
        db.set_taxpayer_archived(&root, archived)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

fn delete_taxpayer(arguments: &gpui_shell::HostArguments) -> HostResult {
    let root = tin_root_argument(arguments.string(0)?)?;
    with_database(|db| {
        guard_admin_taxpayer(db, &root, "deleted")?;
        db.delete_taxpayer(&root)
            .map(|_| HostValue::Null)
            .map_err(db_error)
    })
}

// ---------- S3: form drafts ----------

fn list_drafts(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let year = arguments.integer(1)? as u16;
    with_database(|db| {
        let rows = db
            .list_draft_summaries(&tin, year)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Array(
            rows.iter()
                .map(|row| {
                    HostValue::from(
                        HostObject::new()
                            .field("id", row.id)
                            .field("tin", row.tin.as_str())
                            .field("form_code", row.form_code.as_str())
                            .field("taxable_year", row.taxable_year as i64)
                            .field(
                                "quarter",
                                row.quarter.map_or(HostValue::Null, HostValue::from),
                            )
                            .field("month", row.month.map_or(HostValue::Null, HostValue::from))
                            .field("status", format!("{:?}", row.status))
                            .field("updated_at", row.updated_at.as_str()),
                    )
                })
                .collect(),
        ))
    })
}

fn filing_status(name: &str) -> Result<bir_core::forms::FilingStatus, HostError> {
    use bir_core::forms::FilingStatus::*;
    match name {
        "Draft" => Ok(Draft),
        "Queued" => Ok(Queued),
        "Submitted" | "Filed" => Ok(Submitted),
        "Confirmed" => Ok(Confirmed),
        "Paid" => Ok(Paid),
        other => Err(HostError::new(format!("unknown filing status {other:?}"))),
    }
}

/// `quarter` argument is `Option<u8>`: pass null for annual/open-ended slots
/// and 0 for the first open-ended period (the DB auto-numbers those).
fn optional_quarter(
    arguments: &gpui_shell::HostArguments,
    index: usize,
) -> Result<Option<u8>, HostError> {
    match arguments.get(index) {
        Some(HostValue::Null) | None => Ok(None),
        Some(HostValue::Number(q)) => Ok(Some(*q as u8)),
        Some(other) => Err(HostError::new(format!(
            "quarter must be a number or null, got {}",
            other.describe()
        ))),
    }
}

fn get_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    with_database(|db| {
        let draft: Option<serde_json::Value> = match form_code.as_str() {
            "2551Q" => {
                let q = quarter.unwrap_or(1);
                db.get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            "1601C" => {
                let month = quarter.unwrap_or(1);
                db.get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .map(|d| {
                        serde_json::to_value(&d).map_err(|error| HostError::new(error.to_string()))
                    })
                    .transpose()?
            }
            _ => db
                .get_form_draft(&tin, &form_code, year, quarter)
                .map_err(|error| HostError::new(error.to_string()))?,
        };
        match draft {
            Some(value) => serialize(&value),
            None => Ok(HostValue::Null),
        }
    })
}

/// Fields the script reports merge over a profile-derived draft before the
/// typed save — the same overlay shape the shell UI uses, so the draft
/// keeps every engine-required field even when the UI edits a subset.
fn overlay_fields<T: serde::Serialize + serde::de::DeserializeOwned>(
    base: T,
    data: &serde_json::Value,
) -> Result<T, HostError> {
    let mut merged =
        serde_json::to_value(&base).map_err(|error| HostError::new(error.to_string()))?;
    if let (Some(target), Some(source)) = (merged.as_object_mut(), data.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(merged).map_err(|error| HostError::new(error.to_string()))
}

/// A generic save never stores a client's `trigger_date_model`,
/// `trigger_date_entered` or `trigger_parts_typed` (D8c, P5-7). The row it
/// checks is the one the save writes
/// ([`bir_core::db::Database::form_draft_for_save`]: a new open-ended
/// entry has none); that row's three fields are kept when it is the same
/// draft (`created_at`) and holds exactly these trigger fields, otherwise
/// all three are removed — this path creates no marker itself.
fn keep_stored_trigger_flag(
    db: &Database,
    tin: &str,
    form_code: &str,
    year: u16,
    quarter: Option<u8>,
    data: &mut serde_json::Value,
) -> Result<(), HostError> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    let fields = bir_core::forms::event_trigger::trigger_paths(form_code);
    let stored: Option<serde_json::Value> = if fields.is_empty() {
        None
    } else {
        db.form_draft_for_save(tin, form_code, year, quarter)
            .map_err(|error| HostError::new(error.to_string()))?
    };
    let same = stored.filter(|stored| {
        stored.get("created_at").is_some()
            && stored.get("created_at") == object.get("created_at")
            && fields
                .iter()
                .all(|field| stored.get(*field) == object.get(*field))
    });
    // The marker is host state like the typed record beside it (decision
    // P5-7): the stored row's is copied, a missing stored marker removes
    // the client's — this save is not a creator and never mints one.
    for field in [
        "trigger_date_model",
        "trigger_date_entered",
        "trigger_parts_typed",
    ] {
        match same.as_ref().and_then(|stored| stored.get(field)) {
            Some(value) => object.insert(field.into(), value.clone()),
            None => object.remove(field),
        };
    }
    Ok(())
}

/// `submitted_dry_run` and `submitted_via_relay` are the submission worker's (bir-core
/// `background_cron`, at the moment it marks a return Submitted): a save
/// never stores a client's. Whatever this save writes did not come from
/// the worker's send (a stored return that left Draft cannot be saved over).
pub(crate) fn drop_client_dry_run(data: &mut serde_json::Value) {
    let Some(object) = data.as_object_mut() else {
        return;
    };
    object.remove("submitted_dry_run");
    object.remove("submitted_via_relay");
    // The shapes that keep their lifecycle in a `lifecycle` object.
    if let Some(lifecycle) = object
        .get_mut("lifecycle")
        .and_then(serde_json::Value::as_object_mut)
    {
        lifecycle.remove("submitted_dry_run");
        lifecycle.remove("submitted_via_relay");
    }
}

fn save_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let status = filing_status(arguments.string(4)?)?;
    let mut data = to_json(arguments.value(5)?);
    drop_client_dry_run(&mut data);
    with_database(|db| {
        // The engine requires dedicated typed persistence for these two forms;
        // route them inside the host so the script keeps one uniform call.
        match form_code.as_str() {
            "2551Q" => {
                let profile = db
                    .get_profile_for_year(&tin, year)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let base = bir_core::forms::Form2551QDraft::new_from_profile(&profile, year, q);
                let draft = overlay_fields(base, &data)?;
                db.save_2551q_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            "1601C" => {
                let profile = db
                    .get_profile_for_year(&tin, year)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new(format!("no profile for TIN {tin}")))?;
                let month = quarter.ok_or_else(|| {
                    HostError::new("1601C drafts are monthly: pass the month in the quarter slot")
                })?;
                let base = bir_core::forms::Form1601CDraft::new_from_profile(&profile, year, month);
                let draft = overlay_fields(base, &data)?;
                db.save_1601c_draft(&draft)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
            _ => {
                keep_stored_trigger_flag(db, &tin, &form_code, year, quarter, &mut data)?;
                db.save_form_draft(&tin, &form_code, year, quarter, &status, &data)
                    .map(|id| HostValue::Number(id as f64))
                    .map_err(|error| HostError::new(error.to_string()))
            }
        }
    })
}

/// Every queued submission across profiles — the queue page's feed.
fn list_queued() -> HostResult {
    with_database(|db| {
        let summaries = db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?;
        let items: Vec<serde_json::Value> = summaries
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "tin": s.tin,
                    "form_code": s.form_code,
                    "taxable_year": s.taxable_year,
                    "quarter": s.quarter,
                    "month": s.month,
                    "status": format!("{:?}", s.status),
                    "updated_at": s.updated_at,
                })
            })
            .collect();
        serialize(&items)
    })
}

/// Move a saved draft into the queue. Generic forms go through
/// `QueueableKind` + `save_queued_queueable`; the two engine-guarded forms
/// use their dedicated transition + persistence, mirroring the shell form screens.
fn queue_form_draft(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    if !bir_core::forms::can_queue_for_submission(&form_code) {
        return Err(HostError::new(format!(
            "{form_code} cannot be queued for submission yet"
        )));
    }
    let queued = with_database(|db| {
        let queued: Result<i64, HostError> = (|| match form_code.as_str() {
            "2551Q" => crate::screen_form_module::out_of_line(|| {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let mut draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
                if let Some(profile) = db
                    .get_profile_for_year(&tin, year)
                    .map_err(|error| HostError::new(error.to_string()))?
                {
                    // The year's profile of the branch (its year facts over
                    // the flat fields). The transition still runs the page's
                    // Validate (`filing_errors`).
                    let _ = draft.reconcile_with_effective_profile(&profile);
                }
                // A tax fact not set that changes what it files (decision 3).
                crate::screen_form_module::refuse_unset_facts_of(db, "2551Q", &draft)?;
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_2551q_draft_and_election(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }),
            "1601C" => crate::screen_form_module::out_of_line(|| {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let mut draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
                crate::screen_form_module::refuse_unset_facts_of(db, "1601C", &draft)?;
                draft.transition_to_queued().map_err(|errors| {
                    HostError::new(
                        errors
                            .iter()
                            .map(|(field, msg)| format!("{field}: {msg}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                })?;
                db.save_queued_1601c_draft(&draft)
                    .map_err(|error| HostError::new(error.to_string()))
            }),
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                    let mut draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No saved {form_code} draft for that period"))
                        })?;
                    crate::screen_form_module::refuse_unset_facts_of(db, &form_code, &draft)?;
                    draft
                        .queue(bir_core::filing_queue::QueueAuthSource::Gui)
                        .map_err(|errors| {
                            HostError::new(
                                errors
                                    .iter()
                                    .map(|(field, msg)| format!("{field}: {msg}"))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            )
                        })?;
                    db.save_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))
                }))
            }
        })();
        queued.map(|id| HostValue::Number(id as f64))
    })?;
    wake_submission_worker();
    Ok(queued)
}

/// Cancel an unclaimed queued submission back to Draft (same guards as the
/// shell form screens: claimed or submitted work cannot be canceled this way).
fn cancel_queued(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let cancelled = with_database(|db| {
        let result: Result<(), HostError> = (|| match form_code.as_str() {
            "2551Q" => crate::screen_form_module::out_of_line(|| {
                let q = quarter.ok_or_else(|| {
                    HostError::new("2551Q drafts are quarterly: pass quarter 1-4")
                })?;
                let draft = db
                    .get_2551q_draft(&tin, year, q)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 2551Q draft for that period"))?;
                db.cancel_queued_2551q_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }),
            "1601C" => crate::screen_form_module::out_of_line(|| {
                let month = quarter
                    .ok_or_else(|| HostError::new("1601C drafts are monthly: pass the month"))?;
                let draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No queued 1601C draft for that period"))?;
                db.cancel_queued_1601c_submission(&draft)
                    .map_err(|error| HostError::new(error.to_string()))?;
                Ok(())
            }),
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                    let draft: F = db
                        .get_form_draft(&tin, &form_code, year, quarter)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No queued {form_code} draft for that period"))
                        })?;
                    db.cancel_queued_queueable(&draft)
                        .map_err(|error| HostError::new(error.to_string()))?;
                    Ok(())
                }))
            }
        })();
        result.map(|_| HostValue::Null)
    })?;
    wake_submission_worker();
    Ok(cancelled)
}

/// The sentence for an answer to a return that is not waiting for one.
const NOT_WAITING_FOR_AN_ANSWER: &str =
    "This return is not waiting for your answer about the BIR's email.";
/// The sentence while this process's worker is sending the return.
const SENDING_NOW: &str = "eBIRForms is sending this return now. Wait until it finishes.";
/// The sentence when the stored return moved on while the person answered.
const CHANGED_WHILE_ANSWERING: &str =
    "This return changed while you answered. Open it again to see where it is.";

/// The person's answer for a Queued return whose send ended with an unknown outcome: allowed only while the stored
/// return is Queued, claimed, and not being sent by this process's worker now.
fn refuse_unless_waiting_for_an_answer(
    db: &Database,
    id: Option<i64>,
    status: &bir_core::forms::FilingStatus,
    claimed: bool,
) -> Result<(), HostError> {
    if !matches!(status, bir_core::forms::FilingStatus::Queued) || !claimed {
        return Err(HostError::new(NOT_WAITING_FOR_AN_ANSWER));
    }
    let stored = match id {
        Some(id) => db
            .draft_summary_and_json(id)
            .map_err(|error| HostError::new(error.to_string()))?,
        None => None,
    };
    let Some((summary, json)) = stored else {
        return Err(HostError::new(CHANGED_WHILE_ANSWERING));
    };
    // The same rule the screens read: only a return they offer the two answers on takes one. A claimed return
    // whose upload never started has nothing to answer (nothing left this computer).
    match crate::send_state::send_of_stored(&summary, &json) {
        Some(state) if state.can_resolve => Ok(()),
        Some(state) if state.stage == "sending" => Err(HostError::new(SENDING_NOW)),
        _ => Err(HostError::new(NOT_WAITING_FOR_AN_ANSWER)),
    }
}

/// A core refusal of the person's answer, said plainly (the core sentence is logged).
fn answer_refused(error: bir_core::db::DbError) -> HostError {
    tracing::warn!("resolve_unknown_send: {error}");
    HostError::new(CHANGED_WHILE_ANSWERING)
}

/// What a person answered for a Queued return whose send ended with an unknown outcome (`send.stage` is
/// `needs_attention`): `"received"`, the BIR's confirmation email came, so it becomes Submitted as a real send; or
/// `"not_received"`, no email came, so it goes back to Draft (the human-confirmed abandoned-claim release) to be
/// queued again when the person is ready. The three storage shapes are picked as `cancel_queued` picks them; the
/// period is 2551Q's quarter, 1601C's month, or a generic form's stored period (`stored_period`; null is 0).
fn resolve_unknown_send(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year =
        u16::try_from(arguments.integer(2)?).map_err(|_| HostError::new("year is out of range"))?;
    let period = match arguments.get(3) {
        Some(HostValue::Null) | None => None,
        Some(HostValue::Number(n)) if n.fract() == 0.0 && *n >= 0.0 => Some(*n as i64),
        Some(other) => {
            return Err(HostError::new(format!(
                "period must be a whole number or null, got {}",
                other.describe()
            )));
        }
    };
    let received = match arguments.string(4)? {
        "received" => true,
        "not_received" => false,
        other => {
            return Err(HostError::new(format!(
                "answer must be \"received\" or \"not_received\", got {other:?}"
            )));
        }
    };
    let slot = |what: &str, most: i64| -> Result<u8, HostError> {
        period
            .filter(|p| (1..=most).contains(p))
            .map(|p| p as u8)
            .ok_or_else(|| HostError::new(what.to_string()))
    };
    let reason = bir_core::db::ABANDONED_CLAIM_RELEASE_REASON;
    with_database(|db| {
        let result: Result<(), HostError> = (|| match form_code.as_str() {
            "2551Q" => crate::screen_form_module::out_of_line(|| {
                let quarter = slot("2551Q drafts are quarterly: pass quarter 1-4", 4)?;
                let draft = db
                    .get_2551q_draft(&tin, year, quarter)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 2551Q return for that period"))?;
                let claimed =
                    draft.submission_claim_token.is_some() || draft.submission_claimed_at.is_some();
                refuse_unless_waiting_for_an_answer(db, draft.id, &draft.status, claimed)?;
                if received {
                    db.mark_claimed_2551q_received(&tin, year, quarter)
                        .map(|_| ())
                } else {
                    db.release_abandoned_claimed_2551q_submission(&tin, year, quarter, reason)
                        .map(|_| ())
                }
                .map_err(answer_refused)
            }),
            "1601C" => crate::screen_form_module::out_of_line(|| {
                let month = slot("1601C drafts are monthly: pass the month", 12)?;
                let draft = db
                    .get_1601c_draft(&tin, year, month)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| HostError::new("No saved 1601C return for that period"))?;
                let claimed =
                    draft.submission_claim_token.is_some() || draft.submission_claimed_at.is_some();
                refuse_unless_waiting_for_an_answer(db, draft.id, &draft.status, claimed)?;
                if received {
                    db.mark_claimed_1601c_received(&tin, year, month)
                        .map(|_| ())
                } else {
                    db.release_abandoned_claimed_1601c_submission(&tin, year, month, reason)
                        .map(|_| ())
                }
                .map_err(answer_refused)
            }),
            _ => {
                use bir_core::forms::queueable::QueueableKind;
                let kind = QueueableKind::from_form_code(&form_code)
                    .ok_or_else(|| HostError::new(format!("Form {form_code} is not queueable")))?;
                let period = period.unwrap_or(0);
                bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                    let draft: F = db
                        .get_queueable_draft(&tin, year, period)
                        .map_err(|error| HostError::new(error.to_string()))?
                        .ok_or_else(|| {
                            HostError::new(format!("No saved {form_code} return for that period"))
                        })?;
                    let lifecycle = draft.lifecycle();
                    refuse_unless_waiting_for_an_answer(
                        db,
                        draft.draft_id(),
                        &lifecycle.status,
                        !lifecycle.is_unclaimed(),
                    )?;
                    if received {
                        db.mark_claimed_queueable_received::<F>(&tin, year, period)
                            .map(|_| ())
                    } else {
                        db.release_abandoned_claimed_queueable::<F>(&tin, year, period, reason)
                            .map(|_| ())
                    }
                    .map_err(answer_refused)
                }))
            }
        })();
        result.map(|()| HostValue::Null)
    })
}

/// The painted Background Tasks page merges `job_queue` rows with every
/// queued filing submission — same two sources, same label via
/// `FilingJobLabel`.
fn list_background_jobs() -> HostResult {
    with_reader(|db| {
        let mut items: Vec<serde_json::Value> = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .iter()
            .map(|job| {
                serde_json::json!({
                    "kind": "job",
                    "id": job.id,
                    "name": job.name,
                    "job_type": job.job_type,
                    "cron_expr": job.cron_expr,
                    "status": job.status,
                    "retries": job.retries,
                    "last_run_at": job.last_run_at,
                    "next_run_at": job.next_run_at,
                    "created_at": job.created_at,
                    "output_log": job.output_log,
                })
            })
            .collect();
        let emails_by_tin: std::collections::HashMap<String, String> = db
            .list_profiles()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|profile| {
                profile
                    .tracking_mailbox()
                    .map(|email| (profile.tin.full(), email.to_string()))
            })
            .collect();
        for sum in db
            .list_all_queued_submissions()
            .map_err(|error| HostError::new(error.to_string()))?
        {
            let email = emails_by_tin.get(&sum.tin).map(String::as_str);
            items.push(serde_json::json!({
                "kind": "submission",
                "id": sum.id,
                "tin": sum.tin,
                "form_code": sum.form_code,
                "taxable_year": sum.taxable_year,
                "quarter": sum.quarter,
                "month": sum.month,
                "name": bir_core::job_display::FilingJobLabel::from_summary(&sum, email)
                    .submit_name(),
                "job_type": "System",
                "cron_expr": null,
                "status": format!("{:?}", sum.status),
                "retries": 0,
                "last_run_at": null,
                "next_run_at": null,
                "created_at": sum.updated_at,
                "output_log": null,
            }));
        }
        serialize(&items)
    })
}

fn delete_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        db.delete_job(id)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// Archive a job the painted way — status `Archived` via `save_job`.
fn archive_background_job(arguments: &gpui_shell::HostArguments) -> HostResult {
    let id = arguments.integer(0)?;
    with_database(|db| {
        let mut job = db
            .list_jobs()
            .map_err(|error| HostError::new(error.to_string()))?
            .into_iter()
            .find(|job| job.id == Some(id))
            .ok_or_else(|| HostError::new("No job with that id"))?;
        job.status = "Archived".to_string();
        db.save_job(job)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

fn purge_archived_jobs() -> HostResult {
    with_database(|db| {
        db.delete_archived_jobs()
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Null)
    })
}

/// A JavaScript screen failure. `console.*` already reaches tracing as
/// `gpui_shell::script`; this is the path a caught screen error takes when
/// it never calls `console` (a failed host read, a render throw).
fn record_screen_log(arguments: &gpui_shell::HostArguments) -> HostResult {
    let level = arguments.string(0)?.to_owned();
    let message = arguments.string(1)?.to_owned();
    crate::logging::record_screen_log(&level, &message);
    Ok(HostValue::Null)
}

/// Today's rotated log (`ebirforms.YYYY-MM-DD.log`), tailed like the painted
/// Logs tab — it keeps at most the newest 5,000 lines.
pub(crate) fn job_log_tail() -> HostResult {
    let logs_path = bir_core::log_files::today_log_path(&bir_core::log_files::log_dir());
    let file_name = logs_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (content, lines) = match std::fs::read_to_string(&logs_path) {
        Ok(text) => {
            let total = text.lines().count();
            let tail = if total <= 5_000 {
                text
            } else {
                text.lines()
                    .skip(total - 5_000)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            (tail, total)
        }
        Err(_) => ("No log entries yet today.".to_string(), 0),
    };
    serialize(&serde_json::json!({
        "file_name": file_name,
        "lines": lines,
        "content": content,
    }))
}

/// The frozen print-sheet slug each form's painted `PRINT_SLUG` names.
pub(crate) fn print_slug_for(form_code: &str) -> Option<&'static str> {
    Some(match form_code {
        "0605" => "0605-1999",
        "0619E" => "0619e-2018",
        "0619F" => "0619f-2018",
        "1600PT" => "1600-pt-2018",
        "1600VT" => "1600-vt-2018",
        "1600WP" => "1600wp-2010",
        "1601C" => "1601c-2018",
        "1601EQ" => "1601eq-2019",
        "1601FQ" => "1601-fq-2020",
        "1602Q" => "1602q-2019",
        "1603Q" => "1603q-2018",
        "1604C" => "1604c-2018",
        "1604E" => "1604e-2018",
        "1604F" => "1604f-2018",
        "1606" => "1606-2018",
        "1700" => "1700-2018",
        "1701" => "1701-2018",
        "1701A" => "1701a-2018",
        "1701MS" => "1701ms-2024",
        "1701Q" => "1701q-2018",
        "1702EX" => "1702ex-2018",
        "1702MX" => "1702mx-2018c",
        "1702Q" => "1702q-2018",
        "1702RT" => "1702rt-2018c",
        "1706" => "1706-2018",
        "1707" => "1707-2021",
        "1707A" => "1707a-2021",
        "1800" => "1800-2018",
        "1801" => "1801-2018",
        "2000" => "2000-dst-2018",
        "2000OT" => "2000-ot-2018",
        "2200A" => "2200a-2020",
        "2200AN" => "2200an-2018",
        "2200C" => "2200c-2018",
        "2200M" => "2200m-2018",
        "2200P" => "2200p-2020",
        "2200S" => "2200s-2018",
        "2200T" => "2200t-2020",
        "2550M" => "2550m-2007",
        "2550Q" => "2550q-2024",
        "2551Q" => "2551q-2018",
        "2552" => "2552-2018",
        "2553" => "2553-1999",
        _ => return None,
    })
}

/// The receipt page a confirmed draft appends — the shell port of the painted
/// `form_html_preview_launcher::receipt_page_for`.
pub(crate) fn receipt_page(
    db: &Database,
    receipt_id: Option<i64>,
    mailbox: &str,
) -> Option<bir_print::frozen_html::ReceiptPage> {
    let receipt_id = receipt_id?;
    let receipt = db.get_submission_receipt_by_id(receipt_id).ok().flatten()?;
    Some(bir_print::frozen_html::ReceiptPage {
        filename: receipt.filename.clone(),
        subject: "Tax Return Receipt Confirmation".to_string(),
        from: receipt
            .source_from
            .clone()
            .unwrap_or_else(|| "ebirforms-noreply@bir.gov.ph".to_string()),
        to: mailbox.to_string(),
        received_at: bir_core::background_cron::display_received_at(
            &receipt.received_date,
            &receipt.received_time,
        ),
        email_received_at: receipt
            .email_received_at
            .as_deref()
            .and_then(bir_core::receipt::display_email_received_at),
        body_text: receipt.raw_text.clone(),
        body_html: receipt
            .raw_html
            .as_deref()
            .map(bir_core::receipt::sanitized_receipt_html),
    })
}

/// The stored draft's print field map (writer key -> value) and, for a
/// confirmed return, its receipt page: what `form_print_html` fills the
/// frozen sheet with.
type PrintFieldsResult = Result<
    (
        std::collections::BTreeMap<String, String>,
        Option<bir_print::frozen_html::ReceiptPage>,
        bool,
    ),
    HostError,
>;

pub(crate) fn print_fields(
    db: &Database,
    tin: &str,
    form_code: &str,
    year: u16,
    quarter: Option<u8>,
) -> PrintFieldsResult {
    match form_code {
        "2551Q" => crate::screen_form_module::out_of_line(|| {
            let q = quarter.ok_or_else(|| HostError::new("2551Q drafts are quarterly"))?;
            let draft = db
                .get_2551q_draft(tin, year, q)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 2551Q draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email);
            Ok((
                draft.to_print_field_map(),
                page,
                is_dry_run(&draft.status, draft.submitted_dry_run),
            ))
        }),
        "1601C" => crate::screen_form_module::out_of_line(|| {
            let month = quarter.ok_or_else(|| HostError::new("1601C drafts are monthly"))?;
            let draft = db
                .get_1601c_draft(tin, year, month)
                .map_err(|error| HostError::new(error.to_string()))?
                .ok_or_else(|| HostError::new("No saved 1601C draft for that period"))?;
            let page = receipt_page(db, draft.receipt_id, &draft.email_address);
            Ok((
                draft.to_print_field_map(),
                page,
                is_dry_run(&draft.status, draft.submitted_dry_run),
            ))
        }),
        _ => {
            use bir_core::forms::queueable::QueueableKind;
            let kind = QueueableKind::from_form_code(form_code)
                .ok_or_else(|| HostError::new(format!("Form {form_code} is not printable")))?;
            bir_core::with_queueable_kind!(kind, F => crate::screen_form_module::out_of_line(|| {
                let draft: F = db
                    .get_form_draft(tin, form_code, year, quarter)
                    .map_err(|error| HostError::new(error.to_string()))?
                    .ok_or_else(|| {
                        HostError::new(format!("No saved {form_code} draft for that period"))
                    })?;
                let page = receipt_page(
                    db,
                    draft.lifecycle().receipt_id,
                    draft.submission_email(),
                );
                Ok((draft.print_field_map(), page, is_dry_run(&draft.lifecycle().status, draft.lifecycle().submitted_dry_run)))
            }))
        }
    }
}

/// Fill the form's frozen print sheet, write it to a temp HTML file, and open
/// it with the platform's default handler (the shell app opens it in a wry
/// preview window — the shell has no wry, and `cx.open_url` is HTTP-only, so
/// the printable equivalent is the system browser via xdg-open/open/start).
fn form_print_html(arguments: &gpui_shell::HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    let form_code = arguments.string(1)?.to_owned();
    let year = arguments.integer(2)? as u16;
    let quarter = optional_quarter(arguments, 3)?;
    let slug = print_slug_for(&form_code)
        .ok_or_else(|| HostError::new(format!("Form {form_code} has no print sheet")))?;
    with_database(|db| {
        let (fields, receipt, test_send) = print_fields(db, &tin, &form_code, year, quarter)?;
        let profile = db
            .get_profile_by_tin(&tin)
            .ok()
            .flatten()
            .map(|p| p.full_name.clone());
        open_print_preview(
            slug,
            &form_code,
            year,
            &fields,
            receipt.as_ref(),
            test_send,
            profile,
        )
    })
}

/// Fill `slug`'s frozen sheet with `fields`, write it to the app's own
/// `data_dir()/previews/` folder and hand it to the preview host. Answers
/// `{ path }`. The file is transient: the host reads it synchronously and this
/// function removes anything still left when the host returns.
pub(crate) fn open_print_preview(
    slug: &str,
    form_code: &str,
    year: u16,
    fields: &std::collections::BTreeMap<String, String>,
    receipt: Option<&bir_print::frozen_html::ReceiptPage>,
    test_send: bool,
    profile_name: Option<String>,
) -> HostResult {
    let html = if test_send {
        bir_print::frozen_html::filled_document_test_send(slug, fields)
    } else {
        bir_print::frozen_html::filled_document_with_receipt(slug, fields, receipt)
    }
    .map_err(HostError::new)?;
    // A4 layout + static markup (no scripts, values as text): prints one A4
    // page per form page from any engine; `print-overrides/<slug>.css` on top.
    let html =
        crate::static_html::staticize(&crate::print_sheet::prepare_html_for(slug, &html, true));
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let root = bir_core::platform::print_previews_dir();
    std::fs::create_dir_all(&root).map_err(|error| HostError::new(error.to_string()))?;
    let path = root.join(format!("bir-{form_code}-{year}-{timestamp}-preview.html"));
    std::fs::write(&path, html).map_err(|error| HostError::new(error.to_string()))?;
    // In-app preview window (Blitz view + hidden WebKit PDF for Print/Share);
    // one window per form, re-opening refreshes it in place.
    let title = match profile_name.filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("BIR Form {form_code} \u{2014} {name}"),
        None => format!("BIR Form {form_code}"),
    };
    crate::hooks::hooks().print_preview_request(path.clone(), title, form_code.to_string());
    if let Err(error) = std::fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(path = %path.display(), %error, "print preview source not removed");
    }
    Ok(HostObject::new()
        .field("path", path.display().to_string())
        .into())
}

/// Launch the platform's default HTML viewer on `path` — detached, so the
/// host call returns immediately like the painted preview window does.
#[allow(dead_code)]
fn open_in_system_browser(path: &std::path::Path) -> Result<(), HostError> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args([
            "/c",
            "start",
            "",
            path.as_os_str().to_string_lossy().as_ref(),
        ]);
        cmd
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(path);
        cmd
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| HostError::new(error.to_string()))
}

/// Close the live connection — used before a database-replace import so the
/// next `with_database` reopens the freshly extracted file (and re-acquires
/// the owner lock the closed connection was holding). The submission worker
/// shares the handle, so the connection is swapped for an in-memory
/// placeholder inside it rather than the handle being dropped: dropping it
/// would leave the worker holding the old file and its owner lock.
pub(crate) fn reset_database() -> Result<(), HostError> {
    // A folder scanned for the eBIRForms import was reviewed against the
    // database that is going away: it is not imported into the next one.
    crate::legacy_import_module::forget_scan();
    DATABASE.with(|slot| {
        let slot = slot.borrow();
        let Some(handle) = slot.as_ref() else {
            return Ok(());
        };
        // The reader holds the file open too; the reopen opens a new one.
        drop_reader();
        let placeholder = Database::open_ephemeral().map_err(db_error)?;
        *bir_core::lock_timing::lock(handle)
            .map_err(|_| HostError::new("database lock poisoned"))? = placeholder;
        RELEASED.set(true);
        Ok(())
    })
}

/// Ask the host to open a native path panel and run `work` with the path the
/// user picked.
///
/// A host call runs inside a UI update. A blocking panel would spin a nested
/// modal run loop there, so the host opens the panel after the current update
/// and answers through [`crate::hooks::PathAnswer`], on the thread that owns
/// the database slot, outside any UI borrow; `work` runs in that answer. The
/// promise answers `{ kind: "cancelled" }` when the panel is dismissed.
pub(crate) fn after_path_prompt(
    prompt: impl FnOnce(crate::hooks::PathAnswer) -> bool,
    work: impl FnOnce(std::path::PathBuf) -> HostResult + 'static,
) -> Result<gpui_shell::host_modules::HostFuture, HostError> {
    use crate::hooks::PathChoice;
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    let answer: crate::hooks::PathAnswer = Box::new(move |choice| {
        let result = match choice {
            PathChoice::Picked(path) => {
                let done = work(path);
                if let Err(error) = &done {
                    tracing::warn!("Working with the file picked failed: {error}");
                }
                done
            }
            PathChoice::Cancelled => Ok(HostObject::new().field("kind", "cancelled").into()),
            PathChoice::Failed(error) => {
                tracing::warn!("The file dialog could not open: {error}");
                Err(HostError::new(format!(
                    "The file dialog could not open: {error}"
                )))
            }
        };
        let _ = sender.send(result);
    });
    if !prompt(answer) {
        tracing::warn!("The file dialog could not open: no app is running to open it");
        return Err(HostError::new("no app is running to open the file dialog"));
    }
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("the file dialog ended without an answer")))
    }))
}

/// The page's import. The host always opens the file dialog (the script has
/// none), so the page cannot choose the file: a call that passes any
/// argument is refused before anything opens. Tests and scripted imports
/// call [`import_archive_from`] instead.
fn import_archive(
    arguments: &gpui_shell::HostArguments,
) -> Result<gpui_shell::host_modules::HostFuture, HostError> {
    if !arguments.is_empty() {
        return Err(HostError::new(
            "import_archive takes no path; the host opens the file dialog",
        ));
    }
    after_path_prompt(
        |answer| crate::hooks::hooks().prompt_for_open_path("Import", answer),
        |path| import_archive_from(&path),
    )
}

/// Import the archive at `path`; not a verb. Archive handling stays
/// host-side and mirrors `ImportExportView`: a zip with `bir_data.db` is a
/// full database backup, one with any `profile.json` — top-level, or under
/// `Profiles/<TIN>/` in a Settings all-profiles backup — is a profile
/// export, anything else is invalid.
fn import_archive_from(path: &std::path::Path) -> HostResult {
    // What `import_profile_data` accepts: any `profile.json` — top-level
    // (one profile) or under `Profiles/<TIN>/` (the Settings all-profiles
    // backup). A database backup's marker is a top-level `bir_data.db`.
    let entry_names: Vec<String> = std::fs::File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok())
        .map(|mut archive| {
            (0..archive.len())
                .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let is_db = entry_names.iter().any(|name| name == "bir_data.db");
    let is_profile = entry_names
        .iter()
        .any(|name| name.ends_with("profile.json"));

    if is_db {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        return restore_database_archive(path, stamp);
    }

    if is_profile {
        return with_database(|db| match bir_core::import::import_profile_data(db, path) {
            Ok(()) => Ok(HostObject::new().field("kind", "profile").into()),
            Err(error) => Err(HostError::new(error.to_string())),
        });
    }

    Ok(HostObject::new().field("kind", "invalid").into())
}

/// Replace the open database with the one in the database archive at `archive`.
///
/// Painted flow (import_export.rs), in an order where nothing live changes
/// until the archive is known good and the restore point exists:
///
/// 1. extract the archive's database next to the live file and check it (the
///    plain SQLite header, `PRAGMA integrity_check`, the app's core tables, a
///    schema version this build opens); a bad archive stops here;
/// 2. save a safety copy of the live file under `stamp`; if that fails the
///    import stops here;
/// 3. drop the cached connection (it holds the owner lock and an open handle
///    on the file being replaced) and put the checked file in its place; if
///    that fails, the connection is reopened on the file that is still there.
fn restore_database_archive(archive: &std::path::Path, stamp: u64) -> HostResult {
    let db_path = db::app_database_path();
    let staged = bir_core::import::stage_database_zip(archive, &db_path)
        .map_err(|error| HostError::new(error.to_string()))?;
    restore_staged_database(staged, &db_path, stamp)
}

#[cfg(test)]
thread_local! {
    /// Runs once on the restoring thread between the checkpoint and the safety
    /// copy, with the database lock held: a test's writer tries to commit there.
    static AFTER_RESTORE_CHECKPOINT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Steps 2 and 3 of [`restore_database_archive`] for a database already
/// staged and checked. The staged file is removed on every path out.
fn restore_staged_database(
    staged: bir_core::import::StagedDatabase,
    db_path: &std::path::Path,
    stamp: u64,
) -> HostResult {
    if db_path.exists()
        && let Some(parent) = db_path.parent()
    {
        crate::account_gate::ensure_unlocked()?;
        let handle = open_database_slot()?;
        // One lock across the checkpoint, the safety copy and the swap. The
        // submission worker writes through this same connection (the one
        // `Arc<Mutex<Database>>`), and no other connection of this process
        // writes (the reader is read-only, another process is kept out by the
        // owner lock), so while it is held nothing commits and no
        // auto-checkpoint writes the main file: the copy holds every commit,
        // and the connection let go is the one it was copied from.
        let mut db = bir_core::lock_timing::lock(&handle)
            .map_err(|_| HostError::new("database lock poisoned"))?;
        // Flush the WAL first: a raw copy of bir_data.db alone can miss
        // commits still living in -wal, and this copy is the restore
        // point if the replace goes wrong. A checkpoint a reader keeps from
        // finishing is an error: the import stops here, before the copy and
        // before the install deletes any -wal or -shm, with the live database
        // as it was.
        db.checkpoint().map_err(|error| match error {
            // The one a person can act on, in words without the log's detail.
            bir_core::db::DbError::InUseElsewhere(detail) => {
                tracing::warn!("database import stopped: checkpoint held back ({detail})");
                HostError::new(
                    "The database is in use elsewhere (another program has it open, or this app is still reading it), so nothing was imported. Wait a moment, close any other program that uses it, and try again. The current database was left as it is.",
                )
            }
            error => HostError::new(format!(
                "Could not save a safety copy of the current database ({error}), so nothing was imported. The current database was left as it is."
            )),
        })?;
        #[cfg(test)]
        AFTER_RESTORE_CHECKPOINT.with(|hook| hook.borrow_mut().take().map(|hook| hook()));
        let copy = parent.join(format!("bir_data.safety-{stamp}.db"));
        if let Err(error) = std::fs::copy(db_path, &copy) {
            let _ = std::fs::remove_file(&copy);
            return Err(HostError::new(format!(
                "Could not save a safety copy of the current database ({error}), so nothing was imported. The current database was left as it is."
            )));
        }
        // What `reset_database` does, under the lock already held: the
        // database a folder was scanned against for the eBIRForms import
        // goes away here, so that scan is let go and never imported into the
        // restored one.
        crate::legacy_import_module::forget_scan();
        drop_reader();
        *db = Database::open_ephemeral().map_err(db_error)?;
        RELEASED.set(true);
    } else {
        reset_database()?;
    }
    match staged.install() {
        Ok(()) => Ok(HostObject::new().field("kind", "database").into()),
        Err(error) => {
            // Back on the file that is still there, now rather than at the next call.
            let _ = open_database_slot();
            Err(HostError::new(error.to_string()))
        }
    }
}

/// The page's export: `tin`'s profile as a `.zip` archive under the host's
/// save dialog. The page never chooses the file — the same rule as
/// `import_archive` — so anything after `(tin, pin)`, or a `pin` that is
/// not a string or `null`, is refused before anything opens. Tests and
/// scripted exports call [`export_profile_to`] instead.
///
/// A taxpayer with a PIN exports only under it: the PIN is checked under
/// the same rate limiter as [`verify_taxpayer_pin`], and a refusal —
/// "This profile's PIN is required to export it." or "Wrong PIN." —
/// happens before the dialog, so no archive exists without authorization.
/// A taxpayer without a PIN exports ungated; an authenticator alone is not
/// a PIN.
fn export_profile(
    arguments: &gpui_shell::HostArguments,
) -> Result<gpui_shell::host_modules::HostFuture, HostError> {
    if arguments.len() > 2 {
        return Err(HostError::new(
            "export_profile takes no path; the host opens the save dialog",
        ));
    }
    let tin = arguments.string(0)?.to_string();
    let pin = match arguments.get(1) {
        None | Some(HostValue::Null) => None,
        Some(HostValue::Str(pin)) => Some(pin.clone()),
        Some(_) => {
            return Err(HostError::new(
                "export_profile takes no path; the host opens the save dialog",
            ));
        }
    };
    // The profile names its taxpayer's root, the way `set_profile_pin`
    // resolves a branch TIN; an unknown TIN is refused before the dialog.
    let tin_root =
        with_database(|db| Ok(get_profile_by_tin(db, &tin)?.tin.full()[..9].to_string()))?;
    let mut rehash = None;
    let authorized = auth_limiter::guarded_client(&tin_root, || {
        with_database(|db| {
            // The taxpayer's PIN, else a branch copy: the PIN the export itself
            // checks (`bir_core::export::client_pin_hash`).
            match bir_core::export::client_pin_hash(db, &tin).map_err(db_error)? {
                // Nothing to check — an authenticator alone is not a PIN —
                // and nothing counts against the limiter.
                None => Ok(true),
                Some(_) if pin.is_none() => Err(HostError::new(
                    "This profile's PIN is required to export it.",
                )),
                Some(hash) => Ok(pin.as_deref().is_some_and(|pin| {
                    let (matched, upgrade) = client_pin_check(&tin_root, Some(hash.as_str()), pin);
                    rehash = upgrade;
                    matched
                })),
            }
        })
    })?;
    start_rehash_after(authorized, rehash);
    if !authorized {
        return Err(HostError::new("Wrong PIN."));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let file_name = format!("BIR_Profile_{}_{}.zip", tin, timestamp);
    if let Some(download) = crate::hooks::hooks().export_profile_download(&file_name, &mut |path| {
        export_profile_to(&tin, pin.as_deref(), path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }) {
        return Ok(Box::pin(downloaded_export(download)));
    }
    let directory = std::env::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    after_path_prompt(
        move |answer| crate::hooks::hooks().prompt_for_save_path(&directory, &file_name, answer),
        move |path: std::path::PathBuf| export_profile_to(&tin, pin.as_deref(), &path),
    )
}

async fn downloaded_export(download: crate::hooks::DownloadExport) -> HostResult {
    Ok(match download.await.map_err(HostError::new)? {
        Some(path) => HostObject::new()
            .field("kind", "exported")
            .field("path", path)
            .into(),
        None => HostObject::new().field("kind", "cancelled").into(),
    })
}

/// Write `tin`'s profile archive to `path`; not a verb. The PIN gate is
/// bir-core's: [`export_profile_data`](bir_core::export_profile_data)
/// refuses a PIN'd taxpayer without its PIN before the file exists, so a
/// refusal here leaves nothing on disk either.
fn export_profile_to(tin: &str, pin: Option<&str>, path: &std::path::Path) -> HostResult {
    with_database(|db| {
        bir_core::export_profile_data(db, tin, path, pin).map_err(db_error)?;
        Ok(HostObject::new()
            .field("kind", "exported")
            .field("path", path.display().to_string())
            .into())
    })
}

/// Tax deadlines feed for the calendar page.
fn list_deadlines() -> HostResult {
    with_database(|db| {
        db.list_tax_deadlines()
            .map_err(|error| HostError::new(error.to_string()))
            .and_then(|deadlines| serialize(&deadlines))
    })
}

/// Pick up route-3 filings the account server is holding for this computer.
///
/// Each row is gated by the standing send OK and by the host's agent send
/// ([`crate::hooks::Hooks::agent_send_draft`]),
/// which runs the page's Validate before queueing. The server never contacts BIR.
#[cfg(not(feature = "agent"))]
fn pull_device_filings() -> HostResult {
    Ok(HostValue::Null)
}

#[cfg(feature = "agent")]
fn pull_device_filings() -> HostResult {
    pull_device_filings_agent()
}

#[cfg(feature = "agent")]
fn pull_device_filings_agent() -> HostResult {
    let Some((origin, token)) = crate::cloud_account::device_poll_auth() else {
        return Ok(HostValue::Null);
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| HostError::new(error.to_string()))?;
    let url = format!("{}/api/device-filings", origin.trim_end_matches('/'));
    let response = match client
        .get(&url)
        .header("authorization", format!("Bearer {token}"))
        .send()
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Ok(HostValue::Null),
    };
    let text = response.text().unwrap_or_default();
    let body: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let Some(filings) = body.get("filings").and_then(|value| value.as_array()) else {
        return Ok(HostValue::Null);
    };
    let mut message: Option<String> = None;
    for filing in filings {
        let id = filing
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let outcome = with_database(|db| accept_device_filing(db, filing).map_err(HostError::new))?;
        let status = outcome
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("failed");
        // A missing grant leaves the server row queued. Invalid and picked
        // are reported so the assistant can read them.
        if status != "approval_required" {
            let report = if status == "queued" { "picked" } else { status };
            let payload = serde_json::json!({
                "status": report,
                "error_summary": outcome.get("message").and_then(|value| value.as_str()).unwrap_or(""),
                "field_errors": outcome.get("field_errors").cloned().unwrap_or(serde_json::json!([])),
            });
            let _ = client
                .post(format!("{url}/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(payload.to_string())
                .send();
        }
        if status == "queued" {
            wake_submission_worker();
            message = Some(
                outcome
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Picked up a queued filing.")
                    .to_string(),
            );
        } else if status == "approval_required" {
            message = Some(
                "A filing is waiting. Grant a standing OK on the taxpayer page before it can upload."
                    .to_string(),
            );
        } else if status == "invalid" {
            message = Some(
                outcome
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or("A queued filing failed validation.")
                    .to_string(),
            );
        }
    }
    match message {
        Some(message) => serialize(&serde_json::json!({"message": message, "icon": "send"})),
        None => Ok(HostValue::Null),
    }
}

#[cfg(feature = "agent")]
fn accept_device_filing(
    db: &bir_core::db::Database,
    filing: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bir_core::forms::agent::{self, AgentFormKind};
    let code = filing
        .get("form_code")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let kind = AgentFormKind::from_code(code).ok_or_else(|| format!("unknown form `{code}`"))?;
    let tin = filing
        .get("tin")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let year = filing
        .get("tax_year")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u16;
    let period = filing
        .get("period")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u8;
    if !db
        .has_send_grant(&tin, kind.form_code())
        .map_err(|error| error.to_string())?
    {
        return Ok(serde_json::json!({
            "ok": false,
            "status": "approval_required",
            "pending": true,
            "queued": false,
            "filed": false,
            "form": kind.form_code(),
            "message": "A filing is waiting for a standing OK.",
        }));
    }
    let profile = db
        .get_profile_for_year(&tin, year)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no taxpayer profile for {tin}"))?;
    let mut draft =
        crate::screen_form_module::load_draft(db, kind, &profile, year, period, false, None)?;
    if let Some(fields) = filing.get("fields").and_then(|value| value.as_object()) {
        let mut fill_errors = Vec::new();
        for (key, value) in fields {
            if let Err(message) = agent::fill(kind, &mut draft, key, value, None) {
                fill_errors.push(serde_json::json!({"field": key, "message": message}));
            }
        }
        if !fill_errors.is_empty() {
            return Ok(serde_json::json!({
                "ok": false,
                "status": "invalid",
                "queued": false,
                "filed": false,
                "form": kind.form_code(),
                "field_errors": fill_errors,
                "message": "A box could not be filled. Nothing was queued.",
            }));
        }
    }
    crate::hooks::hooks().agent_send_draft(db, kind, &mut draft)
}

/// The script is on the main window (`open`) or back on the gate, first
/// launch, or the sign-in screen. Rebuilds the menu bar to match.
fn shell_set_chrome(arguments: &gpui_shell::HostArguments) -> HostResult {
    let open = arguments.boolean(0)?;
    crate::hooks::hooks().set_shell_chrome_open(open);
    crate::hooks::hooks().refresh_menu();
    Ok(HostValue::Null)
}

/// Next command the agent host queued for the script, or null.
///
/// The TCP thread never touches the window. It posts work; the GPUI timer
/// runs the host op and pushes a command here; the script polls this.
fn agent_take() -> HostResult {
    // Queued UI commands are tools. A locked gate must not apply them.
    crate::account_gate::ensure_unlocked()?;
    let command = crate::hooks::hooks().agent_pop_command();
    match command {
        Some(value) => Ok(HostValue::Str(value.to_string())),
        None => Ok(HostValue::Null),
    }
}

/// The module the app's JavaScript imports as `"bir"`.
pub fn module() -> HostModule {
    let module = HostModule::new("bir")
        .function("system_info", |_| system_info())
        .function("set_global_hotkey", set_global_hotkey)
        .function("set_log_retention", set_log_retention)
        .function("update_status", |_| update_status())
        .function("install_update", |_| install_update())
        .function("dismiss_update", |_| dismiss_update())
        .function("check_for_updates", |_| check_for_updates())
        .function("set_auto_update_download", set_auto_update_download)
        .function("list_profiles", |_| list_profiles())
        .function("forms_catalog", |_| forms_catalog())
        .function("get_setting", get_setting)
        .function("set_setting", set_setting)
        .function("profile_detail", profile_detail)
        .function("save_profile", save_profile)
        .function("set_profile_archived", set_profile_archived)
        .function("delete_profile", delete_profile)
        .function("set_profile_pin", set_profile_pin)
        .function("set_profile_totp", set_profile_totp)
        .function("auth_state", |_| auth_state())
        .function("taxpayer_auth", taxpayer_auth)
        .function("set_taxpayer_pin", set_taxpayer_pin)
        .function("set_taxpayer_totp", set_taxpayer_totp)
        .function("verify_taxpayer_pin", verify_taxpayer_pin)
        .function("verify_taxpayer_totp", verify_taxpayer_totp)
        .function("verify_admin_pin", verify_admin_pin)
        .function("verify_admin_totp", verify_admin_totp)
        .function("set_admin_pin", set_admin_pin)
        .function("set_admin_totp", set_admin_totp)
        .function("setup_solo", setup_solo)
        .function("setup_helpdesk", setup_helpdesk)
        .function("change_owner", change_owner)
        .function("change_to_helpdesk", change_to_helpdesk)
        .function("create_recovery_key", |_| create_recovery_key())
        .function("use_recovery_key", use_recovery_key)
        .async_function("mac_login", mac_login)
        .function("set_taxpayer_archived", set_taxpayer_archived)
        .function("delete_taxpayer", delete_taxpayer)
        .function("generate_totp_secret", generate_totp_secret)
        .function("delete_setting", delete_setting)
        .function("list_drafts", list_drafts)
        .function("get_form_draft", get_form_draft)
        .function("save_form_draft", save_form_draft)
        .function("list_queued", |_| list_queued())
        .function("queue_form_draft", queue_form_draft)
        .function("cancel_queued", cancel_queued)
        .function("resolve_unknown_send", resolve_unknown_send)
        .function("list_deadlines", |_| list_deadlines())
        .async_function("import_archive", import_archive)
        .async_function("export_profile", export_profile)
        .function("form_print_html", form_print_html)
        .function("list_background_jobs", |_| list_background_jobs())
        .function("delete_background_job", delete_background_job)
        .function("archive_background_job", archive_background_job)
        .function("purge_archived_jobs", |_| purge_archived_jobs())
        .function("job_log_tail", |_| job_log_tail())
        .function("record_screen_log", record_screen_log)
        .function("agent_take", |_| agent_take())
        .function("pull_device_filings", |_| pull_device_filings())
        .function("shell_set_chrome", shell_set_chrome);
    let module = crate::cloud_account::register(module);
    let module = crate::cloud_email::register(module);
    let module = crate::tax_profile_module::register(module);
    let module = crate::screen_form_module::register(module);
    let module = crate::dashboard_module::register(module);
    let module = crate::legacy_import_module::register(module);
    let module = crate::owner_lock_module::register(module);
    // bir-shell appends its scroll exports (GPUI elements) after these.
    module.declarations(format!(
        "{BASE_DECLARATIONS}{}{}{}{}{}{}{}",
        crate::cloud_account::DECLARATIONS,
        crate::cloud_email::DECLARATIONS,
        crate::tax_profile_module::DECLARATIONS,
        crate::screen_form_module::DECLARATIONS,
        crate::dashboard_module::DECLARATIONS,
        crate::legacy_import_module::DECLARATIONS,
        crate::owner_lock_module::DECLARATIONS,
    ))
}

/// TypeScript for the functions registered in this file. The tax-profile and
/// screen-form files append their own (`declarations` replaces, so `module`
/// concatenates them).
const BASE_DECLARATIONS: &str = r#"
export interface HotkeyPreset {
    value: string;
    label: string;
}
export interface SystemInfo {
    os: string;
    arch: string;
    app_version: string;
    database_path: string;
    data_dir: string;
    logs_dir: string;
    /** Null when no global show chord is stored. */
    global_hotkey: string | null;
    hotkey_presets: HotkeyPreset[];
    log_retention_days: number;
    log_retention_choices: number[];
    runtime: string;
}
export interface ProfileSummary {
    id: number;
    tin: string;
    full_name: string;
    rdo_code: string;
    email: string;
    taxpayer_type: string;
    is_vat_registered: boolean;
    is_archived: boolean;
    default_form_type: string;
    applicable_forms: string[];
}
export interface FormCatalogEntry {
    code: string;
    title: string;
    category: string;
    frequency: string;
    requires_employees: boolean;
    is_deprecated: boolean;
}
export function system_info(): SystemInfo;
/** Empty string clears the chord. An unknown key is refused. */
export function set_global_hotkey(combo: string): null;
/** Days of daily log files to keep. Out of range is refused. */
export function set_log_retention(days: number): null;

export interface UpdateStatus {
    /** idle, checking, downloading, ready, failed, up_to_date, available, or installing. */
    phase: string;
    /** The release that was found, or null. */
    version: string | null;
    /** A sentence for a failure or "already current", or null. */
    message: string | null;
    /** Whether a newer release downloads without asking. */
    auto_download: boolean;
    /** The version compiled into this binary. */
    current_version: string;
}
export function update_status(): UpdateStatus;
/** Replace this install with the verified download and relaunch. */
export function install_update(): null;
/** Hide the update prompt until the next check. */
export function dismiss_update(): null;
/** Look for a release now, and download it even when auto-download is off. */
export function check_for_updates(): null;
/** `auto_update_download`. Off is `0`; anything else leaves downloading on. */
export function set_auto_update_download(enabled: boolean): null;
export function list_profiles(): ProfileSummary[];
export function forms_catalog(): FormCatalogEntry[];
export function get_setting(key: string): string | null;
export function set_setting(key: string, value: string): null;

export interface AuthState {
    /** null on a fresh install that has not chosen yet. */
    mode: "solo" | "helpdesk" | null;
    /** solo: the owner's TIN root; helpdesk: "000000000"; null if unset. */
    admin_root: string | null;
    /** solo: the owner taxpayer's PIN; helpdesk: the admin PIN setting. */
    has_admin_pin: boolean;
    has_admin_totp: boolean;
    has_recovery_key: boolean;
    /** Touch ID / account password (macOS) or Windows Hello can confirm the admin. */
    mac_login_available: boolean;
}
export interface ProfileAuth {
    has_pin: boolean;
    has_totp: boolean;
}
export interface TotpSecret {
    secret: string;
    qr_path: string;
}
export interface RecoveryKey {
    /** Shown once, e.g. "7K3M-Q9TB-0XWD-H2RE"; only its hash is kept. */
    recovery_key: string;
}
export function profile_detail(tin: string): object;
export function save_profile(profile: object): object;
export function set_profile_archived(tin: string, archived: boolean): object;
export function delete_profile(tin: string): null;
/** Sets the PIN of the branch's taxpayer (one PIN per TIN root). */
export function set_profile_pin(tin: string, pin: string | null): null;
/** Sets the authenticator of the branch's taxpayer; setting needs a current code. */
export function set_profile_totp(tin: string, secret: string | null, token?: string): null;
export function auth_state(): AuthState;
/** PIN and authenticator of a taxpayer (TIN root), shared by every branch. */
export function taxpayer_auth(tin_root: string): ProfileAuth;
/** pin: exactly 4 digits, or null to clear. */
export function set_taxpayer_pin(tin_root: string, pin: string | null): null;
/** Set (with a current code from the app) or clear (null) a taxpayer's authenticator. */
export function set_taxpayer_totp(tin_root: string, secret: string | null, code?: string): null;
export function verify_taxpayer_pin(tin_root: string, pin: string): boolean;
export function verify_taxpayer_totp(tin_root: string, code: string): boolean;
/** solo: the owner taxpayer's PIN; helpdesk: the admin PIN. false when not set up. */
export function verify_admin_pin(pin: string): boolean;
export function verify_admin_totp(code: string): boolean;
/** Help desk refuses to clear the PIN while there is no admin authenticator. */
export function set_admin_pin(pin: string | null): null;
/** Setting needs a current code; help desk refuses to clear it while there is no admin PIN. */
export function set_admin_totp(secret: string | null, code?: string): null;
/** First launch, after creating the owner's taxpayer. Refused once set up. */
export function setup_solo(tin_root: string): null;
/** First launch for a help desk: admin PIN plus a new recovery key. Refused once set up. */
export function setup_helpdesk(pin: string): RecoveryKey;
/** Admin only: make this taxpayer the owner (solo), from solo or help desk. Needs the current admin PIN or code; the taxpayer needs a PIN or authenticator and an active branch. Help desk's admin PIN and authenticator are removed. Answers a new recovery key. */
export function change_owner(tin_root: string, pin: string | null, code: string | null): RecoveryKey;
/** Admin only: switch from solo to help desk with a new admin PIN. Needs the current admin PIN or code. Answers a new recovery key. */
export function change_to_helpdesk(pin: string | null, code: string | null, new_pin: string): RecoveryKey;
/** Replace the admin recovery key. Needs the app set up. */
export function create_recovery_key(): RecoveryKey;
/** Single use: true (and the key is forgotten) on a match; case, dashes and spaces ignored. */
export function use_recovery_key(key: string): boolean;
/** Touch ID / account password (Windows Hello on Windows); false elsewhere or when cancelled. */
export function mac_login(reason: string): Promise<boolean>;
/** Archive or unarchive every branch. Archiving the admin taxpayer is refused. */
export function set_taxpayer_archived(tin_root: string, archived: boolean): null;
/** Delete the taxpayer and every branch. Refused for the admin taxpayer. */
export function delete_taxpayer(tin_root: string): null;
export function generate_totp_secret(account: string): TotpSecret;
export function delete_setting(key: string): null;

export interface DraftSummary {
    id: number;
    tin: string;
    form_code: string;
    taxable_year: number;
    quarter: number | null;
    month: number | null;
    status: "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
    updated_at: string;
}
export function list_drafts(tin: string, year: number): DraftSummary[];
export function get_form_draft(tin: string, form_code: string, year: number, quarter: number | null): object | null;
export function save_form_draft(tin: string, form_code: string, year: number, quarter: number | null, status: string, data: object): number;
export function list_queued(): DraftSummary[];
export function queue_form_draft(tin: string, form_code: string, year: number, quarter: number | null): number;
export function cancel_queued(tin: string, form_code: string, year: number, quarter: number | null): null;
/** A queued return whose send has an unknown outcome (`send.stage` "needs_attention"): "received", the BIR's confirmation email came, makes it Submitted; "not_received" puts it back to Draft. `period` is 2551Q's quarter, 1601C's month, else the stored period (null is 0). Refused unless the return is waiting for this answer. */
export function resolve_unknown_send(tin: string, form_code: string, year: number, period: number | null, answer: "received" | "not_received"): null;
export interface TaxDeadline {
    id: number | null;
    form_type: string;
    due_date: string;
    description: string;
}
export function list_deadlines(): TaxDeadline[];
export interface ImportResult {
    kind: "database" | "profile" | "invalid" | "cancelled";
}
export function import_archive(): Promise<ImportResult>;
export interface ExportResult {
    kind: "exported" | "cancelled";
    path?: string;
}
/** The taxpayer's profile archive under the host's save dialog. A taxpayer with a PIN needs it as `pin`; the page never passes a path — the host chooses the file. */
export function export_profile(tin: string, pin?: string | null): Promise<ExportResult>;
export interface PrintPreview {
    path: string;
}
export function form_print_html(tin: string, form_code: string, year: number, quarter: number | null): PrintPreview;
export interface BackgroundJob {
    kind: "job" | "submission";
    id: number | null;
    tin?: string;
    form_code?: string;
    taxable_year?: number;
    quarter?: number | null;
    month?: number | null;
    name: string;
    job_type: string;
    cron_expr: string | null;
    status: string;
    retries: number;
    last_run_at: string | null;
    next_run_at: string | null;
    created_at: string;
    output_log: string | null;
}
export function list_background_jobs(): BackgroundJob[];
export function delete_background_job(id: number): null;
export function archive_background_job(id: number): null;
export function purge_archived_jobs(): null;
export interface JobLogTail {
    file_name: string;
    lines: number;
    content: string;
}
export function job_log_tail(): JobLogTail;
/** A screen failure (`error` / `warn` / `info` / `debug`) for the Logs tab and the daily log file. Secrets are redacted. */
export function record_screen_log(level: string, message: string): null;
/** Next UI command from the gpui-agent host, or null when none is waiting. */
export function agent_take(): string | null;
/** Pick up filings an assistant queued on the account server. Null when there is nothing to say. */
export function pull_device_filings(): { message: string; icon: string } | null;
/** True once the main window is up. False through the gate, first launch, and sign-in. */
export function shell_set_chrome(open: boolean): null;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::profile::{PROFILE_CREDENTIAL_KEYS, TaxpayerType};
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
    use gpui_shell::HostArguments;

    const TIN: &str = "12345678800000";

    #[test]
    fn locked_state_refuses_data_verbs() {
        crate::account_gate::force_locked(|| {
            let error = list_profiles().expect_err("profiles are filing data");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = auth_state().expect_err("auth state names taxpayers");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = with_database(|_| Ok(())).expect_err("database is locked");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            let error = agent_take().expect_err("agent commands are tools");
            assert_eq!(error.message(), crate::account_gate::LOCKED_MESSAGE);
            assert!(system_info().is_ok());
        });
    }

    fn setup() {
        use_ephemeral_database();
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: "123456788".into(),
                name: "Example Trading Corp".into(),
                taxpayer_type: TaxpayerType::Corporation,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn print_preview_file_is_in_data_dir_previews_and_removed_after_read() {
        let slug = print_slug_for("2551Q").expect("2551Q print sheet");
        let opened = open_print_preview(
            slug,
            "2551Q",
            2026,
            &std::collections::BTreeMap::new(),
            None,
            None,
        )
        .expect("preview");
        let path = std::path::PathBuf::from(
            opened
                .get("path")
                .and_then(HostValue::as_str)
                .expect("path"),
        );
        assert_eq!(
            path.parent(),
            Some(bir_core::platform::print_previews_dir().as_path()),
            "{path:?}"
        );
        assert!(!path.exists(), "the preview is deleted once read: {path:?}");
    }

    fn args(values: Vec<HostValue>) -> HostArguments {
        HostArguments::new(values)
    }

    /// The three storage shapes `resolve_unknown_send` handles, each with a
    /// period of its own, and the key its reason is kept under.
    const SHAPES: [(&str, i64, &str); 3] = [
        ("2551Q", 1, "last_error"),
        ("1601C", 2, "submission_error"),
        ("0619E", 3, "submission_error"),
    ];
    const RESOLVE_YEAR: u16 = 2024;

    /// A saved Draft of `code`, its row turned into `status` with `set` written
    /// into the stored JSON (as the worker writes them). Returns the row id.
    fn stored_return(
        code: &str,
        period: i64,
        status: &str,
        set: &[(&str, serde_json::Value)],
    ) -> i64 {
        with_database(|db| {
            let profile = db.get_profile(TIN).unwrap().unwrap();
            let id = match code {
                "2551Q" => db.save_2551q_draft(
                    &bir_core::forms::form_2551q::Form2551QDraft::new_from_profile(
                        &profile,
                        RESOLVE_YEAR,
                        period as u8,
                    ),
                ),
                "1601C" => db.save_1601c_draft(&bir_core::forms::Form1601CDraft::new_from_profile(
                    &profile,
                    RESOLVE_YEAR,
                    period as u8,
                )),
                _ => db.save_queueable_draft(
                    &bir_core::forms::form_0619e::Form0619EDraft::new_from_profile(
                        &profile,
                        RESOLVE_YEAR,
                        period as u8,
                    ),
                ),
            }
            .unwrap();
            let raw: String = db
                .conn_for_tests()
                .query_row(
                    "SELECT data_json FROM form_drafts WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            let mut stored: serde_json::Value = serde_json::from_str(&raw).unwrap();
            stored["status"] = serde_json::json!(status);
            for (key, value) in set {
                stored[*key] = value.clone();
            }
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET status = ?1, data_json = ?2 WHERE id = ?3",
                    (status, serde_json::to_string(&stored).unwrap(), id),
                )
                .unwrap();
            Ok(id)
        })
        .unwrap()
    }

    /// A Queued return whose send ended with an unknown outcome, as the worker leaves it.
    fn unknown_outcome(code: &str, period: i64, reason_key: &str) -> i64 {
        stored_return(
            code,
            period,
            "Queued",
            &[
                ("submission_claim_token", serde_json::json!("worker-token")),
                (
                    "submission_claimed_at",
                    serde_json::json!("2024-04-10T01:00:00+00:00"),
                ),
                (
                    "submission_claim_lease_until",
                    serde_json::json!("2024-04-10T01:10:00+00:00"),
                ),
                (
                    "submission_put_started_at",
                    serde_json::json!("2024-04-10T01:00:01+00:00"),
                ),
                (
                    reason_key,
                    serde_json::json!(format!(
                        "{} (the upload timed out)",
                        bir_core::background_cron::OUTCOME_UNKNOWN
                    )),
                ),
            ],
        )
    }

    fn resolve(code: &str, period: i64, answer: &str) -> HostResult {
        resolve_unknown_send(&args(vec![
            TIN.into(),
            code.into(),
            (RESOLVE_YEAR as i64).into(),
            period.into(),
            answer.into(),
        ]))
    }

    /// The stored row: its status column and its JSON.
    fn stored_row(id: i64) -> (String, serde_json::Value) {
        with_database(|db| {
            Ok(db
                .conn_for_tests()
                .query_row(
                    "SELECT status, data_json FROM form_drafts WHERE id = ?1",
                    [id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .map(|(status, json)| (status, serde_json::from_str(&json).unwrap()))
                .unwrap())
        })
        .unwrap()
    }

    fn send_of(id: i64) -> serde_json::Value {
        with_database(|db| Ok(crate::send_state::stored_send(db, id).unwrap())).unwrap()
    }

    #[test]
    fn the_email_came_makes_each_shape_submitted_as_a_real_send() {
        setup();
        for (code, period, reason_key) in SHAPES {
            let id = unknown_outcome(code, period, reason_key);
            assert_eq!(send_of(id)["stage"], "needs_attention", "{code}");
            assert_eq!(
                resolve(code, period, "received").unwrap(),
                HostValue::Null,
                "{code}"
            );
            let (status, stored) = stored_row(id);
            assert_eq!(status, "Submitted", "{code}");
            assert_eq!(stored["status"], "Submitted", "{code}");
            assert!(stored["submitted_at"].is_string(), "{code}: {stored}");
            let filename = stored["submission_filename"].as_str().unwrap_or_default();
            assert!(
                filename.ends_with(".xml") && filename.contains(code),
                "{code}: {filename}"
            );
            for gone in [
                "submitted_dry_run",
                "relay_receipt",
                "submission_claim_token",
                "submission_claimed_at",
                "submission_claim_lease_until",
                "submission_put_started_at",
                "next_retry_at",
            ] {
                assert!(
                    stored.get(gone).is_none_or(serde_json::Value::is_null),
                    "{code}: {gone} in {stored}"
                );
            }
            assert!(stored[reason_key].is_null(), "{code}: {stored}");
            // Submitted with no receipt: the screens keep what they say today.
            assert!(send_of(id).is_null(), "{code}");
            // Asked again, it is no longer waiting for an answer.
            let again = resolve(code, period, "received").unwrap_err();
            assert_eq!(again.message(), NOT_WAITING_FOR_AN_ANSWER, "{code}");
        }
    }

    /// While the worker asks Buwiz how the send went (`checking`), neither answer is taken and nothing moves.
    #[test]
    fn a_send_being_checked_with_buwiz_takes_no_answer() {
        setup();
        for (code, period, reason_key) in SHAPES {
            let id = unknown_outcome(code, period, reason_key);
            with_database(|db| {
                db.conn_for_tests()
                    .execute(
                        "UPDATE form_drafts SET data_json = json_set(data_json, '$.relay_pending', ?1) WHERE id = ?2",
                        (r#"{"submission_id":"s1"}"#, id),
                    )
                    .unwrap();
                Ok(())
            })
            .unwrap();
            assert_eq!(send_of(id)["stage"], "checking", "{code}");
            assert_eq!(send_of(id)["can_resolve"], false, "{code}");
            let before = stored_row(id);
            for answer in ["received", "not_received"] {
                let refused = resolve(code, period, answer).unwrap_err();
                assert_eq!(
                    refused.message(),
                    NOT_WAITING_FOR_AN_ANSWER,
                    "{code} {answer}"
                );
            }
            assert_eq!(stored_row(id), before, "{code}");
        }
    }

    #[test]
    fn it_did_not_arrive_puts_each_shape_back_to_draft() {
        setup();
        for (code, period, reason_key) in SHAPES {
            let id = unknown_outcome(code, period, reason_key);
            assert_eq!(
                resolve(code, period, "not_received").unwrap(),
                HostValue::Null,
                "{code}"
            );
            let (status, stored) = stored_row(id);
            assert_eq!(status, "Draft", "{code}");
            assert_eq!(stored["status"], "Draft", "{code}");
            assert!(
                stored
                    .get("submission_claim_token")
                    .is_none_or(serde_json::Value::is_null),
                "{code}"
            );
            assert!(
                stored
                    .get("submitted_at")
                    .is_none_or(serde_json::Value::is_null),
                "{code}"
            );
            assert_eq!(
                stored[reason_key],
                bir_core::db::abandoned_claim_audit(bir_core::db::ABANDONED_CLAIM_RELEASE_REASON),
                "{code}: the release keeps its audit sentence"
            );
            // A Draft the person put back is no refusal: nothing to say.
            assert!(send_of(id).is_null(), "{code}");
            let again = resolve(code, period, "not_received").unwrap_err();
            assert_eq!(again.message(), NOT_WAITING_FOR_AN_ANSWER, "{code}");
        }
    }

    #[test]
    fn an_answer_is_refused_unless_the_return_waits_for_one() {
        setup();
        for (code, period, reason_key) in SHAPES {
            // Not saved at all.
            let missing = resolve(code, period, "received").unwrap_err();
            assert!(
                missing.message().starts_with("No saved"),
                "{code}: {}",
                missing.message()
            );
            let unchanged = |id: i64, before: &(String, serde_json::Value)| {
                assert_eq!(&stored_row(id), before, "{code}: a refusal changes nothing");
            };
            for (status, set) in [
                (
                    "Draft",
                    vec![(reason_key, serde_json::json!("Refused. (tin_not_active)"))],
                ),
                ("Queued", vec![]),
                ("Queued", vec![(reason_key, serde_json::json!("Held."))]),
                (
                    "Submitted",
                    vec![(
                        "submitted_at",
                        serde_json::json!("2024-04-10T01:00:00+00:00"),
                    )],
                ),
            ] {
                let id = stored_return(code, period, status, &set);
                let before = stored_row(id);
                for answer in ["received", "not_received"] {
                    let error = resolve(code, period, answer).unwrap_err();
                    assert_eq!(
                        error.message(),
                        NOT_WAITING_FOR_AN_ANSWER,
                        "{code} {status}"
                    );
                }
                unchanged(id, &before);
                with_database(|db| {
                    db.conn_for_tests()
                        .execute("DELETE FROM form_drafts WHERE id = ?1", [id])
                        .unwrap();
                    Ok(())
                })
                .unwrap();
            }
            // This process's worker is sending it now. The set of jobs in flight is one per process, and the other
            // tests here answer for the same taxpayer and periods on their own threads: a period no other test uses,
            // so marking this one in flight can never make their returns read as being sent.
            let period = period + SHAPES.len() as i64;
            let id = unknown_outcome(code, period, reason_key);
            let before = stored_row(id);
            let (summary, _) =
                with_database(|db| Ok(db.draft_summary_and_json(id).unwrap().unwrap())).unwrap();
            let key = bir_core::background_cron::submission_job_key(
                &summary.form_code,
                &summary.tin,
                summary.taxable_year,
                summary.month,
                summary.quarter,
            );
            assert_eq!(
                key,
                format!("form:{code}:{TIN}:{RESOLVE_YEAR}:{period}"),
                "the worker's key"
            );
            bir_core::background_cron::get_active_jobs()
                .lock()
                .unwrap()
                .insert(key.clone());
            let sending = send_of(id);
            let refusals: Vec<_> = ["received", "not_received"]
                .into_iter()
                .map(|answer| {
                    resolve(code, period, answer)
                        .unwrap_err()
                        .message()
                        .to_string()
                })
                .collect();
            bir_core::background_cron::get_active_jobs()
                .lock()
                .unwrap()
                .remove(&key);
            assert_eq!(sending["stage"], "sending", "{code}");
            assert_eq!(refusals, [SENDING_NOW, SENDING_NOW], "{code}");
            unchanged(id, &before);
            // An answer that is neither.
            let odd = resolve(code, period, "maybe").unwrap_err();
            assert!(odd.message().contains("received"), "{code}");
            unchanged(id, &before);
        }
        // 2551Q and 1601C need their quarter and month.
        let error = resolve("2551Q", 5, "received").unwrap_err();
        assert!(error.message().contains("quarter"), "{}", error.message());
        let error = resolve("1601C", 13, "received").unwrap_err();
        assert!(error.message().contains("month"), "{}", error.message());
    }

    /// `submitted_dry_run` is the worker's: a save never stores a client's.
    #[test]
    fn a_save_never_stores_a_clients_dry_run_fact() {
        setup();
        let sent = serde_json::json!({
            "created_at": "2026-02-01T00:00:00+00:00",
            "submitted_at": "2026-02-05T00:00:00+00:00",
            "submitted_dry_run": true,
            "lifecycle": { "submitted_dry_run": true },
        });
        save_form_draft(&args(vec![
            TIN.into(),
            "0619E".into(),
            2026_i64.into(),
            1_i64.into(),
            "Submitted".into(),
            json(sent),
        ]))
        .unwrap();
        let stored = with_database(|db| {
            Ok(db
                .get_form_draft::<serde_json::Value>(TIN, "0619E", 2026, Some(1))
                .unwrap()
                .unwrap())
        })
        .unwrap();
        assert_eq!(stored["submitted_at"], "2026-02-05T00:00:00+00:00");
        assert!(stored.get("submitted_dry_run").is_none(), "{stored}");
        assert!(
            stored["lifecycle"].get("submitted_dry_run").is_none(),
            "{stored}"
        );
    }

    /// Round 3, item 4: the generic save never stores a client's
    /// `trigger_date_entered`; a stored flag for the same date is kept.
    #[test]
    fn the_generic_save_keeps_only_the_stored_trigger_flag() {
        setup();
        let save = |data: serde_json::Value| {
            save_form_draft(&args(vec![
                TIN.into(),
                "1800".into(),
                2026_i64.into(),
                1_i64.into(),
                "Draft".into(),
                json(data),
            ]))
            .unwrap()
        };
        let stored = |field: &str| {
            with_database(|db| {
                Ok(db
                    .get_form_draft::<serde_json::Value>(TIN, "1800", 2026, Some(1))
                    .unwrap()
                    .unwrap()[field]
                    .clone())
            })
            .unwrap()
        };
        let gift = serde_json::json!({
            "donation_month": 9, "donation_day": 15, "donation_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
        });
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        // A flag stored for this date (as the page's own save leaves it) is
        // kept on a generic save of the same date, and dropped when the
        // date changes.
        with_database(|db| {
            let mut flagged = gift.clone();
            flagged["trigger_date_entered"] = serde_json::json!(true);
            db.save_form_draft(
                TIN,
                "1800",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &flagged,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), true);
        // Another draft (created_at) with the same date does not inherit it.
        let mut other = gift.clone();
        other["created_at"] = serde_json::json!("2026-10-02T00:00:00+00:00");
        save(other);
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        save(gift.clone());
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        let mut moved = gift;
        moved["donation_day"] = serde_json::json!(16);
        save(moved);
        assert_eq!(stored("trigger_date_entered"), serde_json::Value::Null);
        // The marker is host state too (P5-7): a forged `true` on an
        // unmarked stored row does not persist, and a payload that omits
        // the key keeps a stored row's marker. The row is `gift` again
        // (the moved date above was the last save).
        let mut gift = serde_json::json!({
            "donation_month": 9, "donation_day": 15, "donation_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
        });
        save(gift.clone());
        let mut forged = gift.clone();
        forged["trigger_date_model"] = serde_json::json!(true);
        save(forged);
        assert_eq!(stored("trigger_date_model"), serde_json::Value::Null);
        with_database(|db| {
            let mut marked = gift.clone();
            marked["trigger_date_model"] = serde_json::json!(true);
            marked["trigger_date_entered"] = serde_json::json!(true);
            db.save_form_draft(
                TIN,
                "1800",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &marked,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        // A payload that omits the lifecycle keys keeps the stored row's.
        gift.as_object_mut().unwrap().remove("trigger_date_entered");
        save(gift);
        assert_eq!(stored("trigger_date_model"), true);
        assert_eq!(stored("trigger_date_entered"), true);
    }

    /// Round 5, item 6: on an open-ended form the generic save with no
    /// entry adds a new one, so it checks that (absent) row, not entry 1:
    /// a 1606 sale copied from a flagged entry 1 does not inherit its flag.
    #[test]
    fn the_generic_save_checks_the_row_it_writes() {
        setup();
        let sale = serde_json::json!({
            "transaction_month": 9, "transaction_day": 18, "transaction_year": 2026,
            "created_at": "2026-10-01T00:00:00+00:00",
            "trigger_date_entered": true,
            "trigger_parts_typed": ["transaction_day", "transaction_month", "transaction_year"],
        });
        with_database(|db| {
            db.save_form_draft(
                TIN,
                "1606",
                2026,
                Some(1),
                &bir_core::forms::FilingStatus::Draft,
                &sale,
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
        save_form_draft(&args(vec![
            TIN.into(),
            "1606".into(),
            2026_i64.into(),
            HostValue::Null,
            "Draft".into(),
            json(sale.clone()),
        ]))
        .unwrap();
        let rows: Vec<String> = with_database(|db| {
            let mut stmt = db
                .conn_for_tests()
                .prepare("SELECT data_json FROM form_drafts WHERE form_code = '1606' ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            Ok(rows)
        })
        .unwrap();
        assert_eq!(rows.len(), 2, "a new entry");
        assert!(
            rows[0].contains("trigger_date_entered"),
            "entry 1 as stored"
        );
        assert!(!rows[1].contains("trigger_date_entered"), "{}", rows[1]);
        assert!(!rows[1].contains("trigger_parts_typed"), "{}", rows[1]);
    }

    /// The 2551Q and 1601C arms store no trigger flag. This holds because
    /// `Form2551QDraft` and `Form1601CDraft` have no such field: the arms
    /// overlay the client's data on a fresh typed draft, and serde drops
    /// the unknown key. It fails if either draft type gains the field
    /// without its arm settling it.
    #[test]
    fn the_typed_arms_of_the_generic_save_store_no_trigger_flag() {
        setup();
        for (code, slot) in [("2551Q", 1_i64), ("1601C", 1)] {
            save_form_draft(&args(vec![
                TIN.into(),
                code.into(),
                2026_i64.into(),
                slot.into(),
                "Draft".into(),
                json(serde_json::json!({ "trigger_date_entered": true })),
            ]))
            .unwrap_or_else(|error| panic!("{code}: {error}"));
            let stored: String = with_database(|db| {
                Ok(db
                    .conn_for_tests()
                    .query_row(
                        "SELECT data_json FROM form_drafts WHERE form_code = ?1",
                        [code],
                        |row| row.get(0),
                    )
                    .unwrap())
            })
            .unwrap();
            assert!(!stored.contains("trigger_date_entered"), "{code}: {stored}");
        }
    }

    fn has_totp() -> bool {
        with_database(|db| Ok(taxpayer_credentials(db, ROOT)?.totp_secret.is_some())).unwrap()
    }

    fn current_code(secret: &str) -> String {
        use totp_rs::{Algorithm, Secret, TOTP};
        let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
        TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, None, String::new())
            .unwrap()
            .generate_current()
            .unwrap()
    }

    const ROOT: &str = "123456788";
    const BRANCH: &str = "12345678800001";
    const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

    fn s(value: &str) -> HostValue {
        HostValue::Str(value.into())
    }

    #[test]
    fn global_hotkey_round_trip_rejects_an_unknown_key() {
        setup();
        let info = to_json(&system_info().unwrap());
        assert!(info["global_hotkey"].is_null());
        assert!(
            info["logs_dir"].as_str().unwrap().ends_with("logs"),
            "{}",
            info["logs_dir"]
        );
        // Linux is ~/.taxman-ebir. macOS is the App Group container, Windows
        // is %LOCALAPPDATA%\Taxman\eBIRForms. The field must be that path.
        assert_eq!(
            info["data_dir"].as_str().unwrap(),
            bir_core::platform::data_dir().display().to_string()
        );
        assert!(info["hotkey_presets"].as_array().unwrap().len() >= 4);

        set_global_hotkey(&args(vec![s("control+alt+E")])).unwrap();
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            "control+alt+E"
        );

        let refused = set_global_hotkey(&args(vec![s("control+notakey")])).unwrap_err();
        assert!(refused.message().contains("not one"));
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            "control+alt+E",
            "a refused chord leaves the stored one"
        );

        set_global_hotkey(&args(vec![s("  ")])).unwrap();
        assert!(to_json(&system_info().unwrap())["global_hotkey"].is_null());
    }

    /// The old shell's listener registers a to z, 0 to 9 and F1 to F12: with its hooks (the default ones here) the keys only the
    /// NSH app registers are refused by `set_global_hotkey`, whatever the case, and the stored chord is left as it was.
    #[test]
    fn global_hotkey_refuses_the_keys_only_the_nsh_app_registers() {
        setup();
        set_global_hotkey(&args(vec![s("control+alt+E")])).unwrap();
        for key in crate::hotkey_combo::NSH_KEYS {
            for combo in [
                format!("control+alt+{key}"),
                format!("super+shift+{}", key.to_lowercase()),
            ] {
                let refused = set_global_hotkey(&args(vec![s(&combo)])).unwrap_err();
                assert_eq!(
                    refused.message(),
                    crate::hotkey_combo::NOT_REGISTRABLE,
                    "{combo}"
                );
            }
        }
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            "control+alt+E",
            "a refused chord leaves the stored one"
        );
        // The old keys still register, F12 the last of them.
        set_global_hotkey(&args(vec![s("control+alt+F12")])).unwrap();
        set_global_hotkey(&args(vec![s("")])).unwrap();
    }

    #[test]
    fn log_retention_round_trip_refuses_out_of_range() {
        setup();
        set_log_retention(&args(vec![30_i64.into()])).unwrap();
        assert_eq!(to_json(&system_info().unwrap())["log_retention_days"], 30);

        let refused = set_log_retention(&args(vec![0_i64.into()])).unwrap_err();
        assert!(refused.message().contains("between"));
        assert_eq!(
            to_json(&system_info().unwrap())["log_retention_days"],
            30,
            "a refused value leaves the stored one"
        );

        let refused = set_log_retention(&args(vec![400_i64.into()])).unwrap_err();
        assert!(refused.message().contains("between"));
        assert_eq!(to_json(&system_info().unwrap())["log_retention_days"], 30);
    }

    fn add_branch() {
        with_database(|db| {
            db.create_branch(
                ROOT,
                bir_core::tax_profile::NewBranch {
                    branch_code: "00001".into(),
                    name: "Makati branch".into(),
                    fields: Default::default(),
                },
            )
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn profile_credentials(tin: &str) -> (Option<String>, Option<String>) {
        with_database(|db| {
            let profile = get_profile_by_tin(db, tin)?;
            Ok((
                profile.profile_pin_hash.clone(),
                profile.totp_secret.clone(),
            ))
        })
        .unwrap()
    }

    fn truthy(result: HostResult) -> bool {
        result.unwrap() == HostValue::Bool(true)
    }

    fn state() -> serde_json::Value {
        to_json(&auth_state().unwrap())
    }

    fn set(key: &str, value: &str) {
        with_database(|db| db.set_setting(key, value).map_err(db_error)).unwrap();
    }

    #[test]
    fn set_taxpayer_totp_needs_a_matching_code_and_clears_with_null() {
        setup();
        let wrong = set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s("000000")]));
        if current_code(SECRET) != "000000" {
            assert!(wrong.unwrap_err().message().contains("does not match"));
            assert!(!has_totp());
        }
        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s(&current_code(SECRET))
        ]))));
        set_taxpayer_totp(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!has_totp());
    }

    #[test]
    fn credentials_are_stored_once_per_taxpayer_not_on_branches() {
        setup();
        add_branch();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(profile_credentials(TIN), (None, None));
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1234")
        ]))));
        assert_eq!(
            to_json(&taxpayer_auth(&args(vec![s(ROOT)])).unwrap()),
            serde_json::json!({"has_pin": true, "has_totp": false})
        );

        // The legacy per-branch setter writes the same taxpayer PIN.
        set_profile_pin(&args(vec![s(BRANCH), s("2222")])).unwrap();
        assert_eq!(profile_credentials(BRANCH), (None, None));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("2222")]))));

        assert!(set_taxpayer_pin(&args(vec![s(ROOT), s("12345")])).is_err());
        set_taxpayer_pin(&args(vec![s(ROOT), HostValue::Null])).unwrap();
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("2222")
        ]))));
    }

    #[test]
    fn six_wrong_pins_make_even_the_right_one_wait() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        for _ in 0..5 {
            assert!(!truthy(verify_taxpayer_pin(&args(vec![
                s(ROOT),
                s("1111")
            ]))));
        }
        let sixth = verify_taxpayer_pin(&args(vec![s(ROOT), s("1111")])).unwrap_err();
        assert!(
            sixth.to_string().contains("Too many wrong tries"),
            "{sixth}"
        );
        assert!(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).is_err());
    }

    #[test]
    fn auth_state_is_unset_until_first_launch_setup() {
        setup();
        assert_eq!(
            state(),
            serde_json::json!({
                "mode": null,
                "admin_root": null,
                "has_admin_pin": false,
                "has_admin_totp": false,
                "has_recovery_key": false,
                "mac_login_available": mac_login_available(),
            })
        );
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(set_admin_pin(&args(vec![s("0817")])).is_err());
        assert!(create_recovery_key().is_err());
    }

    const OTHER_ROOT: &str = "987654321";

    fn add_other_taxpayer() {
        with_database(|db| {
            db.create_taxpayer(NewTaxpayer {
                tin_root: OTHER_ROOT.into(),
                name: "Maria Santos".into(),
                taxpayer_type: TaxpayerType::Individual,
                birth_date: None,
                head_office: BranchFacts {
                    rdo_code: "043".into(),
                    registered_address: "2 Sample Road, Makati".into(),
                    zip_code: "1200".into(),
                    phone: String::new(),
                    email: String::new(),
                },
            })
            .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    fn null() -> HostValue {
        HostValue::Null
    }

    fn message(result: HostResult) -> String {
        result.unwrap_err().message().to_owned()
    }

    fn key_of(result: HostResult) -> String {
        to_json(&result.unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn change_owner_moves_the_admin_to_another_taxpayer() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let old_key = key_of(create_recovery_key());

        let to_other = |pin: &str| change_owner(&args(vec![s(OTHER_ROOT), s(pin), null()]));
        assert!(message(to_other("1111")).contains("not the admin"));
        assert!(message(to_other("0817")).contains("no PIN or authenticator"));
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        with_database(|db| db.set_taxpayer_archived(OTHER_ROOT, true).map_err(db_error)).unwrap();
        assert!(message(to_other("0817")).contains("archived"));
        with_database(|db| {
            db.set_taxpayer_archived(OTHER_ROOT, false)
                .map_err(db_error)
        })
        .unwrap();
        assert!(change_owner(&args(vec![s("000000000"), s("0817"), null()])).is_err());
        assert!(message(change_owner(&args(vec![s(ROOT), s("0817"), null()]))).contains("already"));
        assert_eq!(state()["admin_root"], ROOT, "refusals change nothing");

        let new_key = key_of(to_other("0817"));
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        // The old owner keeps their own PIN and is now an ordinary taxpayer.
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(set_taxpayer_archived(&args(vec![s(OTHER_ROOT), HostValue::Bool(true)])).is_err());
        // The previous admin's recovery key is dead; the new one works once.
        assert!(!truthy(use_recovery_key(&args(vec![s(&old_key)]))));
        assert!(truthy(use_recovery_key(&args(vec![s(&new_key)]))));
    }

    #[test]
    fn wrong_admin_codes_at_a_setup_change_count_toward_the_lockout() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        let attempt = |pin: &str| change_owner(&args(vec![s(OTHER_ROOT), s(pin), null()]));
        for _ in 0..5 {
            assert!(message(attempt("1111")).contains("not the admin"));
        }
        assert!(message(attempt("1111")).contains("Too many wrong tries"));
        // Locked: even the right PIN waits, and nothing changed.
        assert!(message(attempt("0817")).contains("Too many wrong tries"));
        assert!(
            verify_admin_pin(&args(vec![s("0817")])).is_err(),
            "admin sign-in shares the same count"
        );
        assert_eq!(state()["admin_root"], ROOT);
    }

    #[test]
    fn change_to_helpdesk_and_back_to_an_owner() {
        setup();
        add_other_taxpayer();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();

        assert!(
            message(change_to_helpdesk(&args(vec![
                s("1111"),
                null(),
                s("4321")
            ])))
            .contains("not the admin")
        );
        assert!(change_to_helpdesk(&args(vec![s("0817"), null(), s("12")])).is_err());
        assert_eq!(state()["mode"], "solo", "refusals change nothing");

        // Confirming with the authenticator code works as well as the PIN.
        let key = key_of(change_to_helpdesk(&args(vec![
            null(),
            s(&current_code(SECRET)),
            s("4321"),
        ])));
        assert_eq!(key.len(), 19);
        assert_eq!(state()["mode"], "helpdesk");
        assert_eq!(state()["admin_root"], "000000000");
        assert_eq!(
            state()["has_admin_totp"],
            false,
            "no authenticator carried over"
        );
        assert!(truthy(verify_admin_pin(&args(vec![s("4321")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        // The former owner is no longer protected from archiving.
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(
            message(change_to_helpdesk(&args(vec![
                s("4321"),
                null(),
                s("5555")
            ])))
            .contains("already")
        );

        // Back to solo with another taxpayer as the owner.
        set_taxpayer_pin(&args(vec![s(OTHER_ROOT), s("2468")])).unwrap();
        assert!(
            change_owner(&args(vec![s(OTHER_ROOT), s("0817"), null()])).is_err(),
            "help desk PIN needed"
        );
        change_owner(&args(vec![s(OTHER_ROOT), s("4321"), null()])).unwrap();
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], OTHER_ROOT);
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));
        let leftover = with_database(|db| Ok(setting(db, ADMIN_PIN_HASH)?.is_some())).unwrap();
        assert!(!leftover, "the help desk admin PIN is removed");
    }

    #[test]
    fn solo_admin_is_the_owner_taxpayer() {
        setup();
        assert!(setup_solo(&args(vec![s("000000000")])).is_err());
        assert!(
            setup_solo(&args(vec![s("999999999")])).is_err(),
            "no such taxpayer"
        );
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        assert!(setup_solo(&args(vec![s(ROOT)])).is_err(), "only once");
        assert!(setup_helpdesk(&args(vec![s("1111")])).is_err(), "only once");
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], ROOT);

        // One code: the owner's taxpayer PIN is the admin PIN.
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        assert_eq!(state()["has_admin_pin"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        set_admin_pin(&args(vec![s("4321")])).unwrap();
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("4321")]))));
        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        // The help desk settings play no part in solo mode.
        set(ADMIN_PIN_HASH, &bir_core::crypto::hash_pin("9999"));
        assert!(!truthy(verify_admin_pin(&args(vec![s("9999")]))));

        // Clearing the owner's PIN and authenticator is allowed in solo.
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        set_admin_totp(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        assert_eq!(state()["has_admin_totp"], false);
    }

    #[test]
    fn helpdesk_admin_uses_the_settings_and_keeps_one_way_in() {
        setup();
        assert!(setup_helpdesk(&args(vec![s("12")])).is_err());
        let created = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap());
        assert_eq!(created["recovery_key"].as_str().unwrap().len(), 19);
        let st = state();
        assert_eq!(st["mode"], "helpdesk");
        assert_eq!(st["admin_root"], "000000000");
        assert_eq!(st["has_admin_pin"], true);
        assert_eq!(st["has_admin_totp"], false);
        assert_eq!(st["has_recovery_key"], true);
        assert!(truthy(verify_admin_pin(&args(vec![s("0817")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("1234")]))));

        // A taxpayer's PIN is not the admin PIN in help desk mode.
        set_taxpayer_pin(&args(vec![s(ROOT), s("5555")])).unwrap();
        assert!(!truthy(verify_admin_pin(&args(vec![s("5555")]))));

        // The only way in cannot be removed.
        let refused = set_admin_pin(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_pin"], true);

        set_admin_totp(&args(vec![s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(truthy(verify_admin_totp(&args(vec![s(&current_code(
            SECRET
        ))]))));
        set_admin_pin(&args(vec![HostValue::Null])).unwrap();
        assert_eq!(state()["has_admin_pin"], false);
        let refused = set_admin_totp(&args(vec![HostValue::Null])).unwrap_err();
        assert!(refused.message().contains("at least one way"));
        assert_eq!(state()["has_admin_totp"], true);
    }

    #[test]
    fn a_recovery_key_works_once_and_a_new_one_replaces_it() {
        setup();
        let first = to_json(&setup_helpdesk(&args(vec![s("0817")])).unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        let second = to_json(&create_recovery_key().unwrap())["recovery_key"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&first)]))),
            "replaced"
        );
        let typed = second.to_lowercase().replace('-', " ");
        assert!(truthy(use_recovery_key(&args(vec![s(&typed)]))));
        assert_eq!(state()["has_recovery_key"], false);
        assert!(
            !truthy(use_recovery_key(&args(vec![s(&second)]))),
            "single use"
        );
    }

    #[test]
    fn the_admin_taxpayer_cannot_be_archived_or_deleted() {
        setup();
        add_branch();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        for result in [
            set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])),
            delete_taxpayer(&args(vec![s(ROOT)])),
            delete_taxpayer(&args(vec![s("000000000")])),
        ] {
            assert!(result.unwrap_err().message().contains("admin account"));
        }
        // One branch may go, but not the last one.
        delete_profile(&args(vec![s(BRANCH)])).unwrap();
        assert!(delete_profile(&args(vec![s(TIN)])).is_err());
        assert!(set_profile_archived(&args(vec![s(TIN), HostValue::Bool(true)])).is_err());
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_some());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn other_taxpayers_archive_and_delete_across_every_branch() {
        setup();
        add_branch();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(true)])).unwrap();
        assert!(profile_detail_archived(TIN) && profile_detail_archived(BRANCH));
        with_database(|db| {
            assert!(
                db.get_taxpayer(ROOT)
                    .map_err(db_error)?
                    .unwrap()
                    .taxpayer
                    .archived
            );
            Ok(())
        })
        .unwrap();
        set_taxpayer_archived(&args(vec![s(ROOT), HostValue::Bool(false)])).unwrap();
        assert!(!profile_detail_archived(TIN));
        delete_taxpayer(&args(vec![s(ROOT)])).unwrap();
        with_database(|db| {
            assert!(db.get_taxpayer(ROOT).map_err(db_error)?.is_none());
            assert!(db.get_profile_by_tin(BRANCH).map_err(db_error)?.is_none());
            Ok(())
        })
        .unwrap();
    }

    fn profile_detail_archived(tin: &str) -> bool {
        with_database(|db| Ok(get_profile_by_tin(db, tin)?.is_archived)).unwrap()
    }

    // ---------- the profile's credentials stay host-side ----------

    /// Distinctive values the host stores before each credential test, in
    /// `PROFILE_CREDENTIAL_KEYS` order, so a leak or an overwrite cannot
    /// pass unnoticed.
    const STORED_CREDENTIALS: [&str; 5] = [
        "stored-totp-K7Q2",
        "stored-pass-W9X4",
        "stored-access-M3R8",
        "stored-refresh-T5V1",
        "stored-pin-hash-H6N3",
    ];

    /// What a page might forge, in the same order.
    const FORGED_CREDENTIALS: [&str; 5] = [
        "forged-totp",
        "forged-pass",
        "forged-access",
        "forged-refresh",
        "forged-pin-hash",
    ];

    fn store_credentials(tin: &str) {
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, tin)?;
            let [totp, password, access, refresh, pin_hash] =
                STORED_CREDENTIALS.map(|value| Some(value.to_string()));
            profile.totp_secret = totp;
            profile.imap_app_password = password;
            profile.oauth_access_token = access;
            profile.oauth_refresh_token = refresh;
            profile.profile_pin_hash = pin_hash;
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
    }

    /// `tin`'s five stored credentials, in `PROFILE_CREDENTIAL_KEYS` order.
    fn stored_credentials(tin: &str) -> [Option<String>; 5] {
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, tin)?;
            Ok([
                profile.totp_secret.take(),
                profile.imap_app_password.take(),
                profile.oauth_access_token.take(),
                profile.oauth_refresh_token.take(),
                profile.profile_pin_hash.take(),
            ])
        })
        .unwrap()
    }

    /// Every credential stored on `tin` is still the one `store_credentials`
    /// wrote. A failure names the key, never a value.
    fn assert_credentials_kept(tin: &str, after: &str) {
        let stored = stored_credentials(tin);
        for ((key, value), expected) in PROFILE_CREDENTIAL_KEYS
            .into_iter()
            .zip(stored)
            .zip(STORED_CREDENTIALS)
        {
            assert!(
                value.as_deref() == Some(expected),
                "{after} changed the stored {key}"
            );
        }
    }

    /// `verb`'s profile answer carries neither a credential key nor any of
    /// `values` (in `PROFILE_CREDENTIAL_KEYS` order). A failure names the
    /// key, never a value.
    fn assert_no_credentials(answer: &HostValue, verb: &str, values: [&str; 5]) {
        let record = to_json(answer);
        let fields = record
            .as_object()
            .unwrap_or_else(|| panic!("{verb} answers a profile record"));
        let raw = record.to_string();
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(values) {
            assert!(!fields.contains_key(key), "{verb} sent the {key} key");
            assert!(!raw.contains(value), "{verb} sent the {key} value");
        }
    }

    /// The page reads a profile without its credentials: no key, no value,
    /// and the rest of the record is there.
    #[test]
    fn profile_detail_sends_no_credential() {
        setup();
        store_credentials(TIN);
        let detail = profile_detail(&args(vec![s(TIN)])).unwrap();
        assert_no_credentials(&detail, "profile_detail", STORED_CREDENTIALS);
        assert_eq!(to_json(&detail)["full_name"], "Example Trading Corp");
    }

    /// The page's round trip of `profile_detail` (a payload without the
    /// credential keys) saves its own edit and keeps all five stored values.
    #[test]
    fn save_profile_without_the_credential_keys_keeps_the_stored_ones() {
        setup();
        store_credentials(TIN);
        let mut page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        for key in PROFILE_CREDENTIAL_KEYS {
            assert!(page.get(key).is_none(), "the payload carries no {key}");
        }
        page["phone"] = serde_json::json!("0281112222");
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept(TIN, "a save without the credential keys");
        let phone = with_database(|db| Ok(get_profile_by_tin(db, TIN)?.phone.clone())).unwrap();
        assert_eq!(phone, "0281112222", "the rest of the payload is saved");
    }

    /// Forged values leave the stored ones as they were, and so do forged
    /// nulls: the page can neither set nor clear a credential.
    #[test]
    fn save_profile_ignores_forged_credentials() {
        setup();
        store_credentials(TIN);
        let page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        let (mut forged, mut nulled) = (page.clone(), page);
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
            forged[key] = serde_json::json!(value);
            nulled[key] = serde_json::Value::Null;
        }
        let saved = save_profile(&args(vec![json(forged)])).unwrap();
        assert_no_credentials(&saved, "save_profile", FORGED_CREDENTIALS);
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept(TIN, "a save with forged credentials");
        save_profile(&args(vec![json(nulled)])).unwrap();
        assert_credentials_kept(TIN, "a save with null credentials");
    }

    /// A save for a TIN with no stored row stores no credential, whatever
    /// the payload carries.
    #[test]
    fn save_profile_for_a_new_tin_stores_no_credential() {
        setup();
        let mut page = serde_json::json!({
            "id": null,
            "full_name": "Sample Holdings Inc",
            "tin": {"segment1": "987", "segment2": "654", "segment3": "320", "branch": "00000"},
            "rdo_code": "043",
            "line_of_business": "Holding company",
            "registered_address": "2 Sample Road, Makati",
            "zip_code": "1200",
            "phone": "0280000001",
            "email": "books@example.com",
            "default_form_type": "1702Q",
            "taxpayer_type": "Corporation",
        });
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(FORGED_CREDENTIALS) {
            page[key] = serde_json::json!(value);
        }
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", FORGED_CREDENTIALS);
        let stored = stored_credentials("98765432000000");
        for (key, value) in PROFILE_CREDENTIAL_KEYS.into_iter().zip(stored) {
            assert!(value.is_none(), "a new profile stored a {key}");
        }
    }

    /// A round trip that moves the profile to a new TIN writes the same row
    /// (re-keyed by its `id`), so that row's credentials are the ones kept;
    /// the new TIN has no row of its own to take them from.
    #[test]
    fn save_profile_that_moves_the_tin_keeps_the_rows_credentials() {
        setup();
        add_branch();
        store_credentials(BRANCH);
        let mut page = to_json(&profile_detail(&args(vec![s(BRANCH)])).unwrap());
        page["tin"]["branch"] = serde_json::json!("00002");
        let saved = save_profile(&args(vec![json(page)])).unwrap();
        assert_no_credentials(&saved, "save_profile", STORED_CREDENTIALS);
        assert_credentials_kept("12345678800002", "a save that moves the TIN");
        let moved =
            with_database(|db| Ok(db.get_profile_by_tin(BRANCH).map_err(db_error)?.is_none()))
                .unwrap();
        assert!(moved, "the row moved off its old TIN");
    }

    /// The third verb that answers a profile: archiving keeps the
    /// credentials stored and leaves them out of the answer.
    #[test]
    fn set_profile_archived_answers_without_credentials() {
        setup();
        store_credentials(TIN);
        let archived = set_profile_archived(&args(vec![s(TIN), HostValue::Bool(true)])).unwrap();
        assert_no_credentials(&archived, "set_profile_archived", STORED_CREDENTIALS);
        assert_eq!(to_json(&archived)["is_archived"], true);
        assert_credentials_kept(TIN, "archiving");
    }

    /// The authenticator's own writers still work: `set_taxpayer_totp` keeps
    /// the secret on the taxpayer, a page round trip of every branch profile
    /// neither shows it nor removes it, and the legacy per-branch
    /// `set_profile_totp` still clears and sets it.
    #[test]
    fn the_authenticator_writers_survive_a_profile_round_trip() {
        setup();
        add_branch();
        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        for tin in [TIN, BRANCH] {
            let page = profile_detail(&args(vec![s(tin)])).unwrap();
            assert!(
                !to_json(&page).to_string().contains(SECRET),
                "profile_detail sent the authenticator secret"
            );
            save_profile(&args(vec![page])).unwrap();
        }
        assert!(has_totp(), "the round trip kept the authenticator");
        assert!(truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s(&current_code(SECRET))
        ]))));
        set_profile_totp(&args(vec![s(BRANCH), HostValue::Null])).unwrap();
        assert!(!has_totp());
        set_profile_totp(&args(vec![s(BRANCH), s(SECRET), s(&current_code(SECRET))])).unwrap();
        assert!(has_totp());
    }

    /// A page cannot plant a PIN. For a taxpayer without one, a PIN hash
    /// sent through `save_profile` is neither answered nor stored: the
    /// profile row's `profile_pin_hash` stays JSON null. The next launch
    /// moves only a text value onto the taxpayer (bir-core's
    /// `test_v22_moves_credentials_onto_the_taxpayer_and_clears_branch_copies`),
    /// so the taxpayer stays without a PIN.
    #[test]
    fn a_pin_hash_from_the_page_is_not_stored() {
        setup();
        let planted = bir_core::crypto::hash_pin("1234");
        let mut page = to_json(&profile_detail(&args(vec![s(TIN)])).unwrap());
        page["profile_pin_hash"] = serde_json::json!(planted);
        let saved = to_json(&save_profile(&args(vec![json(page)])).unwrap());
        assert!(
            saved.get("profile_pin_hash").is_none(),
            "save_profile sent the profile_pin_hash key"
        );
        assert!(
            !saved.to_string().contains(&planted),
            "save_profile sent the PIN hash"
        );
        let stored: Option<String> = with_database(|db| {
            Ok(db
                .conn_for_tests()
                .query_row(
                    "SELECT json_type(data_json, '$.profile_pin_hash') FROM profiles
                     WHERE tin = ?1",
                    [TIN],
                    |row| row.get(0),
                )
                .unwrap())
        })
        .unwrap();
        assert_eq!(stored.as_deref(), Some("null"), "the row's PIN hash type");
        assert_eq!(
            to_json(&taxpayer_auth(&args(vec![s(ROOT)])).unwrap()),
            serde_json::json!({"has_pin": false, "has_totp": false})
        );
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1234")
        ]))));
    }

    /// A PIN hash a profile row already holds (as an older build or the
    /// legacy profile editor leaves it until the next launch moves it onto
    /// the taxpayer) is in neither verb's answer, and a page round trip
    /// leaves it on the row.
    #[test]
    fn a_stored_pin_hash_survives_a_page_round_trip_unseen() {
        setup();
        let stored = bir_core::crypto::hash_pin("0817");
        with_database(|db| {
            let mut profile = get_profile_by_tin(db, TIN)?;
            profile.profile_pin_hash = Some(stored.clone());
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        let detail = profile_detail(&args(vec![s(TIN)])).unwrap();
        let saved = save_profile(&args(vec![detail.clone()])).unwrap();
        for (verb, answer) in [("profile_detail", detail), ("save_profile", saved)] {
            let record = to_json(&answer);
            assert!(
                record.get("profile_pin_hash").is_none(),
                "{verb} sent the profile_pin_hash key"
            );
            assert!(
                !record.to_string().contains(&stored),
                "{verb} sent the PIN hash"
            );
        }
        let kept = with_database(|db| {
            let mut profile = get_profile_by_tin(db, TIN)?;
            Ok(profile.profile_pin_hash.take())
        })
        .unwrap();
        assert!(
            kept.as_deref() == Some(stored.as_str()),
            "the round trip changed the stored PIN hash"
        );
    }

    /// The PIN's own writers still work around a page round trip. Each one
    /// stores the PIN on the taxpayer (the help desk admin's in settings); a
    /// round trip of every branch profile shows no hash of it, and the PIN
    /// still verifies afterwards while the one it replaced does not. The
    /// writers: `set_taxpayer_pin`, the legacy per-branch `set_profile_pin`,
    /// `set_admin_pin` for the solo owner and for the help desk, and
    /// `change_to_helpdesk`'s new admin PIN.
    #[test]
    fn the_pin_writers_survive_a_profile_round_trip() {
        setup();
        add_branch();
        let round_trip = |pin: &str| {
            let hash = bir_core::crypto::hash_pin(pin);
            for tin in [TIN, BRANCH] {
                let page = profile_detail(&args(vec![s(tin)])).unwrap();
                assert!(
                    !to_json(&page).to_string().contains(&hash),
                    "profile_detail sent the PIN hash"
                );
                save_profile(&args(vec![page])).unwrap();
            }
        };
        let taxpayer_pin = |pin: &str| truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s(pin)])));
        let admin_pin = |pin: &str| truthy(verify_admin_pin(&args(vec![s(pin)])));

        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        round_trip("0817");
        assert!(taxpayer_pin("0817"), "set_taxpayer_pin's PIN");

        set_profile_pin(&args(vec![s(BRANCH), s("2468")])).unwrap();
        round_trip("2468");
        assert!(taxpayer_pin("2468"), "set_profile_pin's PIN");
        assert!(!taxpayer_pin("0817"), "the PIN set_profile_pin replaced");

        setup_solo(&args(vec![s(ROOT)])).unwrap();
        set_admin_pin(&args(vec![s("4321")])).unwrap();
        round_trip("4321");
        assert!(admin_pin("4321"), "set_admin_pin's solo PIN");
        assert!(taxpayer_pin("4321"), "the solo admin PIN is the owner's");
        assert!(!taxpayer_pin("2468"), "the PIN set_admin_pin replaced");

        change_to_helpdesk(&args(vec![s("4321"), null(), s("5555")])).unwrap();
        round_trip("5555");
        assert!(admin_pin("5555"), "change_to_helpdesk's admin PIN");
        assert!(
            !admin_pin("4321"),
            "the admin PIN change_to_helpdesk replaced"
        );
        assert!(taxpayer_pin("4321"), "the former owner keeps their PIN");

        set_admin_pin(&args(vec![s("6666")])).unwrap();
        round_trip("6666");
        assert!(admin_pin("6666"), "set_admin_pin's help desk PIN");
        assert!(!admin_pin("5555"), "the admin PIN set_admin_pin replaced");
    }

    /// PLAN §B.1, client PINs: a match against an old SHA-256 value rehashes it to an Argon2id PHC string in
    /// `taxpayers.pin_hash`, on a thread of its own, and clears the branch copy; both stores. The rehashed PIN
    /// still opens, a wrong one does not, and a PIN changed before the rehash lands is never overwritten.
    #[test]
    fn client_sha_pin_rehashes_on_first_match() {
        setup();
        add_branch();
        let stored_pin = || {
            with_database(|db| Ok(taxpayer_credentials(db, ROOT)?.pin_hash))
                .unwrap()
                .unwrap_or_default()
        };
        let branch_copies = || -> i64 {
            with_database(|db| {
                Ok(db
                    .conn_for_tests()
                    .query_row(
                        "SELECT count(*) FROM profiles
                         WHERE substr(tin, 1, 9) = ?1 AND json_type(data_json, '$.profile_pin_hash') = 'text'",
                        [ROOT],
                        |row| row.get(0),
                    )
                    .unwrap())
            })
            .unwrap()
        };

        // taxpayers.pin_hash, as an earlier version stored it.
        let old = bir_core::crypto::hash_pin("0817");
        with_database(|db| db.set_taxpayer_pin_hash(ROOT, Some(&old)).map_err(db_error)).unwrap();
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1111")
        ]))));
        join_last_rehash();
        assert_eq!(stored_pin(), old, "a miss rehashes nothing");
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        join_last_rehash();
        let phc = stored_pin();
        assert!(phc.starts_with("$argon2id$"), "{phc}");
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1111")
        ]))));
        join_last_rehash();
        assert_eq!(stored_pin(), phc, "an Argon2id match is not rehashed again");

        // The branch copy: the taxpayer has no PIN and a branch row still holds the old value (until the next
        // open's v22 repair lifts it). Its first match moves a PHC string onto the taxpayer and clears the copy.
        let legacy = bir_core::crypto::hash_pin("6810");
        with_database(|db| {
            db.set_taxpayer_pin_hash(ROOT, None).map_err(db_error)?;
            let mut profile = get_profile_by_tin(db, BRANCH)?;
            profile.profile_pin_hash = Some(legacy.clone());
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(branch_copies(), 1);
        let hash =
            with_database(|db| bir_core::export::client_pin_hash(db, BRANCH).map_err(db_error))
                .unwrap();
        assert_eq!(hash.as_deref(), Some(legacy.as_str()));
        let (matched, rehash) = client_pin_check(ROOT, hash.as_deref(), "6810");
        assert!(matched);
        start_rehash_after(matched, rehash);
        join_last_rehash();
        let lifted = stored_pin();
        assert!(lifted.starts_with("$argon2id$"), "{lifted}");
        assert_eq!(branch_copies(), 0, "the branch copy is cleared");
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("6810")]))));

        // A PIN changed between the match and the rehash is never overwritten.
        set_taxpayer_pin(&args(vec![s(ROOT), s("2468")])).unwrap();
        join_last_rehash();
        let changed = stored_pin();
        let written = with_database(|db| {
            db.upgrade_client_pin_hash(ROOT, &legacy, &bir_core::pin::hash("6810").unwrap())
                .map_err(db_error)
        })
        .unwrap();
        assert!(!written);
        assert_eq!(stored_pin(), changed);
    }

    /// N3: a client or help-desk admin PIN set through the page's synchronous verbs is stored in the old form for a
    /// moment and rehashed to Argon2id on its own thread straight after; it opens either way.
    #[test]
    fn pins_written_from_b1_on_become_argon2id() {
        setup();
        let stored_pin = || {
            with_database(|db| Ok(taxpayer_credentials(db, ROOT)?.pin_hash))
                .unwrap()
                .unwrap_or_default()
        };
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        join_last_rehash();
        assert!(stored_pin().starts_with("$argon2id$"));
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));

        setup_helpdesk(&args(vec![s("2468")])).unwrap();
        join_last_rehash();
        let admin = stored_setting(ADMIN_PIN_HASH).unwrap_or_default();
        assert!(admin.starts_with("$argon2id$"), "setup_helpdesk");
        assert!(truthy(verify_admin_pin(&args(vec![s("2468")]))));

        set_admin_pin(&args(vec![s("1357")])).unwrap();
        join_last_rehash();
        assert!(
            stored_setting(ADMIN_PIN_HASH)
                .unwrap_or_default()
                .starts_with("$argon2id$"),
            "set_admin_pin"
        );
        assert!(truthy(verify_admin_pin(&args(vec![s("1357")]))));
        assert!(!truthy(verify_admin_pin(&args(vec![s("2468")]))));

        // A help-desk admin PIN an earlier version stored is rehashed on its first match.
        let old = bir_core::crypto::hash_pin("1357");
        with_database(|db| db.set_setting(ADMIN_PIN_HASH, &old).map_err(db_error)).unwrap();
        assert!(truthy(verify_admin_pin(&args(vec![s("1357")]))));
        join_last_rehash();
        assert!(
            stored_setting(ADMIN_PIN_HASH)
                .unwrap_or_default()
                .starts_with("$argon2id$"),
            "on its match"
        );
    }

    /// A client's count is kept in the device store (PLAN §B.2), keyed by the room and the TIN root, and a
    /// wrong authenticator code shares the PIN's count.
    #[test]
    fn a_clients_count_is_kept_in_the_device_store() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let count = || {
            bir_core::rooms::device_store_here()
                .with(false, |store| {
                    store.attempts(&bir_core::owner_lock::AttemptKey::client(None, ROOT))
                })
                .unwrap()
                .unwrap()
                .failures
        };
        assert!(!truthy(verify_taxpayer_pin(&args(vec![
            s(ROOT),
            s("1111")
        ]))));
        assert!(!truthy(verify_taxpayer_totp(&args(vec![
            s(ROOT),
            s("000000")
        ]))));
        assert_eq!(count(), 2);
        assert!(truthy(verify_taxpayer_pin(&args(vec![s(ROOT), s("0817")]))));
        assert_eq!(count(), 0);
        join_last_rehash();
    }

    // ---------- the page reaches only its own settings ----------

    /// The host's own read of `key`.
    fn stored_setting(key: &str) -> Option<String> {
        with_database(|db| setting(db, key)).unwrap()
    }

    /// `verb`'s answer for `key` is the page refusal, whose exact wording
    /// names the key and nothing stored under it. A failure prints neither
    /// the answer nor the refusal, since either could carry a stored value.
    fn assert_refused(answer: HostResult, verb: &str, key: &str) {
        match answer {
            Ok(_) => panic!("{verb} answered the page for `{key}`"),
            Err(refusal) => assert!(
                refusal.message() == format!("setting `{key}` is not available to the page"),
                "{verb} refused `{key}` in other words"
            ),
        }
    }

    /// (a) The page's one setting round-trips through the three verbs, and
    /// each step reaches the device store (accounts-and-lock A1: the theme
    /// is device-wide).
    #[test]
    fn the_theme_preference_round_trips_through_the_page_verbs() {
        setup();
        let key = || s("theme_preference");
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), HostValue::Null);
        assert_eq!(
            set_setting(&args(vec![key(), s("Dark")])).unwrap(),
            HostValue::Null
        );
        assert_eq!(
            bir_core::rooms::device_setting("theme_preference").as_deref(),
            Some("Dark")
        );
        assert_eq!(stored_setting("theme_preference"), None, "not the room's");
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), s("Dark"));
        assert_eq!(delete_setting(&args(vec![key()])).unwrap(), HostValue::Null);
        assert_eq!(bir_core::rooms::device_setting("theme_preference"), None);
        assert_eq!(get_setting(&args(vec![key()])).unwrap(), HostValue::Null);
    }

    /// What these tests prune, open or write when they name no folder (`set_log_retention`'s log pruning, the
    /// device store `system_info` reads, the device files) is in the temp folder, never the real data folder.
    #[test]
    fn host_tests_keep_out_of_the_real_data_folder() {
        temp_env::with_vars(
            [("BIR_LOG_DIR", None::<&str>), ("BIR_DATABASE_PATH", None)],
            || {
                db::set_database_path_override(None);
                let temp = std::env::temp_dir().canonicalize().unwrap();
                let logs = bir_core::log_files::log_dir();
                std::fs::create_dir_all(&logs).unwrap();
                for dir in [
                    logs,
                    bir_core::platform::data_dir(),
                    bir_core::platform::device_dir(),
                ] {
                    assert!(
                        dir.canonicalize().unwrap().starts_with(&temp),
                        "{} is outside the temp folder",
                        dir.display()
                    );
                }
            },
        );
    }

    /// Accounts-and-lock A1 (O1): the device-wide settings a person set
    /// before A1 live in the open database. Each reader answers them until
    /// `device_settings` has a value; every write goes to `device_settings`.
    #[test]
    fn device_settings_fall_back_to_the_open_database() {
        setup();
        with_database(|db| {
            for (key, value) in [
                ("theme_preference", "Dark"),
                ("global_hotkey_key", "control+alt+B"),
                ("log_retention_days", "30"),
                (crate::hooks::AUTO_DOWNLOAD_SETTING, "0"),
            ] {
                db.set_setting(key, value).map_err(db_error)?;
            }
            Ok(())
        })
        .unwrap();
        let theme = || get_setting(&args(vec![s("theme_preference")])).unwrap();
        assert_eq!(theme(), s("Dark"));
        let info = to_json(&system_info().unwrap());
        assert_eq!(info["global_hotkey"], "control+alt+B");
        assert_eq!(info["log_retention_days"], 30);
        #[cfg(not(feature = "mas_build"))]
        assert_eq!(stored_global_hotkey().as_deref(), Some("control+alt+B"));
        assert_eq!(
            device_setting(crate::hooks::AUTO_DOWNLOAD_SETTING).as_deref(),
            Some("0"),
            "auto-download off in the legacy file reads off"
        );

        set_setting(&args(vec![s("theme_preference"), s("Light")])).unwrap();
        set_global_hotkey(&args(vec![s("control+alt+E")])).unwrap();
        set_log_retention(&args(vec![14_i64.into()])).unwrap();
        set_auto_update_download(&args(vec![HostValue::Bool(true)])).unwrap();
        assert_eq!(theme(), s("Light"));
        let info = to_json(&system_info().unwrap());
        assert_eq!(info["global_hotkey"], "control+alt+E");
        assert_eq!(info["log_retention_days"], 14);
        assert_eq!(
            device_setting(crate::hooks::AUTO_DOWNLOAD_SETTING).as_deref(),
            Some("1")
        );
        for (key, legacy) in [
            ("theme_preference", "Dark"),
            ("global_hotkey_key", "control+alt+B"),
            ("log_retention_days", "30"),
            (crate::hooks::AUTO_DOWNLOAD_SETTING, "0"),
        ] {
            assert_eq!(
                stored_setting(key).as_deref(),
                Some(legacy),
                "{key}: the write went to device_settings"
            );
            assert!(bir_core::rooms::device_setting(key).is_some(), "{key}");
        }
        // A delete leaves no legacy value to fall back to.
        delete_setting(&args(vec![s("theme_preference")])).unwrap();
        assert_eq!(theme(), HostValue::Null);
        // A cleared hotkey stays cleared: the device row wins.
        set_global_hotkey(&args(vec![s("")])).unwrap();
        assert_eq!(
            to_json(&system_info().unwrap())["global_hotkey"],
            serde_json::Value::Null
        );
        bir_core::log_files::remember_retention(bir_core::log_files::DEFAULT_RETENTION_DAYS);
    }

    // ---------- W3: the welcome names no account to the page ----------

    const WELCOME_SERVER: &str = "http://127.0.0.1:9";
    const A_ID: &str = "user-a-31c9";
    const A_EMAIL: &str = "Alma.Reyes@Example.test";
    const B_ID: &str = "user-b-77d2";
    const B_EMAIL: &str = "bayani@example.test";

    /// `body` with `user_id` (and `email`) as the host's saved sign-in, in a scratch file (keychain off).
    fn signed_in_as<R>(user_id: &str, email: &str, body: impl FnOnce() -> R) -> R {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("account.json");
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("BUWIZ_SERVER_URL", Some(WELCOME_SERVER)),
                (
                    bir_core::cloud_account::ENV_ACCOUNT_FILE,
                    Some(file.to_str().unwrap()),
                ),
            ],
            || {
                bir_core::cloud_account::save_stored_account(
                    &bir_core::cloud_account::StoredAccount {
                        server_url: WELCOME_SERVER.into(),
                        access_token: "access".into(),
                        refresh_token: "refresh".into(),
                        access_expires_at_unix: i64::MAX / 2,
                        user_id: user_id.into(),
                        email: Some(email.into()),
                        offline: false,
                        last_refresh_unix: 1_000,
                        practice: false,
                    },
                )
                .unwrap();
                body()
            },
        )
    }

    /// `body` with no saved sign-in.
    fn signed_out<R>(body: impl FnOnce() -> R) -> R {
        let dir = tempfile::tempdir().unwrap();
        temp_env::with_vars(
            [
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
                ("BUWIZ_SERVER_URL", Some(WELCOME_SERVER)),
                (
                    bir_core::cloud_account::ENV_ACCOUNT_FILE,
                    Some(dir.path().join("account.json").to_str().unwrap()),
                ),
            ],
            body,
        )
    }

    fn welcome() -> bool {
        truthy(crate::cloud_account::welcome_first_time())
    }

    /// No trace of A: neither the id nor the address, in any case.
    fn assert_no_trace_of_a(what: &str, text: &str) {
        let text = text.to_lowercase();
        for trace in [A_ID, A_EMAIL] {
            assert!(
                !text.contains(&trace.to_lowercase()),
                "{what} holds a trace of account A"
            );
        }
    }

    /// The device settings W3 keeps, and the list #417 kept: none of them the page's.
    const WELCOME_KEYS: [&str; 3] = [
        bir_core::welcome::OLD_WELCOMED_SETTING,
        bir_core::welcome::WELCOMED_HASHES_SETTING,
        bir_core::welcome::WELCOME_KEY_SETTING,
    ];

    /// W3: after A was welcomed, a page signed in as B reads nothing of A: every page setting's answer is free of
    /// A's id and address, and the welcome's own settings are refused, as the admin's secret is.
    #[test]
    fn a_page_signed_in_as_b_learns_nothing_about_a_from_any_page_readable_setting() {
        use_ephemeral_database();
        signed_in_as(A_ID, A_EMAIL, || assert!(welcome(), "A's first sign-in"));
        signed_in_as(B_ID, B_EMAIL, || {
            assert!(welcome(), "B's first sign-in");
            for key in PAGE_SETTING_KEYS {
                let answer = get_setting(&args(vec![s(key)]));
                assert_no_trace_of_a(key, &format!("{answer:?}"));
            }
            for key in WELCOME_KEYS {
                assert!(!PAGE_SETTING_KEYS.contains(&key), "{key} is the page's");
                let answer = get_setting(&args(vec![s(key)]));
                assert_no_trace_of_a(key, &format!("{answer:?}"));
                assert_refused(answer, "get_setting", key);
                assert_refused(
                    set_setting(&args(vec![s(key), s("[]")])),
                    "set_setting",
                    key,
                );
                assert_refused(delete_setting(&args(vec![s(key)])), "delete_setting", key);
            }
            // The verb answers B's own first time and nothing about anyone else.
            assert_eq!(
                crate::cloud_account::welcome_first_time().unwrap(),
                HostValue::Bool(false)
            );
        });
    }

    /// W3: true the first time an account signs in on this device, false after; another account has its own first
    /// time; with no account signed in, false and nothing recorded.
    #[test]
    fn welcome_first_time_answers_once_per_account() {
        use_ephemeral_database();
        signed_out(|| assert!(!welcome(), "no account signed in"));
        assert_eq!(
            bir_core::rooms::device_setting(bir_core::welcome::WELCOMED_HASHES_SETTING),
            None,
            "nothing recorded with no account signed in"
        );
        signed_in_as(A_ID, A_EMAIL, || {
            assert!(welcome());
            assert!(!welcome());
        });
        signed_in_as(B_ID, B_EMAIL, || {
            assert!(welcome());
            assert!(!welcome());
        });
        signed_in_as(A_ID, A_EMAIL, || assert!(!welcome(), "A again"));
        // An account without a user id is known by its address, trimmed and in lower case.
        signed_in_as("", " Carmen@Example.test ", || assert!(welcome()));
        signed_in_as("", "carmen@example.test", || assert!(!welcome()));
    }

    /// W3 upgrade: #417's list of ids and addresses is dropped from `device_settings` and from the open
    /// database's settings, by the welcome check and by the launch.
    #[test]
    fn the_old_welcomed_list_is_dropped_from_both_stores() {
        use_ephemeral_database();
        let old = bir_core::welcome::OLD_WELCOMED_SETTING;
        let plant = || {
            let list = format!("[\"{A_ID}\",\"{}\"]", A_EMAIL.to_lowercase());
            bir_core::rooms::set_device_setting(old, &list).unwrap();
            with_database(|db| db.set_setting(old, &list).map_err(db_error)).unwrap();
            assert!(bir_core::rooms::device_setting(old).is_some());
            assert!(stored_setting(old).is_some());
        };
        plant();
        signed_in_as(B_ID, B_EMAIL, || assert!(welcome()));
        assert_eq!(
            bir_core::rooms::device_setting(old),
            None,
            "device_settings"
        );
        assert_eq!(stored_setting(old), None, "the open database");
        // The launch drops it too (prepare_launch), signed in or not.
        plant();
        crate::cloud_account::drop_old_welcomed_list();
        assert_eq!(
            bir_core::rooms::device_setting(old),
            None,
            "device_settings"
        );
        assert_eq!(stored_setting(old), None, "the open database");
    }

    /// W3: what the device keeps is HMAC-SHA256 of the account's key under this device's own random key: no id,
    /// no address, and another device's hash of the same account differs.
    #[test]
    fn the_stored_value_is_a_keyed_hash_not_the_id_or_email() {
        use_ephemeral_database();
        signed_in_as(A_ID, A_EMAIL, || assert!(welcome()));
        signed_in_as("", A_EMAIL, || assert!(welcome()));
        let read = |key| bir_core::rooms::device_setting(key).unwrap();
        let stored = read(bir_core::welcome::WELCOMED_HASHES_SETTING);
        assert_no_trace_of_a("the stored hashes", &stored);
        let device_key = hex_decode(&read(bir_core::welcome::WELCOME_KEY_SETTING));
        assert_eq!(device_key.len(), 32, "a 32-byte device key");
        let hashes: Vec<String> = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            hashes,
            [
                bir_core::welcome::keyed_hash(&device_key, A_ID),
                bir_core::welcome::keyed_hash(&device_key, &A_EMAIL.to_lowercase()),
            ]
        );
        // Another device (its own store, so its own key) keeps another hash of A.
        use_ephemeral_database();
        signed_in_as(A_ID, A_EMAIL, || assert!(welcome()));
        let other: Vec<String> =
            serde_json::from_str(&read(bir_core::welcome::WELCOMED_HASHES_SETTING)).unwrap();
        assert_ne!(other[0], hashes[0], "the hash is keyed by the device");
    }

    fn hex_decode(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
            .collect()
    }

    /// A2 (#423's review, item 5): with `rooms/` on disk and `device.db`
    /// unreadable, the page's theme and the other device-setting writes still
    /// succeed (into the open database), and read back; the store is left as it
    /// was.
    #[test]
    fn setting_writes_go_through_when_the_device_store_is_unusable() {
        setup();
        bir_core::rooms::forget_ephemeral_device_store();
        let folder = tempfile::tempdir().unwrap();
        db::set_database_path_override(Some(folder.path().join("bir_data.db")));
        std::fs::create_dir_all(folder.path().join("rooms").join("x")).unwrap();
        let garbage = b"not a device store, and long enough to be read".to_vec();
        std::fs::write(folder.path().join("device.db"), &garbage).unwrap();

        set_setting(&args(vec![s("theme_preference"), s("Dark")])).unwrap();
        set_global_hotkey(&args(vec![s("control+alt+E")])).unwrap();
        // (Log retention's verb also prunes the log folder, which is not this test's: its write is the same call.)
        set_auto_update_download(&args(vec![HostValue::Bool(false)])).unwrap();
        assert_eq!(
            get_setting(&args(vec![s("theme_preference")])).unwrap(),
            s("Dark")
        );
        assert_eq!(stored_setting("theme_preference").as_deref(), Some("Dark"));
        assert_eq!(
            stored_setting("global_hotkey_key").as_deref(),
            Some("control+alt+E")
        );
        assert_eq!(
            device_setting(crate::hooks::AUTO_DOWNLOAD_SETTING).as_deref(),
            Some("0")
        );
        delete_setting(&args(vec![s("theme_preference")])).unwrap();
        assert_eq!(
            get_setting(&args(vec![s("theme_preference")])).unwrap(),
            HostValue::Null
        );
        assert_eq!(
            std::fs::read(folder.path().join("device.db")).unwrap(),
            garbage
        );

        db::set_database_path_override(None);
        bir_core::rooms::use_ephemeral_device_store();
    }

    /// (b) The help desk admin's authenticator secret stays host-side: the
    /// page's read is refused, and the refusal carries no value.
    #[test]
    fn the_page_cannot_read_the_admin_authenticator_secret() {
        setup();
        set(ADMIN_TOTP_SECRET, SECRET);
        let answer = get_setting(&args(vec![s(ADMIN_TOTP_SECRET)]));
        assert!(
            !format!("{answer:?}").contains(SECRET),
            "get_setting sent the admin authenticator secret"
        );
        assert_refused(answer, "get_setting", ADMIN_TOTP_SECRET);
        assert!(
            stored_setting(ADMIN_TOTP_SECRET).as_deref() == Some(SECRET),
            "the refused read changed the stored secret"
        );
    }

    /// (c) The page cannot rewrite the access mode.
    #[test]
    fn the_page_cannot_rewrite_the_access_mode() {
        setup();
        setup_solo(&args(vec![s(ROOT)])).unwrap();
        let answer = set_setting(&args(vec![s(ACCESS_MODE), s("helpdesk")]));
        assert_refused(answer, "set_setting", ACCESS_MODE);
        assert_eq!(stored_setting(ACCESS_MODE).as_deref(), Some("solo"));
        assert_eq!(state()["mode"], "solo");
        assert_eq!(state()["admin_root"], ROOT);
    }

    /// (d) The page cannot delete the admin's recovery key: the stored hash
    /// is the same afterwards, and the key it belongs to still works.
    #[test]
    fn the_page_cannot_delete_the_recovery_key() {
        setup();
        let key = key_of(setup_helpdesk(&args(vec![s("0817")])));
        let before = stored_setting(RECOVERY_KEY_HASH);
        assert!(before.is_some(), "setup stores a recovery key hash");
        let answer = delete_setting(&args(vec![s(RECOVERY_KEY_HASH)]));
        assert_refused(answer, "delete_setting", RECOVERY_KEY_HASH);
        assert!(
            stored_setting(RECOVERY_KEY_HASH) == before,
            "the refused delete changed the stored recovery key hash"
        );
        assert!(truthy(use_recovery_key(&args(vec![s(&key)]))));
    }

    /// Each of the three verbs refuses every host setting, whatever is
    /// stored under it, and leaves the stored value as it was; so too a key
    /// that only looks like the page's.
    #[test]
    fn every_other_setting_is_refused_by_all_three_verbs() {
        setup();
        const STORED: &str = "stored-setting-P8D2";
        for key in [
            ACCESS_MODE,
            ADMIN_TIN_ROOT,
            ADMIN_PIN_HASH,
            ADMIN_TOTP_SECRET,
            RECOVERY_KEY_HASH,
            "Theme_Preference",
            " theme_preference",
        ] {
            set(key, STORED);
            let read = get_setting(&args(vec![s(key)]));
            assert!(
                !format!("{read:?}").contains(STORED),
                "get_setting sent the value of `{key}`"
            );
            assert_refused(read, "get_setting", key);
            assert_refused(
                set_setting(&args(vec![s(key), s("page-value")])),
                "set_setting",
                key,
            );
            assert_refused(delete_setting(&args(vec![s(key)])), "delete_setting", key);
            assert!(
                stored_setting(key).as_deref() == Some(STORED),
                "a refused verb changed `{key}`"
            );
        }
    }

    #[test]
    fn mac_login_answers_without_blocking_the_caller_off_macos() {
        if mac_login_available() {
            return; // would show a real OS prompt
        }
        let future = module()
            .begin("mac_login", &args(vec![s("unlock")]))
            .expect("sync half");
        let answer = futures::executor::block_on(future).unwrap();
        assert_eq!(answer, HostValue::Bool(false));
    }

    // ---------- import: the host always opens the file dialog ----------

    /// A directory under the system temp dir, removed when this drops.
    struct ScratchDir(std::path::PathBuf);

    impl ScratchDir {
        fn new(label: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bir-shell-import-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const ARCHIVE_TIN: &str = "98765432000000";

    fn new_taxpayer(tin_root: &str, name: &str) -> NewTaxpayer {
        NewTaxpayer {
            tin_root: tin_root.into(),
            name: name.into(),
            taxpayer_type: TaxpayerType::Corporation,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "043".into(),
                registered_address: "2 Sample Road, Makati".into(),
                zip_code: "1200".into(),
                phone: "0280000001".into(),
                email: "books@example.com".into(),
            },
        }
    }

    /// A profile archive of `ARCHIVE_TIN` with one submission, exported
    /// from a database of its own.
    fn profile_archive(scratch: &ScratchDir) -> std::path::PathBuf {
        let source = Database::open_ephemeral().unwrap();
        source
            .create_taxpayer(new_taxpayer("987654320", "Archived Holdings Inc"))
            .unwrap();
        source
            .save_submission(bir_core::db::Submission {
                id: None,
                tin: ARCHIVE_TIN.into(),
                form_type: "1702Q".into(),
                period: "2026Q1".into(),
                status: "Draft".into(),
                form_data: Default::default(),
                submitted_at: None,
                filename: None,
                created_at: None,
                updated_at: None,
            })
            .unwrap();
        let archive = scratch.path().join("profile.zip");
        bir_core::export::export_profile_data(&source, ARCHIVE_TIN, &archive, None).unwrap();
        archive
    }

    fn archive_imported() -> bool {
        with_database(|db| {
            Ok(db
                .get_profile_by_tin(ARCHIVE_TIN)
                .map_err(db_error)?
                .is_some())
        })
        .unwrap()
    }

    /// The page cannot choose the file. The registered verb refuses a call
    /// with any argument, a path included, before a dialog opens, and
    /// imports nothing; `import_archive_from` is no verb at all.
    #[test]
    fn import_archive_refuses_any_argument() {
        setup();
        let scratch = ScratchDir::new("refused");
        let archive = profile_archive(&scratch);
        let module = module();
        assert!(module.has("import_archive"));
        assert!(!module.has("import_archive_from"), "not a verb");
        // The path first: a verb that took it would import it here instead
        // of falling through to a dialog.
        for argument in [
            s(&archive.display().to_string()),
            HostValue::Null,
            HostValue::Bool(true),
        ] {
            match module.begin("import_archive", &args(vec![argument])) {
                Ok(_) => panic!("import_archive answered a call with an argument"),
                Err(refusal) => assert_eq!(
                    refusal.message(),
                    "import_archive takes no path; the host opens the file dialog"
                ),
            }
            assert!(!archive_imported(), "a refused call imported the archive");
        }
    }

    /// `import_archive_from` imports a profile archive into the open
    /// database, and answers `invalid` for a zip that is neither export.
    #[test]
    fn import_archive_from_imports_a_profile_archive() {
        setup();
        let scratch = ScratchDir::new("profile");
        let archive = profile_archive(&scratch);
        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "profile" })
        );
        with_database(|db| {
            let profile = db
                .get_profile_by_tin(ARCHIVE_TIN)
                .map_err(db_error)?
                .expect("the imported profile");
            assert_eq!(profile.full_name, "Archived Holdings Inc");
            let submissions = db.list_submissions_for_tin(ARCHIVE_TIN).map_err(db_error)?;
            assert_eq!(submissions.len(), 1);
            assert_eq!(submissions[0].form_type, "1702Q");
            Ok(())
        })
        .unwrap();

        let other = scratch.path().join("other.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&other).unwrap());
        zip.start_file("notes.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"not an export").unwrap();
        zip.finish().unwrap();
        assert_eq!(
            to_json(&import_archive_from(&other).unwrap()),
            serde_json::json!({ "kind": "invalid" })
        );
    }

    /// The Settings all-profiles backup puts each profile under
    /// `Profiles/<TIN>/`: `import_archive_from` reads every profile in it,
    /// not just a top-level `profile.json`.
    #[test]
    fn import_archive_from_imports_a_combined_archive() {
        setup();
        let scratch = ScratchDir::new("combined");
        let source = Database::open_ephemeral().unwrap();
        source
            .create_taxpayer(new_taxpayer("111222333", "First Combined Corp"))
            .unwrap();
        source
            .create_taxpayer(new_taxpayer("444555666", "Second Combined Corp"))
            .unwrap();
        let archive = scratch.path().join("all-profiles.zip");
        bir_core::export::export_all_profiles_data(&source, &archive).unwrap();
        drop(source);
        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "profile" })
        );
        with_database(|db| {
            for (tin, name) in [
                ("11122233300000", "First Combined Corp"),
                ("44455566600000", "Second Combined Corp"),
            ] {
                let profile = db
                    .get_profile_by_tin(tin)
                    .map_err(db_error)?
                    .unwrap_or_else(|| panic!("the imported profile for {tin}"));
                assert_eq!(profile.full_name, name);
            }
            Ok(())
        })
        .unwrap();
    }

    /// `import_archive_from` replaces the open database with a database
    /// archive, after a safety copy of the file it replaces. That path
    /// writes the app's database file and reads its key, so it runs in a
    /// child test process whose `BIR_DATABASE_PATH` is a scratch file and
    /// whose `EBIR_TEST_ENV` keeps the key off the keychain: never the live
    /// database.
    #[test]
    fn import_archive_from_restores_a_database_archive() {
        const SCRATCH: &str = "BIR_SHELL_IMPORT_ARCHIVE_SCRATCH";
        if let Some(scratch) = std::env::var_os(SCRATCH) {
            restore_a_database_archive_in(std::path::Path::new(&scratch));
            return;
        }
        let scratch = ScratchDir::new("database");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bir_module::tests::import_archive_from_restores_a_database_archive",
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(SCRATCH, scratch.path())
            .env("BIR_DATABASE_PATH", scratch.path().join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the database import child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "the child ran the import"
        );
    }

    /// The child's half of the test above, in the scratch environment its
    /// parent set; it refuses to run anywhere else.
    fn restore_a_database_archive_in(scratch: &std::path::Path) {
        let file = scratch.join("bir_data.db");
        assert_eq!(db::app_database_path(), file, "the scratch database");
        assert!(
            bir_core::db::test_environment(),
            "the key stays off the keychain"
        );
        with_database(|db| {
            db.create_taxpayer(new_taxpayer("123456788", "Replaced Trading Corp"))
                .map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        let source = Database::open(scratch.join("source.db")).unwrap();
        source
            .create_taxpayer(new_taxpayer("987654320", "Restored Holdings Inc"))
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);

        // A folder under review for the eBIRForms import, read against the
        // database about to be replaced.
        let legacy_folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../bir-core/tests/legacy-ebirforms");
        crate::legacy_import_module::scan_folder(&legacy_folder).unwrap();
        assert!(crate::legacy_import_module::holds_scan());

        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            serde_json::json!({ "kind": "database" })
        );
        assert!(
            !crate::legacy_import_module::holds_scan(),
            "a scan reviewed against the replaced database is still held"
        );
        let safety_copies: Vec<_> = std::fs::read_dir(scratch)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("bir_data.safety-")
            })
            .collect();
        assert_eq!(safety_copies.len(), 1, "a safety copy of the replaced file");
        // The safety copy holds the just-created taxpayer because the import
        // checkpoints the WAL before copying — a raw copy alone can leave
        // committed data still in -wal, out of the file it saved.
        let safety_copy = Database::open(&safety_copies[0]).unwrap();
        assert!(
            safety_copy
                .list_profiles()
                .unwrap()
                .iter()
                .any(|profile| profile.full_name == "Replaced Trading Corp"),
            "the safety copy missed the committed profile"
        );
        drop(safety_copy);
        let tins = with_database(|db| {
            Ok(db
                .list_profiles()
                .map_err(db_error)?
                .iter()
                .map(|profile| profile.tin.full())
                .collect::<Vec<_>>())
        })
        .unwrap();
        assert_eq!(tins, [ARCHIVE_TIN], "the archive's database is open");
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// Run the test `name` again in a child process whose database is
    /// `<scratch>/data/bir_data.db` and whose `EBIR_TEST_ENV` keeps the key off
    /// the keychain: a database import writes the app's database file, so it
    /// never runs against this process's. The child writes `child-done`.
    fn run_in_scratch_process(name: &str, scratch: &ScratchDir) {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("bir_module::tests::{name}"),
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(IMPORT_CHILD, scratch.path())
            .env(
                "BIR_DATABASE_PATH",
                scratch.path().join("data").join("bir_data.db"),
            )
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .status()
            .unwrap();
        assert!(status.success(), "{name}: the child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "{name}: the child ran to its end"
        );
    }

    const IMPORT_CHILD: &str = "BIR_IMPORT_ARCHIVE_CHILD_SCRATCH";

    /// The child's start: its scratch folder, after the checks that it runs
    /// on a scratch database and never the live one.
    fn import_child(scratch: &std::ffi::OsStr) -> std::path::PathBuf {
        let scratch = std::path::PathBuf::from(scratch);
        std::fs::create_dir_all(scratch.join("inputs")).unwrap();
        assert_eq!(
            db::app_database_path(),
            scratch.join("data").join("bir_data.db"),
            "the scratch database"
        );
        assert!(
            bir_core::db::test_environment(),
            "the key stays off the keychain"
        );
        with_database(|db| {
            db.create_taxpayer(new_taxpayer("123456788", "Kept Trading Corp"))
                .map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        scratch
    }

    /// A database backup archive of a database holding "Restored Holdings Inc".
    fn good_database_archive(scratch: &std::path::Path) -> std::path::PathBuf {
        let source = Database::open(scratch.join("inputs").join("source.db")).unwrap();
        source
            .create_taxpayer(new_taxpayer("987654320", "Restored Holdings Inc"))
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.join("inputs").join("good.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);
        archive
    }

    /// The names of the profiles in the open database.
    fn profile_names() -> Vec<String> {
        with_database(|db| {
            Ok(db
                .list_profiles()
                .map_err(db_error)?
                .into_iter()
                .map(|profile| profile.full_name.clone())
                .collect())
        })
        .unwrap()
    }

    /// Files next to the database that an import must not leave behind or
    /// make before it is sure: safety copies and staging files.
    fn import_leftovers(scratch: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(scratch.join("data"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("bir_data.safety-") || name.contains(".import-"))
            .collect()
    }

    /// An archive whose database cannot replace the live one is refused
    /// before anything changes: the open database keeps its data and
    /// connection, no safety copy is made, no staging file is left.
    #[test]
    fn a_database_archive_that_fails_its_checks_changes_nothing() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            let archive = scratch.join("inputs").join("bad.zip");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            zip.start_file("bir_data.db", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, &b"\x01\x02not-sqlite-".repeat(512)).unwrap();
            zip.finish().unwrap();

            match import_archive_from(&archive) {
                Ok(_) => panic!("an archive that is not a database was imported"),
                Err(error) => {
                    assert!(
                        error.message().contains("not a plain SQLite"),
                        "{}",
                        error.message()
                    );
                    assert!(error.message().contains("left as it is"));
                }
            }
            assert_eq!(import_leftovers(&scratch), Vec::<String>::new());
            assert!(!RELEASED.get(), "the live connection was never let go");
            assert_eq!(profile_names(), ["Kept Trading Corp"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_database_archive_that_fails_its_checks_changes_nothing",
            &ScratchDir::new("unfit"),
        );
    }

    /// No safety copy, no import: when the copy of the file about to be
    /// replaced cannot be made, the import stops with that said, the live
    /// database keeps its data, and nothing is left behind.
    #[test]
    fn a_failed_safety_copy_stops_the_database_import() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            let archive = good_database_archive(&scratch);
            // The copy's name is taken by a folder, so copying onto it fails.
            std::fs::create_dir(scratch.join("data").join("bir_data.safety-7.db")).unwrap();

            match restore_database_archive(&archive, 7) {
                Ok(_) => panic!("the import went on without a safety copy"),
                Err(error) => {
                    assert!(
                        error.message().contains("safety copy"),
                        "{}",
                        error.message()
                    );
                    assert!(error.message().contains("nothing was imported"));
                }
            }
            assert_eq!(
                import_leftovers(&scratch),
                ["bir_data.safety-7.db"],
                "only the folder that was in the way"
            );
            assert!(!RELEASED.get(), "the live connection was never let go");
            assert_eq!(profile_names(), ["Kept Trading Corp"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_failed_safety_copy_stops_the_database_import",
            &ScratchDir::new("safety"),
        );
    }

    /// A checkpoint that another connection's read snapshot holds back is an
    /// error, and the import stops on it: no safety copy, the connection is
    /// never let go, the live database keeps its latest commit and no file
    /// beside it (the -wal holding that commit, the -shm) was deleted.
    /// (Before, the busy answer was ignored: the safety copy missed the
    /// commit still in the -wal, and the install deleted the -wal.)
    #[test]
    fn a_checkpoint_a_reader_holds_back_stops_the_database_import() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            let archive = good_database_archive(&scratch);
            let db_path = db::app_database_path();
            let reader = Database::open_existing_read_only(&db_path).unwrap();
            reader.conn_for_tests().execute_batch("BEGIN").unwrap();
            let _: i64 = reader
                .conn_for_tests()
                .query_row("SELECT count(*) FROM profiles", [], |row| row.get(0))
                .unwrap();
            with_database(|db| {
                db.create_taxpayer(new_taxpayer("111222333", "Latest Commit Corp"))
                    .map_err(db_error)?;
                Ok(())
            })
            .unwrap();
            let files = || {
                let mut names: Vec<String> = std::fs::read_dir(scratch.join("data"))
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                names
            };
            let before = files();
            assert!(
                before.contains(&"bir_data.db-wal".to_string()),
                "{before:?}"
            );

            match restore_database_archive(&archive, 11) {
                Ok(_) => panic!("the import went on over a checkpoint that did not finish"),
                Err(error) => {
                    assert!(
                        error.message().contains("in use elsewhere"),
                        "{}",
                        error.message()
                    );
                    assert!(error.message().contains("nothing was imported"));
                    // Words for a person: no error kind, no page counts.
                    assert!(
                        !error.message().contains("Other error"),
                        "{}",
                        error.message()
                    );
                    assert!(
                        !error.message().contains("log pages"),
                        "{}",
                        error.message()
                    );
                }
            }
            assert_eq!(files(), before, "no file made, none deleted");
            assert_eq!(import_leftovers(&scratch), Vec::<String>::new());
            assert!(!RELEASED.get(), "the live connection was never let go");
            assert_eq!(profile_names(), ["Kept Trading Corp", "Latest Commit Corp"]);

            // The reader done, the same import goes through.
            reader.conn_for_tests().execute_batch("COMMIT").unwrap();
            drop(reader);
            restore_database_archive(&archive, 12).expect("nothing holds the log now");
            assert_eq!(profile_names(), ["Restored Holdings Inc"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_checkpoint_a_reader_holds_back_stops_the_database_import",
            &ScratchDir::new("busy"),
        );
    }

    /// The safety copy holds every commit: one lock is held from the
    /// checkpoint to the swap, so a writer on the shared connection (as the
    /// submission worker is) that tries to commit between the checkpoint and
    /// the copy waits, or its commit is in the copy. (Before, the lock was let
    /// go after the checkpoint: such a commit went to the -wal only, after the
    /// checkpoint, and the copy missed it.)
    #[test]
    fn a_commit_between_the_checkpoint_and_the_safety_copy_waits_or_is_copied() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            let archive = good_database_archive(&scratch);
            let handle = open_database_slot().unwrap();
            let committed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let seen_before_copy = std::rc::Rc::new(std::cell::Cell::new(None));
            let writer = std::rc::Rc::new(std::cell::RefCell::new(None));
            {
                let committed = committed.clone();
                let seen_before_copy = seen_before_copy.clone();
                let writer = writer.clone();
                AFTER_RESTORE_CHECKPOINT.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        let flag = committed.clone();
                        *writer.borrow_mut() = Some(std::thread::spawn(move || {
                            let db = bir_core::lock_timing::lock(&handle).unwrap();
                            db.create_taxpayer(new_taxpayer("111222333", "Gap Writer Corp"))
                                .unwrap();
                            flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        }));
                        // Give the writer every chance to commit before the copy.
                        let until = std::time::Instant::now() + std::time::Duration::from_secs(1);
                        while !committed.load(std::sync::atomic::Ordering::SeqCst)
                            && std::time::Instant::now() < until
                        {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        seen_before_copy
                            .set(Some(committed.load(std::sync::atomic::Ordering::SeqCst)));
                    }));
                });
            }

            restore_database_archive(&archive, 21).expect("the import");
            let writer = writer.borrow_mut().take().expect("the hook ran");
            writer.join().expect("the writer committed after the swap");
            let committed_before_copy = seen_before_copy.get().expect("the hook ran");

            let copy = Database::open(scratch.join("data").join("bir_data.safety-21.db")).unwrap();
            let in_copy = copy
                .list_profiles()
                .unwrap()
                .iter()
                .any(|profile| profile.full_name == "Gap Writer Corp");
            assert!(
                !committed_before_copy || in_copy,
                "a commit made before the copy is missing from it"
            );
            assert!(!committed_before_copy, "the writer waited for the lock");
            assert!(
                copy.list_profiles()
                    .unwrap()
                    .iter()
                    .any(|profile| profile.full_name == "Kept Trading Corp")
            );
            drop(copy);
            assert_eq!(profile_names(), ["Restored Holdings Inc"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_commit_between_the_checkpoint_and_the_safety_copy_waits_or_is_copied",
            &ScratchDir::new("gap"),
        );
    }

    /// Plain build: the replace fails after the connection was let go (the
    /// checked file is gone, so the rename has nothing to move). The import
    /// reports it and the connection is back on the file that is still
    /// there, at once and not at the next call, with its data.
    #[cfg(not(feature = "sqlcipher"))]
    #[test]
    fn a_database_replace_that_fails_puts_the_connection_back() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            let archive = good_database_archive(&scratch);
            let db_path = db::app_database_path();
            let staged = bir_core::import::stage_database_zip(&archive, &db_path).unwrap();
            std::fs::remove_file(staged.path()).unwrap();

            assert!(restore_staged_database(staged, &db_path, 9).is_err());
            assert!(
                !RELEASED.get(),
                "the connection was reopened by the failure"
            );
            assert_eq!(profile_names(), ["Kept Trading Corp"]);
            assert_eq!(import_leftovers(&scratch), ["bir_data.safety-9.db"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_database_replace_that_fails_puts_the_connection_back",
            &ScratchDir::new("replace"),
        );
    }

    // ---------- the submission worker ----------

    /// The shell's launch starts the submission worker on the database it
    /// opens, so a queued return is sent without another GUI process or
    /// `bir` running. Opening the database reads its key, so this
    /// runs in a child test process on a scratch `BIR_DATABASE_PATH` with
    /// `EBIR_TEST_ENV` (key off the keychain) and `BIR_SFTP_DRY_RUN`: never
    /// the live database, never BIR.
    #[test]
    fn the_shell_launch_starts_the_submission_worker() {
        const SCRATCH: &str = "BIR_SHELL_SUBMISSION_WORKER_SCRATCH";
        if let Some(scratch) = std::env::var_os(SCRATCH) {
            start_the_submission_worker_in(std::path::Path::new(&scratch));
            return;
        }
        let scratch = ScratchDir::new("worker");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bir_module::tests::the_shell_launch_starts_the_submission_worker",
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(SCRATCH, scratch.path())
            .env("BIR_DATABASE_PATH", scratch.path().join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the submission worker child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "the child ran the launch"
        );
    }

    /// The host's refusal holds where the worker starts, not only by the name
    /// of the host function the NSH door refuses: with the latch set, the
    /// launch and a sign-in's "session opened" (through the module, as the
    /// page calls it) open the database and start no worker. A child process:
    /// the latch and the worker are once per process.
    #[test]
    fn a_refused_worker_is_started_by_no_route() {
        if let Some(scratch) = std::env::var_os(IMPORT_CHILD) {
            let scratch = import_child(&scratch);
            assert!(!submission_worker_refused());
            refuse_submission_worker();
            assert!(submission_worker_refused());

            open_database_at_launch();
            assert!(!submission_worker_started(), "the launch");
            module()
                .call(
                    "cloud_account_session_opened",
                    &gpui_shell::HostArguments::new([]),
                )
                .expect("the session opens");
            assert!(!submission_worker_started(), "a sign-in's session opened");
            open_database_at_launch();
            assert!(!submission_worker_started(), "a second launch");
            // The database itself opens as before.
            assert_eq!(profile_names(), ["Kept Trading Corp"]);
            std::fs::write(scratch.join("child-done"), "ok").unwrap();
            return;
        }
        run_in_scratch_process(
            "a_refused_worker_is_started_by_no_route",
            &ScratchDir::new("refused"),
        );
    }

    /// The child's half of the test above. A profile with the test
    /// notification on makes every worker tick record a heartbeat alert, so
    /// the alert appearing proves the worker runs on the shell's database.
    fn start_the_submission_worker_in(scratch: &std::path::Path) {
        assert_eq!(
            db::app_database_path(),
            scratch.join("bir_data.db"),
            "the scratch database"
        );
        assert!(
            bir_core::db::test_environment(),
            "the key stays off the keychain"
        );
        assert!(!submission_worker_started(), "nothing runs before launch");

        open_database_at_launch();
        assert!(submission_worker_started(), "the launch started the worker");

        with_database(|db| {
            db.create_taxpayer(new_taxpayer("123456788", "Worker Trading Corp"))
                .map_err(db_error)?;
            let mut profile = db
                .list_profiles()
                .map_err(db_error)?
                .into_iter()
                .next()
                .expect("the created profile");
            profile.test_notification_enabled = true;
            db.save_profile(profile).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        wake_submission_worker();

        let heartbeat = || {
            with_database(|db| {
                Ok(db
                    .list_active_alerts(None)
                    .map_err(db_error)?
                    .iter()
                    .any(|alert| alert.kind == db::alert_kinds::CRON_HEARTBEAT))
            })
            .unwrap()
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !heartbeat() {
            assert!(
                std::time::Instant::now() < deadline,
                "the submission worker never ticked on the shell's database"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    // ---------- one engine after another in one process ----------
    //
    // An Android process outlives a Back-exit or a swipe from Recents, and the
    // next launch starts a new engine (a new thread with its own database
    // slot) in the same process; an iPhone or iPad scene that reconnects does
    // the same. Each test runs in a child process of its own (the worker slot
    // is the process's), on a scratch database, with the dry run. An engine
    // here is a thread that opens the database through the launch the door
    // uses and is ended by the call the door's guard makes.

    const ENGINE_CHILD: &str = "BIR_ENGINE_LIFECYCLE_CHILD_SCRATCH";

    fn run_engine_child(name: &str) {
        if std::env::var_os(ENGINE_CHILD).is_some() {
            return;
        }
        let scratch = ScratchDir::new("engines");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("bir_module::tests::{name}"),
                "--nocapture",
            ])
            .current_dir(scratch.path())
            .env(ENGINE_CHILD, scratch.path())
            .env(
                "BIR_DATABASE_PATH",
                scratch.path().join("data").join("bir_data.db"),
            )
            // A child that installs the app's logger writes its files here, at info.
            .env("BIR_LOG_DIR", scratch.path().join("logs"))
            .env("RUST_LOG", "info")
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_AUTH_BYPASS", "1")
            .env("BUWIZ_NO_KEYCHAIN", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .env_remove("BIR_SFTP_LIVE")
            .env_remove("BIR_SFTP_HOST")
            .env_remove("BIR_RELAY_URL")
            .env_remove("BUWIZ_RELAY_SEND")
            .status()
            .unwrap();
        assert!(status.success(), "{name}: the child failed");
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("child-done"))
                .ok()
                .as_deref(),
            Some("ok"),
            "{name}: the child ran to its end"
        );
    }

    /// The child's scratch folder, after the checks that it runs on a scratch
    /// database, never the live one, and never sends anywhere.
    fn engine_child() -> Option<std::path::PathBuf> {
        let scratch = std::path::PathBuf::from(std::env::var_os(ENGINE_CHILD)?);
        assert_eq!(
            db::app_database_path(),
            scratch.join("data").join("bir_data.db"),
            "the scratch database"
        );
        assert!(
            bir_core::db::test_environment(),
            "the key stays off the keychain"
        );
        assert!(bir_core::transport::is_sftp_dry_run(), "the dry run");
        Some(scratch)
    }

    /// What the door's `ShutdownGuard` does when an engine thread ends.
    struct EngineEnd;

    impl Drop for EngineEnd {
        fn drop(&mut self) {
            shutdown_for_quit();
        }
    }

    /// Run `body` as one engine: a thread of its own, the launch first (the
    /// database and the submission worker), its end last.
    fn engine<R: Send>(body: impl FnOnce() -> R + Send) -> R {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _end = EngineEnd;
                    open_database_at_launch();
                    body()
                })
                .join()
                .expect("the engine thread")
        })
    }

    /// Nobody in this process holds the owner lock: a fresh open takes it.
    fn owner_lock_is_free() -> bool {
        bir_core::db::try_acquire_live_owner_lock(&db::app_database_path()).is_ok()
    }

    /// Juana Santos, TIN 000-123-453-00000, the demo taxpayer, email tracking off.
    fn demo_profile() -> bir_core::profile::TaxpayerProfile {
        serde_json::from_value(serde_json::json!({
            "id": null,
            "full_name": "Juana Santos",
            "tin": {"segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000"},
            "rdo_code": "040",
            "line_of_business": "Retail",
            "registered_address": "1 Example Street, Pasig",
            "zip_code": "1600",
            "phone": "0280000000",
            "email": "juana@example.com",
            "default_form_type": "2551Qv2018",
            "taxpayer_type": "Individual",
            "business_start_date": "2010-01-01"
        }))
        .unwrap()
    }

    const ENGINE_YEAR: u16 = 2024;

    /// Queue the demo taxpayer's 1601C for `month` in this engine, and wake its worker.
    fn queue_1601c(month: u8) -> bir_core::forms::form_1601c::Form1601CDraft {
        let profile = demo_profile();
        let mut draft = bir_core::forms::form_1601c::Form1601CDraft::new_from_profile(
            &profile,
            ENGINE_YEAR,
            month,
        );
        draft.any_taxes_withheld = false;
        draft.transition_to_queued().expect("the demo 1601C queues");
        with_database(|db| {
            if db.list_profiles().map_err(db_error)?.is_empty() {
                db.save_profile(profile.clone()).map_err(db_error)?;
            }
            db.save_queued_1601c_draft(&draft).map_err(db_error)?;
            Ok(())
        })
        .unwrap();
        wake_submission_worker();
        draft
    }

    fn stored_1601c(month: u8) -> bir_core::forms::form_1601c::Form1601CDraft {
        with_database(|db| {
            db.get_1601c_draft(&demo_profile().tin.full(), ENGINE_YEAR, month)
                .map_err(db_error)
        })
        .unwrap()
        .expect("the 1601C row")
    }

    fn job_key_1601c(month: u8) -> String {
        bir_core::background_cron::submission_job_key(
            "1601C",
            &demo_profile().tin.full(),
            ENGINE_YEAR,
            Some(month),
            None,
        )
    }

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "never: {what}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// Three engines in one process, one after the other: each one's launch
    /// starts a worker of its own, each queues a return, and each one's worker
    /// sends it (the dry run). After each end the worker is gone and the owner
    /// lock is free, so the next engine opens the database. (The second engine
    /// used to have no worker at all, silently, and the third was refused the
    /// database.)
    #[test]
    fn one_engine_after_another_each_sends_what_it_queued() {
        run_engine_child("one_engine_after_another_each_sends_what_it_queued");
        let Some(scratch) = engine_child() else {
            return;
        };
        for month in 1..=3u8 {
            engine(|| {
                assert!(
                    submission_worker_runs_for_this_engine(),
                    "engine {month}: the launch started its own worker"
                );
                assert!(!owner_lock_released(), "engine {month}: it holds the lock");
                queue_1601c(month);
                wait_until(&format!("engine {month} sent its 1601C"), || {
                    stored_1601c(month).status == bir_core::forms::FilingStatus::Submitted
                });
                let sent = stored_1601c(month);
                assert!(sent.submitted_dry_run, "engine {month}: a dry run");
                for earlier in 1..month {
                    assert_eq!(
                        stored_1601c(earlier).status,
                        bir_core::forms::FilingStatus::Submitted,
                        "engine {month}: the earlier engines' returns stay sent"
                    );
                }
            });
            assert!(
                !submission_worker_started(),
                "engine {month}: its worker went with it"
            );
            assert!(
                owner_lock_released(),
                "engine {month}: its database was closed"
            );
            assert!(
                owner_lock_is_free(),
                "engine {month}: the lock is free for the next"
            );
        }
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// A quit (or an update's restart) ends the process's sending whoever started the worker: called on a thread that
    /// did not start it, it stops it all the same. Before, it stopped nothing there, and the worker ran on until its
    /// own engine ended. The database is not the quit's to close there: it belongs to its engine's thread, stays open
    /// (the owner lock held) while that engine lives, and closes at its end.
    #[test]
    fn a_quit_on_another_thread_stops_the_worker() {
        run_engine_child("a_quit_on_another_thread_stops_the_worker");
        let Some(scratch) = engine_child() else {
            return;
        };
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _end = EngineEnd;
                    open_database_at_launch();
                    assert!(
                        submission_worker_runs_for_this_engine(),
                        "the engine's worker"
                    );
                    scope
                        .spawn(shutdown_for_quit)
                        .join()
                        .expect("the quit on another thread");
                    assert!(!submission_worker_started(), "the quit stopped it");
                    assert!(
                        !owner_lock_released(),
                        "the engine's database is still open"
                    );
                    assert!(!owner_lock_is_free(), "and its owner lock still held");
                    assert!(
                        database_handle().is_ok(),
                        "the engine still uses its database"
                    );
                })
                .join()
                .expect("the engine");
        });
        assert!(owner_lock_released(), "closed at the engine's end");
        assert!(owner_lock_is_free(), "the engine's end freed the lock");
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// The door's end guard is a thread local, armed before the engine first
    /// logs, so at the thread's end the logger's own per-thread buffer is
    /// destroyed before the guard runs. Here the end logs (the worker is
    /// another engine's) with the app's logger and panic hook installed: before,
    /// that log line panicked, the panic hook's own log line panicked again and
    /// the process aborted (14 screen-test files on macOS). Now the engine ends,
    /// the other engine's worker runs on, and that engine's end frees the lock.
    #[test]
    fn an_engine_end_that_logs_while_its_thread_goes_does_not_abort() {
        run_engine_child("an_engine_end_that_logs_while_its_thread_goes_does_not_abort");
        let Some(scratch) = engine_child() else {
            return;
        };
        assert!(bir_core::logging::install(
            bir_core::logging::InstallOptions {
                developer_mode: false,
                default_filter: "info",
            }
        ));

        struct DoorGuard;
        impl Drop for DoorGuard {
            fn drop(&mut self) {
                let _ = std::panic::catch_unwind(shutdown_at_engine_end);
            }
        }
        thread_local! {
            static ARMED: DoorGuard = const { DoorGuard };
        }

        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _end = EngineEnd;
                    open_database_at_launch();
                    assert!(
                        submission_worker_runs_for_this_engine(),
                        "engine A started the worker"
                    );
                    scope
                        .spawn(|| {
                            // As the door boots an engine: the slots, the guard, then the first log line.
                            prepare_engine();
                            ARMED.with(|_| {});
                            tracing::info!("engine B boots");
                        })
                        .join()
                        .expect("engine B's thread ends without a panic");
                    assert!(
                        submission_worker_runs_for_this_engine(),
                        "engine B's end left engine A's worker running"
                    );
                })
                .join()
                .expect("engine A");
        });
        assert!(
            !submission_worker_started(),
            "engine A's worker went with it"
        );
        assert!(owner_lock_is_free(), "engine A's end freed the lock");
        let logs = std::fs::read_dir(scratch.join("logs"))
            .unwrap()
            .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap_or_default())
            .collect::<String>();
        assert!(
            logs.contains("an engine without a submission worker of its own ended"),
            "engine B's end said why it stopped nothing"
        );
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// An engine ends while its worker is sending (a relay on this computer
    /// that takes the request and never answers): the worker is dropped within
    /// the bound, the return keeps its claim and "upload started", its job key
    /// is gone, and the database closes. The next engine's worker finds the
    /// claim and does not send it again: the relay saw one request in all,
    /// what a desktop relaunch after a quit mid-send does.
    #[test]
    fn an_engine_ended_mid_send_abandons_the_send_and_the_next_does_not_resend() {
        run_engine_child("an_engine_ended_mid_send_abandons_the_send_and_the_next_does_not_resend");
        let Some(scratch) = engine_child() else {
            return;
        };
        let relay = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", relay.local_addr().unwrap());
        // Uploads the relay took (health checks are not counted).
        let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = requests.clone();
        // Answers the health check before the claim; takes each upload and never answers it.
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut held = Vec::new();
            for mut stream in relay.incoming().flatten() {
                let mut head = [0u8; 4096];
                let Ok(read) = stream.read(&mut head) else {
                    continue;
                };
                if head[..read].starts_with(b"GET /v1/health") {
                    let body = format!(
                        r#"{{"service":"{}","version":1,"ok":true,"mock":true}}"#,
                        bir_core::transport::MOCK_RELAY_SERVICE
                    );
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    continue;
                }
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                held.push(stream);
            }
        });
        bir_core::config::force(bir_core::transport::RELAY_URL_ENV, Some(&url));

        let month = 4;
        let key = job_key_1601c(month);
        let ending = engine(|| {
            queue_1601c(month);
            wait_until("the send reached the relay", || {
                requests.load(std::sync::atomic::Ordering::SeqCst) == 1
            });
            assert!(stored_1601c(month).submission_put_started_at.is_some());
            assert!(
                bir_core::background_cron::get_active_jobs()
                    .lock()
                    .unwrap()
                    .contains(&key),
                "the send is in flight"
            );
            std::time::Instant::now()
        });
        let took = ending.elapsed();
        assert!(
            took < WORKER_STOP_BOUND + WORKER_DROP_GRACE * 2,
            "the engine ended within the bound: {took:?}"
        );
        assert!(!submission_worker_started());
        assert!(
            !bir_core::background_cron::get_active_jobs()
                .lock()
                .unwrap()
                .contains(&key),
            "the abandoned send's key is gone"
        );
        assert!(
            owner_lock_is_free(),
            "the lock is free after a send was dropped"
        );

        engine(|| {
            assert!(
                submission_worker_runs_for_this_engine(),
                "the next engine has a worker"
            );
            let claimed = stored_1601c(month);
            assert_eq!(claimed.status, bir_core::forms::FilingStatus::Queued);
            assert!(claimed.submission_claim_token.is_some(), "the claim stays");
            assert!(
                claimed.submission_put_started_at.is_some(),
                "upload started stays"
            );
            assert!(claimed.submitted_at.is_none());
            // Ticks: the wake, then another wake a moment later.
            wake_submission_worker();
            std::thread::sleep(std::time::Duration::from_secs(2));
            wake_submission_worker();
            std::thread::sleep(std::time::Duration::from_secs(2));
            assert_eq!(
                requests.load(std::sync::atomic::Ordering::SeqCst),
                1,
                "the claimed return was not sent again"
            );
            let after = stored_1601c(month);
            assert_eq!(after.submission_claim_token, claimed.submission_claim_token);
            assert_eq!(after.status, bir_core::forms::FilingStatus::Queued);
        });
        assert!(owner_lock_is_free());
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// The desktop quit and the update install: one engine, `shutdown_for_quit`
    /// on request, the worker stopped within the bound and the lock released;
    /// a second call does nothing. While something else still holds the
    /// handle, the lock is not released and `owner_lock_released` says so (the
    /// update does not relaunch); it is free once that goes.
    #[test]
    fn a_desktop_quit_stops_the_worker_and_releases_the_lock() {
        run_engine_child("a_desktop_quit_stops_the_worker_and_releases_the_lock");
        let Some(scratch) = engine_child() else {
            return;
        };
        engine(|| {
            assert!(submission_worker_runs_for_this_engine());
            let quit = std::time::Instant::now();
            shutdown_for_quit();
            assert!(quit.elapsed() < WORKER_STOP_BOUND, "{:?}", quit.elapsed());
            assert!(!submission_worker_started());
            assert!(owner_lock_released());
            assert!(owner_lock_is_free());
            shutdown_for_quit();
            assert!(owner_lock_released());
        });

        let held = engine(|| {
            let held = database_handle().unwrap();
            shutdown_for_quit();
            assert!(
                !submission_worker_started(),
                "the worker stops all the same"
            );
            assert!(!owner_lock_released(), "the update would not relaunch");
            held
        });
        assert!(!owner_lock_is_free(), "still held");
        drop(held);
        assert!(owner_lock_is_free(), "free once the last holder is gone");
        assert!(owner_lock_released());

        // A slow holder (a blocking call that outlived the worker's runtime)
        // that lets go within the wait: the database closes at the quit.
        engine(|| {
            let slow = database_handle().unwrap();
            let holder = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                drop(slow);
            });
            let quit = std::time::Instant::now();
            shutdown_for_quit();
            assert!(
                quit.elapsed() < RELEASE_WAIT + std::time::Duration::from_secs(1),
                "{:?}",
                quit.elapsed()
            );
            assert!(owner_lock_released(), "closed once the slow holder let go");
            assert!(owner_lock_is_free());
            holder.join().unwrap();
        });
        // A quit on another thread: `a_quit_on_another_thread_stops_the_worker`.
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// Two engines one after the other on ONE thread (a host that reuses its
    /// engine thread): the first's quit stops its worker and closes the
    /// database, and the second's launch on the same thread starts a worker of
    /// its own, which sends what the second queues.
    #[test]
    fn two_engines_on_one_thread_each_send_what_they_queued() {
        run_engine_child("two_engines_on_one_thread_each_send_what_they_queued");
        let Some(scratch) = engine_child() else {
            return;
        };
        engine(|| {
            for month in [5u8, 6] {
                if month == 6 {
                    open_database_at_launch();
                }
                assert!(
                    submission_worker_runs_for_this_engine(),
                    "month {month}: this engine's worker"
                );
                queue_1601c(month);
                wait_until(&format!("the 1601C for month {month} was sent"), || {
                    stored_1601c(month).status == bir_core::forms::FilingStatus::Submitted
                });
                shutdown_for_quit();
                assert!(
                    !submission_worker_started(),
                    "month {month}: the worker went"
                );
                assert!(owner_lock_released(), "month {month}: closed");
                assert!(owner_lock_is_free(), "month {month}: the lock is free");
            }
        });
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    /// "Try again" on the locked card: an engine whose launch found the
    /// database held by the engine before it is refused until that one ends;
    /// then the next call opens it, and finishes the launch with a worker of
    /// its own.
    #[test]
    fn a_launch_that_found_the_database_held_finishes_on_the_next_open() {
        run_engine_child("a_launch_that_found_the_database_held_finishes_on_the_next_open");
        let Some(scratch) = engine_child() else {
            return;
        };
        let (first_opened, wait_for_open) = std::sync::mpsc::channel::<()>();
        let (first_ended, wait_for_first) = std::sync::mpsc::channel::<()>();
        let (second_refused, first_may_end) = std::sync::mpsc::channel::<()>();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                engine(move || {
                    assert!(submission_worker_runs_for_this_engine());
                    first_opened.send(()).unwrap();
                    first_may_end.recv().unwrap();
                });
                first_ended.send(()).unwrap();
            });
            scope.spawn(move || {
                wait_for_open.recv().unwrap();
                engine(move || {
                    let refused = profile_names_or_error();
                    assert!(refused.contains("already open"), "{refused}");
                    assert!(
                        !submission_worker_runs_for_this_engine(),
                        "no database, no worker"
                    );
                    second_refused.send(()).unwrap();
                    wait_for_first.recv().unwrap();
                    // Try again.
                    assert_eq!(profile_names_or_error(), "[]");
                    assert!(
                        submission_worker_runs_for_this_engine(),
                        "the launch finished: this engine's worker runs"
                    );
                });
            });
        });
        assert!(!submission_worker_started());
        assert!(owner_lock_is_free());
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
    }

    fn profile_names_or_error() -> String {
        match with_database(|db| db.list_profiles().map_err(db_error)) {
            Ok(profiles) => format!(
                "{:?}",
                profiles.iter().map(|p| &p.full_name).collect::<Vec<_>>()
            ),
            Err(error) => error.message().to_owned(),
        }
    }

    // ---------- export_profile ----------

    #[tokio::test]
    async fn downloaded_profile_export_uses_the_save_panels_result_shape() {
        let result = downloaded_export(Box::pin(async { Ok(None) }))
            .await
            .unwrap();
        assert_eq!(to_json(&result), serde_json::json!({ "kind": "cancelled" }));
        let result = downloaded_export(Box::pin(async { Ok(Some("Files: profile.zip".into())) }))
            .await
            .unwrap();
        assert_eq!(
            to_json(&result),
            serde_json::json!({ "kind": "exported", "path": "Files: profile.zip" })
        );
        let result = downloaded_export(Box::pin(async {
            Err("The export sheet could not open.".into())
        }))
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "The export sheet could not open."
        );
    }

    /// `export_profile_to` writes `TIN`'s archive under the bir-core gate:
    /// a PIN'd taxpayer refuses — no PIN, wrong PIN — before the file
    /// exists, and the right PIN writes it.
    #[test]
    fn export_profile_to_needs_a_pinned_taxpayers_pin() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        let scratch = ScratchDir::new("export");
        let archive = scratch.path().join("profile.zip");

        let refusal = export_profile_to(TIN, None, &archive).unwrap_err();
        assert_eq!(
            refusal.message(),
            "This profile's PIN is required to export it."
        );
        assert!(!archive.exists(), "a refused export left a file");

        let refusal = export_profile_to(TIN, Some("1234"), &archive).unwrap_err();
        assert_eq!(refusal.message(), "Wrong PIN.");
        assert!(!archive.exists(), "a refused export left a file");

        let answer = to_json(&export_profile_to(TIN, Some("0817"), &archive).unwrap());
        assert_eq!(
            answer,
            serde_json::json!({ "kind": "exported", "path": archive.display().to_string() })
        );
        assert!(archive.exists());
    }

    /// A taxpayer without a PIN exports ungated — `None` or a stray PIN —
    /// and an authenticator alone is not a PIN.
    #[test]
    fn export_profile_to_exports_ungated_without_a_pin() {
        setup();
        let scratch = ScratchDir::new("export-open");
        let archive = scratch.path().join("profile.zip");
        assert_eq!(
            to_json(&export_profile_to(TIN, None, &archive).unwrap())["kind"],
            "exported"
        );
        assert!(archive.exists());

        set_taxpayer_totp(&args(vec![s(ROOT), s(SECRET), s(&current_code(SECRET))])).unwrap();
        let archive = scratch.path().join("totp-only.zip");
        assert_eq!(
            to_json(&export_profile_to(TIN, None, &archive).unwrap())["kind"],
            "exported"
        );
    }

    /// The page cannot choose the file. The registered verb refuses a call
    /// past `(tin, pin)` — a path included — and a non-string `pin` before
    /// a dialog opens; `export_profile_to` is no verb at all.
    #[test]
    fn export_profile_refuses_anything_past_the_pin() {
        setup();
        let module = module();
        assert!(module.has("export_profile"));
        assert!(!module.has("export_profile_to"), "not a verb");
        for call in [
            args(vec![s(TIN), s("0817"), s("/tmp/chosen.zip")]),
            args(vec![
                s(TIN),
                json(serde_json::json!({"path": "/tmp/chosen.zip"})),
            ]),
        ] {
            match module.begin("export_profile", &call) {
                Ok(_) => panic!("export_profile answered a call with a path"),
                Err(refusal) => assert_eq!(
                    refusal.message(),
                    "export_profile takes no path; the host opens the save dialog"
                ),
            }
        }
    }

    /// Past the argument checks, both verbs hand the panel to GPUI, never to
    /// a blocking dialog: with no running app there is nothing to open it on,
    /// so the call refuses at once instead of spinning a modal run loop, and
    /// nothing is written or imported.
    #[test]
    fn file_dialog_verbs_need_a_running_app() {
        setup();
        let module = module();
        for (verb, call) in [
            ("export_profile", args(vec![s(TIN)])),
            ("import_archive", args(vec![])),
        ] {
            assert!(module.is_async(verb), "{verb} answers a promise");
            match module.begin(verb, &call) {
                Ok(_) => panic!("{verb} started without an app"),
                Err(refusal) => assert_eq!(
                    refusal.message(),
                    "no app is running to open the file dialog"
                ),
            }
        }
    }

    /// A PIN'd taxpayer's verb refusals happen before any dialog and share
    /// `verify_taxpayer_pin`'s limiter: no PIN is the required refusal, a
    /// wrong PIN counts, six in a row lock the account — the right PIN
    /// waits — and a missing PIN is not a try.
    #[test]
    fn export_profile_wrong_pins_share_the_taxpayer_lockout() {
        setup();
        set_taxpayer_pin(&args(vec![s(ROOT), s("0817")])).unwrap();
        // `map(drop)`: the future is not `Debug`; only the refusal matters.
        let attempt = |pin: &str| export_profile(&args(vec![s(TIN), s(pin)])).map(drop);
        let none = || export_profile(&args(vec![s(TIN)])).map(drop);

        assert_eq!(
            none().unwrap_err().message(),
            "This profile's PIN is required to export it."
        );
        for _ in 0..5 {
            assert_eq!(attempt("1111").unwrap_err().message(), "Wrong PIN.");
        }
        // Not a try: the required refusal still answers, not the lockout.
        assert_eq!(
            none().unwrap_err().message(),
            "This profile's PIN is required to export it."
        );
        let sixth = attempt("1111").unwrap_err();
        assert!(sixth.message().contains("Too many wrong tries"), "{sixth}");
        assert!(
            attempt("0817")
                .unwrap_err()
                .message()
                .contains("Too many wrong tries"),
            "the right PIN waits too"
        );
    }
}

#[cfg(test)]
mod reader_tests;
