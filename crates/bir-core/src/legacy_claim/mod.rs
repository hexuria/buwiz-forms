//! The legacy claim: the one `bir_data.db` of an install from before rooms becomes the room of the account that may
//! open it (accounts-and-lock PLAN §A.3).
//!
//! **No call site in the app** (O2). Only this module's tests and the rehearsal harness call it; A3a adds the launch
//! resume and the sign-in call, A3a-2 the legacy unlock page. Until then nothing here runs in a build that ships.
//!
//! What it guarantees, step by step (PLAN §A.3.2):
//! 1. **Start.** Signed in, not practice, not single-room mode, the legacy file there, its owner lock held through the
//!    process holder ([`crate::db::hold_owner_lock`], X1) from here to the end of the move.
//! 2. **The probe and the unlock** ([`probe_legacy`], [`LockedClaim::unlock`]), on a scratch copy under the device
//!    folder: the legacy file is not opened in place, so "Not mine" leaves it exactly as it was. The record names a
//!    room only once the credential passed, and for the account signed in now (N3).
//! 3. **The raw safety copy** (M2): a bare connection with no create and no migrations checkpoints the file and reads
//!    its `user_version`; the bytes become the single `bir_data.db` of `backups/bir_data.pre-rooms-<stamp>-<sha16>.zip`,
//!    written as `.partial`, fsynced, renamed without replacing, and read back. One zip per raw file.
//! 4. **The move**, never replacing a file: the zip re-checked (M1), the room folder made, an empty room's file set
//!    aside, `hard_link` then unlink (a no-replace rename where links are not supported), folders fsynced, the safety
//!    files beside the legacy file moved along. A `-wal` that came back is transient, with its cause.
//! 5. **The room row** in `device.db`.
//! 6. **Fixups** in the room file (this build's open, `buwiz_server_url` gone, `hand_to_clients`) and in `device.db`
//!    (the device keys, `owner_lock`).
//! 7. **The run-time checks** (ruling 2): the room file's SHA-256 equals the raw one before step 6, and the room opens
//!    with `quick_check` ok after it. A failure after the move is "moved, not opened": no empty room, the zip kept.
//! 8. **Done.**
//!
//! Resume is decided from the files, never from the record ([`LegacyClaim::start`], PLAN §A.3.3); the record in
//! `device.db`'s `migration` table is for support and the log. Every file is opened with no create: a missing file
//! is an answer ("moved" or "nothing to claim"), never a new empty file.

mod files;
#[cfg(test)]
pub(crate) mod fixtures;
mod probe;
#[cfg(test)]
mod rehearsal;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::{Database, OwnerLock};
use crate::rooms::{DeviceStore, DeviceStoreError, Layout, RoomId, RoomLookup, RoomRecord};

pub use probe::{CredentialKinds, LegacyProbe, probe_legacy};

/// The `migration` row of the claim's record (PLAN §A.3.2).
pub const RECORD_KEY: &str = "legacy";
/// The `migration` row listing the rooms whose account said "Not mine" (a JSON array of room ids).
pub const NOT_MINE_KEY: &str = "legacy.not_mine";
/// The `migration` row naming the offered file consumed into a room (B2/B2b). The value keys the
/// consumption by room and content hash, never by inode: a copy or a reappeared inode of the same
/// bytes is the same offer. `importing` resumes; `imported` skips only the import itself.
pub const OFFER_CONSUMED_KEY: &str = "legacy.offer_consumed";

/// Whether the offered file's import has not yet been witnessed as committed or has been.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OfferConsumptionState {
    Importing,
    Imported,
}

/// The offer-consumption mark. The hash is of the exact scratch copy exported; the room is the one
/// account room the import belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OfferConsumption {
    pub room: String,
    pub sha256: String,
    pub state: OfferConsumptionState,
}

/// How the file now at the legacy path matches the consumption mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfferConsumptionMatch {
    None,
    Own(OfferConsumptionState),
    OtherRoom,
}
/// The `migration` row that says "Use this computer's login" was used once for the claim (PLAN §A.3.1). Once per
/// claim, never once forever: the row is deleted whenever the claim ends — at Done, at Closed, in
/// [`LegacyClaim::clear_permanent`], and on any transient or permanent failure before the move (a pre-move resume
/// needs a new unlock, so that counts as the claim ending: the person is never left without their fallback).
pub const OS_LOGIN_USED_KEY: &str = "legacy.os_login_used";

/// The device folders whose claim ran `start()` in this session (R1). A room's database is never opened before
/// its folder is in here: the open runs migrations and a checkpoint, and those writes change the bytes the
/// move's hash check still needs as evidence — the move would close unfinished.
static STARTED_THIS_SESSION: std::sync::Mutex<std::collections::BTreeSet<std::path::PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// R1(d): the set's key is canonical — a folder reached through a symlinked parent, or a differently cased
/// path on a case-insensitive filesystem, is the same folder, so a person is never refused their own room.
/// Canonicalized on insert and on lookup, the same way, as `ClaimRun::begin` keys its set.
fn started_key(device_dir: &std::path::Path) -> std::path::PathBuf {
    device_dir
        .canonicalize()
        .unwrap_or_else(|_| device_dir.to_path_buf())
}

/// Tests simulate a new session — a kill and relaunch — by forgetting what this session answered.
#[cfg(test)]
pub(crate) fn clear_started_for_test(device_dir: &std::path::Path) {
    let key = started_key(device_dir);
    STARTED_THIS_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&key);
}

fn started_insert(device_dir: &std::path::Path) {
    STARTED_THIS_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(started_key(device_dir));
}

/// The evidence half of the room-open rule (G1): the room does not open while the claim's record names
/// it and the step is in [Copied, HashChecked) — a migration or a checkpoint would change the bytes the
/// hash check still needs. The record unreadable is a refusal whenever any rooms exist (failing closed:
/// an unreadable record can hide exactly the move this rule protects). bir-headless serve calls this
/// directly: its process never runs `start()`, so the started half cannot apply to it.
pub fn room_move_evidence_clear(
    layout: &crate::rooms::Layout,
    room_id: &crate::rooms::RoomId,
) -> Result<(), String> {
    let unreadable = || {
        format!(
            "the legacy claim's record cannot be read while rooms exist on this device; the room {} does not open since the record can hide an unfinished move",
            room_id
        )
    };
    let record = match LegacyClaim::at(layout.clone()) {
        Ok(claim) => match claim.record() {
            Ok(record) => record,
            Err(_) if layout.has_rooms() => return Err(unreadable()),
            Err(_) => None,
        },
        Err(_) if layout.has_rooms() => return Err(unreadable()),
        Err(_) => None,
    };
    let pending = record.is_some_and(|record| {
        record.step >= Step::Copied
            && record.step < Step::HashChecked
            && record.room.as_deref() == Some(room_id.as_str())
    });
    if pending {
        return Err(
            "the legacy claim's move is unfinished on this device (the record is not yet hash-checked); the room does not open until the claim finishes it"
                .into(),
        );
    }
    Ok(())
}

/// The whole room-open rule (G1): `start()` ran to a definitive answer for the device in this session,
/// and no unchecked move evidence names this room. Every open of a room's database — the slot's real
/// path, the restore's reopen, the legacy choke point — goes through here.
pub fn room_open_allowed(
    layout: &crate::rooms::Layout,
    room_id: &crate::rooms::RoomId,
) -> Result<(), String> {
    if !claim_started_this_session(layout.device_dir()) {
        return Err(
            "the legacy claim's start() has not run for this device folder in this session; a room's database is never opened before it (R1)"
                .into(),
        );
    }
    room_move_evidence_clear(layout, room_id)
}

pub fn claim_started_this_session(device_dir: &std::path::Path) -> bool {
    let key = started_key(device_dir);
    STARTED_THIS_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(&key)
}
/// The scratch folder under the device folder: probe copies, swept at every launch.
pub const SCRATCH_DIR: &str = "scratch";
/// What the pre-rooms safety zips are called, before `<stamp>-<sha16>.zip`.
pub const ZIP_PREFIX: &str = "bir_data.pre-rooms-";
/// The device-wide settings keys (PLAN §A.1 point 5) step 6 copies into `device_settings` where it has none.
pub const DEVICE_SETTING_KEYS: [&str; 4] = [
    "theme_preference",
    "global_hotkey_key",
    crate::log_files::RETENTION_SETTING,
    "auto_update_download",
];
/// The setting step 6 deletes from the room file (it is device-wide now).
pub const OBSOLETE_SERVER_URL_SETTING: &str = "buwiz_server_url";
/// The room file's hand-to-clients switch, written from the access mode at step 6 (§C.1).
pub const HAND_TO_CLIENTS_SETTING: &str = "hand_to_clients";
/// The pre-rooms zip is kept at least this long after the claim (PLAN §A.3.8).
pub const ZIP_KEEP_DAYS: i64 = 30;
/// … and until the app was launched on this many distinct days since.
pub const ZIP_KEEP_LAUNCH_DAYS: usize = 5;

/// The build that wrote a failure: a different build retries a permanent failure (a later build may handle it).
const BUILD: &str = env!("CARGO_PKG_VERSION");

// ------------------------------------------------------------------------------------------------ who claims

/// The account signed in now, from the saved sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimant {
    /// The saved sign-in's `server_url` (normalised here, [`crate::rooms::room_origin`]).
    pub server_url: String,
    pub user_id: String,
    pub email: Option<String>,
    /// The store-review account (`StoredAccount.practice`): it never claims (PLAN §A.3.1).
    pub practice: bool,
}

/// The claimant's room: its id and the identity a `rooms` row is written with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaimRoom {
    room_id: RoomId,
    origin: String,
    user_id: String,
    email: Option<String>,
}

/// How the legacy file was opened at the legacy unlock (PLAN §A.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockedBy {
    /// The file had no lock.
    NoLock,
    Pin,
    Authenticator,
    RecoveryKey,
    /// "Use this computer's login" (A3a-2's OS check said yes).
    ComputerLogin,
}

/// What the person typed (or did) at the legacy unlock.
pub enum LegacyCredential<'a> {
    /// The 4 digits.
    Pin(&'a str),
    /// The 6-digit code of the admin's authenticator.
    AuthenticatorCode(&'a str),
    /// The recovery key, as typed.
    RecoveryKey(&'a str),
    /// The OS confirmed the computer's owner (LocalAuthentication / Windows Hello, A3a-2). Accepted once per
    /// unlock offer: every use is a fresh OS check, and the used mark is cleared when the claim ends (at Done,
    /// at Closed, in `clear_permanent`, on any failure before the move) and whenever `start()` will offer an
    /// unlock again, so the fallback is never spent forever.
    ComputerLogin,
}

// ------------------------------------------------------------------------------------------------ the record

/// Where a claim got to, as recorded. Resume never trusts it (PLAN §A.3.3); it is for support and the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// A claim began and no credential has passed yet (the record names no room).
    #[default]
    Started,
    /// Step 2 passed: `room` and `unlocked_by` are written.
    Unlocked,
    /// Step 3 passed: the zip is written and checked.
    Copied,
    /// Step 4 passed: the file is in the room.
    Moved,
    /// Step 5 passed.
    RoomRow,
    /// Step 7's hash check passed (before step 6).
    HashChecked,
    /// Step 6 passed.
    Fixed,
    /// Step 7's open check passed.
    OpenChecked,
    Done,
    /// The legacy file and the room file were both gone: nothing to claim.
    Closed,
}

/// A failure the record keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordedFailure {
    Transient {
        cause: TransientCause,
        step: Step,
        at: String,
    },
    Permanent {
        reason: PermanentReason,
        detail: String,
        at: String,
        build: String,
    },
    MovedNotOpened {
        reason: MovedNotOpenedReason,
        detail: String,
        at: String,
        build: String,
    },
}

/// `device.db`'s `migration` row `legacy` (PLAN §A.3.2), written at `synchronous = FULL` after each step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Record {
    pub stamp: String,
    /// The claimant's room: written with `unlocked_by` at the end of step 2, never before (N3).
    pub room: Option<String>,
    /// The claimant's normalised origin, user id and email, written with `room`, so steps 5–8 of a claim cut after the
    /// move write the claimer's row whoever is signed in when it resumes.
    pub origin: Option<String>,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub unlocked_by: Option<UnlockedBy>,
    /// The claimant had an empty room, set aside at step 4.
    pub empty_room_aside: bool,
    /// The name it was set aside under, in the room folder.
    pub empty_room_file: Option<String>,
    pub os_login_used: bool,
    pub raw_user_version: Option<i32>,
    pub raw_sha256: Option<String>,
    /// The zip's file name in `backups/`.
    pub zip: Option<String>,
    pub zip_sha256: Option<String>,
    pub hash_checked: bool,
    pub open_checked: bool,
    pub step: Step,
    pub failure: Option<RecordedFailure>,
    pub done_at: Option<String>,
    /// The distinct local calendar days the app was launched on since `done_at` (PLAN §A.3.8).
    pub launch_days: Vec<String>,
    /// The room was opened since `done_at`.
    pub room_opened_since: bool,
    pub zip_deleted_at: Option<String>,
    /// The legacy file's identity when it was probed (C1). A link or a rename keeps it, so a room file with this
    /// identity is the legacy file moved; any other room file is not, whatever the record says.
    #[serde(default)]
    pub legacy_file: Option<crate::safe_fs::FileId>,
    /// The PHC string of the PIN typed at the legacy unlock, kept only until step 6 writes it into `owner_lock`, so a
    /// claim cut after the move still gives the owner the PIN they typed. Cleared at done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_pin_phc: Option<PinPhc>,
}

/// The PHC string of the owner PIN typed at the legacy unlock: stored as it is, never printed (C9).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PinPhc(String);

impl std::fmt::Debug for PinPhc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PinPhc(<redacted>)")
    }
}

impl Record {
    fn room_id(&self) -> Option<RoomId> {
        self.room.as_deref().and_then(RoomId::parse)
    }

    fn claim_room(&self) -> Option<ClaimRoom> {
        Some(ClaimRoom {
            room_id: self.room_id()?,
            origin: self.origin.clone()?,
            user_id: self.user_id.clone()?,
            email: self.email.clone(),
        })
    }

    fn moved(&self) -> bool {
        self.step >= Step::Moved && self.step != Step::Closed
    }
}

// ------------------------------------------------------------------------------------------------ failures

/// A transient failure: nothing moved, or the move is resumable; Try again (PLAN §A.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransientCause {
    /// The owner lock is held: another copy of the app, an old version running.
    OwnerLockHeld,
    DiskFull,
    Io,
    /// A `-wal` or `-shm` came back after the checkpoint: another program has the file open.
    WalReturned,
    /// The legacy file was replaced since it was probed, or the zip no longer matches: start again.
    FileChanged,
    /// A file is already where the move would put the legacy file, and it is not an empty room.
    RoomFileTaken,
    /// Another claim of this device folder is running in this process (C4).
    ClaimInProgress,
}

/// Why a file can never be claimed by this build (PLAN §A.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermanentReason {
    /// An encrypted file of the painted era, or not SQLite at all.
    NotPlainSqlite,
    /// `quick_check` did not answer ok.
    Damaged,
    /// A schema newer than this build.
    NewerSchema,
    /// This build's open (its migrations) failed on a copy.
    CannotOpen,
    /// `bir_data.db` is a symbolic link (C8): the claim never moves a link in place of a file.
    Symlink,
}

/// Why a moved file did not open (PLAN §A.3.2 step 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovedNotOpenedReason {
    /// The room file's SHA-256 is not the raw file's.
    HashMismatch,
    /// This build's open (its migrations) failed.
    Open,
    /// The fixups failed.
    Fixups,
    /// `quick_check` did not answer ok.
    QuickCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transient {
    pub cause: TransientCause,
    /// Step 3 passed its check in this claim: only then may a page say a copy is saved (M3, ruling 6).
    pub copy_saved: bool,
    /// The error came from finishing the record's own room path, not from this claimant's pre-move probe.
    pub record_room: bool,
    /// The log's detail, with no row data.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permanent {
    pub reason: PermanentReason,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedNotOpened {
    pub room: RoomId,
    pub reason: MovedNotOpenedReason,
    pub detail: String,
}

/// A failure before the move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Transient(Transient),
    Permanent(Permanent),
}

/// "this app's storage" on phones, else the device folder (PLAN §A.3.4).
pub fn storage_label(layout: &Layout, phone: bool) -> String {
    if phone {
        "this app's storage".to_owned()
    } else {
        layout.device_dir().display().to_string()
    }
}

impl Transient {
    /// The page's words (PLAN §A.3.4 and ruling 6): the copy is named only once step 3 passed its check. `folder` is
    /// [`storage_label`]'s.
    pub fn message(&self, folder: &str) -> String {
        let mut text = String::from("Your tax profiles could not be moved to this account yet. ");
        if self.copy_saved {
            text.push_str(&format!(
                "Nothing was changed: they are still where they were, and a copy is saved in {folder}/backups."
            ));
        } else {
            text.push_str("Nothing was changed: they are still where they were.");
        }
        if self.cause == TransientCause::WalReturned {
            text.push_str(
                " Another program has your tax profiles open (an older eBIRForms, a backup or antivirus tool). \
                 Close it and press Try again.",
            );
        }
        text
    }
}

impl Permanent {
    /// The note on Home (PLAN §A.3.4, ruled).
    pub fn message(&self) -> String {
        let reason = match self.reason {
            PermanentReason::NotPlainSqlite => {
                "the file is encrypted by an older version, or is not a database"
            }
            PermanentReason::Damaged => "the file is damaged",
            PermanentReason::NewerSchema => "they were saved by a newer version of eBIRForms",
            PermanentReason::CannotOpen => "this version cannot open the file",
            PermanentReason::Symlink => "the file is a link to another place",
        };
        format!(
            "Tax profiles from an earlier version of eBIRForms could not be moved ({reason}). They are still on this \
             device, unchanged. Write to support@buwiz.com."
        )
    }
}

impl MovedNotOpened {
    /// The one page this account sees (PLAN §A.3.2 step 7), with Sign out.
    pub fn message(&self, folder: &str) -> String {
        format!(
            "Your tax profiles were moved to this account, but this version could not open them. They are unchanged, \
             and a copy is saved in {folder}/backups. Write to support@buwiz.com."
        )
    }
}

// ------------------------------------------------------------------------------------------------ outcomes

/// How a claim (or its resume) ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every step ran: the file is the claimer's room, both run-time checks passed.
    Done {
        room: RoomId,
        unlocked_by: UnlockedBy,
    },
    /// The file is in the room and this build could not open it: no empty room, the zip kept, the support page.
    MovedNotOpened(MovedNotOpened),
    /// Try again: no room is opened.
    Transient(Transient),
    /// Never claimable by this build: the file left in place, an empty room, the note on Home.
    Permanent(Permanent),
}

impl From<Failure> for Outcome {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Transient(transient) => Self::Transient(transient),
            Failure::Permanent(permanent) => Self::Permanent(permanent),
        }
    }
}

/// Why the claim does not run for this account here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotHere {
    /// The store-review account: an empty room; the legacy file waits.
    Practice,
    /// Single-room mode (a named database in a debug or test run): that file is the room.
    SingleRoom,
    /// The debug "separate dev database" is chosen: the claim takes only `bir_data.db`, never while it is.
    DevDatabase,
}

/// What [`LegacyClaim::start`] found on disk for the account signed in now.
#[derive(Debug)]
pub enum Start {
    /// No legacy file, and nothing to finish.
    Nothing,
    NotHere(NotHere),
    /// This account said "Not mine": the page is not offered to it again.
    DeclinedHere,
    /// A claim cut after the move was finished from the files (steps 5–8). Its room may be another account's.
    Finished(Outcome),
    /// The legacy file holds a credential: the legacy unlock page (A3a-2), or legacy mode before it (§A.3.7).
    Locked(LockedClaim),
    /// No credential: claim straight away.
    Ready(ReadyClaim),
    /// The record says done (the file reappeared), or this account's room has data: offered, never merged (§A.3.6).
    Offer(OfferStart),
    /// The claim cannot run now (transient) or ever with this build (permanent).
    Failed(Failure),
}

// ------------------------------------------------------------------------------------------------ the claim

/// Where the claim runs: a device folder and its device store.
pub struct LegacyClaim {
    layout: Layout,
    store: DeviceStore,
    mode: Option<NotHere>,
}

/// Whether the record's owner fields name `who`. They are written with the room at unlock; an
/// older or partial record they cannot identify is treated as the claimant's, failing closed.
fn record_is_claimant(record: &Record, who: &Claimant) -> bool {
    let (Some(origin), Some(user_id)) = (&record.origin, &record.user_id) else {
        return true;
    };
    let Ok(who_origin) = crate::rooms::room_origin(&who.server_url) else {
        return true;
    };
    origin == &who_origin && user_id == &who.user_id
}

impl LegacyClaim {
    /// The claim of this process: the device folder of [`Layout::here`], refused in single-room mode and while the dev
    /// database is chosen.
    pub fn here() -> Result<Self, DeviceStoreError> {
        let mode = if crate::db::single_room_database().is_some() {
            Some(NotHere::SingleRoom)
        } else if crate::dev_mode::dev_database_path().is_some() {
            Some(NotHere::DevDatabase)
        } else {
            None
        };
        let mut claim = Self::at(Layout::here())?;
        claim.mode = mode;
        Ok(claim)
    }

    /// The claim under `layout`, as on a normal install (the harness and the tests). `device.db` is opened as
    /// [`DeviceStore::open`] does: a lost one is an error, never made again here.
    pub fn at(layout: Layout) -> Result<Self, DeviceStoreError> {
        let store = DeviceStore::open(&layout)?;
        Ok(Self {
            layout,
            store,
            mode: None,
        })
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn store(&self) -> &DeviceStore {
        &self.store
    }

    /// The claim's record, if one was written.
    pub fn record(&self) -> Result<Option<Record>, DeviceStoreError> {
        let raw = self.migration_value(RECORD_KEY)?;
        Ok(raw.and_then(|raw| match serde_json::from_str(&raw) {
            Ok(record) => Some(record),
            Err(error) => {
                tracing::warn!("legacy claim record unreadable: {error}");
                None
            }
        }))
    }

    fn write_record(&self, record: &Record) -> Result<(), DeviceStoreError> {
        let value = serde_json::to_string(record)
            .map_err(|error| DeviceStoreError::Io(std::io::Error::other(error)))?;
        self.set_migration_value(RECORD_KEY, &value)
    }

    fn delete_record(&self) -> Result<(), DeviceStoreError> {
        self.store
            .conn()
            .execute("DELETE FROM migration WHERE key = ?1", params![RECORD_KEY])?;
        Ok(())
    }

    fn migration_value(&self, key: &str) -> Result<Option<String>, DeviceStoreError> {
        Ok(self
            .store
            .conn()
            .query_row(
                "SELECT value FROM migration WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn set_migration_value(&self, key: &str, value: &str) -> Result<(), DeviceStoreError> {
        self.store.conn().execute(
            "INSERT INTO migration (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// The stored offer-consumption mark. A malformed mark fails closed: it cannot be used to
    /// prove that another account may take the file.
    fn offer_consumption(&self) -> Result<Option<OfferConsumption>, DeviceStoreError> {
        let Some(raw) = self.migration_value(OFFER_CONSUMED_KEY)? else {
            return Ok(None);
        };
        // An old-format or otherwise unknown mark is never a start failure. It protects nothing
        // (the new marks are room plus content hash), but it must not strand the sign-in either.
        Ok(serde_json::from_str(&raw).ok())
    }

    /// How `sha256` at the legacy path relates to the consumption mark for `room_id`.
    pub(crate) fn offer_consumption_match(
        &self,
        room_id: &RoomId,
        sha256: &str,
    ) -> Result<OfferConsumptionMatch, DeviceStoreError> {
        let Some(mark) = self.offer_consumption()? else {
            return Ok(OfferConsumptionMatch::None);
        };
        if mark.sha256 != sha256 {
            return Ok(OfferConsumptionMatch::None);
        }
        if mark.room == room_id.as_str() {
            Ok(OfferConsumptionMatch::Own(mark.state))
        } else {
            Ok(OfferConsumptionMatch::OtherRoom)
        }
    }

    #[cfg(test)]
    pub(crate) fn write_offer_consumption(
        &self,
        room_id: &RoomId,
        sha256: &str,
        state: OfferConsumptionState,
    ) -> Result<(), DeviceStoreError> {
        let value = serde_json::to_string(&OfferConsumption {
            room: room_id.as_str().to_owned(),
            sha256: sha256.to_owned(),
            state,
        })
        .map_err(|error| DeviceStoreError::Io(std::io::Error::other(error)))?;
        self.set_migration_value(OFFER_CONSUMED_KEY, &value)
    }

    fn not_mine_rooms(&self) -> Result<Vec<String>, DeviceStoreError> {
        Ok(self
            .migration_value(NOT_MINE_KEY)?
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default())
    }

    /// "Use this computer's login" was used once for this claim: the button is not shown again (PLAN §A.3.1).
    pub fn os_login_used(&self) -> Result<bool, DeviceStoreError> {
        Ok(self.migration_value(OS_LOGIN_USED_KEY)?.is_some())
    }

    /// Whether the computer login is offered right now: the inverse of the standing mark, as the
    /// locked page reads it. Once per unlock offer; every use is a fresh OS check (L1).
    #[cfg(test)]
    pub(crate) fn offers_computer_login_standing(&self) -> bool {
        !self.os_login_used().unwrap_or(true)
    }

    /// "Use this computer's login" is usable once per unlock offer, never once forever (PLAN §A.3.1): every use
    /// is a fresh OS check, and the used mark goes whenever the claim it was spent in ends — at Done, at Closed,
    /// in [`LegacyClaim::clear_permanent`], on any transient or permanent failure before the move — and whenever
    /// `start()` will offer an unlock (the record is not moved()), so a kill at the wrong second never strands
    /// the fallback.
    fn clear_os_login_used(&self) {
        if let Err(error) = self.store.conn().execute(
            "DELETE FROM migration WHERE key = ?1",
            params![OS_LOGIN_USED_KEY],
        ) {
            tracing::warn!("legacy claim: the computer-login-used mark was not cleared: {error}");
        }
    }

    /// Let an account that said "Not mine" be offered the page again (its own choice, from Settings). Takes the
    /// claim guard ([`ClaimRun::begin`]): the `migration` rows are never written under a running claim.
    pub fn forget_not_mine(&self, who: &Claimant) -> Result<(), DeviceStoreError> {
        let Some(_run) = ClaimRun::begin(&self.layout) else {
            return Err(DeviceStoreError::Io(std::io::Error::other(
                "another claim of this device folder is running in this process",
            )));
        };
        let room = self.claim_room(who).map_err(|error| match error {
            Failure::Transient(transient) => {
                DeviceStoreError::Io(std::io::Error::other(transient.detail))
            }
            Failure::Permanent(permanent) => {
                DeviceStoreError::Io(std::io::Error::other(permanent.detail))
            }
        })?;
        let mut rooms = self.not_mine_rooms()?;
        rooms.retain(|id| id != room.room_id.as_str());
        self.set_migration_value(
            NOT_MINE_KEY,
            &serde_json::to_string(&rooms).unwrap_or_default(),
        )
    }

    /// Allow a permanent failure to be tried again (support, or a later build with a fix): the record keeps the rest.
    /// The retry is a new claim, so the computer-login-used mark goes with the old one. Takes the claim guard
    /// ([`ClaimRun::begin`]): the record is never written under a running claim.
    pub fn clear_permanent(&self) -> Result<(), DeviceStoreError> {
        let Some(_run) = ClaimRun::begin(&self.layout) else {
            return Err(DeviceStoreError::Io(std::io::Error::other(
                "another claim of this device folder is running in this process",
            )));
        };
        if let Some(mut record) = self.record()?
            && matches!(record.failure, Some(RecordedFailure::Permanent { .. }))
        {
            record.failure = None;
            self.write_record(&record)?;
            self.clear_os_login_used();
        }
        Ok(())
    }

    fn claim_room(&self, who: &Claimant) -> Result<ClaimRoom, Failure> {
        let lookup = crate::rooms::room_for(Some(&self.store), &who.server_url, &who.user_id)
            .map_err(|_error| {
                Failure::Transient(Transient {
                    cause: TransientCause::Io,
                    copy_saved: false,
                    record_room: false,
                    detail: "the account's room could not be looked up".into(),
                })
            })?;
        let (room_id, origin) = match lookup {
            RoomLookup::Known(record) => (record.room_id, record.origin),
            RoomLookup::New { room_id, origin } => (room_id, origin),
        };
        Ok(ClaimRoom {
            room_id,
            origin,
            user_id: who.user_id.clone(),
            email: who.email.clone(),
        })
    }

    /// Decide from the files what the claim does for the account signed in now (PLAN §A.3.3), and finish a claim that
    /// was cut after the move. Before the unlock no record names a room, and nothing is written beside the legacy file
    /// but its `.owner.lock` (made by any earlier version too, and by the holder here).
    pub fn start(&self, who: &Claimant) -> Start {
        let record_before = self.record().ok().flatten();
        let inner = self.start_inner(who);
        // R1(a): the folder goes into the process-wide set only when start() has a definitive answer. Never on
        // a transient — in M1's own case (the record at Copied, the legacy file gone, the room read failing) an
        // open room would migrate its bytes, and the next start() would close the move unfinished. A permanent
        // failure is definitive: the claim is over, and the account opens its empty room.
        let foreign_unfinished = record_before.as_ref().is_some_and(|record| {
            !matches!(record.step, Step::Done | Step::Closed) && !record_is_claimant(record, who)
        });
        let definitive = match &inner {
            // The ruling's item 6: a finish that lands on a transient after the move (the record at
            // Moved or RoomRow) is no definitive answer for the claimer — the move is still
            // unfinished, and the guards' step check is what stands between it and that room open.
            // When the record names somebody else, though, it is definitive for the account asking
            // now: nothing of theirs is unfinished, and the evidence rule still guards the claimer's
            // own room by name. The same holds when finishing that record failed transiently:
            // a non-claimant never loops on another account's move.
            Ok(Start::Finished(Outcome::Transient(transient))) => {
                foreign_unfinished && transient.record_room
            }
            Err(Failure::Transient(transient)) => foreign_unfinished && transient.record_room,
            Ok(_) | Err(Failure::Permanent(_)) => true,
        };
        if definitive {
            started_insert(self.layout.device_dir());
        }
        match inner {
            Ok(start) => start,
            Err(failure) => Start::Failed(match failure {
                Failure::Transient(mut transient) => {
                    if foreign_unfinished {
                        // N4: the non-claimant is told nothing about the other account's data. The
                        // session normally opens its own room instead of showing even this; the
                        // sanitising is the belt under that.
                        transient.copy_saved = false;
                        transient.detail = "the account could not be opened just now".to_owned();
                    }
                    Failure::Transient(transient)
                }
                failure => failure,
            }),
        }
    }

    fn start_inner(&self, who: &Claimant) -> Result<Start, Failure> {
        // One claim of this device folder at a time in this process (C4), from here to the token's end.
        let run = ClaimRun::begin(&self.layout).ok_or_else(|| {
            Failure::Transient(Transient {
                cause: TransientCause::ClaimInProgress,
                copy_saved: false,
                record_room: false,
                detail: "another claim of this device folder is running in this process".into(),
            })
        })?;
        let legacy = self.layout.legacy_database();
        let record = self.record().map_err(store_failure)?;
        // L1, the orchestrator's ruling: the mark protects nothing and can strand a person, so it clears
        // whenever start() will offer an unlock — whenever the record is not moved(). A kill during the copy
        // (record at Unlocked) or before the move (record at Copied, the legacy file present) needs a new
        // unlock, and the person must never lose their fallback because the app was closed at the wrong
        // second. Within one live claim — no start() between the uses — the mark stands: once per unlock
        // offer, and every use is a fresh OS check.
        if record.as_ref().is_none_or(|record| !record.moved()) {
            self.clear_os_login_used();
        }
        let legacy_present = files::exists(&legacy).map_err(io_failure)?;

        // The rows the record's room decides: moved, and the link window (N2). They finish whoever is signed in now:
        // the file is already in the claimer's own room folder. Only a claim that moved its file, or whose room file
        // is provably the legacy file, finishes (C1): a Closed or Done record never does, and neither does a record cut
        // before its move whose room has since got a file of its own.
        if let Some(record) = &record
            && !matches!(record.step, Step::Done | Step::Closed)
            && let Some(room) = record.claim_room()
        {
            let room_file = self.layout.room_database(&room.room_id);
            if files::exists(&room_file).map_err(|error| record_room_failure(io_failure(error)))? {
                if legacy_present
                    && self
                        .same_file(&legacy, &room_file, record)
                        .map_err(|error| record_room_failure(io_failure(error)))?
                {
                    // Linked, not yet unlinked: finish step 4's unlink, then steps 5–8. Under the legacy lock and with
                    // no -wal or -shm beside the file (C2): an older version or bir-headless may have opened it since
                    // the cut, and its log would be left behind, or read twice into one file.
                    let lock = hold_legacy_lock(&legacy).map_err(record_room_failure)?;
                    if files::sidecars_present(&legacy)
                        .map_err(|error| record_room_failure(io_failure(error)))?
                    {
                        return Err(Failure::Transient(Transient {
                            cause: TransientCause::WalReturned,
                            copy_saved: true,
                            record_room: true,
                            detail: "a -wal or -shm is beside the legacy file in the link window"
                                .into(),
                        }));
                    }
                    files::remove_and_sync(&legacy, self.layout.device_dir())
                        .map_err(|error| record_room_failure(io_failure(error)))?;
                    drop(lock);
                    let mut record = record.clone();
                    record.step = record.step.max(Step::Moved);
                    self.write_record(&record)
                        .map_err(|error| record_room_failure(store_failure(error)))?;
                    return Ok(Start::Finished(record_room_outcome(
                        self.finish_after_move(&room, record),
                    )));
                }
                let moved_here = record.moved()
                    || (!legacy_present
                        && match self.room_is_the_legacy(record, &room_file) {
                            Ok(the_same) => the_same,
                            Err(error) => {
                                tracing::warn!(
                                    step = ?record.step,
                                    %error,
                                    "legacy claim: the room file could not be read; transient, never a close"
                                );
                                return Err(record_room_failure(io_failure(error)));
                            }
                        });
                if moved_here {
                    let outcome =
                        record_room_outcome(self.finish_after_move(&room, record.clone()));
                    if !legacy_present || !matches!(outcome, Outcome::Done { .. }) {
                        return Ok(Start::Finished(outcome));
                    }
                    // Done now, and a legacy file is there again: it is offered below.
                }
            }
        }
        let record = self.record().map_err(store_failure)?;

        if !legacy_present {
            if let Some(mut record) = record
                && !matches!(record.step, Step::Done | Step::Closed)
                && !matches!(record.failure, Some(RecordedFailure::MovedNotOpened { .. }))
            {
                // No legacy file, and no room file that is it: nothing to claim. Kept for support, never deleted.
                // The claim is over: the computer login is the next claim's again.
                tracing::warn!(step = ?record.step, "legacy claim closed: the legacy file is gone and no room file is it");
                record.step = Step::Closed;
                self.write_record(&record).map_err(store_failure)?;
                self.clear_os_login_used();
            }
            return Ok(Start::Nothing);
        }

        if let Some(mode) = self.mode {
            return Ok(Start::NotHere(mode));
        }
        if who.practice {
            return Ok(Start::NotHere(NotHere::Practice));
        }
        if let Some(Record {
            failure:
                Some(RecordedFailure::Permanent {
                    reason,
                    detail,
                    build,
                    ..
                }),
            ..
        }) = &record
            && build == BUILD
        {
            return Ok(Start::Failed(Failure::Permanent(Permanent {
                reason: *reason,
                detail: detail.clone(),
            })));
        }
        let room = self.claim_room(who)?;
        if self
            .not_mine_rooms()
            .map_err(store_failure)?
            .iter()
            .any(|id| id == room.room_id.as_str())
        {
            return Ok(Start::DeclinedHere);
        }

        // A file seen while the record says moved or done reappeared; a room with data never takes it by a rename.
        let mut offer = record.as_ref().is_some_and(Record::moved);
        let room_file = self.layout.room_database(&room.room_id);
        let mut empty_room_aside = false;
        if !offer && files::exists(&room_file).map_err(io_failure)? {
            if files::room_is_empty(&room_file) {
                empty_room_aside = true;
            } else {
                offer = true;
            }
        }

        // B2b: consumption is keyed by content and room, never inode. A file already consumed into
        // another room is nobody else's claim or offer; consumed into this room it stays this
        // room's offer so the cut-off rename can finish. This also closes the race in which an
        // import commits after Ready was returned but before ReadyClaim::claim moves the file.
        let content_sha256 = files::sha256_file(&legacy).map_err(io_failure)?;
        // A file A already moved is marked by the claim record's room and raw content hash. A
        // byte-for-byte reappearance is therefore nothing for every other account: never Ready,
        // never offered, even when no Add-consumption mark exists.
        if let Some(record) = &record
            && record.moved()
            && record.raw_sha256.as_deref() == Some(content_sha256.as_str())
            && record.room.as_deref() != Some(room.room_id.as_str())
        {
            return Ok(Start::Nothing);
        }
        match self
            .offer_consumption_match(&room.room_id, &content_sha256)
            .map_err(store_failure)?
        {
            OfferConsumptionMatch::OtherRoom => return Ok(Start::Nothing),
            OfferConsumptionMatch::Own(_) => offer = true,
            OfferConsumptionMatch::None => {}
        }

        let lock = hold_legacy_lock(&legacy)?;
        let probe = match probe::probe(&self.layout, &legacy) {
            Ok(probe) => probe,
            Err(Failure::Permanent(permanent)) => {
                self.record_permanent(&permanent).map_err(store_failure)?;
                return Err(Failure::Permanent(permanent));
            }
            Err(failure) => return Err(failure),
        };
        let gate = Gate {
            lock,
            probe,
            room,
            _run: run,
        };
        Ok(if offer {
            Start::Offer(if gate.probe.locked() {
                OfferStart::Locked(LockedOffer { gate })
            } else {
                OfferStart::Open(Offer { gate })
            })
        } else if gate.probe.locked() {
            Start::Locked(LockedClaim {
                gate,
                empty_room_aside,
            })
        } else {
            Start::Ready(ReadyClaim {
                gate,
                empty_room_aside,
                unlocked_by: UnlockedBy::NoLock,
                owner_pin_phc: None,
            })
        })
    }

    /// Whether `legacy` and `room_file` are one file: equal identity (device and inode; the Windows volume serial and
    /// file index), else, where no identity can be read, equal `raw_sha256` (N2).
    fn same_file(&self, legacy: &Path, room_file: &Path, record: &Record) -> std::io::Result<bool> {
        match (
            crate::safe_fs::file_id(legacy),
            crate::safe_fs::file_id(room_file),
        ) {
            (Ok(a), Ok(b)) => Ok(a == b),
            _ => Ok(match &record.raw_sha256 {
                Some(raw) => {
                    files::sha256_file(legacy)? == *raw && files::sha256_file(room_file)? == *raw
                }
                None => false,
            }),
        }
    }

    /// Whether `room_file` is the legacy file moved (C1). ext4 and f2fs reuse freed inode numbers (N1): a deleted
    /// legacy file's inode can land on the account's own new room file, so the recorded identity alone never decides.
    /// Required, all three: the record is cut at Copied (the only step the move can be done but unrecorded at — before
    /// it there is no copy, after it the record says moved), an identity match (a link or a rename keeps it), and the
    /// room file byte for byte the raw file: before step 6 the room is byte-equal to the raw file, so this never
    /// rejects a real move, and a reused inode with any other content never passes.
    fn room_is_the_legacy(&self, record: &Record, room_file: &Path) -> std::io::Result<bool> {
        #[cfg(test)]
        if ROOM_CHECK_ERR.with(std::cell::Cell::get) {
            ROOM_CHECK_ERR.with(|err| err.set(false));
            return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        }
        // Closed only on a definite mismatch (M1): the step is not Copied, the identity differs, or the hash
        // differs. A read error (an EIO, a sharing violation, an antivirus filter) is not a mismatch: it is
        // transient, or the move would be stranded for good with no room row and no step 6.
        if record.step != Step::Copied {
            return Ok(false);
        }
        let identity_matches = match (record.legacy_file, crate::safe_fs::file_id(room_file)) {
            (Some(recorded), Ok(id)) => recorded == id,
            (None, _) => false,
            (_, Err(error)) => return Err(error),
        };
        #[cfg(test)]
        let identity_matches = identity_matches || SAME_ID.with(std::cell::Cell::get);
        if !identity_matches {
            return Ok(false);
        }
        match record.raw_sha256.as_ref() {
            None => Ok(false),
            Some(raw) => Ok(files::sha256_file(room_file)? == *raw),
        }
    }

    /// Record a permanent failure. Every one of these is before the move (the probe, the copy), so the claim is
    /// over: a pre-move resume needs a new unlock, and the computer login must not stay spent — the person is never
    /// left without their fallback.
    fn record_permanent(&self, permanent: &Permanent) -> Result<(), DeviceStoreError> {
        let mut record = self.record()?.unwrap_or_else(new_record);
        tracing::warn!(reason = ?permanent.reason, "legacy claim: permanent failure: {}", permanent.detail);
        record.failure = Some(RecordedFailure::Permanent {
            reason: permanent.reason,
            detail: permanent.detail.clone(),
            at: now_text(),
            build: BUILD.to_owned(),
        });
        let written = self.write_record(&record);
        if written.is_ok() {
            self.clear_os_login_used();
        }
        written
    }

    fn record_transient(&self, transient: &Transient, step: Step) {
        tracing::warn!(cause = ?transient.cause, ?step, "legacy claim: transient failure: {}", transient.detail);
        let result = (|| {
            let mut record = self.record()?.unwrap_or_else(new_record);
            record.failure = Some(RecordedFailure::Transient {
                cause: transient.cause,
                step,
                at: now_text(),
            });
            self.write_record(&record)
        })();
        if let Err(error) = result {
            tracing::warn!("legacy claim: transient failure not recorded: {error}");
        }
    }

    // -------------------------------------------------------------------------------------------- steps 5 to 8

    /// Steps 5–8 for a file already in `room`'s folder. Idempotent: each run picks up where the record says, and the
    /// hash check runs only before step 6 has changed the file.
    fn finish_after_move(&self, room: &ClaimRoom, mut record: Record) -> Outcome {
        let room_file = self.layout.room_database(&room.room_id);
        if let Some(RecordedFailure::MovedNotOpened {
            reason,
            detail,
            build,
            ..
        }) = &record.failure
            && (build == BUILD || *reason == MovedNotOpenedReason::HashMismatch)
        {
            // A later build may open it; this one already could not, and a changed file needs support.
            return Outcome::MovedNotOpened(MovedNotOpened {
                room: room.room_id.clone(),
                reason: *reason,
                detail: detail.clone(),
            });
        }
        record.failure = None;
        record.step = record.step.max(Step::Moved);

        // 5. The room row.
        if let Err(error) = self.write_room_row(room) {
            tracing::warn!("legacy claim room row not written: {error}");
            return self.transient_after_move(
                &mut record,
                TransientCause::Io,
                "the room cannot be recorded".into(),
            );
        }
        record.step = record.step.max(Step::RoomRow);
        if let Err(_error) = self.write_record(&record) {
            return self.transient_after_move(
                &mut record,
                TransientCause::Io,
                "the claim record cannot be written".into(),
            );
        }
        cut(Cut::AfterRoomRow);

        // 7, first half: the hash, before step 6 changes the file.
        if !record.hash_checked {
            let hash = match files::sha256_file(&room_file) {
                Ok(hash) => hash,
                Err(error) => {
                    tracing::warn!("legacy claim room hash not read: {error}");
                    return self.transient_after_move(
                        &mut record,
                        io_cause(&error),
                        "the room copy cannot be checked".into(),
                    );
                }
            };
            if record.raw_sha256.as_deref() != Some(hash.as_str()) {
                return self.moved_not_opened(
                    &mut record,
                    room,
                    MovedNotOpenedReason::HashMismatch,
                    "the room file is not the file that was copied".into(),
                );
            }
            record.hash_checked = true;
            record.step = record.step.max(Step::HashChecked);
            if let Err(_error) = self.write_record(&record) {
                return self.transient_after_move(
                    &mut record,
                    TransientCause::Io,
                    "the claim record cannot be written".into(),
                );
            }
        }
        cut(Cut::AfterHashChecked);

        // 6. The fixups, then 7's open check.
        let room_lock = match crate::db::hold_owner_lock(&room_file) {
            Ok(lock) => lock,
            Err(_error) => {
                return self.transient_after_move(
                    &mut record,
                    TransientCause::OwnerLockHeld,
                    "the room database is open elsewhere".into(),
                );
            }
        };
        // Run again whenever the open check is not recorded yet (C6): the room file is at synchronous = NORMAL, so a
        // power cut can lose fix-ups the record (at FULL) already calls done. Every fix-up is idempotent.
        if record.step < Step::OpenChecked {
            match self.fixups(&room_file, room, &record) {
                Ok(()) => {}
                Err(StepFailure::Passing(cause, detail)) => {
                    return self.transient_after_move(&mut record, cause, detail);
                }
                Err(StepFailure::Moved(reason, detail)) => {
                    return self.moved_not_opened(&mut record, room, reason, detail);
                }
            }
            record.step = record.step.max(Step::Fixed);
            if let Err(_error) = self.write_record(&record) {
                return self.transient_after_move(
                    &mut record,
                    TransientCause::Io,
                    "the claim record cannot be written".into(),
                );
            }
        }
        cut(Cut::AfterFixups);
        match open_check(&room_file) {
            Ok(()) => {}
            Err(StepFailure::Passing(cause, detail)) => {
                return self.transient_after_move(&mut record, cause, detail);
            }
            Err(StepFailure::Moved(reason, detail)) => {
                return self.moved_not_opened(&mut record, room, reason, detail);
            }
        }
        drop(room_lock);
        record.open_checked = true;
        record.step = Step::OpenChecked;
        if let Err(_error) = self.write_record(&record) {
            return self.transient_after_move(
                &mut record,
                TransientCause::Io,
                "the claim record cannot be written".into(),
            );
        }
        cut(Cut::AfterOpenChecked);

        // 8. Done. The accounts that said "Not mine" are forgotten: there is no legacy file left to say it to.
        record.step = Step::Done;
        record.done_at = Some(now_text());
        record.owner_pin_phc = None;
        if let Err(_error) = self.write_record(&record) {
            return self.transient_after_move(
                &mut record,
                TransientCause::Io,
                "the claim record cannot be written".into(),
            );
        }
        let _ = self.set_migration_value(NOT_MINE_KEY, "[]");
        // The claim is done: the computer login is the next claim's again, never spent forever.
        self.clear_os_login_used();
        tracing::info!(room = %room.room_id, "legacy claim done");
        Outcome::Done {
            room: room.room_id.clone(),
            unlocked_by: record.unlocked_by.unwrap_or(UnlockedBy::NoLock),
        }
    }

    fn write_room_row(&self, room: &ClaimRoom) -> Result<(), DeviceStoreError> {
        match self.store.find_room(&room.origin, &room.user_id)? {
            Some(existing) if existing.room_id == room.room_id => {
                self.store.conn().execute(
                    "UPDATE rooms SET from_legacy = 1 WHERE room_id = ?1",
                    params![room.room_id.as_str()],
                )?;
                Ok(())
            }
            Some(existing) => Err(DeviceStoreError::Io(std::io::Error::other(format!(
                "the account already has the room {}",
                existing.room_id
            )))),
            None => self.store.add_room(&RoomRecord {
                room_id: room.room_id.clone(),
                origin: room.origin.clone(),
                user_id: room.user_id.clone(),
                email: room.email.clone(),
                created_at: now_text(),
                from_legacy: true,
            }),
        }
    }

    /// Step 6 (PLAN §A.3.2): the room file in one transaction after this build's open, then `device.db`. Each write is
    /// idempotent.
    fn fixups(
        &self,
        room_file: &Path,
        room: &ClaimRoom,
        record: &Record,
    ) -> Result<(), StepFailure> {
        if let Some(error) = injected(Inject::Open) {
            return Err(db_failure(
                MovedNotOpenedReason::Open,
                crate::db::DbError::Io(error),
            ));
        }
        let db = Database::open(room_file)
            .map_err(|error| db_failure(MovedNotOpenedReason::Open, error))?;
        let fixed = (|| -> Result<Vec<(String, String)>, crate::db::DbError> {
            #[cfg(test)]
            if injected(Inject::Fixups).is_some() {
                return Err(sqlite_full());
            }
            let transaction = db.conn.unchecked_transaction()?;
            transaction.execute(
                "DELETE FROM settings WHERE key = ?1",
                params![OBSOLETE_SERVER_URL_SETTING],
            )?;
            let mode: Option<String> = transaction
                .query_row(
                    "SELECT value FROM settings WHERE key = 'access_mode'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            let hand_to_clients = if mode.as_deref() == Some("helpdesk") {
                "on"
            } else {
                "off"
            };
            transaction.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
                params![HAND_TO_CLIENTS_SETTING, hand_to_clients],
            )?;
            let mut device_values = Vec::new();
            for key in DEVICE_SETTING_KEYS {
                if let Some(value) = transaction
                    .query_row(
                        "SELECT value FROM settings WHERE key = ?1",
                        params![key],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                {
                    device_values.push((key.to_owned(), value));
                }
            }
            transaction.commit()?;
            Ok(device_values)
        })()
        .map_err(|error| db_failure(MovedNotOpenedReason::Fixups, error))?;
        drop(db);

        let device = (|| -> Result<(), DeviceStoreError> {
            #[cfg(feature = "test-support")]
            if FAIL_DEVICE_ROWS_FOR_TEST.swap(false, std::sync::atomic::Ordering::SeqCst) {
                return Err(DeviceStoreError::Io(std::io::Error::from(
                    std::io::ErrorKind::StorageFull,
                )));
            }
            if let Some(error) = injected(Inject::DeviceRows) {
                return Err(DeviceStoreError::Io(error));
            }
            let conn = self.store.conn();
            for (key, value) in &fixed {
                conn.execute(
                    "INSERT INTO device_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
                    params![key, value],
                )?;
            }
            conn.execute(
                "INSERT INTO owner_lock (room_id, pin_phc, set_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(room_id) DO NOTHING",
                params![
                    room.room_id.as_str(),
                    record_pin_phc(record),
                    record_pin_phc(record).map(|_| now_text())
                ],
            )?;
            Ok(())
        })();
        // The device store's rows are not the room file's: any failure there is passing.
        device.map_err(|error| {
            let cause = match &error {
                DeviceStoreError::Io(io) => io_cause(io),
                DeviceStoreError::Sqlite(sqlite) => {
                    sqlite_cause(sqlite).unwrap_or(TransientCause::Io)
                }
                _ => TransientCause::Io,
            };
            StepFailure::Passing(cause, "the device store cannot be used".into())
        })
    }

    fn moved_not_opened(
        &self,
        record: &mut Record,
        room: &ClaimRoom,
        reason: MovedNotOpenedReason,
        detail: String,
    ) -> Outcome {
        tracing::warn!(?reason, room = %room.room_id, "legacy claim: moved, not opened: {detail}");
        record.failure = Some(RecordedFailure::MovedNotOpened {
            reason,
            detail: detail.clone(),
            at: now_text(),
            build: BUILD.to_owned(),
        });
        if let Err(error) = self.write_record(record) {
            tracing::warn!("legacy claim: moved-not-opened not recorded: {error}");
        }
        Outcome::MovedNotOpened(MovedNotOpened {
            room: room.room_id.clone(),
            reason,
            detail,
        })
    }

    fn transient_after_move(
        &self,
        record: &mut Record,
        cause: TransientCause,
        detail: String,
    ) -> Outcome {
        let transient = Transient {
            cause,
            copy_saved: true,
            record_room: false,
            detail,
        };
        self.record_transient(&transient, record.step);
        Outcome::Transient(transient)
    }

    // -------------------------------------------------------------------------------------------- launches and the zip

    /// At every launch (A3a's call): sweep the scratch folder (a probe cut short leaves no copy) and stray `.partial`
    /// zips.
    pub fn sweep(&self) {
        // Never under a running claim's feet (C4): its scratch copy and `.partial` zip are live.
        if let Some(_run) = ClaimRun::begin(&self.layout) {
            files::sweep_scratch(&self.layout);
        }
    }

    /// Count `today` (the local calendar day) as a launch day, once the claim is done (PLAN §A.3.8).
    pub fn note_launch(&self, today: NaiveDate) -> Result<(), DeviceStoreError> {
        let Some(mut record) = self.record()? else {
            return Ok(());
        };
        if record.step != Step::Done {
            return Ok(());
        }
        let day = today.format("%Y-%m-%d").to_string();
        let done_day = record
            .done_at
            .as_deref()
            .and_then(|done| DateTime::parse_from_rfc3339(done).ok())
            .map(|done| done.with_timezone(&chrono::Local).date_naive());
        if done_day == Some(today) || record.launch_days.contains(&day) {
            return Ok(());
        }
        record.launch_days.push(day);
        record.launch_days.sort();
        self.write_record(&record)
    }

    /// The claimed room was opened (A3a's room open).
    pub fn note_room_opened(&self, room: &RoomId) -> Result<(), DeviceStoreError> {
        let Some(mut record) = self.record()? else {
            return Ok(());
        };
        if record.step != Step::Done
            || record.room.as_deref() != Some(room.as_str())
            || record.room_opened_since
        {
            return Ok(());
        }
        record.room_opened_since = true;
        self.write_record(&record)
    }

    /// Whether the pre-rooms zip may go now (PLAN §A.3.8): 30 days by the wall clock since done, launches on 5
    /// distinct days, both run-time checks recorded, no failure state (no failure in the record, no legacy file waiting
    /// as an offer), and the room opened since. `Err` says why it is kept.
    pub fn zip_due(&self, now: DateTime<Utc>) -> Result<Vec<PathBuf>, ZipKept> {
        let record = self.record().ok().flatten().ok_or(ZipKept::NoClaim)?;
        if record.zip_deleted_at.is_some() {
            return Err(ZipKept::AlreadyDeleted);
        }
        if record.step != Step::Done {
            return Err(ZipKept::NotDone);
        }
        if record.failure.is_some() {
            return Err(ZipKept::FailureState);
        }
        if !(record.hash_checked && record.open_checked) {
            return Err(ZipKept::ChecksMissing);
        }
        if files::exists(&self.layout.legacy_database()).unwrap_or(true) {
            return Err(ZipKept::OfferWaiting);
        }
        let done = record
            .done_at
            .as_deref()
            .and_then(|done| DateTime::parse_from_rfc3339(done).ok())
            .ok_or(ZipKept::NotDone)?;
        if now.signed_duration_since(done) < chrono::Duration::days(ZIP_KEEP_DAYS) {
            return Err(ZipKept::TooSoon {
                until: done.with_timezone(&Utc) + chrono::Duration::days(ZIP_KEEP_DAYS),
            });
        }
        if record.launch_days.len() < ZIP_KEEP_LAUNCH_DAYS {
            return Err(ZipKept::TooFewLaunchDays(record.launch_days.len()));
        }
        if !record.room_opened_since {
            return Err(ZipKept::RoomNotOpened);
        }
        Ok(files::pre_rooms_zips(&self.layout))
    }

    /// Delete the pre-rooms zips when [`LegacyClaim::zip_due`] says so; the deletion is logged and recorded.
    pub fn delete_zip_if_due(&self, now: DateTime<Utc>) -> Result<Vec<PathBuf>, ZipKept> {
        let zips = self.zip_due(now)?;
        let mut deleted = Vec::new();
        for zip in zips {
            match std::fs::remove_file(&zip) {
                Ok(()) => {
                    tracing::info!(zip = %zip.display(), "pre-rooms safety copy deleted (30 days, 5 launch days, both checks)");
                    deleted.push(zip);
                }
                Err(error) => {
                    tracing::warn!(zip = %zip.display(), "pre-rooms safety copy not deleted: {error}")
                }
            }
        }
        let _ = crate::safe_fs::sync_dir(&self.layout.backups_dir());
        if let Ok(Some(mut record)) = self.record() {
            record.zip_deleted_at = Some(now.to_rfc3339());
            let _ = self.write_record(&record);
        }
        Ok(deleted)
    }
}

/// Why the pre-rooms zip is kept (PLAN §A.3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipKept {
    NoClaim,
    AlreadyDeleted,
    NotDone,
    FailureState,
    ChecksMissing,
    OfferWaiting,
    TooSoon { until: DateTime<Utc> },
    TooFewLaunchDays(usize),
    RoomNotOpened,
}

// ------------------------------------------------------------------------------------------------ the tokens

/// What a claim or an offer holds from its start to its end: the legacy owner lock (M4), the probe, the account.
struct Gate {
    lock: OwnerLock,
    probe: probe::Probe,
    room: ClaimRoom,
    /// This process's claim of the device folder (C4), released with the token.
    _run: ClaimRun,
}

/// The device folders with a claim running in this process (C4). The legacy owner lock is re-entrant within a
/// process ([`crate::db::hold_owner_lock`]), so it keeps out only other processes; this keeps two runs of one process
/// (the launch resume and the sign-in call) from overlapping: their `.partial` zips, scratch copies and record writes
/// would otherwise race. A3a closes its own connection to the legacy file before it starts a claim.
static RUNNING: std::sync::Mutex<std::collections::BTreeSet<PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// One running claim of a device folder; the folder is free again when it is dropped (also through an unwind).
struct ClaimRun(PathBuf);

impl ClaimRun {
    /// `None` while another claim of the same device folder runs in this process.
    fn begin(layout: &Layout) -> Option<Self> {
        let key = layout
            .device_dir()
            .canonicalize()
            .unwrap_or_else(|_| layout.device_dir().to_path_buf());
        let mut running = RUNNING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Never build the guard on a refusal: dropping it would lock `RUNNING` again while it is held here.
        if running.insert(key.clone()) {
            Some(Self(key))
        } else {
            None
        }
    }
}

impl Drop for ClaimRun {
    fn drop(&mut self) {
        RUNNING
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.0);
    }
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("room", &self.room.room_id)
            .field("kinds", &self.probe.kinds())
            .finish_non_exhaustive()
    }
}

impl Gate {
    /// Check a credential against the probe's values (PLAN §A.3.1). A client taxpayer's PIN never opens the file.
    /// One try at the legacy unlock under B1's persisted limit (C5): the PIN, the authenticator code and the
    /// recovery key each count in `pin_attempts('', 'legacy')`, in B1's order (refuse during a wait, check, commit,
    /// then answer), so no caller can try without limit. The check reads only the probe's values in memory; nothing
    /// inside it takes a store or another lock (B1's lock-order rule). "Use this computer's login" is the operating
    /// system's own check, with its own limit, and once per unlock offer: every use is a fresh OS check, and it is
    /// not counted here. The once is cleared when the claim ends (at Done, at Closed, in `clear_permanent`, on any
    /// failure before the move) and whenever `start()` will offer an unlock again, never spent forever.
    fn try_credential(
        &self,
        claim: &LegacyClaim,
        credential: &LegacyCredential<'_>,
    ) -> Result<UnlockedBy, RefusedReason> {
        if matches!(credential, LegacyCredential::ComputerLogin) {
            return self
                .check(claim, credential)
                .ok_or(RefusedReason::Wrong { wait_started: None });
        }
        let store = crate::rooms::DeviceStoreRef::File(claim.layout.clone());
        let mut unlocked_by = None;
        let attempt = crate::owner_lock::guarded(
            &store,
            &crate::owner_lock::AttemptKey::legacy(),
            &crate::boot_clock::read(),
            || {
                unlocked_by = self.check(claim, credential);
                Ok::<bool, std::convert::Infallible>(unlocked_by.is_some())
            },
        );
        match attempt {
            Ok(crate::owner_lock::Attempt::Matched) => {
                unlocked_by.ok_or(RefusedReason::Wrong { wait_started: None })
            }
            Ok(crate::owner_lock::Attempt::Missed { wait }) => {
                Err(RefusedReason::Wrong { wait_started: wait })
            }
            Ok(crate::owner_lock::Attempt::Waiting(wait)) => Err(RefusedReason::Waiting(wait)),
            Err(crate::owner_lock::GuardError::Store(error)) => {
                Err(RefusedReason::Unanswered(error.to_string()))
            }
            Err(crate::owner_lock::GuardError::Check(never)) => match never {},
        }
    }

    fn check(&self, claim: &LegacyClaim, credential: &LegacyCredential<'_>) -> Option<UnlockedBy> {
        match credential {
            LegacyCredential::Pin(pin) => self.probe.pin_matches(pin).then_some(UnlockedBy::Pin),
            LegacyCredential::AuthenticatorCode(code) => self
                .probe
                .code_matches(code)
                .then_some(UnlockedBy::Authenticator),
            LegacyCredential::RecoveryKey(key) => self
                .probe
                .recovery_key_matches(key)
                .then_some(UnlockedBy::RecoveryKey),
            LegacyCredential::ComputerLogin => {
                let unused = !claim.os_login_used().unwrap_or(true);
                if unused
                    && claim
                        .set_migration_value(OS_LOGIN_USED_KEY, &now_text())
                        .is_ok()
                {
                    Some(UnlockedBy::ComputerLogin)
                } else {
                    None
                }
            }
        }
    }

    /// "Not mine" (ruled): nothing was written beside the legacy file, the lock goes, and this account is not offered
    /// the page again. The record goes too (it names no room of this round).
    fn not_mine(self, claim: &LegacyClaim) -> Result<(), DeviceStoreError> {
        let mut rooms = claim.not_mine_rooms()?;
        if !rooms.iter().any(|id| id == self.room.room_id.as_str()) {
            rooms.push(self.room.room_id.as_str().to_owned());
        }
        claim.set_migration_value(
            NOT_MINE_KEY,
            &serde_json::to_string(&rooms).unwrap_or_default(),
        )?;
        if claim.record()?.is_some_and(|record| !record.moved()) {
            claim.delete_record()?;
        }
        drop(self.lock);
        Ok(())
    }
}

/// The legacy file holds a credential: the legacy unlock page (A3a-2) asks for it.
#[derive(Debug)]
pub struct LockedClaim {
    gate: Gate,
    empty_room_aside: bool,
}

/// A try that did not open the file: why ([`Refused::reason`]), and the page back for the next try
/// ([`Refused::into_inner`]). Every try was counted by the limiter (C5), so handing the page back opens nothing.
#[derive(Debug)]
pub struct Refused<T> {
    page: Box<T>,
    reason: RefusedReason,
}

/// Why a try at the legacy unlock did not open the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusedReason {
    /// Not the old credential; `wait_started`: the wait this miss began (after every 6th in a row).
    Wrong {
        wait_started: Option<std::time::Duration>,
    },
    /// A wait is running: nothing was checked or counted.
    Waiting(std::time::Duration),
    /// The limit could not be read or the result committed: no answer (try again).
    Unanswered(String),
}

impl<T> Refused<T> {
    pub fn reason(&self) -> &RefusedReason {
        &self.reason
    }

    pub fn into_inner(self) -> T {
        *self.page
    }
}

impl LockedClaim {
    /// Which ways in the file has: the page offers only these.
    pub fn kinds(&self) -> CredentialKinds {
        self.gate.probe.kinds()
    }

    /// Offer "Use this computer's login" (A3a-2): only until it was used once for this unlock offer. Every use
    /// is a fresh OS check: the used mark is cleared when the claim ends — at Done, at Closed, in
    /// `clear_permanent`, on any failure before the move — and whenever `start()` will offer an unlock again, so
    /// it is offered again in the next claim and after any kill.
    pub fn offers_computer_login(&self, claim: &LegacyClaim) -> bool {
        !claim.os_login_used().unwrap_or(true)
    }

    /// The old credential opens it: the claim may run, for this account only (N3).
    pub fn unlock(
        self,
        claim: &LegacyClaim,
        credential: LegacyCredential<'_>,
    ) -> Result<ReadyClaim, Refused<Self>> {
        match self.gate.try_credential(claim, &credential) {
            Ok(unlocked_by) => Ok(ReadyClaim {
                gate: self.gate,
                empty_room_aside: self.empty_room_aside,
                unlocked_by,
                owner_pin_phc: None,
            }),
            Err(reason) => Err(Refused {
                page: Box::new(self),
                reason,
            }),
        }
    }

    /// "Not mine": the file stays exactly as it is; this account gets an empty room.
    pub fn not_mine(self, claim: &LegacyClaim) -> Result<(), DeviceStoreError> {
        self.gate.not_mine(claim)
    }
}

/// The claim may run: no credential, or the old one passed.
#[derive(Debug)]
pub struct ReadyClaim {
    gate: Gate,
    empty_room_aside: bool,
    unlocked_by: UnlockedBy,
    owner_pin_phc: Option<PinPhc>,
}

impl ReadyClaim {
    pub fn unlocked_by(&self) -> UnlockedBy {
        self.unlocked_by
    }

    pub fn room(&self) -> &RoomId {
        &self.gate.room.room_id
    }

    /// The owner PIN step 6 writes: the PHC string of the PIN typed at the legacy unlock, hashed by the owner-PIN store
    /// (§B.1). Without one, `owner_lock.pin_phc` stays empty and Set PIN follows.
    pub fn with_owner_pin_phc(mut self, phc: String) -> Self {
        self.owner_pin_phc = Some(PinPhc(phc));
        self
    }

    /// Run the claim: steps 2 (its record), 3, 4, then 5–8.
    pub fn claim(self, claim: &LegacyClaim) -> Outcome {
        let ReadyClaim {
            gate,
            empty_room_aside,
            unlocked_by,
            owner_pin_phc,
        } = self;
        let Gate {
            lock,
            probe,
            room,
            _run: run,
        } = gate;
        let legacy = claim.layout.legacy_database();

        // The file is the one probed (the lock was held since).
        match crate::safe_fs::file_id(&legacy) {
            Ok(id) if Some(id) == probe.identity() => {}
            Ok(_) | Err(_) => {
                return claim.failed(
                    Transient {
                        cause: TransientCause::FileChanged,
                        copy_saved: false,
                        record_room: false,
                        detail: "the legacy file is not the one probed".into(),
                    },
                    Step::Started,
                );
            }
        }

        // B2b: the file may have been consumed by the offer path after this Ready was decided.
        // Recheck the content-keyed mark before the record or the move begins; the next start then
        // shows this room the offer that finishes the rename.
        let content_sha256 = match files::sha256_file(&legacy) {
            Ok(sha256) => sha256,
            Err(_error) => {
                return Outcome::Transient(Transient {
                    cause: TransientCause::Io,
                    copy_saved: false,
                    record_room: false,
                    detail: "the legacy file could not be read".into(),
                });
            }
        };
        match claim.offer_consumption_match(&room.room_id, &content_sha256) {
            Ok(OfferConsumptionMatch::None) => {}
            Ok(_) => {
                return Outcome::Transient(Transient {
                    cause: TransientCause::FileChanged,
                    copy_saved: false,
                    record_room: false,
                    detail: "the legacy file was already added to a room; start again".into(),
                });
            }
            Err(error) => return claim.failed(store_transient(error, false), Step::Started),
        }

        // End of step 2: the record names this account's room, and only now (N3).
        let mut record = match claim.record() {
            // A claim that already moved its file owns its record (and its zip's lifetime): a new file is offered,
            // never claimed over it ([`LegacyClaim::start`] gives no ReadyClaim then).
            Ok(Some(record)) if record.moved() => {
                return Outcome::Transient(Transient {
                    cause: TransientCause::FileChanged,
                    copy_saved: false,
                    record_room: false,
                    detail: "a finished claim's record is in place; start again".into(),
                });
            }
            Ok(record) => record.unwrap_or_else(new_record),
            Err(error) => return claim.failed(store_transient(error, false), Step::Started),
        };
        record.room = Some(room.room_id.as_str().to_owned());
        record.origin = Some(room.origin.clone());
        record.user_id = Some(room.user_id.clone());
        record.email = room.email.clone();
        record.unlocked_by = Some(unlocked_by);
        record.empty_room_aside = empty_room_aside;
        record.legacy_file = probe.identity();
        record.os_login_used = unlocked_by == UnlockedBy::ComputerLogin || record.os_login_used;
        record.failure = None;
        record.step = Step::Unlocked;
        record.hash_checked = false;
        record.open_checked = false;
        record.owner_pin_phc = owner_pin_phc;
        if let Err(error) = claim.write_record(&record) {
            return claim.failed(store_transient(error, false), Step::Unlocked);
        }
        cut(Cut::AfterUnlockRecorded);

        // 3. The raw safety copy.
        let copy = match files::raw_safety_copy(&claim.layout, &legacy) {
            Ok(copy) => copy,
            Err(files::CopyError::Permanent(permanent)) => {
                let _ = claim.record_permanent(&permanent);
                return Outcome::Permanent(permanent);
            }
            Err(files::CopyError::Transient(transient)) => {
                return claim.failed(transient, Step::Unlocked);
            }
        };
        record.raw_user_version = Some(copy.user_version);
        record.raw_sha256 = Some(copy.raw_sha256.clone());
        record.zip = Some(copy.zip_name.clone());
        record.zip_sha256 = Some(copy.zip_sha256.clone());
        record.step = Step::Copied;
        if let Err(error) = claim.write_record(&record) {
            return claim.failed(store_transient(error, true), Step::Copied);
        }
        cut(Cut::AfterCopyRecorded);

        // 4. The move.
        let moved = files::move_into_room(
            &claim.layout,
            &legacy,
            &room.room_id,
            &copy,
            empty_room_aside,
        );
        let aside = match moved {
            Ok(aside) => aside,
            Err(transient) => return claim.failed(transient, Step::Copied),
        };
        record.empty_room_file = aside;
        record.step = Step::Moved;
        if let Err(error) = claim.write_record(&record) {
            // The files say "moved"; the next start finishes from them.
            return claim.failed(store_transient(error, true), Step::Moved);
        }
        cut(Cut::AfterMoveRecorded);
        // The owner lock goes with the move (PLAN §A.3.2 step 4); the whole-run hold, if any, keeps its own.
        drop(lock);

        let outcome = claim.finish_after_move(&room, record);
        drop(run);
        outcome
    }
}

/// The offer of a legacy file that reappeared, or of one an account with data meets (§A.3.5, §A.3.6), when the file
/// holds a credential: the legacy unlock comes before "Add" (N1).
#[derive(Debug)]
pub struct LockedOffer {
    gate: Gate,
}

impl LockedOffer {
    pub fn kinds(&self) -> CredentialKinds {
        self.gate.probe.kinds()
    }

    pub fn unlock(
        self,
        claim: &LegacyClaim,
        credential: LegacyCredential<'_>,
    ) -> Result<Offer, Refused<Self>> {
        match self.gate.try_credential(claim, &credential) {
            Ok(_) => Ok(Offer { gate: self.gate }),
            Err(reason) => Err(Refused {
                page: Box::new(self),
                reason,
            }),
        }
    }

    pub fn not_mine(self, claim: &LegacyClaim) -> Result<(), DeviceStoreError> {
        self.gate.not_mine(claim)
    }
}

/// How an offer starts.
#[derive(Debug)]
pub enum OfferStart {
    /// The unlock first.
    Locked(LockedOffer),
    /// "Tax profiles from an older version of eBIRForms were found on this device." Add · Not now.
    Open(Offer),
}

/// "Tax profiles from an older version of eBIRForms were found on this device." **Add them to this account** · **Not
/// now** (§A.3.6). Dropping it is "Not now": the file stays, and is offered again at the next launch.
#[derive(Debug)]
pub struct Offer {
    gate: Gate,
}

/// What "Add" did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The profiles the archive carried into the room.
    pub profiles: usize,
    /// What the legacy file is called now (in the device folder).
    pub offered_file: PathBuf,
}

impl Offer {
    /// "No": this account never takes the file; it stays exactly where it is and is not offered
    /// to this account again.
    pub fn not_mine(self, claim: &LegacyClaim) -> Result<(), DeviceStoreError> {
        self.gate.not_mine(claim)
    }

    /// The words of the offer (ruled).
    pub const MESSAGE: &'static str =
        "Tax profiles from an older version of eBIRForms were found on this device.";

    /// Test-only remnant of the removed Add path: the whole-profile export of the file imported into
    /// `room`, then the file renamed aside. Production answers Add with refusal instead.
    #[cfg(test)]
    pub fn add(self, claim: &LegacyClaim, room: &Database) -> Result<Added, Failure> {
        let added = files::add_offered(claim, room, &self.gate.room, self.gate.probe.identity());
        drop(self.gate.lock);
        added
    }
}

#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
pub use files::{cut_next_offer_after_importing_mark_for_test, fail_next_offer_rename_for_test};

// ------------------------------------------------------------------------------------------------ helpers

impl LegacyClaim {
    /// A transient failure of the claim run. Before the move the claim is over: a pre-move resume needs a new
    /// unlock, so the computer-login-used mark goes with it — the person is never left without their fallback.
    /// After the move the resume finishes from the files and needs no unlock, so the mark stays as it is.
    fn failed(&self, transient: Transient, step: Step) -> Outcome {
        self.record_transient(&transient, step);
        if step < Step::Moved {
            self.clear_os_login_used();
        }
        Outcome::Transient(transient)
    }
}

fn record_pin_phc(record: &Record) -> Option<&str> {
    record.owner_pin_phc.as_ref().map(|phc| phc.0.as_str())
}

fn new_record() -> Record {
    Record {
        stamp: now_text(),
        ..Record::default()
    }
}

fn now_text() -> String {
    Utc::now().to_rfc3339()
}

/// The legacy owner lock through the process holder (X1); held elsewhere is transient.
fn record_room_failure(failure: Failure) -> Failure {
    match failure {
        Failure::Transient(mut transient) => {
            transient.record_room = true;
            Failure::Transient(transient)
        }
        permanent => permanent,
    }
}

fn record_room_outcome(outcome: Outcome) -> Outcome {
    match outcome {
        Outcome::Transient(mut transient) => {
            transient.record_room = true;
            Outcome::Transient(transient)
        }
        outcome => outcome,
    }
}

#[cfg(any(test, feature = "test-support"))]
static FAIL_NEXT_LEGACY_LOCK: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn fail_next_legacy_lock_for_test() {
    FAIL_NEXT_LEGACY_LOCK.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn clear_fail_next_legacy_lock_for_test() {
    FAIL_NEXT_LEGACY_LOCK.store(false, std::sync::atomic::Ordering::SeqCst);
}

fn hold_legacy_lock(legacy: &Path) -> Result<OwnerLock, Failure> {
    #[cfg(any(test, feature = "test-support"))]
    if FAIL_NEXT_LEGACY_LOCK.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(Failure::Transient(Transient {
            cause: TransientCause::OwnerLockHeld,
            copy_saved: false,
            record_room: false,
            detail: "injected legacy owner-lock hold".into(),
        }));
    }
    crate::db::hold_owner_lock(legacy).map_err(|error| {
        Failure::Transient(Transient {
            cause: match &error {
                crate::db::DbError::LiveDatabaseInUse(_) => TransientCause::OwnerLockHeld,
                _ => TransientCause::Io,
            },
            copy_saved: false,
            record_room: false,
            detail: match &error {
                crate::db::DbError::LiveDatabaseInUse(_) => {
                    "the legacy file is open elsewhere".into()
                }
                _ => "the legacy file cannot be locked".into(),
            },
        })
    })
}

fn io_cause(error: &std::io::Error) -> TransientCause {
    if files::is_disk_full(error) {
        TransientCause::DiskFull
    } else {
        TransientCause::Io
    }
}

fn io_failure(error: std::io::Error) -> Failure {
    Failure::Transient(Transient {
        cause: io_cause(&error),
        copy_saved: false,
        record_room: false,
        detail: "the device cannot complete the file operation".into(),
    })
}

fn store_failure(error: DeviceStoreError) -> Failure {
    Failure::Transient(store_transient(error, false))
}

fn store_transient(error: DeviceStoreError, copy_saved: bool) -> Transient {
    Transient {
        record_room: false,
        cause: match &error {
            DeviceStoreError::Io(io) if files::is_disk_full(io) => TransientCause::DiskFull,
            _ => TransientCause::Io,
        },
        copy_saved,
        detail: "the device store cannot be used".into(),
    }
}

/// Step 7's second half: the room opens with this build and `quick_check` answers ok.
fn open_check(room_file: &Path) -> Result<(), StepFailure> {
    if let Some(error) = injected(Inject::OpenCheck) {
        return Err(db_failure(
            MovedNotOpenedReason::Open,
            crate::db::DbError::Io(error),
        ));
    }
    let db =
        Database::open(room_file).map_err(|error| db_failure(MovedNotOpenedReason::Open, error))?;
    let answer: String = db
        .conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| db_failure(MovedNotOpenedReason::QuickCheck, error.into()))?;
    if answer == "ok" {
        Ok(())
    } else {
        Err(StepFailure::Moved(MovedNotOpenedReason::QuickCheck, answer))
    }
}

/// How a step after the move failed (C3): a passing state of the device (a full disk, an I/O error, the file busy)
/// is transient and tried again; only what the file itself shows is "moved, not opened".
enum StepFailure {
    Passing(TransientCause, String),
    Moved(MovedNotOpenedReason, String),
}

/// A SQLite error that is a passing state of the device, not of the file: its transient cause.
fn sqlite_cause(error: &rusqlite::Error) -> Option<TransientCause> {
    match error {
        rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
            rusqlite::ErrorCode::DiskFull => Some(TransientCause::DiskFull),
            rusqlite::ErrorCode::SystemIoFailure
            | rusqlite::ErrorCode::CannotOpen
            | rusqlite::ErrorCode::OutOfMemory => Some(TransientCause::Io),
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                Some(TransientCause::WalReturned)
            }
            _ => None,
        },
        _ => None,
    }
}

/// [`StepFailure`] of a database error at a step after the move (the split `probe::open_failure` makes before it).
fn db_failure(reason: MovedNotOpenedReason, error: crate::db::DbError) -> StepFailure {
    let cause = match &error {
        crate::db::DbError::Io(io) => Some(io_cause(io)),
        crate::db::DbError::Sqlite(sqlite) => sqlite_cause(sqlite),
        crate::db::DbError::InUseElsewhere(_) => Some(TransientCause::WalReturned),
        _ => None,
    };
    match cause {
        Some(cause) => StepFailure::Passing(cause, "the room database cannot be used".into()),
        None => {
            tracing::warn!(?reason, "room open check failed: {error}");
            let detail = match reason {
                MovedNotOpenedReason::Open => "the room database cannot be opened",
                MovedNotOpenedReason::Fixups => "the room database cannot be prepared",
                MovedNotOpenedReason::QuickCheck => "the room database failed its integrity check",
                MovedNotOpenedReason::HashMismatch => {
                    "the room copy does not match the legacy file"
                }
            };
            StepFailure::Moved(reason, detail.into())
        }
    }
}

/// The points a test cuts the claim at, after each durable act (PLAN, rehearsal shape 14). Never in a build that
/// ships: outside tests [`cut`] does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cut {
    /// The probe's copy is in `scratch/` (an observation point).
    ProbeCopied,
    AfterUnlockRecorded,
    AfterZipWritten,
    AfterCopyRecorded,
    AfterRoomDir,
    AfterEmptyAside,
    AfterLink,
    AfterUnlink,
    AfterSafetyFiles,
    AfterMoveRecorded,
    AfterRoomRow,
    AfterHashChecked,
    AfterFixups,
    AfterOpenChecked,
}

/// Where a test makes the disk full (C3): each step's own write, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inject {
    /// Step 3: the zip.
    Zip,
    /// Step 4: the link.
    Link,
    /// Step 6: this build's open of the room.
    Open,
    /// Step 6: the room file's transaction (made as SQLite's own `SQLITE_FULL`, only in tests).
    #[cfg_attr(not(test), allow(dead_code))]
    Fixups,
    /// Step 6: the device store's rows.
    DeviceRows,
    /// Step 7: the open check.
    OpenCheck,
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    pub(crate) static INJECT_AT: std::cell::Cell<Option<Inject>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
thread_local! {
    /// A test's reused inode (N1): the room file reads as having the legacy file's recorded identity.
    pub(crate) static SAME_ID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A test's read error at the room-is-the-legacy check (M1): one io error, then cleared.
    pub(crate) static ROOM_CHECK_ERR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A test's full disk at `point`, once; nothing outside tests.
#[inline]
pub(crate) fn injected(point: Inject) -> Option<std::io::Error> {
    #[cfg(any(test, feature = "test-support"))]
    if INJECT_AT.with(|at| at.get()) == Some(point) {
        INJECT_AT.with(|at| at.set(None));
        return Some(std::io::Error::from(std::io::ErrorKind::StorageFull));
    }
    #[cfg(not(any(test, feature = "test-support")))]
    let _ = point;
    None
}

#[cfg(feature = "test-support")]
static FAIL_DEVICE_ROWS_FOR_TEST: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Another crate's host test: the next device-store row write of a finish after the move fails
/// once, as a full disk or a transient store error would. `test-support` never ships in the app.
#[cfg(feature = "test-support")]
pub fn fail_next_device_rows_write_for_test() {
    FAIL_DEVICE_ROWS_FOR_TEST.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// A full disk as SQLite reports it (`SQLITE_FULL`), for [`Inject::Fixups`].
#[cfg(test)]
fn sqlite_full() -> crate::db::DbError {
    crate::db::DbError::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL),
        None,
    ))
}

/// A test's observation, run once when the claim reaches its point.
#[cfg(test)]
type Observe = Box<dyn FnOnce()>;

#[cfg(test)]
thread_local! {
    pub(crate) static CUT_AT: std::cell::Cell<Option<Cut>> = const { std::cell::Cell::new(None) };
    pub(crate) static AT_POINT: std::cell::RefCell<Option<(Cut, Observe)>> = const { std::cell::RefCell::new(None) };
}

/// A test's cut: a panic at `point` (as a kill would stop the process there; the locks go with it), or its
/// observation. Nothing outside tests.
#[inline]
pub(crate) fn cut(point: Cut) {
    #[cfg(test)]
    {
        let observe = AT_POINT.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.as_ref().is_some_and(|(at, _)| *at == point) {
                slot.take().map(|(_, observe)| observe)
            } else {
                None
            }
        });
        if let Some(observe) = observe {
            observe();
        }
        if CUT_AT.with(|at| at.get()) == Some(point) {
            CUT_AT.with(|at| at.set(None));
            panic!("legacy claim cut at {point:?}");
        }
    }
    #[cfg(not(test))]
    let _ = point;
}
