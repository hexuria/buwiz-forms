//! The owner PIN's host verbs (accounts-and-lock PLAN §B.1, §B.2; pull request B1).
//!
//! `owner_lock_state`, `set_owner_pin`, `verify_owner_pin` and `lock_now`, over the store in
//! `bir_core::owner_lock` for the room open on this engine. No page calls them yet: B3 builds the lock page.
//!
//! - The PIN hash runs on one worker thread of its own (`owner-pin`), as `mac_login`'s prompt does, so the engine
//!   never waits on it: the two verbs that hash are async. The worker takes one job at a time from a queue, so
//!   calls sent together are checked one after another (I1) and only one 19 MiB hash is in memory at once.
//! - Hash, commit, answer (L5): the result of the hash is committed (failures + 1 or the reset) before the page is
//!   answered (`bir_core::owner_lock::guarded`).
//! - `lock_now` is a stub in B1: it records the lock (this process forgets the room's unlock), and B2 makes the
//!   host enforce it (`ROOM_UNLOCKED` in `ensure_unlocked`). A room with no owner PIN has no lock: `lock_now` does
//!   nothing there (N5). A lock that comes in while a set or a check is hashing wins over it (N7).
//! - A lost `device.db` (I2) reads as `store_lost`, never as "no PIN": the room is not unlocked, and no PIN may be
//!   set until the emailed reset (B3).
//! - Typed PINs are wiped from memory once used (`Zeroizing`), and Argon2's working memory too (N5).
//! - These verbs read `device.db` only. They check the account gate as every verb does today; B2 moves them to its
//!   `ensure_signed_in`, so they answer while the room is locked.

use std::sync::Mutex;

use bir_core::owner_lock::{self, AttemptKey, OwnerPinError};
use bir_core::pin::Zeroizing;
use bir_core::rooms::RoomId;
use gpui_shell::host_modules::HostFuture;
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

/// Which room this process's own owner check unlocked, and how many times `lock_now` locked.
struct Unlock {
    /// The room a match of `verify_owner_pin`, or the owner setting the PIN, unlocked, until `lock_now`.
    room: Option<RoomId>,
    /// Counts every `lock_now` that locked: a set or check that began before one never unlocks (N7).
    locks: u64,
}

/// A process static, so a check finished on the hash thread sets it. B1 records it only; B2 enforces it, and
/// clears it at room close and at launch.
static UNLOCKED: Mutex<Unlock> = Mutex::new(Unlock {
    room: None,
    locks: 0,
});

fn unlock_state() -> std::sync::MutexGuard<'static, Unlock> {
    UNLOCKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn is_unlocked(room: &RoomId) -> bool {
    unlock_state().room.as_ref() == Some(room)
}

/// The number of locks so far, taken when a set or check is asked for.
fn locks_so_far() -> u64 {
    unlock_state().locks
}

/// Unlock `room`, unless a lock came in since `locks_then` was taken (N7: the lock wins).
fn unlock_unless_locked_since(room: RoomId, locks_then: u64) {
    let mut state = unlock_state();
    if state.locks == locks_then {
        state.room = Some(room);
    }
}

fn host_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

const NO_ROOM: &str = "No account is open on this device yet.";

/// The room open on this engine.
fn open_room() -> Result<RoomId, HostError> {
    bir_core::db::open_room_id().ok_or_else(|| HostError::new(NO_ROOM))
}

/// The typed PIN, wiped from memory when dropped (N5).
fn pin_text(arguments: &HostArguments) -> Result<Zeroizing<String>, HostError> {
    let typed = arguments.string(0)?;
    if owner_lock::is_pin(typed) {
        Ok(Zeroizing::new(typed.to_owned()))
    } else {
        Err(HostError::new("A PIN is 4 digits."))
    }
}

/// What the lock page reads: whether a room is open, whether its lock store was lost, whether it has an owner PIN
/// (and since when), whether this process unlocked it, and the owner's count and wait.
fn owner_lock_state() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let Some(room) = bir_core::db::open_room_id() else {
        return Ok(HostObject::new()
            .field("room_open", false)
            .field("store_lost", false)
            .field("has_pin", false)
            .field("set_at", HostValue::Null)
            .field("unlocked", false)
            .field("failures", 0.0)
            .field("wait_secs", 0.0)
            .into());
    };
    let store = bir_core::rooms::device_store_here();
    let lost = owner_lock::store_lost(&store, &room).map_err(host_error)?;
    let lock = if lost {
        None
    } else {
        owner_lock::owner_lock(&store, &room).map_err(host_error)?
    };
    let has_pin = lock.as_ref().is_some_and(|lock| lock.pin_phc.is_some());
    let attempts = store
        .with(false, |store| store.attempts(&AttemptKey::owner(&room)))
        .map_err(host_error)?
        .unwrap_or_default();
    let wait = attempts.wait_left_secs(&bir_core::boot_clock::read());
    Ok(HostObject::new()
        .field("room_open", true)
        .field("store_lost", lost)
        .field("has_pin", has_pin)
        .field(
            "set_at",
            lock.and_then(|lock| lock.set_at)
                .map_or(HostValue::Null, HostValue::from),
        )
        .field("unlocked", !lost && (!has_pin || is_unlocked(&room)))
        .field("failures", attempts.failures as f64)
        .field("wait_secs", wait as f64)
        .into())
}

#[cfg(test)]
mod hold {
    //! Tests only: hold the hash thread before it hashes, and record the thread each hash ran on.
    use std::sync::{Condvar, Mutex};

    pub static HELD: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
    pub static HASHED_ON: Mutex<Vec<std::thread::ThreadId>> = Mutex::new(Vec::new());

    pub fn before_the_hash() {
        let (held, released) = &HELD;
        let mut held = held.lock().unwrap();
        while *held {
            held = released.wait(held).unwrap();
        }
        HASHED_ON.lock().unwrap().push(std::thread::current().id());
    }
}

type Job = Box<dyn FnOnce() + Send>;

/// The one `owner-pin` worker's queue, started on first use. A job that panics is caught, so the worker lives on;
/// a worker that could not start leaves a queue nobody reads, and every call answers an error.
fn worker() -> std::sync::mpsc::Sender<Job> {
    static QUEUE: std::sync::OnceLock<Mutex<std::sync::mpsc::Sender<Job>>> =
        std::sync::OnceLock::new();
    QUEUE
        .get_or_init(|| {
            let (queue, jobs) = std::sync::mpsc::channel::<Job>();
            let started = std::thread::Builder::new()
                .name("owner-pin".into())
                .spawn(move || {
                    for job in jobs {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                    }
                });
            if let Err(error) = started {
                tracing::error!("the owner PIN worker did not start: {error}");
            }
            Mutex::new(queue)
        })
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Queue `work` on the `owner-pin` worker and answer its result to the page when it ends.
fn off_the_engine(
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<HostFuture, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    worker()
        .send(Box::new(move || {
            #[cfg(test)]
            hold::before_the_hash();
            let _ = sender.send(work());
        }))
        .map_err(|_| HostError::new("the PIN check could not start"))?;
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("the PIN check ended without an answer")))
    }))
}

/// Set the owner PIN of the open room: a room with no PIN yet, or one this process unlocked with its PIN; never
/// while its lock store is lost (I2). The room counts as unlocked afterwards (the owner just chose it). A lock that
/// came in after the set was asked for wins: no PIN is stored, and the answer says the device was locked (N7, F2).
fn set_owner_pin(arguments: &HostArguments) -> Result<HostFuture, HostError> {
    crate::account_gate::ensure_unlocked()?;
    let typed = pin_text(arguments)?;
    let room = open_room()?;
    let store = bir_core::rooms::device_store_here();
    if owner_lock::store_lost(&store, &room).map_err(host_error)? {
        return Err(host_error(OwnerPinError::StoreLost));
    }
    let has_pin = owner_lock::owner_lock(&store, &room)
        .map_err(host_error)?
        .is_some_and(|lock| lock.pin_phc.is_some());
    if has_pin && !is_unlocked(&room) {
        return Err(HostError::new("Unlock with your current PIN first."));
    }
    let locks_then = locks_so_far();
    off_the_engine(move || {
        // F2: the lock counter is read again once the hash is made, and held across the write and the unlock, so a
        // lock_now that came after this set was queued means no PIN is stored.
        owner_lock::set_owner_pin_with(&store, &room, &typed, |write| {
            let mut state = unlock_state();
            if state.locks != locks_then {
                return Err(OwnerPinError::LockedMeanwhile);
            }
            write()?;
            state.room = Some(room.clone());
            Ok(())
        })
        .map_err(host_error)?;
        Ok(HostValue::Null)
    })
}

/// Check the owner PIN of the open room under the persisted limit. True on a match (the room counts as unlocked
/// from then on, whatever thread the check ended on, unless a lock came in while it hashed), false on a miss; a
/// wait running, or started by this miss, is an error that says how long.
fn verify_owner_pin(arguments: &HostArguments) -> Result<HostFuture, HostError> {
    crate::account_gate::ensure_unlocked()?;
    let typed = pin_text(arguments)?;
    let room = open_room()?;
    let store = bir_core::rooms::device_store_here();
    let locks_then = locks_so_far();
    off_the_engine(move || {
        let now = bir_core::boot_clock::read();
        let attempt =
            owner_lock::verify_owner_pin(&store, &room, &typed, &now).map_err(host_error)?;
        let matched = crate::auth_limiter::answer(attempt)?;
        if matched {
            unlock_unless_locked_since(room, locks_then);
        }
        Ok(HostValue::Bool(matched))
    })
}

/// Lock the open room now: true when it has a lock (its owner PIN, or a lost store, which never reads as
/// unlocked) and this process forgot its unlock; false, and nothing done, in a room with no lock (N5) or with no
/// room open. B2 makes the host enforce it.
fn lock_now() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let Some(room) = bir_core::db::open_room_id() else {
        return Ok(HostValue::Bool(false));
    };
    let store = bir_core::rooms::device_store_here();
    let locked = owner_lock::store_lost(&store, &room).map_err(host_error)?
        || owner_lock::owner_lock(&store, &room)
            .map_err(host_error)?
            .is_some_and(|lock| lock.pin_phc.is_some());
    if !locked {
        return Ok(HostValue::Bool(false));
    }
    let mut state = unlock_state();
    state.locks += 1;
    if state.room.as_ref() == Some(&room) {
        state.room = None;
    }
    Ok(HostValue::Bool(true))
}

pub fn register(module: HostModule) -> HostModule {
    module
        .function("owner_lock_state", |_| owner_lock_state())
        .async_function("set_owner_pin", set_owner_pin)
        .async_function("verify_owner_pin", verify_owner_pin)
        .function("lock_now", |_| lock_now())
}

pub const DECLARATIONS: &str = r#"
export interface OwnerLockState {
    /** An account's room is open on this device. */
    room_open: boolean;
    /** This device's lock store was lost (device.db missing or made again): never unlocked, and no PIN may be set until the emailed reset. */
    store_lost: boolean;
    /** The room has an owner PIN. */
    has_pin: boolean;
    /** When the owner PIN was set (RFC 3339), or null. */
    set_at: string | null;
    /** This app's own check unlocked the room since its last lock, or the room has no PIN (nothing to unlock). */
    unlocked: boolean;
    /** Wrong owner PINs in a row. */
    failures: number;
    /** Seconds left of the owner's current wait; 0 when none. */
    wait_secs: number;
}
/** The owner lock of the open room; reads the device store only. */
export function owner_lock_state(): OwnerLockState;
/** Set the owner PIN (exactly 4 digits) of the open room: one with no PIN yet, or one unlocked with its current PIN; refused while the store is lost. Hashed off the engine thread. */
export function set_owner_pin(pin: string): Promise<null>;
/** Check the owner PIN under the persisted limit, hashed off the engine thread: true on a match, false on a miss; a running wait (or the miss that starts one) is an error that says how long. */
export function verify_owner_pin(pin: string): Promise<boolean>;
/** Lock the open room now. False, and nothing done, in a room with no owner PIN. */
export function lock_now(): boolean;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::rooms::{DeviceStore, Layout, RoomRecord, room_id_for};

    /// A device folder in a temporary directory with one room open on this thread, and its `rooms` row.
    struct Room {
        _folder: tempfile::TempDir,
        id: RoomId,
        _turn: std::sync::MutexGuard<'static, ()>,
    }

    impl Room {
        fn open(user: &str) -> Self {
            let turn = crate::open_room::room_test_turn();
            let folder = tempfile::tempdir().unwrap();
            bir_core::rooms::forget_ephemeral_device_store();
            // Single-room mode names the device folder; the room is open beside it.
            bir_core::db::set_database_path_override(Some(folder.path().join("bir_data.db")));
            let layout = Layout::at(folder.path());
            let id = room_id_for("https://api.buwiz.com", user);
            // device.db first, as on a new install: with rooms/ on disk and no device.db it would be "lost".
            DeviceStore::open(&layout)
                .unwrap()
                .add_room(&RoomRecord {
                    room_id: id.clone(),
                    origin: "https://api.buwiz.com".into(),
                    user_id: user.into(),
                    email: None,
                    created_at: "2026-11-27T00:00:00+00:00".into(),
                    from_legacy: false,
                })
                .unwrap();
            layout.create_room_dir(&id).unwrap();
            bir_core::db::set_open_room_database(Some(layout.room_database(&id)));
            Self {
                _folder: folder,
                id,
                _turn: turn,
            }
        }
    }

    impl Drop for Room {
        fn drop(&mut self) {
            bir_core::db::set_open_room_database(None);
            bir_core::db::set_database_path_override(None);
            let mut state = unlock_state();
            if state.room.as_ref() == Some(&self.id) {
                state.room = None;
            }
        }
    }

    fn call(name: &str, values: Vec<HostValue>) -> HostResult {
        let module = register(HostModule::new("bir"));
        let arguments = HostArguments::new(values);
        if name == "owner_lock_state" || name == "lock_now" {
            return module.call(name, &arguments);
        }
        futures::executor::block_on(module.begin(name, &arguments)?)
    }

    fn state() -> serde_json::Value {
        crate::bir_module::to_json(&call("owner_lock_state", vec![]).unwrap())
    }

    fn pin(value: &str) -> Vec<HostValue> {
        vec![HostValue::Str(value.into())]
    }

    #[test]
    fn the_owner_verbs_set_verify_and_lock() {
        let room = Room::open("user-verbs");
        let before = state();
        assert_eq!(before["room_open"], true);
        assert_eq!(before["has_pin"], false);
        assert_eq!(before["unlocked"], true, "no PIN, nothing to unlock (N5)");
        assert_eq!(
            call("lock_now", vec![]).unwrap(),
            HostValue::Bool(false),
            "lock_now does nothing in a room with no lock"
        );
        assert!(call("set_owner_pin", pin("12345")).is_err());

        call("set_owner_pin", pin("0817")).unwrap();
        let set = state();
        assert_eq!(set["has_pin"], true);
        assert!(set["set_at"].is_string());
        assert_eq!(set["unlocked"], true);

        assert_eq!(call("lock_now", vec![]).unwrap(), HostValue::Bool(true));
        assert_eq!(state()["unlocked"], false, "the lock is recorded");
        assert_eq!(
            call("set_owner_pin", pin("2468")).unwrap_err().message(),
            "Unlock with your current PIN first."
        );
        assert_eq!(
            call("verify_owner_pin", pin("0000")).unwrap(),
            HostValue::Bool(false)
        );
        assert_eq!(state()["failures"], 1.0);
        assert_eq!(
            call("verify_owner_pin", pin("0817")).unwrap(),
            HostValue::Bool(true)
        );
        let open = state();
        assert_eq!(open["unlocked"], true);
        assert_eq!(open["failures"], 0.0);

        // Six wrong in a row: the sixth says how long, and even the right PIN waits.
        for _ in 0..5 {
            assert_eq!(
                call("verify_owner_pin", pin("0000")).unwrap(),
                HostValue::Bool(false)
            );
        }
        let sixth = call("verify_owner_pin", pin("0000")).unwrap_err();
        assert!(
            sixth.message().contains("Try again in 1:00."),
            "{}",
            sixth.message()
        );
        assert!(call("verify_owner_pin", pin("0817")).is_err());
        let waiting = state();
        assert_eq!(waiting["failures"], 6.0);
        assert!(waiting["wait_secs"].as_f64().unwrap() > 50.0);

        // The PIN in device.db is Argon2id.
        let store = bir_core::rooms::device_store_here();
        let phc = owner_lock::owner_lock(&store, &room.id)
            .unwrap()
            .unwrap()
            .pin_phc
            .unwrap();
        assert!(phc.starts_with("$argon2id$"), "{phc}");
    }

    #[test]
    fn with_no_room_open_the_verbs_say_so() {
        let _turn = crate::open_room::room_test_turn();
        bir_core::db::set_open_room_database(None);
        bir_core::rooms::use_ephemeral_device_store();
        assert_eq!(state()["room_open"], false);
        assert_eq!(call("lock_now", vec![]).unwrap(), HostValue::Bool(false));
        assert_eq!(
            call("verify_owner_pin", pin("0817")).unwrap_err().message(),
            NO_ROOM
        );
        assert_eq!(
            call("set_owner_pin", pin("0817")).unwrap_err().message(),
            NO_ROOM
        );
        bir_core::rooms::forget_ephemeral_device_store();
    }

    /// The hash runs on its own thread: with that thread held before it hashes, the verb's synchronous half has
    /// already returned to the engine and its answer is still pending; released, it answers, and every hash ran
    /// on a thread that is not the engine's.
    #[test]
    fn the_hash_runs_off_the_engine_thread() {
        let _room = Room::open("user-thread");
        let module = register(HostModule::new("bir"));
        let engine = std::thread::current().id();

        for (verb, typed, answer) in [
            ("set_owner_pin", "0817", HostValue::Null),
            ("verify_owner_pin", "0817", HostValue::Bool(true)),
        ] {
            let release = Hold::on();
            let started = std::time::Instant::now();
            let mut future = module
                .begin(verb, &HostArguments::new(pin(typed)))
                .expect("the synchronous half");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(1),
                "{verb}: the engine did not wait"
            );
            let waker = futures::task::noop_waker();
            let mut context = std::task::Context::from_waker(&waker);
            assert!(
                future.as_mut().poll(&mut context).is_pending(),
                "{verb}: no answer while the hash thread is held"
            );
            drop(release);
            assert_eq!(
                futures::executor::block_on(future).unwrap(),
                answer,
                "{verb}"
            );
        }
        let hashed_on = hold::HASHED_ON.lock().unwrap();
        assert!(!hashed_on.is_empty());
        assert!(hashed_on.iter().all(|thread| *thread != engine));
    }

    /// Holds the worker before its next hash until dropped.
    struct Hold;

    impl Hold {
        fn on() -> Self {
            *hold::HELD.0.lock().unwrap() = true;
            Hold
        }
    }

    impl Drop for Hold {
        fn drop(&mut self) {
            let (held, released) = &hold::HELD;
            *held.lock().unwrap() = false;
            released.notify_all();
        }
    }

    fn owner_failures(room: &Room) -> i64 {
        bir_core::rooms::device_store_here()
            .with(false, |store| store.attempts(&AttemptKey::owner(&room.id)))
            .unwrap()
            .unwrap()
            .failures
    }

    /// I1 at the verbs: twenty `verify_owner_pin` calls sent together from 5 wrong in a row. The worker checks them
    /// one after another: the first is the 6th miss and starts the wait, the rest answer it. No yes/no answer comes
    /// back, and the count stops at 6.
    #[test]
    fn twenty_verifies_sent_together_stop_at_the_wait() {
        let room = Room::open("user-burst");
        call("set_owner_pin", pin("0817")).unwrap();
        for _ in 0..5 {
            assert_eq!(
                call("verify_owner_pin", pin("0000")).unwrap(),
                HostValue::Bool(false)
            );
        }
        let module = register(HostModule::new("bir"));
        let futures: Vec<_> = {
            let _held = Hold::on();
            (0..20)
                .map(|_| {
                    module
                        .begin("verify_owner_pin", &HostArguments::new(pin("0000")))
                        .unwrap()
                })
                .collect()
        };
        let answers: Vec<HostResult> = futures
            .into_iter()
            .map(futures::executor::block_on)
            .collect();
        assert!(answers.iter().all(|answer| answer.is_err()), "{answers:?}");
        assert!(answers.iter().all(|answer| {
            answer
                .as_ref()
                .unwrap_err()
                .message()
                .contains("Try again in")
        }));
        assert_eq!(owner_failures(&room), 6);
        assert_eq!(state()["wait_secs"].as_f64().unwrap() > 50.0, true);
    }

    /// N7 and F2: a lock that comes in after a set or a check was queued wins. The set stores no PIN and says the
    /// device was locked; the check still answers, but the room stays locked.
    #[test]
    fn a_lock_during_the_hash_wins() {
        let _room = Room::open("user-lock-race");
        call("set_owner_pin", pin("0817")).unwrap();
        assert_eq!(state()["unlocked"], true);
        let set_at = state()["set_at"].clone();
        let module = register(HostModule::new("bir"));

        let held = Hold::on();
        let set = module
            .begin("set_owner_pin", &HostArguments::new(pin("2468")))
            .unwrap();
        assert_eq!(call("lock_now", vec![]).unwrap(), HostValue::Bool(true));
        drop(held);
        let refused = futures::executor::block_on(set).unwrap_err();
        assert!(
            refused.message().contains("was locked"),
            "{}",
            refused.message()
        );
        let after_set = state();
        assert_eq!(after_set["unlocked"], false, "the lock won over the set");
        assert_eq!(after_set["set_at"], set_at, "no PIN was stored");

        let held = Hold::on();
        let check = module
            .begin("verify_owner_pin", &HostArguments::new(pin("2468")))
            .unwrap();
        drop(held);
        assert_eq!(
            futures::executor::block_on(check).unwrap(),
            HostValue::Bool(false),
            "the PIN asked for after the lock was never stored"
        );

        let held = Hold::on();
        let check = module
            .begin("verify_owner_pin", &HostArguments::new(pin("0817")))
            .unwrap();
        assert_eq!(call("lock_now", vec![]).unwrap(), HostValue::Bool(true));
        drop(held);
        assert_eq!(
            futures::executor::block_on(check).unwrap(),
            HostValue::Bool(true)
        );
        assert_eq!(state()["unlocked"], false, "the lock won over the check");
        assert_eq!(
            call("verify_owner_pin", pin("0817")).unwrap(),
            HostValue::Bool(true)
        );
        assert_eq!(state()["unlocked"], true);
    }

    /// I2 under a running app (Unix, where an open file can be deleted): device.db and its -wal/-shm go WHILE
    /// the host holds its connection. The state reads `store_lost`, never an answer from the unlinked file.
    #[cfg(unix)]
    #[test]
    fn a_device_db_deleted_under_the_held_connection_reads_lost() {
        let room = Room::open("user-lost-held");
        call("set_owner_pin", pin("0817")).unwrap();
        let layout = Layout::at(room._folder.path());
        assert!(
            bir_core::rooms::device_store_is_held(&layout),
            "the PIN's write left its connection held"
        );
        std::fs::remove_file(layout.device_db()).unwrap();
        for file in ["device.db-wal", "device.db-shm"] {
            match std::fs::remove_file(room._folder.path().join(file)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("{file} was not removed: {error}"),
            }
        }
        let lost = state();
        assert_eq!(lost["store_lost"], true);
        assert_eq!(lost["unlocked"], false);
        assert!(
            call("set_owner_pin", pin("2468"))
                .unwrap_err()
                .message()
                .contains("lock was lost")
        );
        assert!(
            !room._folder.path().join("device.db").exists(),
            "nothing made it again silently"
        );
    }

    /// I2: with device.db gone while the room's folder is on disk, the state reads `store_lost`, never "no PIN,
    /// unlocked"; no PIN may be set, a check is refused, and `lock_now` locks.
    ///
    /// The loss is made as it can happen on every system: the app's held connection is closed first (a stop and
    /// a relaunch), then the files go. On Windows an open SQLite file cannot be deleted, so under a running app
    /// device.db never goes missing; the case that matters there is a loss between runs, which this is. Each
    /// delete is checked, so a delete that silently fails can no longer pass for a loss.
    #[test]
    fn a_lost_device_db_reads_lost_and_refuses_a_set() {
        let room = Room::open("user-lost");
        call("set_owner_pin", pin("0817")).unwrap();
        bir_core::rooms::release_held_device_stores();
        for file in ["device.db", "device.db-wal", "device.db-shm"] {
            match std::fs::remove_file(room._folder.path().join(file)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("{file} was not removed: {error}"),
            }
        }
        assert!(
            !room._folder.path().join("device.db").exists(),
            "the store is gone"
        );
        let lost = state();
        assert_eq!(lost["store_lost"], true);
        assert_eq!(lost["has_pin"], false);
        assert_eq!(lost["unlocked"], false);
        assert!(
            call("set_owner_pin", pin("2468"))
                .unwrap_err()
                .message()
                .contains("lock was lost")
        );
        assert!(call("verify_owner_pin", pin("0817")).is_err());
        assert_eq!(call("lock_now", vec![]).unwrap(), HostValue::Bool(true));
        assert!(
            !room._folder.path().join("device.db").exists(),
            "nothing made it again silently"
        );
    }
}
