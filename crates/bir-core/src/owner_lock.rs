//! The owner PIN store and the persisted retry limit, over `device.db` (accounts-and-lock PLAN §B.1, §B.2).
//!
//! - **The owner PIN** of a room lives in `owner_lock.pin_phc`: an Argon2id PHC string ([`crate::pin`]), never an
//!   old SHA-256 value. [`set_owner_pin`] writes it, [`verify_owner_pin`] checks it under the limit, and
//!   [`adopt_legacy_pin`] is the legacy upgrade: the old credential typed once (L1) becomes the owner PIN, hashed
//!   afresh with its own salt.
//! - **The retry limit** lives in `pin_attempts`, one row per room and subject: the owner, each client (TIN
//!   root) and the legacy lock. It survives a restart. Today's schedule is kept: after every 6th wrong PIN in a
//!   row, a wait of 1 min, 5 min, 15 min, 30 min, 1 h, then 24 h for each later round.
//! - **Hash, commit, answer (L5).** [`DeviceStore::begin_try`] only reads whether a wait is running; then the hash
//!   runs; then [`DeviceStore::settle_try`] commits the result (failures + 1 for a miss, and the wait the miss
//!   that reaches the threshold starts; a reset to 0 for a match); only then is the page answered. A process
//!   killed before the commit gave no answer, so an attacker gains nothing, and a crash never costs a try. A check
//!   that fails (a store or database error, not a wrong PIN) commits nothing, as today.
//! - **One try at a time per subject (I1).** In this process, the whole read, hash and commit of a try holds that
//!   subject's lock ([`guarded`]), so tries sent together are checked one after another and the 6-try round
//!   holds. Across processes, [`DeviceStore::settle_try`] looks at the row again inside its write transaction: a
//!   wait that began since the try was read wins (the miss still counts; a match resets nothing), and the answer
//!   is that wait.
//! - **The clock (L4, X3).** A wait records the wall clock and the boot ([`crate::boot_clock`]) it started in.
//!   Within the same boot (the same boot id) the time since boot decides, so moving the wall clock changes
//!   nothing; after a reboot the wall clock does, and a clock set back never shortens a wait. Every read caps the
//!   time left at `min(wait, 24 h)` and floors it at 0.

use std::time::Duration;

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::boot_clock::ClockReading;
use crate::pin;
use crate::rooms::{DeviceStore, DeviceStoreError, DeviceStoreRef, RoomId};

/// Wrong PINs in a row per wait.
pub const ATTEMPTS_PER_WAIT: i64 = 6;
/// The wait after the 6th, 12th, 18th... wrong PIN in a row, in seconds; the last one repeats from then on.
pub const LOCKOUT_WAITS_SECS: [i64; 6] = [60, 300, 900, 1800, 3600, 86_400];
/// No wait is ever read as longer than this (L4).
pub const MAX_WAIT_SECS: i64 = 86_400;

/// Whose PIN a try is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// The room's owner PIN.
    Owner,
    /// A client: one taxpayer (9-digit TIN root); its PIN and authenticator share the count.
    Client(String),
    /// The lock of the legacy file, before it has a room (A3a-2).
    Legacy,
}

impl Subject {
    /// The `pin_attempts.subject` value.
    pub fn key(&self) -> String {
        match self {
            Self::Owner => "owner".to_owned(),
            Self::Client(tin_root) => format!("client:{tin_root}"),
            Self::Legacy => "legacy".to_owned(),
        }
    }
}

/// One `pin_attempts` row's key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptKey {
    /// The room, or `''` where there is none yet (the legacy lock, and clients of the legacy file).
    pub room_id: String,
    pub subject: Subject,
}

impl AttemptKey {
    pub fn owner(room: &RoomId) -> Self {
        Self {
            room_id: room.as_str().to_owned(),
            subject: Subject::Owner,
        }
    }

    /// A client of `room`, or of the legacy file while no room is open.
    pub fn client(room: Option<&RoomId>, tin_root: &str) -> Self {
        Self {
            room_id: room
                .map(|room| room.as_str().to_owned())
                .unwrap_or_default(),
            subject: Subject::Client(tin_root.to_owned()),
        }
    }

    pub fn legacy() -> Self {
        Self {
            room_id: String::new(),
            subject: Subject::Legacy,
        }
    }
}

/// One `pin_attempts` row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttemptRow {
    /// Wrong PINs in a row.
    pub failures: i64,
    /// The wait the last failure started, in seconds; 0 when none ever did.
    pub wait_secs: i64,
    pub started_unix: i64,
    pub started_boot_secs: i64,
    /// `''` when the boot could not be read: the wait is then timed on the wall clock alone.
    pub started_boot_id: String,
}

impl AttemptRow {
    /// Seconds of the wait left at `now` (L4): on time since boot within the boot it started in, else on the wall
    /// clock; capped at `min(wait_secs, 24 h)`, floored at 0.
    pub fn wait_left_secs(&self, now: &ClockReading) -> i64 {
        if self.wait_secs <= 0 {
            return 0;
        }
        let cap = self.wait_secs.min(MAX_WAIT_SECS);
        let elapsed = match &now.boot {
            Some(boot) if !self.started_boot_id.is_empty() && boot.id == self.started_boot_id => {
                boot.secs - self.started_boot_secs
            }
            _ => now.unix_secs - self.started_unix,
        };
        (cap - elapsed.max(0)).clamp(0, cap)
    }

    pub fn wait_left(&self, now: &ClockReading) -> Option<Duration> {
        let left = self.wait_left_secs(now);
        (left > 0).then(|| Duration::from_secs(left as u64))
    }
}

/// The wait the `failures`-th wrong PIN in a row starts, if it starts one.
pub fn wait_after(failures: i64) -> Option<i64> {
    if failures <= 0 || failures % ATTEMPTS_PER_WAIT != 0 {
        return None;
    }
    let round = (failures / ATTEMPTS_PER_WAIT - 1) as usize;
    Some(LOCKOUT_WAITS_SECS[round.min(LOCKOUT_WAITS_SECS.len() - 1)])
}

/// "Too many wrong tries. Try again in 1:00." (minutes:seconds, or hours and minutes for the long waits).
pub fn lockout_message(wait: Duration) -> String {
    let secs = wait.as_secs().max(1);
    let left = if secs >= 3600 {
        format!("{} h {:02} min", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    };
    format!("Too many wrong tries. Try again in {left}.")
}

/// What [`DeviceStore::begin_try`] found.
#[derive(Debug)]
pub enum Begun {
    /// A wait is running: no check may run, and nothing is counted.
    Waiting(Duration),
    /// No wait: run the check, then commit its result with [`DeviceStore::settle_try`].
    Ready(PendingTry),
}

/// A try whose check may run; nothing about it is written yet.
#[derive(Debug, Clone)]
pub struct PendingTry {
    key: AttemptKey,
    now: ClockReading,
}

/// What [`DeviceStore::settle_try`] committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// The result stands. For a miss: the wait it started, if it started one.
    Done { wait_started: Option<Duration> },
    /// A wait began after the try was read (another process's miss): it wins, and this is the time left. A miss
    /// was still counted; a match reset nothing.
    Waiting(Duration),
}

/// How a check ended, to commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryOutcome {
    /// The PIN matched: the count goes back to 0.
    Matched,
    /// A wrong PIN: the count goes up by one, and the miss that reaches the threshold starts a wait.
    Missed,
}

impl DeviceStore {
    /// The row of `key`; an empty one where there is none.
    pub fn attempts(&self, key: &AttemptKey) -> Result<AttemptRow, DeviceStoreError> {
        Ok(read_attempts(&self.conn, key)?.unwrap_or_default())
    }

    /// Whether a try of `key` may run its check at `now`. Reads only: nothing is written before the check.
    pub fn begin_try(
        &self,
        key: &AttemptKey,
        now: &ClockReading,
    ) -> Result<Begun, DeviceStoreError> {
        let row = read_attempts(&self.conn, key)?.unwrap_or_default();
        Ok(match row.wait_left(now) {
            Some(wait) => Begun::Waiting(wait),
            None => Begun::Ready(PendingTry {
                key: key.clone(),
                now: now.clone(),
            }),
        })
    }

    /// Commit the result of a check, in one write transaction that reads the row as it is now: a match resets the
    /// count; a miss adds one and, on the 6th, 12th... in a row, starts the wait it reaches. A wait that is running
    /// now, though none was when the try was read, wins: the miss is counted all the same, a match resets nothing,
    /// and the answer is [`Settled::Waiting`].
    pub fn settle_try(
        &self,
        pending: &PendingTry,
        outcome: TryOutcome,
    ) -> Result<Settled, DeviceStoreError> {
        let key = &pending.key;
        let now = &pending.now;
        let transaction =
            rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let mut row = read_attempts(&transaction, key)?.unwrap_or_default();
        let running = row.wait_left(now);
        if outcome == TryOutcome::Matched {
            if let Some(wait) = running {
                transaction.commit()?;
                return Ok(Settled::Waiting(wait));
            }
            transaction.execute(
                "DELETE FROM pin_attempts WHERE room_id = ?1 AND subject = ?2",
                params![key.room_id, key.subject.key()],
            )?;
            transaction.commit()?;
            return Ok(Settled::Done { wait_started: None });
        }
        row.failures += 1;
        let started = wait_after(row.failures);
        if let Some(wait) = started {
            row.wait_secs = wait;
            row.started_unix = now.unix_secs;
            match &now.boot {
                Some(boot) => {
                    row.started_boot_secs = boot.secs;
                    row.started_boot_id = boot.id.clone();
                }
                None => {
                    row.started_boot_secs = 0;
                    row.started_boot_id = String::new();
                }
            }
        }
        write_attempts(&transaction, key, &row)?;
        transaction.commit()?;
        Ok(match running {
            Some(_) => Settled::Waiting(row.wait_left(now).unwrap_or(Duration::from_secs(1))),
            None => Settled::Done {
                wait_started: started.and_then(|_| row.wait_left(now)),
            },
        })
    }

    /// The owner lock of `room`, if it has a row.
    pub fn owner_lock(&self, room: &RoomId) -> Result<Option<OwnerLock>, DeviceStoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT pin_phc, set_at, biometrics_on FROM owner_lock WHERE room_id = ?1",
                params![room.as_str()],
                |row| {
                    Ok(OwnerLock {
                        pin_phc: row.get(0)?,
                        set_at: row.get(1)?,
                        biometrics_on: row.get::<_, i64>(2)? != 0,
                    })
                },
            )
            .optional()?)
    }

    /// Store `phc` as `room`'s owner PIN, set now, and reset the owner's count, in one commit.
    fn store_owner_pin_phc(&self, room: &RoomId, phc: &str) -> Result<(), DeviceStoreError> {
        let transaction =
            rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO owner_lock (room_id, pin_phc, set_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(room_id) DO UPDATE SET pin_phc = excluded.pin_phc, set_at = excluded.set_at",
            params![room.as_str(), phc, chrono::Utc::now().to_rfc3339()],
        )?;
        transaction.execute(
            "DELETE FROM pin_attempts WHERE room_id = ?1 AND subject = ?2",
            params![room.as_str(), Subject::Owner.key()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Replace `room`'s owner PIN hash `old` with `new` (a rehash of the same PIN); not when it changed meanwhile.
    fn replace_owner_pin_phc(
        &self,
        room: &RoomId,
        old: &str,
        new: &str,
    ) -> Result<bool, DeviceStoreError> {
        Ok(self.conn.execute(
            "UPDATE owner_lock SET pin_phc = ?3 WHERE room_id = ?1 AND pin_phc = ?2",
            params![room.as_str(), old, new],
        )? == 1)
    }
}

fn read_attempts(
    conn: &rusqlite::Connection,
    key: &AttemptKey,
) -> Result<Option<AttemptRow>, DeviceStoreError> {
    Ok(conn
        .query_row(
            "SELECT failures, wait_secs, started_unix, started_boot_secs, started_boot_id
             FROM pin_attempts WHERE room_id = ?1 AND subject = ?2",
            params![key.room_id, key.subject.key()],
            |row| {
                Ok(AttemptRow {
                    failures: row.get(0)?,
                    wait_secs: row.get(1)?,
                    started_unix: row.get(2)?,
                    started_boot_secs: row.get(3)?,
                    started_boot_id: row.get(4)?,
                })
            },
        )
        .optional()?)
}

fn write_attempts(
    conn: &rusqlite::Connection,
    key: &AttemptKey,
    row: &AttemptRow,
) -> Result<(), DeviceStoreError> {
    conn.execute(
        "INSERT INTO pin_attempts
             (room_id, subject, failures, wait_secs, started_unix, started_boot_secs, started_boot_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(room_id, subject) DO UPDATE SET
             failures = excluded.failures, wait_secs = excluded.wait_secs,
             started_unix = excluded.started_unix, started_boot_secs = excluded.started_boot_secs,
             started_boot_id = excluded.started_boot_id",
        params![
            key.room_id,
            key.subject.key(),
            row.failures,
            row.wait_secs,
            row.started_unix,
            row.started_boot_secs,
            row.started_boot_id,
        ],
    )?;
    Ok(())
}

/// How a guarded check ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    Matched,
    /// A wrong PIN; `wait`: the wait this miss started, if it started one.
    Missed {
        wait: Option<Duration>,
    },
    /// A wait is running; no check ran and nothing was counted.
    Waiting(Duration),
}

/// Why a guarded check gave no answer.
#[derive(Debug)]
pub enum GuardError<E> {
    /// The limit could not be read, or the result could not be committed: no answer.
    Store(DeviceStoreError),
    /// The check itself failed; nothing was committed.
    Check(E),
}

/// The per-subject locks of this process ([`subject_lock`]).
type SubjectLocks =
    std::collections::HashMap<(String, String), std::sync::Arc<std::sync::Mutex<()>>>;
static SUBJECT_LOCKS: std::sync::Mutex<Option<SubjectLocks>> = std::sync::Mutex::new(None);

/// The lock every try of `key` holds from its read to its commit in this process (I1). One small entry per
/// subject ever tried; never removed.
fn subject_lock(key: &AttemptKey) -> std::sync::Arc<std::sync::Mutex<()>> {
    let mut locks = SUBJECT_LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks
        .get_or_insert_with(Default::default)
        .entry(subject_id(key))
        .or_default()
        .clone()
}

fn subject_id(key: &AttemptKey) -> (String, String) {
    (key.room_id.clone(), key.subject.key())
}

thread_local! {
    /// The subjects whose lock this thread holds inside [`guarded`] (F4). The per-subject `Mutex` is not
    /// re-entrant: taking the same subject's lock again on this thread would deadlock.
    static HELD_SUBJECTS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Whether this thread is inside [`guarded`] for `key`.
fn inside_guarded(key: &AttemptKey) -> bool {
    let id = subject_id(key);
    HELD_SUBJECTS.with(|held| held.borrow().contains(&id))
}

/// Marks `key` as held by this thread for as long as it lives (and through an unwind).
struct HeldSubject((String, String));

impl HeldSubject {
    fn enter(key: &AttemptKey) -> Self {
        let id = subject_id(key);
        HELD_SUBJECTS.with(|held| held.borrow_mut().push(id.clone()));
        Self(id)
    }
}

impl Drop for HeldSubject {
    fn drop(&mut self) {
        HELD_SUBJECTS.with(|held| {
            let mut held = held.borrow_mut();
            if let Some(at) = held.iter().rposition(|id| *id == self.0) {
                held.remove(at);
            }
        });
    }
}

/// Run `check` for `key` under the limit, in the order above (L5): refuse during a wait; run the check (the hash);
/// commit its result; only then answer. A result that cannot be committed gives no answer. The whole try holds
/// the subject's lock, so tries of one subject in this process never overlap (I1); a wait another process
/// started meanwhile wins at the commit.
pub fn guarded<E>(
    store: &DeviceStoreRef,
    key: &AttemptKey,
    now: &ClockReading,
    check: impl FnOnce() -> Result<bool, E>,
) -> Result<Attempt, GuardError<E>> {
    let lock = subject_lock(key);
    let _one_at_a_time = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _held = HeldSubject::enter(key);
    let pending = match store
        .with(true, |store| store.begin_try(key, now))
        .map_err(GuardError::Store)?
    {
        Some(Begun::Ready(pending)) => pending,
        Some(Begun::Waiting(wait)) => return Ok(Attempt::Waiting(wait)),
        None => return Err(GuardError::Store(not_found())),
    };
    let matched = check().map_err(GuardError::Check)?;
    let outcome = if matched {
        TryOutcome::Matched
    } else {
        TryOutcome::Missed
    };
    let settled = store
        .with(true, |store| store.settle_try(&pending, outcome))
        .map_err(GuardError::Store)?
        .ok_or_else(|| GuardError::Store(not_found()))?;
    Ok(match settled {
        Settled::Waiting(wait) => Attempt::Waiting(wait),
        Settled::Done { .. } if matched => Attempt::Matched,
        Settled::Done { wait_started } => Attempt::Missed { wait: wait_started },
    })
}

fn not_found() -> DeviceStoreError {
    DeviceStoreError::Io(std::io::ErrorKind::NotFound.into())
}

// ------------------------------------------------------------------------------------------- the owner PIN store

/// One `owner_lock` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerLock {
    /// The Argon2id PHC string; `None` while no PIN is set.
    pub pin_phc: Option<String>,
    pub set_at: Option<String>,
    pub biometrics_on: bool,
}

/// Why the owner PIN store did not answer.
#[derive(Debug)]
pub enum OwnerPinError {
    /// The room has no `rooms` row.
    NoRoom,
    /// The room has no owner PIN.
    NoPin,
    /// Not exactly 4 digits.
    NotAPin,
    /// The device store was lost (I2, N6): no PIN may be set until the emailed reset code (§B.6, B3).
    StoreLost,
    /// The device was locked after the set was asked for and before the PIN was stored (F2): nothing was stored.
    LockedMeanwhile,
    Hash(pin::PinError),
    Store(DeviceStoreError),
}

impl std::fmt::Display for OwnerPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoom => f.write_str("This account's room is not set up yet."),
            Self::NoPin => f.write_str("No PIN is set for this account on this device."),
            Self::NotAPin => f.write_str("A PIN is 4 digits."),
            Self::LockedMeanwhile => f.write_str(
                "This device was locked before the new PIN was saved. Unlock it and set the PIN again.",
            ),
            Self::StoreLost => f.write_str(
                "This device's lock was lost. Reset it with the code we email you before setting a PIN.",
            ),
            Self::Hash(error) => write!(f, "{error}"),
            Self::Store(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OwnerPinError {}

impl From<DeviceStoreError> for OwnerPinError {
    fn from(error: DeviceStoreError) -> Self {
        Self::Store(error)
    }
}

impl From<pin::PinError> for OwnerPinError {
    fn from(error: pin::PinError) -> Self {
        Self::Hash(error)
    }
}

/// Exactly 4 ASCII digits (the PIN stays 4 digits, PLAN §B.1).
pub fn is_pin(text: &str) -> bool {
    text.len() == 4 && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn found<T>(answer: Option<T>) -> Result<T, OwnerPinError> {
    answer.ok_or(OwnerPinError::Store(DeviceStoreError::Io(
        std::io::ErrorKind::NotFound.into(),
    )))
}

/// The owner lock of `room`; `None` while it has no row. Reads only: never makes `device.db`. A `device.db` that
/// is missing while `rooms/` is on disk was lost (L2): that is [`DeviceStoreError::Lost`], never "no PIN" (I2).
pub fn owner_lock(
    store: &DeviceStoreRef,
    room: &RoomId,
) -> Result<Option<OwnerLock>, OwnerPinError> {
    match store.with(false, |store| store.owner_lock(room))? {
        Some(lock) => Ok(lock),
        None => match store {
            DeviceStoreRef::File(layout) if layout.has_rooms() => Err(OwnerPinError::Store(
                DeviceStoreError::Lost(layout.device_db()),
            )),
            _ => Ok(None),
        },
    }
}

/// Whether `room`'s lock was lost (I2, N6): `device.db` is missing while rooms are on disk, or it was made again
/// after a loss ([`DeviceStore::recreate_after_loss`]) and this room has no owner PIN since. Until the emailed
/// reset (§B.6, B3) sets one, no PIN may be set here.
pub fn store_lost(store: &DeviceStoreRef, room: &RoomId) -> Result<bool, OwnerPinError> {
    match owner_lock(store, room) {
        Err(OwnerPinError::Store(DeviceStoreError::Lost(_))) => Ok(true),
        Err(error) => Err(error),
        Ok(Some(lock)) if lock.pin_phc.is_some() => Ok(false),
        Ok(_) => Ok(store
            .with(false, |store| store.lost_at())?
            .flatten()
            .is_some()),
    }
}

/// Set `room`'s owner PIN: Argon2id with its own salt, hashed on the caller's thread (run it off the engine
/// thread). The owner's count goes back to 0. Refused while the lock is lost ([`store_lost`]). Whether the caller
/// may set it otherwise is the caller's rule.
pub fn set_owner_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
) -> Result<(), OwnerPinError> {
    set_owner_pin_with(store, room, typed, |write| write())
}

/// [`set_owner_pin`], with the write handed to `commit` once the hash is made: `commit` calls `write` to store the
/// PIN, or refuses without calling it. A caller holds its own state across the call, so a lock it records can
/// never land between its check and the write (F2).
///
/// Takes the owner's per-subject lock, which is not re-entrant: never call it (nor [`adopt_legacy_pin`]) from
/// inside [`guarded`] for this room's owner (F4; a debug build asserts it).
pub fn set_owner_pin_with(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
    commit: impl FnOnce(&dyn Fn() -> Result<(), OwnerPinError>) -> Result<(), OwnerPinError>,
) -> Result<(), OwnerPinError> {
    if !is_pin(typed) {
        return Err(OwnerPinError::NotAPin);
    }
    debug_assert!(
        !inside_guarded(&AttemptKey::owner(room)),
        "set_owner_pin inside guarded(owner) would deadlock on the owner's lock"
    );
    let lock = subject_lock(&AttemptKey::owner(room));
    let _one_at_a_time = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if store_lost(store, room)? {
        return Err(OwnerPinError::StoreLost);
    }
    if found(store.with(true, |store| store.room(room))?)?.is_none() {
        return Err(OwnerPinError::NoRoom);
    }
    let phc = pin::hash(typed)?;
    commit(&|| {
        found(store.with(true, |store| store.store_owner_pin_phc(room, &phc))?)?;
        Ok(())
    })
}

/// Check `typed` against `room`'s owner PIN under the limit ([`guarded`]); hashed on the caller's thread. The PIN
/// is read inside the try, so a PIN set meanwhile is the one checked (N7). Only an Argon2id string is accepted
/// (N2). A match against a hash cheaper than today's cost is rehashed.
pub fn verify_owner_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
    now: &ClockReading,
) -> Result<Attempt, OwnerPinError> {
    let mut checked_phc = None;
    let mut upgrade = false;
    let attempt = guarded(store, &AttemptKey::owner(room), now, || {
        let Some(phc) = owner_lock(store, room)?.and_then(|lock| lock.pin_phc) else {
            return Err(OwnerPinError::NoPin);
        };
        let checked = pin::check_phc(&phc, typed)?;
        upgrade = matches!(checked, pin::Check::Match { upgrade: true });
        checked_phc = Some(phc);
        Ok(checked.matched())
    })
    .map_err(|error| match error {
        GuardError::Store(error) => OwnerPinError::Store(error),
        GuardError::Check(error) => error,
    })?;
    if let (Attempt::Matched, true, Some(phc)) = (attempt, upgrade, checked_phc) {
        let rehashed = pin::hash(typed)
            .map_err(OwnerPinError::from)
            .and_then(|new| {
                Ok(store.with(true, |store| store.replace_owner_pin_phc(room, &phc, &new))?)
            });
        if let Err(error) = rehashed {
            tracing::warn!("owner PIN not rehashed at today's cost: {error}");
        }
    }
    Ok(attempt)
}

/// The legacy upgrade (L1): the old credential, typed once at the legacy unlock, against the hash the legacy file
/// stored (the old SHA-256 form, or a client PIN already rehashed). On a match the typed PIN becomes `room`'s owner
/// PIN, hashed afresh with Argon2id and its own salt; the old value is never copied. Answers whether it matched.
/// The limit is the caller's (the legacy lock's subject): it may run inside [`guarded`] for the legacy subject,
/// never inside [`guarded`] for this room's owner, whose lock it takes and which is not re-entrant (F4; a debug
/// build asserts it).
pub fn adopt_legacy_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
    legacy_stored: &str,
) -> Result<bool, OwnerPinError> {
    if !is_pin(typed) {
        return Err(OwnerPinError::NotAPin);
    }
    debug_assert!(
        !inside_guarded(&AttemptKey::owner(room)),
        "adopt_legacy_pin inside guarded(owner) would deadlock on the owner's lock"
    );
    if !pin::check(legacy_stored, typed)?.matched() {
        return Ok(false);
    }
    set_owner_pin(store, room, typed)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_clock::Boot;
    use crate::rooms::{Layout, RoomRecord, room_id_for};

    const HOUR: i64 = 3600;

    fn reading(unix_secs: i64, boot: Option<(&str, i64)>) -> ClockReading {
        ClockReading {
            unix_secs,
            boot: boot.map(|(id, secs)| Boot {
                id: id.to_owned(),
                secs,
            }),
        }
    }

    fn file_store(folder: &std::path::Path) -> DeviceStoreRef {
        DeviceStoreRef::File(Layout::at(folder))
    }

    fn memory_store() -> DeviceStoreRef {
        DeviceStoreRef::Ephemeral(std::sync::Arc::new(std::sync::Mutex::new(
            DeviceStore::open_in_memory().unwrap(),
        )))
    }

    fn with_room(store: &DeviceStoreRef) -> RoomId {
        let room = room_id_for("https://api.buwiz.com", "user-1");
        store
            .with(true, |store| {
                store.add_room(&RoomRecord {
                    room_id: room.clone(),
                    origin: "https://api.buwiz.com".into(),
                    user_id: "user-1".into(),
                    email: None,
                    created_at: "2026-11-27T00:00:00+00:00".into(),
                    from_legacy: false,
                })
            })
            .unwrap();
        room
    }

    fn miss(store: &DeviceStoreRef, key: &AttemptKey, now: &ClockReading) -> Attempt {
        guarded(store, key, now, || Ok::<_, ()>(false)).unwrap()
    }

    fn hit(store: &DeviceStoreRef, key: &AttemptKey, now: &ClockReading) -> Attempt {
        guarded(store, key, now, || Ok::<_, ()>(true)).unwrap()
    }

    fn row(store: &DeviceStoreRef, key: &AttemptKey) -> AttemptRow {
        store
            .with(true, |store| store.attempts(key))
            .unwrap()
            .unwrap()
    }

    /// Six misses in a row at `now`: the sixth starts a wait.
    fn six_misses(store: &DeviceStoreRef, key: &AttemptKey, now: &ClockReading) -> Duration {
        for _ in 0..5 {
            assert_eq!(miss(store, key, now), Attempt::Missed { wait: None });
        }
        match miss(store, key, now) {
            Attempt::Missed { wait: Some(wait) } => wait,
            other => panic!("the sixth miss starts a wait: {other:?}"),
        }
    }

    #[test]
    fn todays_schedule_is_kept() {
        let waits: Vec<Option<i64>> = (1..=48).map(wait_after).filter(Option::is_some).collect();
        assert_eq!(
            waits,
            [60, 300, 900, 1800, 3600, 86_400, 86_400, 86_400].map(Some),
            "1 min, 5, 15, 30, 1 h, then 24 h for each later round"
        );
        assert_eq!(wait_after(5), None);
        assert_eq!(
            lockout_message(Duration::from_secs(86_400)),
            "Too many wrong tries. Try again in 24 h 00 min."
        );

        let store = memory_store();
        let key = AttemptKey::client(None, "123456788");
        let mut now = reading(1_000_000, Some(("boot-a", 500)));
        for expected in [60i64, 300, 900] {
            assert_eq!(
                six_misses(&store, &key, &now),
                Duration::from_secs(expected as u64)
            );
            // Even the right PIN waits, and the wait counts nothing.
            assert!(matches!(hit(&store, &key, &now), Attempt::Waiting(_)));
            now.unix_secs += expected;
            now.boot.as_mut().unwrap().secs += expected;
        }
        assert_eq!(row(&store, &key).failures, 18);
        assert_eq!(hit(&store, &key, &now), Attempt::Matched);
        assert_eq!(
            row(&store, &key),
            AttemptRow::default(),
            "a match resets the count"
        );
    }

    #[test]
    fn subjects_and_rooms_count_apart_and_a_failed_check_commits_nothing() {
        let store = memory_store();
        let now = reading(1_000_000, Some(("boot-a", 500)));
        let room = room_id_for("https://api.buwiz.com", "user-1");
        let owner = AttemptKey::owner(&room);
        six_misses(&store, &owner, &now);
        for other in [
            AttemptKey::client(Some(&room), "123456788"),
            AttemptKey::client(None, "123456788"),
            AttemptKey::legacy(),
            AttemptKey::owner(&room_id_for("https://api.buwiz.com", "user-2")),
        ] {
            assert_eq!(
                miss(&store, &other, &now),
                Attempt::Missed { wait: None },
                "{other:?}"
            );
        }

        let key = AttemptKey::legacy();
        for _ in 0..10 {
            assert!(matches!(
                guarded(&store, &key, &now, || Err::<bool, _>("unreachable")),
                Err(GuardError::Check("unreachable"))
            ));
        }
        assert_eq!(
            row(&store, &key).failures,
            1,
            "only the one real miss counts"
        );
        // A failed check on the try that would start a wait starts none.
        for _ in 0..4 {
            miss(&store, &key, &now);
        }
        assert!(guarded(&store, &key, &now, || Err::<bool, _>("unreachable")).is_err());
        let after = row(&store, &key);
        assert_eq!((after.failures, after.wait_secs), (5, 0));
        assert!(matches!(
            miss(&store, &key, &now),
            Attempt::Missed { wait: Some(_) }
        ));
    }

    /// The same-boot rule, within a boot: time since boot decides, so a wall clock moved forward or back changes
    /// nothing (X3: a clock set forward never reads as a reboot, because the boot id does not move with it).
    #[test]
    fn wall_clock_jumps_do_not_change_a_wait_in_the_same_boot() {
        let store = memory_store();
        let key = AttemptKey::legacy();
        let start = reading(1_000_000, Some(("boot-a", 500)));
        assert_eq!(six_misses(&store, &key, &start), Duration::from_secs(60));
        let stored = row(&store, &key);
        assert_eq!(stored.started_boot_id, "boot-a");
        assert_eq!(stored.started_boot_secs, 500);

        // 20 s of boot time later, with the wall clock set 3 h forward, then 3 h back: 40 s left both times.
        for wall in [1_000_000 + 3 * HOUR + 20, 1_000_000 - 3 * HOUR + 20] {
            let now = reading(wall, Some(("boot-a", 520)));
            assert_eq!(stored.wait_left_secs(&now), 40, "wall {wall}");
            assert!(
                matches!(hit(&store, &key, &now), Attempt::Waiting(wait) if wait.as_secs() == 40)
            );
        }
        // 60 s of boot time later the wait is over, whatever the wall clock says.
        let over = reading(1_000_000 - 3 * HOUR, Some(("boot-a", 560)));
        assert_eq!(stored.wait_left_secs(&over), 0);
        assert_eq!(hit(&store, &key, &over), Attempt::Matched);
    }

    /// The same-boot rule, across a boot: a new boot id hands the wait to the wall clock and `started_unix`, even
    /// when the new boot's uptime is longer than the old one's was (the delta read).
    #[test]
    fn across_a_boot_id_change_the_wall_clock_decides() {
        let store = memory_store();
        let key = AttemptKey::legacy();
        let start = reading(1_000_000, Some(("boot-a", 500)));
        six_misses(&store, &key, &start);
        let stored = row(&store, &key);
        // Rebooted; the new boot has been up longer (900 s) than the old one was when the wait began.
        assert_eq!(
            stored.wait_left_secs(&reading(1_000_000 + 10, Some(("boot-b", 900)))),
            50
        );
        assert_eq!(
            stored.wait_left_secs(&reading(1_000_000 + 60, Some(("boot-b", 900)))),
            0
        );
        // A reading with no boot at all is the wall clock too.
        assert_eq!(stored.wait_left_secs(&reading(1_000_000 + 30, None)), 30);
        // A wait started with no boot is timed on the wall clock even in a boot that can be read.
        let unbooted = memory_store();
        six_misses(&unbooted, &key, &reading(2_000_000, None));
        let stored = row(&unbooted, &key);
        assert_eq!(stored.started_boot_id, "");
        assert_eq!(
            stored.wait_left_secs(&reading(2_000_000 + 45, Some(("boot-a", 10)))),
            15
        );
    }

    #[test]
    fn clock_set_back_does_not_shorten_a_wait() {
        let store = memory_store();
        let key = AttemptKey::legacy();
        // The 12th miss: a 5-minute wait.
        let mut now = reading(1_000_000, Some(("boot-a", 500)));
        six_misses(&store, &key, &now);
        now.unix_secs += 60;
        now.boot.as_mut().unwrap().secs += 60;
        assert_eq!(six_misses(&store, &key, &now), Duration::from_secs(300));
        let stored = row(&store, &key);
        // After a reboot, the clock set back a day: the whole wait is left, never more.
        let back = reading(now.unix_secs - 24 * HOUR, Some(("boot-b", 30)));
        assert_eq!(stored.wait_left_secs(&back), 300);
        // In the same boot, set back: boot time decides.
        let same = reading(now.unix_secs - 24 * HOUR, Some(("boot-a", 560 + 100)));
        assert_eq!(stored.wait_left_secs(&same), 200);
    }

    /// L4: clock forward, fail, clock back (after a reboot, where only the wall clock is left): the wait is never
    /// longer than its own length, and never longer than 24 h, however the row was written.
    #[test]
    fn a_wait_never_exceeds_24h_or_its_own_length() {
        let store = memory_store();
        let key = AttemptKey::legacy();
        let forward = reading(1_000_000 + 400 * 24 * HOUR, Some(("boot-a", 500)));
        assert_eq!(six_misses(&store, &key, &forward), Duration::from_secs(60));
        let stored = row(&store, &key);
        let back = reading(1_000_000, Some(("boot-b", 20)));
        assert_eq!(
            stored.wait_left_secs(&back),
            60,
            "its own length, not 400 days"
        );

        let mut planted = stored.clone();
        planted.wait_secs = 30 * 24 * HOUR;
        assert_eq!(
            planted.wait_left_secs(&back),
            MAX_WAIT_SECS,
            "never beyond 24 h"
        );
        planted.started_boot_id = "boot-b".into();
        planted.started_boot_secs = 1_000_000;
        assert_eq!(
            planted.wait_left_secs(&back),
            MAX_WAIT_SECS,
            "a boot time from the future reads as no time passed"
        );
        planted.wait_secs = -5;
        assert_eq!(planted.wait_left_secs(&back), 0, "floored at 0");
    }

    // --------------------------------------------------------------------- restarts and kills (real processes)

    /// The variable that makes [`child_counts_tries`] act; unset, it does nothing.
    const CHILD_FOLDER: &str = "BIR_PIN_ATTEMPTS_CHILD_FOLDER";
    const CHILD_ROLE: &str = "BIR_PIN_ATTEMPTS_CHILD_ROLE";

    /// Run in a child process by the two tests below; a plain test run skips it.
    #[test]
    fn child_counts_tries() {
        let (Ok(folder), Ok(role)) = (std::env::var(CHILD_FOLDER), std::env::var(CHILD_ROLE))
        else {
            return;
        };
        let store = file_store(std::path::Path::new(&folder));
        let key = AttemptKey::client(None, "123456788");
        let now = crate::boot_clock::read();
        match role.as_str() {
            "six-misses" => {
                six_misses(&store, &key, &now);
            }
            "hang-in-the-check" => {
                let folder = std::path::Path::new(&folder);
                // A wrong PIN, killed while its hash runs: had it reached the commit it would count.
                let answer = guarded(&store, &key, &now, || {
                    std::fs::write(folder.join("in-the-check"), b"1").unwrap();
                    std::thread::sleep(Duration::from_secs(120));
                    Ok::<_, ()>(false)
                });
                std::fs::write(folder.join("answered"), format!("{answer:?}")).unwrap();
            }
            other => panic!("unknown role {other}"),
        }
    }

    fn child(folder: &std::path::Path, role: &str) -> std::process::Command {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "owner_lock::tests::child_counts_tries",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD_FOLDER, folder)
            .env(CHILD_ROLE, role)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }

    #[test]
    fn failures_survive_a_restart() {
        let folder = tempfile::tempdir().unwrap();
        let status = child(folder.path(), "six-misses").status().unwrap();
        assert!(status.success());
        // This process is the new one: it reads the count and the wait from device.db.
        let store = file_store(folder.path());
        let key = AttemptKey::client(None, "123456788");
        let stored = row(&store, &key);
        assert_eq!(stored.failures, 6);
        assert_eq!(stored.wait_secs, 60);
        let now = crate::boot_clock::read();
        let left = stored.wait_left_secs(&now);
        assert!((50..=60).contains(&left), "{left} s left");
        assert!(matches!(hit(&store, &key, &now), Attempt::Waiting(_)));
        assert_eq!(
            row(&store, &key).failures,
            6,
            "a refused try counts nothing"
        );
    }

    /// L5, hash then commit then answer: a process killed while the hash runs never reached the commit, so the
    /// count is what it was and the page got no answer.
    #[test]
    fn a_kill_before_the_commit_counts_nothing_and_answers_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let store = file_store(folder.path());
        let key = AttemptKey::client(None, "123456788");
        let now = crate::boot_clock::read();
        for _ in 0..2 {
            assert_eq!(miss(&store, &key, &now), Attempt::Missed { wait: None });
        }
        let mut running = child(folder.path(), "hang-in-the-check").spawn().unwrap();
        let marker = folder.path().join("in-the-check");
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !marker.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the child never reached the check"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        running.kill().unwrap();
        let status = running.wait().unwrap();
        assert!(!status.success(), "killed, not finished");
        assert!(
            !folder.path().join("answered").exists(),
            "the page got no answer"
        );
        let stored = row(&store, &key);
        assert_eq!(stored.failures, 2, "the killed try counts nothing");
        assert_eq!(stored.wait_secs, 0);
    }

    // ----------------------------------------------------------------------------------------- the owner PIN

    #[test]
    fn the_owner_pin_is_set_verified_and_limited() {
        let folder = tempfile::tempdir().unwrap();
        let store = file_store(folder.path());
        let room = room_id_for("https://api.buwiz.com", "user-1");
        assert!(matches!(
            set_owner_pin(&store, &room, "0817"),
            Err(OwnerPinError::NoRoom)
        ));
        let room = with_room(&store);
        assert!(matches!(
            set_owner_pin(&store, &room, "817"),
            Err(OwnerPinError::NotAPin)
        ));
        let now = crate::boot_clock::read();
        assert!(matches!(
            verify_owner_pin(&store, &room, "0817", &now),
            Err(OwnerPinError::NoPin)
        ));

        set_owner_pin(&store, &room, "0817").unwrap();
        let lock = owner_lock(&store, &room).unwrap().unwrap();
        let phc = lock.pin_phc.unwrap();
        assert!(phc.starts_with("$argon2id$"), "{phc}");
        assert!(lock.set_at.is_some());
        assert_eq!(
            verify_owner_pin(&store, &room, "0000", &now).unwrap(),
            Attempt::Missed { wait: None }
        );
        assert_eq!(
            verify_owner_pin(&store, &room, "0817", &now).unwrap(),
            Attempt::Matched
        );
        let key = AttemptKey::owner(&room);
        assert_eq!(row(&store, &key).failures, 0);
        for _ in 0..5 {
            verify_owner_pin(&store, &room, "0000", &now).unwrap();
        }
        assert!(matches!(
            verify_owner_pin(&store, &room, "0000", &now).unwrap(),
            Attempt::Missed { wait: Some(_) }
        ));
        assert!(matches!(
            verify_owner_pin(&store, &room, "0817", &now).unwrap(),
            Attempt::Waiting(_)
        ));
        // A new PIN resets the count.
        set_owner_pin(&store, &room, "2468").unwrap();
        assert_eq!(row(&store, &key), AttemptRow::default());
        assert_eq!(
            verify_owner_pin(&store, &room, "2468", &now).unwrap(),
            Attempt::Matched
        );
    }

    /// I1: twenty tries sent at once, from 5 wrong in a row. Exactly one more yes/no answer comes back (the 6th
    /// miss, which starts the wait); the other nineteen answer the wait, and count nothing.
    #[test]
    fn twenty_tries_at_once_get_one_answer_before_the_wait() {
        let folder = tempfile::tempdir().unwrap();
        let store = file_store(folder.path());
        let room = with_room(&store);
        set_owner_pin(&store, &room, "0817").unwrap();
        let now = crate::boot_clock::read();
        for _ in 0..5 {
            verify_owner_pin(&store, &room, "0000", &now).unwrap();
        }
        let start = std::sync::Arc::new(std::sync::Barrier::new(20));
        let tries: Vec<_> = (0..20)
            .map(|_| {
                let (store, room, now, start) =
                    (store.clone(), room.clone(), now.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    verify_owner_pin(&store, &room, "0000", &now).unwrap()
                })
            })
            .collect();
        let answers: Vec<Attempt> = tries.into_iter().map(|t| t.join().unwrap()).collect();
        let answered = answers
            .iter()
            .filter(|a| matches!(a, Attempt::Matched | Attempt::Missed { .. }))
            .count();
        assert_eq!(answered, 1, "{answers:?}");
        assert!(
            answers
                .iter()
                .any(|a| matches!(a, Attempt::Missed { wait: Some(_) }))
        );
        assert_eq!(
            answers
                .iter()
                .filter(|a| matches!(a, Attempt::Waiting(_)))
                .count(),
            19
        );
        let stored = row(&store, &AttemptKey::owner(&room));
        assert_eq!((stored.failures, stored.wait_secs), (6, 60));
    }

    /// I1(b), the backstop across processes: a match read before a parallel 6th miss committed must not erase the
    /// wait that miss started, and answers the wait; a miss settled during a wait it did not see still counts, and
    /// answers the wait too.
    #[test]
    fn a_match_settled_after_a_parallel_sixth_miss_keeps_its_wait() {
        let store = memory_store();
        let key = AttemptKey::legacy();
        let now = reading(1_000_000, Some(("boot-a", 500)));
        for _ in 0..5 {
            miss(&store, &key, &now);
        }
        let begin = || match store
            .with(true, |s| s.begin_try(&key, &now))
            .unwrap()
            .unwrap()
        {
            Begun::Ready(pending) => pending,
            Begun::Waiting(wait) => panic!("no wait yet, found {wait:?}"),
        };
        let the_match = begin();
        let late_miss = begin();
        let sixth = begin();
        let settle = |pending: &PendingTry, outcome| {
            store
                .with(true, |s| s.settle_try(pending, outcome))
                .unwrap()
                .unwrap()
        };
        assert_eq!(
            settle(&sixth, TryOutcome::Missed),
            Settled::Done {
                wait_started: Some(Duration::from_secs(60))
            }
        );
        assert!(matches!(
            settle(&the_match, TryOutcome::Matched),
            Settled::Waiting(_)
        ));
        let kept = row(&store, &key);
        assert_eq!(
            (kept.failures, kept.wait_secs),
            (6, 60),
            "the wait was not erased"
        );
        assert!(matches!(
            settle(&late_miss, TryOutcome::Missed),
            Settled::Waiting(_)
        ));
        assert_eq!(row(&store, &key).failures, 7, "the late miss still counts");
    }

    /// I2 under a running app (Unix, where an open file can be deleted): device.db and its -wal/-shm go WHILE
    /// the process holds its connection. The held connection is dropped when the file is missing
    /// (`DeviceStoreRef::with`), so the state reads Lost, never an answer from the unlinked file.
    #[cfg(unix)]
    #[test]
    fn a_device_db_deleted_under_the_held_connection_reads_lost() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let store = file_store(folder.path());
        let room = with_room(&store);
        set_owner_pin(&store, &room, "0817").unwrap();
        layout.create_room_dir(&room).unwrap();
        assert!(
            crate::rooms::device_store_is_held(&layout),
            "the PIN's write left its connection held"
        );
        std::fs::remove_file(layout.device_db()).unwrap();
        for file in ["device.db-wal", "device.db-shm"] {
            match std::fs::remove_file(folder.path().join(file)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("{file} was not removed: {error}"),
            }
        }
        assert!(matches!(
            owner_lock(&store, &room),
            Err(OwnerPinError::Store(DeviceStoreError::Lost(_)))
        ));
        assert!(store_lost(&store, &room).unwrap());
        assert!(matches!(
            set_owner_pin(&store, &room, "2468"),
            Err(OwnerPinError::StoreLost)
        ));
        assert!(
            !layout.device_db().exists(),
            "nothing made it again silently"
        );
    }

    /// I2: a device.db missing while rooms are on disk is "lost", never "no PIN"; so is one made again after the
    /// loss, for a room with no PIN since. No PIN may be set there until the emailed reset stores one (B3).
    #[test]
    fn a_lost_device_db_is_never_no_pin() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let store = file_store(folder.path());
        let room = with_room(&store);
        set_owner_pin(&store, &room, "0817").unwrap();
        drop(DeviceStore::open(&layout).unwrap());
        // Gone, with the room's folder on disk (the held connection is dropped when the file is missing).
        layout.create_room_dir(&room).unwrap();
        // As between two runs: no connection held (Windows cannot delete an open SQLite file), and every delete
        // checked, so a delete that silently fails cannot pass for a loss.
        crate::rooms::release_held_device_stores();
        for file in ["device.db", "device.db-wal", "device.db-shm"] {
            match std::fs::remove_file(folder.path().join(file)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("{file} was not removed: {error}"),
            }
        }
        assert!(!layout.device_db().exists(), "the store is gone");
        assert!(matches!(
            owner_lock(&store, &room),
            Err(OwnerPinError::Store(DeviceStoreError::Lost(_)))
        ));
        assert!(store_lost(&store, &room).unwrap());
        assert!(matches!(
            set_owner_pin(&store, &room, "2468"),
            Err(OwnerPinError::StoreLost)
        ));
        assert!(
            !layout.device_db().exists(),
            "nothing made it again silently"
        );

        // Made again after the loss: still lost for this room, and still no set.
        drop(DeviceStore::recreate_after_loss(&layout).unwrap());
        assert!(owner_lock(&store, &room).unwrap().is_none());
        assert!(store_lost(&store, &room).unwrap());
        assert!(matches!(
            set_owner_pin(&store, &room, "2468"),
            Err(OwnerPinError::StoreLost)
        ));
        // The reset (B3) stores a PIN: no longer lost, and the PIN can be changed again.
        let reset = pin::hash("1357").unwrap();
        store
            .with(true, |s| {
                s.add_room(&RoomRecord {
                    room_id: room.clone(),
                    origin: "https://api.buwiz.com".into(),
                    user_id: "user-1".into(),
                    email: None,
                    created_at: "2026-11-27T00:00:00+00:00".into(),
                    from_legacy: false,
                })?;
                s.store_owner_pin_phc(&room, &reset)
            })
            .unwrap();
        assert!(!store_lost(&store, &room).unwrap());
        set_owner_pin(&store, &room, "2468").unwrap();
    }

    /// F4: the legacy adoption may run inside the legacy subject's try, and a debug build refuses it inside the
    /// owner's own try (whose lock is not re-entrant) instead of deadlocking.
    #[test]
    fn adopt_legacy_pin_never_runs_inside_the_owners_try() {
        let store = memory_store();
        let room = with_room(&store);
        let legacy = crate::crypto::hash_pin("0817");
        let now = reading(1_000_000, None);
        let adopted = guarded(&store, &AttemptKey::legacy(), &now, || {
            adopt_legacy_pin(&store, &room, "0817", &legacy)
        })
        .unwrap();
        assert_eq!(adopted, Attempt::Matched);
        if cfg!(debug_assertions) {
            let inside_owner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = guarded(&store, &AttemptKey::owner(&room), &now, || {
                    adopt_legacy_pin(&store, &room, "0817", &legacy)
                });
            }));
            assert!(inside_owner.is_err(), "the debug assertion fired");
            assert!(
                !inside_guarded(&AttemptKey::owner(&room)),
                "the mark went with the unwind"
            );
        }
    }

    /// N2: an old SHA-256 value in the owner store is refused, and counts nothing.
    #[test]
    fn the_owner_check_refuses_an_old_sha_value() {
        let store = memory_store();
        let room = with_room(&store);
        let sha = crate::crypto::hash_pin("0817");
        store
            .with(true, |s| s.store_owner_pin_phc(&room, &sha))
            .unwrap();
        let now = reading(1_000_000, None);
        assert!(matches!(
            verify_owner_pin(&store, &room, "0817", &now),
            Err(OwnerPinError::Hash(_))
        ));
        assert_eq!(row(&store, &AttemptKey::owner(&room)).failures, 0);
    }

    /// The owner store never holds an old SHA-256 value. The legacy upgrade takes the old credential typed once
    /// and stores a fresh Argon2id hash of it; a cheaper Argon2id owner hash is rehashed on its first match.
    #[test]
    fn legacy_sha_pin_upgrades_on_first_match() {
        let store = memory_store();
        let room = with_room(&store);
        let legacy = crate::crypto::hash_pin("0817");
        assert!(!adopt_legacy_pin(&store, &room, "1111", &legacy).unwrap());
        assert!(
            owner_lock(&store, &room).unwrap().is_none(),
            "a miss stores nothing"
        );
        assert!(adopt_legacy_pin(&store, &room, "0817", &legacy).unwrap());
        let phc = owner_lock(&store, &room).unwrap().unwrap().pin_phc.unwrap();
        assert_eq!(pin::stored_kind(&phc), Some(pin::Stored::Phc));
        assert_ne!(phc, legacy);
        assert!(!phc.contains(&legacy));
        let now = reading(1_000_000, None);
        assert_eq!(
            verify_owner_pin(&store, &room, "0817", &now).unwrap(),
            Attempt::Matched
        );

        // A legacy file whose client PIN was already rehashed is adopted the same way, with a salt of its own.
        let other = memory_store();
        let room = with_room(&other);
        let rehashed = pin::hash("2468").unwrap();
        assert!(adopt_legacy_pin(&other, &room, "2468", &rehashed).unwrap());
        assert_ne!(
            owner_lock(&other, &room).unwrap().unwrap().pin_phc.unwrap(),
            rehashed
        );
    }
}
