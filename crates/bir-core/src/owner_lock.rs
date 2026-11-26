//! The owner PIN store and the persisted retry limit, over `device.db` (accounts-and-lock PLAN §B.1, §B.2).
//!
//! - **The owner PIN** of a room lives in `owner_lock.pin_phc`: an Argon2id PHC string ([`crate::pin`]), never an
//!   old SHA-256 value. [`set_owner_pin`] writes it, [`verify_owner_pin`] checks it under the limit, and
//!   [`adopt_legacy_pin`] is the legacy upgrade: the old credential typed once (L1) becomes the owner PIN, hashed
//!   afresh with its own salt.
//! - **The retry limit** lives in `pin_attempts`, one row per room and subject: the owner, each client (TIN
//!   root) and the legacy lock. It survives a restart. Today's schedule is kept: after every 6th wrong PIN in a
//!   row, a wait of 1 min, 5 min, 15 min, 30 min, 1 h, then 24 h for each later round.
//! - **A try is counted before its check.** [`DeviceStore::begin_try`] commits the count (and the wait it starts)
//!   first; then the hash runs; then [`DeviceStore::settle_try`] commits the reset on a match; only then is the page
//!   answered. A process killed during the check has already paid for the try, and no answer ever leaves before
//!   its commit. A check that fails (a store or database error, not a wrong PIN) gives its try back, as today.
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
    /// Wrong PINs in a row (tries in progress included).
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
    /// A wait is running: nothing was counted and no check may run.
    Waiting(Duration),
    /// The try is counted and committed; run the check, then [`DeviceStore::settle_try`].
    Counted(PendingTry),
}

/// A try that is counted and not settled yet.
#[derive(Debug, Clone)]
pub struct PendingTry {
    key: AttemptKey,
    before: AttemptRow,
    after: AttemptRow,
    /// This try is the one that starts a wait (the 6th, 12th... in a row).
    starts_wait: bool,
    now: ClockReading,
}

impl PendingTry {
    /// The wait this try started if it is a miss (the miss that starts a wait says so).
    pub fn wait_if_missed(&self) -> Option<Duration> {
        self.starts_wait
            .then(|| self.after.wait_left(&self.now))
            .flatten()
    }
}

/// How a counted try ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryOutcome {
    /// The PIN matched: the count goes back to 0.
    Matched,
    /// A wrong PIN: the count stays.
    Missed,
    /// The check itself failed (not a wrong PIN): the try is given back.
    Failed,
}

impl DeviceStore {
    /// The row of `key`; an empty one where there is none.
    pub fn attempts(&self, key: &AttemptKey) -> Result<AttemptRow, DeviceStoreError> {
        Ok(read_attempts(&self.conn, key)?.unwrap_or_default())
    }

    /// Count a try of `key` at `now` and commit it, before the check runs. A running wait refuses instead and
    /// counts nothing.
    pub fn begin_try(
        &self,
        key: &AttemptKey,
        now: &ClockReading,
    ) -> Result<Begun, DeviceStoreError> {
        let transaction =
            rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let before = read_attempts(&transaction, key)?.unwrap_or_default();
        if let Some(wait) = before.wait_left(now) {
            transaction.commit()?;
            return Ok(Begun::Waiting(wait));
        }
        let mut after = before.clone();
        after.failures += 1;
        let starts_wait = wait_after(after.failures).is_some();
        if let Some(wait) = wait_after(after.failures) {
            after.wait_secs = wait;
            after.started_unix = now.unix_secs;
            match &now.boot {
                Some(boot) => {
                    after.started_boot_secs = boot.secs;
                    after.started_boot_id = boot.id.clone();
                }
                None => {
                    after.started_boot_secs = 0;
                    after.started_boot_id = String::new();
                }
            }
        }
        write_attempts(&transaction, key, &after)?;
        transaction.commit()?;
        Ok(Begun::Counted(PendingTry {
            key: key.clone(),
            before,
            after,
            starts_wait,
            now: now.clone(),
        }))
    }

    /// Commit how a counted try ended: a match resets the count; a miss is already counted; a failed check gives
    /// the try back (and the wait it started).
    pub fn settle_try(
        &self,
        pending: &PendingTry,
        outcome: TryOutcome,
    ) -> Result<(), DeviceStoreError> {
        match outcome {
            TryOutcome::Missed => Ok(()),
            TryOutcome::Matched => {
                self.conn.execute(
                    "DELETE FROM pin_attempts WHERE room_id = ?1 AND subject = ?2",
                    params![pending.key.room_id, pending.key.subject.key()],
                )?;
                Ok(())
            }
            TryOutcome::Failed => {
                let transaction = rusqlite::Transaction::new_unchecked(
                    &self.conn,
                    TransactionBehavior::Immediate,
                )?;
                let mut row = read_attempts(&transaction, &pending.key)?.unwrap_or_default();
                row.failures = (row.failures - 1).max(0);
                if pending.starts_wait {
                    row.wait_secs = pending.before.wait_secs;
                    row.started_unix = pending.before.started_unix;
                    row.started_boot_secs = pending.before.started_boot_secs;
                    row.started_boot_id = pending.before.started_boot_id.clone();
                }
                write_attempts(&transaction, &pending.key, &row)?;
                transaction.commit()?;
                Ok(())
            }
        }
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
    /// The limit could not be read or counted: no check ran.
    Store(DeviceStoreError),
    /// The check itself failed; its try was given back.
    Check(E),
}

/// Run `check` for `key` under the limit, in the order above: count and commit, check, commit the reset on a
/// match, answer. A settle that cannot be written after a match or a failed check is logged: the answer still
/// stands, and the try stays counted.
pub fn guarded<E>(
    store: &DeviceStoreRef,
    key: &AttemptKey,
    now: &ClockReading,
    check: impl FnOnce() -> Result<bool, E>,
) -> Result<Attempt, GuardError<E>> {
    let pending = match store
        .with(true, |store| store.begin_try(key, now))
        .map_err(GuardError::Store)?
    {
        Some(Begun::Counted(pending)) => pending,
        Some(Begun::Waiting(wait)) => return Ok(Attempt::Waiting(wait)),
        None => {
            return Err(GuardError::Store(DeviceStoreError::Io(
                std::io::ErrorKind::NotFound.into(),
            )));
        }
    };
    let checked = check();
    let outcome = match &checked {
        Ok(true) => TryOutcome::Matched,
        Ok(false) => TryOutcome::Missed,
        Err(_) => TryOutcome::Failed,
    };
    if let Err(error) = store.with(true, |store| store.settle_try(&pending, outcome)) {
        tracing::warn!(subject = %key.subject.key(), "PIN try not settled: {error}");
    }
    match checked {
        Ok(true) => Ok(Attempt::Matched),
        Ok(false) => Ok(Attempt::Missed {
            wait: pending.wait_if_missed(),
        }),
        Err(error) => Err(GuardError::Check(error)),
    }
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
    Hash(pin::PinError),
    Store(DeviceStoreError),
}

impl std::fmt::Display for OwnerPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoom => f.write_str("This account's room is not set up yet."),
            Self::NoPin => f.write_str("No PIN is set for this account on this device."),
            Self::NotAPin => f.write_str("A PIN is 4 digits."),
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

/// The owner lock of `room`; `None` while it has no row. Reads only: never makes `device.db`.
pub fn owner_lock(
    store: &DeviceStoreRef,
    room: &RoomId,
) -> Result<Option<OwnerLock>, OwnerPinError> {
    Ok(store.with(false, |store| store.owner_lock(room))?.flatten())
}

/// Set `room`'s owner PIN: Argon2id with its own salt, hashed on the caller's thread (run it off the engine
/// thread). The owner's count goes back to 0. Whether the caller may set it is the caller's rule.
pub fn set_owner_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
) -> Result<(), OwnerPinError> {
    if !is_pin(typed) {
        return Err(OwnerPinError::NotAPin);
    }
    if found(store.with(true, |store| store.room(room))?)?.is_none() {
        return Err(OwnerPinError::NoRoom);
    }
    let phc = pin::hash(typed)?;
    found(store.with(true, |store| store.store_owner_pin_phc(room, &phc))?)?;
    Ok(())
}

/// Check `typed` against `room`'s owner PIN under the limit ([`guarded`]); hashed on the caller's thread. A match
/// against a hash cheaper than today's cost is rehashed.
pub fn verify_owner_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
    now: &ClockReading,
) -> Result<Attempt, OwnerPinError> {
    let Some(phc) = owner_lock(store, room)?.and_then(|lock| lock.pin_phc) else {
        return Err(OwnerPinError::NoPin);
    };
    let mut upgrade = false;
    let attempt = guarded(store, &AttemptKey::owner(room), now, || {
        let checked = pin::check(&phc, typed)?;
        upgrade = matches!(checked, pin::Check::Match { upgrade: true });
        Ok::<_, pin::PinError>(checked.matched())
    })
    .map_err(|error| match error {
        GuardError::Store(error) => OwnerPinError::Store(error),
        GuardError::Check(error) => OwnerPinError::Hash(error),
    })?;
    if attempt == Attempt::Matched && upgrade {
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
/// The limit is the caller's (the legacy lock's subject).
pub fn adopt_legacy_pin(
    store: &DeviceStoreRef,
    room: &RoomId,
    typed: &str,
    legacy_stored: &str,
) -> Result<bool, OwnerPinError> {
    if !is_pin(typed) {
        return Err(OwnerPinError::NotAPin);
    }
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
    fn subjects_and_rooms_count_apart_and_a_failed_check_gives_its_try_back() {
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
        // A failed check on the try that would start a wait gives the wait back too.
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
                let marker = std::path::Path::new(&folder).join("in-the-check");
                let _ = guarded(&store, &key, &now, || {
                    std::fs::write(&marker, b"1").unwrap();
                    std::thread::sleep(Duration::from_secs(120));
                    Ok::<_, ()>(true)
                });
                unreachable!("killed during the check");
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

    /// The try is counted before the check: a process killed while the hash runs gave no answer and still paid for
    /// the try (the brief's L5 reading; the PLAN's §B.8 named this "counts nothing").
    #[test]
    fn a_kill_during_the_check_still_counts() {
        let folder = tempfile::tempdir().unwrap();
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
        let stored = row(
            &file_store(folder.path()),
            &AttemptKey::client(None, "123456788"),
        );
        assert_eq!(stored.failures, 1, "the killed try is counted");
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
