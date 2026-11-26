//! The owner PIN's host verbs (accounts-and-lock PLAN §B.1, §B.2; pull request B1).
//!
//! `owner_lock_state`, `set_owner_pin`, `verify_owner_pin` and `lock_now`, over the store in
//! `bir_core::owner_lock` for the room open on this engine. No page calls them yet: B3 builds the lock page.
//!
//! - The PIN hash runs on a thread of its own (`owner-pin`), as `mac_login`'s prompt does, so the engine never
//!   waits on it: the two verbs that hash are async.
//! - Hash, commit, answer (L5): the result of the hash is committed (failures + 1 or the reset) before the page is
//!   answered (`bir_core::owner_lock::guarded`).
//! - `lock_now` is a stub in B1: it records the lock (this process forgets the room's unlock), and B2 makes the
//!   host enforce it (`ROOM_UNLOCKED` in `ensure_unlocked`). A room with no owner PIN has no lock: `lock_now` does
//!   nothing there (N5).
//! - These verbs read `device.db` only. They check the account gate as every verb does today; B2 moves them to its
//!   `ensure_signed_in`, so they answer while the room is locked.

use std::sync::Mutex;

use bir_core::owner_lock::{self, AttemptKey, OwnerPinError};
use bir_core::rooms::RoomId;
use gpui_shell::host_modules::HostFuture;
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

/// The room this process's own owner check unlocked (a match of `verify_owner_pin`, or the owner setting the PIN),
/// until `lock_now`. A process static, so a check finished on the hash thread sets it. B1 records it only; B2
/// enforces it, and clears it at room close and at launch.
static UNLOCKED_ROOM: Mutex<Option<RoomId>> = Mutex::new(None);

fn unlocked_room() -> std::sync::MutexGuard<'static, Option<RoomId>> {
    UNLOCKED_ROOM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn is_unlocked(room: &RoomId) -> bool {
    unlocked_room().as_ref() == Some(room)
}

fn host_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

const NO_ROOM: &str = "No account is open on this device yet.";

/// The room open on this engine.
fn open_room() -> Result<RoomId, HostError> {
    bir_core::db::open_room_id().ok_or_else(|| HostError::new(NO_ROOM))
}

fn pin_text(arguments: &HostArguments) -> Result<String, HostError> {
    let typed = arguments.string(0)?;
    if owner_lock::is_pin(typed) {
        Ok(typed.to_owned())
    } else {
        Err(HostError::new("A PIN is 4 digits."))
    }
}

/// What the lock page reads: whether a room is open, whether it has an owner PIN (and since when), whether this
/// process unlocked it, and the owner's count and wait.
fn owner_lock_state() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let Some(room) = bir_core::db::open_room_id() else {
        return Ok(HostObject::new()
            .field("room_open", false)
            .field("has_pin", false)
            .field("set_at", HostValue::Null)
            .field("unlocked", false)
            .field("failures", 0.0)
            .field("wait_secs", 0.0)
            .into());
    };
    let store = bir_core::rooms::device_store_here();
    let lock = owner_lock::owner_lock(&store, &room).map_err(host_error)?;
    let has_pin = lock.as_ref().is_some_and(|lock| lock.pin_phc.is_some());
    let attempts = store
        .with(false, |store| store.attempts(&AttemptKey::owner(&room)))
        .map_err(host_error)?
        .unwrap_or_default();
    let wait = attempts.wait_left_secs(&bir_core::boot_clock::read());
    Ok(HostObject::new()
        .field("room_open", true)
        .field("has_pin", has_pin)
        .field(
            "set_at",
            lock.and_then(|lock| lock.set_at)
                .map_or(HostValue::Null, HostValue::from),
        )
        .field("unlocked", !has_pin || is_unlocked(&room))
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

/// Run `work` on the `owner-pin` thread and answer its result to the page when it ends.
fn off_the_engine(
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<HostFuture, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel::<HostResult>();
    std::thread::Builder::new()
        .name("owner-pin".into())
        .spawn(move || {
            #[cfg(test)]
            hold::before_the_hash();
            let _ = sender.send(work());
        })
        .map_err(host_error)?;
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("the PIN check ended without an answer")))
    }))
}

/// Set the owner PIN of the open room: a room with no PIN yet, or one this process unlocked with its PIN. The
/// room counts as unlocked afterwards (the owner just chose it).
fn set_owner_pin(arguments: &HostArguments) -> Result<HostFuture, HostError> {
    crate::account_gate::ensure_unlocked()?;
    let typed = pin_text(arguments)?;
    let room = open_room()?;
    let store = bir_core::rooms::device_store_here();
    let has_pin = owner_lock::owner_lock(&store, &room)
        .map_err(host_error)?
        .is_some_and(|lock| lock.pin_phc.is_some());
    if has_pin && !is_unlocked(&room) {
        return Err(HostError::new("Unlock with your current PIN first."));
    }
    off_the_engine(move || {
        owner_lock::set_owner_pin(&store, &room, &typed).map_err(host_error)?;
        *unlocked_room() = Some(room);
        Ok(HostValue::Null)
    })
}

/// Check the owner PIN of the open room under the persisted limit. True on a match (the room counts as unlocked
/// from then on, whatever thread the check ended on), false on a miss; a wait running, or started by this miss,
/// is an error that says how long.
fn verify_owner_pin(arguments: &HostArguments) -> Result<HostFuture, HostError> {
    crate::account_gate::ensure_unlocked()?;
    let typed = pin_text(arguments)?;
    let room = open_room()?;
    let store = bir_core::rooms::device_store_here();
    off_the_engine(move || {
        let now = bir_core::boot_clock::read();
        let attempt = match owner_lock::verify_owner_pin(&store, &room, &typed, &now) {
            Ok(attempt) => attempt,
            Err(OwnerPinError::NoPin) => {
                return Err(HostError::new(OwnerPinError::NoPin.to_string()));
            }
            Err(error) => return Err(host_error(error)),
        };
        let matched = crate::auth_limiter::answer(attempt)?;
        if matched {
            *unlocked_room() = Some(room);
        }
        Ok(HostValue::Bool(matched))
    })
}

/// Lock the open room now: true when it has a lock (its owner PIN) and this process forgot its unlock; false, and
/// nothing done, in a room with no lock (N5) or with no room open. B2 makes the host enforce it.
fn lock_now() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    let Some(room) = bir_core::db::open_room_id() else {
        return Ok(HostValue::Bool(false));
    };
    let store = bir_core::rooms::device_store_here();
    let has_pin = owner_lock::owner_lock(&store, &room)
        .map_err(host_error)?
        .is_some_and(|lock| lock.pin_phc.is_some());
    if !has_pin {
        return Ok(HostValue::Bool(false));
    }
    let mut unlocked = unlocked_room();
    if unlocked.as_ref() == Some(&room) {
        *unlocked = None;
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
/** Set the owner PIN (exactly 4 digits) of the open room: one with no PIN yet, or one unlocked with its current PIN. Hashed off the engine thread. */
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

    /// The unlocked room is one slot for the process, as the app has one room open at a time: tests that open a
    /// room take turns.
    static ONE_ROOM_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// A device folder in a temporary directory with one room open on this thread, and its `rooms` row.
    struct Room {
        _folder: tempfile::TempDir,
        id: RoomId,
        _turn: std::sync::MutexGuard<'static, ()>,
    }

    impl Room {
        fn open(user: &str) -> Self {
            let turn = ONE_ROOM_AT_A_TIME
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
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
            let mut unlocked = unlocked_room();
            if unlocked.as_ref() == Some(&self.id) {
                *unlocked = None;
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

        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                let (held, released) = &hold::HELD;
                *held.lock().unwrap() = false;
                released.notify_all();
            }
        }

        for (verb, typed, answer) in [
            ("set_owner_pin", "0817", HostValue::Null),
            ("verify_owner_pin", "0817", HostValue::Bool(true)),
        ] {
            *hold::HELD.0.lock().unwrap() = true;
            let release = Release;
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
}
