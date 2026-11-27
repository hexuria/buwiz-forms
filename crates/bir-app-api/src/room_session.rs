//! A3a: how a session gets its room (PLAN §A.4). The order, at the launch resume and at
//! `session_opened` alike: read the saved sign-in, run or resume the legacy claim (§A.3), open the
//! account's room, start or wake the worker, rebuild the agent session. §A.3.7 legacy mode: while
//! A3a-2 is absent and the probe finds a credential in the legacy file, a non-practice account opens
//! it in place as its one slot under its old lock — no record, no zip, no room, nothing written
//! beside it.

use bir_core::cloud_account::StoredAccount;
use bir_core::legacy_claim::{Claimant, LegacyClaim, NotHere, Outcome, Start};
use bir_core::rooms::{Layout, RoomId};

use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::open_room;

/// What the session's claim decided, for the page and the log. The room behind it is open unless
/// the claim is transient or moved-not-opened: then no room opens and the page is a migration page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRoom {
    /// The account's room is the one slot.
    Room { room_id: RoomId },
    /// Legacy mode (§A.3.7): the legacy file in place is the one slot.
    LegacyInPlace,
    /// Single-room mode: the named file is the slot, as before rooms.
    SingleRoom,
    /// Nothing opens: a transient failure. The page is "could not be moved yet".
    BlockedTransient { copy_saved: bool, detail: String },
    /// The file is in the room but this build cannot open it: the support page, no empty room.
    MovedNotOpened { detail: String },
    /// A permanent failure before the move: the account's empty room opens, the note is the
    /// migration state the page reads.
    PermanentEmptyRoom { room_id: RoomId, detail: String },
    /// The legacy file reappeared after the move (or this account's room has data): offered, never
    /// merged. The account's room opens; the offer waits for the page (§A.3.6).
    Offered { room_id: RoomId },
}

/// The migration state the page reads (the moving and failure pages, the offer). Written by
/// [`open_session_room`], read by the host verbs the pages call. `None` is an ordinary sign-in.
static MIGRATION: std::sync::Mutex<Option<MigrationState>> = std::sync::Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    Moving,
    Transient {
        copy_saved: bool,
        detail: String,
    },
    Permanent {
        detail: String,
    },
    MovedNotOpened {
        detail: String,
    },
    /// X2: the offer is the account's it was made to — the answer refuses unless the slot is still
    /// that room, for that user, at that generation.
    Offered {
        user_id: String,
        room_id: RoomId,
        generation: u64,
    },
}

pub fn migration_state() -> Option<MigrationState> {
    MIGRATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn set_migration(state: Option<MigrationState>) {
    *MIGRATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
}

fn claimant(stored: &StoredAccount) -> Claimant {
    Claimant {
        server_url: stored.server_url.clone(),
        user_id: stored.user_id.clone(),
        email: stored.email.clone(),
        practice: stored.practice,
    }
}

/// Open the account's own room as the one slot: the room the claim names, or the room lookup of a
/// sign-in with nothing to claim. `create` is whether an empty room file is made when there is
/// none — a sign-in always may; the room is created on disk by the open itself.
fn origin_of(stored: &StoredAccount) -> String {
    bir_core::rooms::room_origin(&stored.server_url).unwrap_or_default()
}

fn open_own_room(
    layout: &Layout,
    room_id: RoomId,
    user_id: &str,
    origin: &str,
) -> Result<SessionRoom, String> {
    layout
        .create_room_dir(&room_id)
        .map_err(|error| error.to_string())?;
    open_room::open_room(
        layout,
        room_id.clone(),
        user_id.to_owned(),
        origin.to_owned(),
    )?;
    // F13: the zip rule's room-opened check never fired for want of this call — the opened room
    // is the claim's evidence that the move's data is in use, so count it now.
    if let Ok(claim) = LegacyClaim::at(layout.clone())
        && let Err(error) = claim.note_room_opened(&room_id)
    {
        tracing::warn!("room-opened not counted toward the zip rule: {error}");
    }
    // The path the screens keep their values under (I4): the open room's file, on this thread.
    bir_core::db::set_open_room_database(Some(layout.room_database(&room_id)));
    Ok(SessionRoom::Room { room_id })
}

/// Close the session's room state (F2): the one close every sign-out runs — the user's own, the
/// server-ended refresh, the removed device. The slot's Arc and the generation go; the scan is
/// forgotten; the process-wide room path clears. Sends already in flight finish against their own
/// Arc, and the room's file and its PIN stay.
pub(crate) fn close_session() {
    crate::open_room::close_room();
    crate::legacy_import_module::forget_scan();
    bir_core::db::set_open_room_database(None);
    set_migration(None);
}

/// The room of the account signed in now, claimed or looked up: `session_opened` and the launch
/// resume both run this, in the §A.4 order. Never opens anything before the claim's `start()` ran
/// (R1): it is the first thing here.
#[cfg(test)]
static OPEN_SESSION_HOOK: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>> =
    std::sync::Mutex::new(None);

/// F10's test: the worker thread blocks in the hook until the test lets it through, so the test
/// can tell a synchronous session_opened (blocked) from an asynchronous one (returns).
#[cfg(test)]
pub(crate) fn block_next_open_session_for_test() -> std::sync::mpsc::Sender<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    *OPEN_SESSION_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(rx);
    tx
}

#[cfg(test)]
pub(crate) fn take_open_session_hook() -> Option<std::sync::mpsc::Receiver<()>> {
    OPEN_SESSION_HOOK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

pub fn open_session_room(stored: &StoredAccount) -> SessionRoom {
    #[cfg(test)]
    if let Some(rx) = take_open_session_hook() {
        let _ = rx.recv();
    }
    let layout = Layout::here();
    // X1: once per process. A second engine's boot runs the launch resume again (prepare_launch →
    // here); while the slot already holds this user id, nothing reruns — not the claim, not the
    // open, not the generation.
    // X1: once per process, keyed by (origin, user id) as the rooms are (the reviewer's note):
    // the user id alone would skip a same-user sign-in on another server into the wrong room.
    if crate::open_room::origin().as_deref()
        == bir_core::rooms::room_origin(&stored.server_url)
            .ok()
            .as_deref()
        && crate::open_room::user_id().as_deref() == Some(stored.user_id.as_str())
    {
        if let Some(room_id) = crate::open_room::room_id() {
            return SessionRoom::Room { room_id };
        }
        if crate::open_room::legacy_mode() {
            return SessionRoom::LegacyInPlace;
        }
    }
    // X2: no migration state outlives the session it belongs to.
    set_migration(None);
    // F2: whatever a previous session left is closed before this account's claim and open run —
    // no worker tick and no screen of the new sign-in ever meets the old room.
    close_session();
    let who = claimant(stored);
    // F12: here(), not at() — mode-aware, so single-room mode (BIR_DATABASE_PATH or the thread's
    // named file) and the dev database never run a real claim on the named file.
    let claim = match LegacyClaim::here() {
        Ok(claim) => claim,
        Err(error) => {
            // F11, narrowed: with a legacy file present, an empty room beside it is forbidden
            // (§A.3.4 — the person would see their tax profiles vanish), so the file opens in
            // place as the one slot, writing nothing beside it, as §A.3.7's legacy mode does.
            // A practice account is never given the legacy file: its own empty room, as ever.
            // With no legacy file the account's room opens as an ordinary sign-in. Never a
            // permanent refusal either way. The claim cannot run in this process at all, so the
            // started-rule is discharged for the session; the evidence half reads what it can.
            tracing::warn!("legacy claim not opened ({error})");
            bir_core::legacy_claim::note_claim_impossible(layout.device_dir());
            if layout.legacy_database().is_file() && !stored.practice {
                tracing::warn!(
                    "legacy file present and the claim store unusable: legacy mode in place"
                );
                return match open_room::open_legacy_in_place(
                    &layout,
                    stored.user_id.clone(),
                    origin_of(stored),
                ) {
                    Ok(_) => SessionRoom::LegacyInPlace,
                    Err(open_error) => SessionRoom::BlockedTransient {
                        copy_saved: false,
                        detail: open_error,
                    },
                };
            }
            return open_lookup_room(&layout, stored);
        }
    };
    // F8: a live connection to the legacy file — a screen's or send's Arc that outlived the close —
    // means the file is still written. The claim must not copy and move it under those writes, so
    // Ready is refused while one lives: the answer is transient, and the next sign-in or retry asks
    // again when the connection is gone.
    if crate::open_room::live_entry_for(&layout.legacy_database()) {
        return SessionRoom::BlockedTransient {
            copy_saved: false,
            detail: "the earlier database is still in use; try again in a moment".into(),
        };
    }
    let started = claim.start(&who);
    // §A.3.8, wired at launch: count today toward the zip's launch days, then delete the pre-rooms
    // zip when the rule says it goes (30 days, 5 distinct launch days, both checks, no failure
    // state). Never in legacy mode's way: a locked file has no record and nothing is written.
    if !matches!(started, Start::Locked(_)) {
        let today = chrono::Local::now().date_naive();
        if let Err(error) = claim.note_launch(today) {
            tracing::warn!("launch day not counted: {error}");
        }
        match claim.delete_zip_if_due(chrono::Utc::now()) {
            Ok(deleted) => {
                for path in &deleted {
                    tracing::info!(?path, "pre-rooms safety zip deleted by the 30-day rule");
                }
            }
            Err(kept) => {
                tracing::info!(?kept, "pre-rooms safety zip kept");
            }
        }
    }
    match started {
        Start::Nothing | Start::DeclinedHere => open_lookup_room(&layout, stored),
        Start::NotHere(NotHere::SingleRoom) | Start::NotHere(NotHere::DevDatabase) => {
            // Single-room mode: the named file is the slot, opened as before rooms (the caller's
            // existing slot path). No claim, no rooms.
            SessionRoom::SingleRoom
        }
        Start::NotHere(NotHere::Practice) => open_lookup_room(&layout, stored),
        Start::Locked(_locked) => {
            // §A.3.7: A3a-2 is absent — a credential in the legacy file means legacy mode. The file
            // opens in place as the one slot; nothing is written beside it.
            match open_room::open_legacy_in_place(
                &layout,
                stored.user_id.clone(),
                origin_of(stored),
            ) {
                Ok(_) => {
                    bir_core::db::set_open_room_database(Some(layout.legacy_database()));
                    set_migration(None);
                    SessionRoom::LegacyInPlace
                }
                Err(error) => {
                    tracing::warn!("legacy mode: the legacy file did not open: {error}");
                    SessionRoom::BlockedTransient {
                        copy_saved: false,
                        detail: error,
                    }
                }
            }
        }
        Start::Ready(ready) => {
            set_migration(Some(MigrationState::Moving));
            let outcome = ready.claim(&claim);
            finish_outcome(&layout, stored, outcome)
        }
        Start::Finished(outcome) => finish_outcome(&layout, stored, outcome),
        Start::Offer(offer) => match offer {
            bir_core::legacy_claim::OfferStart::Locked(_locked_offer) => {
                // §A.3.6: the legacy unlock page is A3a-2; until then legacy-mode rules apply.
                match open_room::open_legacy_in_place(
                    &layout,
                    stored.user_id.clone(),
                    origin_of(stored),
                ) {
                    Ok(_) => {
                        bir_core::db::set_open_room_database(Some(layout.legacy_database()));
                        SessionRoom::LegacyInPlace
                    }
                    Err(error) => SessionRoom::BlockedTransient {
                        copy_saved: false,
                        detail: error,
                    },
                }
            }
            bir_core::legacy_claim::OfferStart::Open(_open_offer) => {
                // The offer waits for the page; the account's room opens, never merged (§A.3.6).
                // X2: never to a practice account — the store-review account is nobody's heir.
                match open_lookup_room(&layout, stored) {
                    SessionRoom::Room { room_id } => {
                        if !stored.practice
                            && let Some((_, generation)) = crate::open_room::current()
                        {
                            set_migration(Some(MigrationState::Offered {
                                user_id: stored.user_id.clone(),
                                room_id: room_id.clone(),
                                generation,
                            }));
                        }
                        SessionRoom::Offered { room_id }
                    }
                    other => other,
                }
            }
        },
        Start::Failed(failure) => finish_outcome(&layout, stored, Outcome::from(failure)),
    }
}

/// The account's room by the device-store lookup (rooms A1): known, or new and empty.
fn open_lookup_room(layout: &Layout, stored: &StoredAccount) -> SessionRoom {
    // F16: the store, never None — with None the Known branch is dead, and a sign-in after a
    // restart never finds the room the account already has.
    let store = bir_core::rooms::DeviceStore::open(layout).ok();
    let lookup = bir_core::rooms::room_for(store.as_ref(), &stored.server_url, &stored.user_id);
    match lookup {
        Ok(bir_core::rooms::RoomLookup::Known(record)) => {
            let room_id = record.room_id.clone();
            match open_own_room(layout, room_id.clone(), &stored.user_id, &origin_of(stored)) {
                Ok(room) => room,
                Err(error) => {
                    tracing::warn!("room {room_id} not opened: {error}");
                    SessionRoom::BlockedTransient {
                        copy_saved: false,
                        detail: error,
                    }
                }
            }
        }
        Ok(bir_core::rooms::RoomLookup::New { room_id, .. }) => {
            match open_own_room(layout, room_id.clone(), &stored.user_id, &origin_of(stored)) {
                Ok(room) => room,
                Err(error) => SessionRoom::BlockedTransient {
                    copy_saved: false,
                    detail: error,
                },
            }
        }
        Err(error) => {
            tracing::warn!("room lookup failed: {error}");
            SessionRoom::BlockedTransient {
                copy_saved: false,
                detail: error.to_string(),
            }
        }
    }
}

/// What an outcome of the claim leaves open (§A.3.4): Done is the claimer's room; transient opens
/// no room; permanent before the move opens the empty room; moved-not-opened opens nothing.
fn finish_outcome(layout: &Layout, stored: &StoredAccount, outcome: Outcome) -> SessionRoom {
    match outcome {
        Outcome::Done { room, .. } => {
            set_migration(None);
            match open_own_room(layout, room.clone(), &stored.user_id, &origin_of(stored)) {
                Ok(opened) => opened,
                Err(error) => SessionRoom::BlockedTransient {
                    copy_saved: true,
                    detail: error,
                },
            }
        }
        Outcome::Transient(transient) => {
            set_migration(Some(MigrationState::Transient {
                copy_saved: transient.copy_saved,
                detail: transient.detail.clone(),
            }));
            SessionRoom::BlockedTransient {
                copy_saved: transient.copy_saved,
                detail: transient.detail,
            }
        }
        Outcome::Permanent(permanent) => {
            set_migration(Some(MigrationState::Permanent {
                detail: permanent.detail.clone(),
            }));
            match open_lookup_room(layout, stored) {
                SessionRoom::Room { room_id } => SessionRoom::PermanentEmptyRoom {
                    room_id,
                    detail: permanent.detail,
                },
                other => other,
            }
        }
        Outcome::MovedNotOpened(moved) => {
            set_migration(Some(MigrationState::MovedNotOpened {
                detail: moved.detail.clone(),
            }));
            SessionRoom::MovedNotOpened {
                detail: moved.detail,
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    set_migration(None);
}

/// One at a time across both test modules: the device-dir override, the slot and the guards are
/// process-wide, so no two room tests may run in parallel in this process.
#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::db::Database;
    use bir_core::profile::TaxpayerType;
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};

    /// The §A.8 host tests share one process-wide slot (the design's one room at a time), so they
    /// take turns.
    fn stored(user: &str) -> StoredAccount {
        StoredAccount {
            server_url: "https://api.buwiz.test".into(),
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            access_expires_at_unix: 0,
            user_id: user.into(),
            email: Some(format!("{user}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        }
    }

    fn taxpayer(db: &Database, root: &str, name: &str) {
        db.create_taxpayer(NewTaxpayer {
            tin_root: root.into(),
            name: name.into(),
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
        .unwrap();
    }

    struct Scratch {
        _folder: tempfile::TempDir,
        layout: Layout,
    }

    /// A scratch device folder as this thread's device dir, the override cleared on drop.
    fn scratch() -> Scratch {
        // Process-wide state the tests before this one left (their order is not fixed): each test
        // starts from the state's nothing-open nothing-offered baseline, whatever ran before.
        crate::open_room::reset_for_tests();
        reset_for_tests();
        bir_core::db::set_open_room_database(None);
        crate::account_gate::set_unlocked(false);
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        Scratch {
            layout: Layout::at(folder.path().to_path_buf()),
            _folder: folder,
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            bir_core::platform::set_device_dir_override(None);
            crate::open_room::close_room();
            bir_core::db::set_open_room_database(None);
        }
    }

    /// A legacy install with no lock: one taxpayer, checkpointed, closed.
    fn legacy_unlocked(layout: &Layout, root: &str, name: &str) {
        let db = Database::open(layout.legacy_database()).unwrap();
        taxpayer(&db, root, name);
        db.checkpoint().unwrap();
        drop(db);
    }

    fn names_in_room(layout: &Layout, room_id: &RoomId) -> Vec<String> {
        let db = Database::open(layout.room_database(room_id)).unwrap();
        let names = db
            .list_profiles()
            .unwrap()
            .iter()
            .map(|profile| profile.full_name.clone())
            .collect();
        drop(db);
        names
    }

    /// §A.8: a second account signing in on the same device never sees the first account's tax
    /// profiles (the owner's bug this PR fixes): its room is empty, and it is another room.
    #[test]
    fn a_second_account_gets_an_empty_room() {
        let _turn = crate::open_room::room_test_turn();
        let scratch = scratch();
        legacy_unlocked(&scratch.layout, "123456788", "A's Trading Corp");

        // The first real account claims the unlocked install at its session_opened.
        let a = stored("user-a");
        let room_a = match open_session_room(&a) {
            SessionRoom::Room { room_id } => room_id,
            other => panic!("A did not get a room: {other:?}"),
        };
        assert_eq!(
            names_in_room(&scratch.layout, &room_a),
            vec!["A's Trading Corp".to_string()],
            "the claim moved A's legacy data into A's room"
        );
        crate::open_room::close_room();

        // The second account: nothing to claim, a room of its own — and empty.
        let b = stored("user-b");
        let room_b = match open_session_room(&b) {
            SessionRoom::Room { room_id } => room_id,
            other => panic!("B did not get a room: {other:?}"),
        };
        assert_ne!(room_a, room_b, "B must never share A's room");
        assert!(
            names_in_room(&scratch.layout, &room_b).is_empty(),
            "B's room holds another account's profiles"
        );
    }

    /// §A.8: before A3a-2, a locked legacy file is never claimed: each non-practice account opens
    /// it in place as its one slot, and the practice account gets an empty room.
    #[test]
    fn locked_legacy_is_not_claimed_before_a3a2() {
        let _turn = crate::open_room::room_test_turn();
        let scratch = scratch();
        // A locked legacy file: the helpdesk access mode with a PIN.
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        taxpayer(&db, "123456788", "Locked Trading Corp");
        db.set_setting("access_mode", "helpdesk").unwrap();
        db.set_setting("app_lock_pin_hash", &bir_core::crypto::hash_pin("1357"))
            .unwrap();
        db.checkpoint().unwrap();
        drop(db);

        let a = stored("user-a");
        let session = open_session_room(&a);
        assert!(
            matches!(session, SessionRoom::LegacyInPlace),
            "a locked legacy file is opened in place, not claimed: {session:?}"
        );
        assert!(crate::open_room::legacy_mode(), "the slot is legacy mode");
        // Nothing beside it: no record in device.db, no zip, no room.
        assert!(
            !scratch.layout.rooms_dir().exists()
                || std::fs::read_dir(scratch.layout.rooms_dir())
                    .unwrap()
                    .next()
                    .is_none(),
            "legacy mode made a room"
        );
        assert!(
            !scratch.layout.backups_dir().exists()
                || std::fs::read_dir(scratch.layout.backups_dir())
                    .unwrap()
                    .next()
                    .is_none(),
            "legacy mode made a zip"
        );
        crate::open_room::close_room();

        // A second account: the same in-place slot, still nothing written.
        let b = stored("user-b");
        let session = open_session_room(&b);
        assert!(
            matches!(session, SessionRoom::LegacyInPlace),
            "the second account also opens it in place: {session:?}"
        );
        crate::open_room::close_room();

        // The practice account gets an empty room instead.
        let mut practice = stored("practice-user");
        practice.practice = true;
        let session = open_session_room(&practice);
        assert!(
            matches!(session, SessionRoom::Room { .. }),
            "the practice account gets an empty room: {session:?}"
        );
    }

    /// §A.8: after a launch on a locked legacy file and a sign-out, device.db has no migration
    /// row, backups/ and scratch/ are empty, and no lock is held beyond the process holder.
    #[test]
    fn locked_launch_and_sign_out_leave_no_record_no_zip_no_lock() {
        let _turn = crate::open_room::room_test_turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        taxpayer(&db, "123456788", "Locked Trading Corp");
        db.set_setting("access_mode", "helpdesk").unwrap();
        db.set_setting("app_lock_pin_hash", &bir_core::crypto::hash_pin("1357"))
            .unwrap();
        db.checkpoint().unwrap();
        drop(db);

        let a = stored("user-a");
        let session = open_session_room(&a);
        assert!(matches!(session, SessionRoom::LegacyInPlace), "{session:?}");
        crate::open_room::close_room();

        // No migration row in device.db.
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        let claim = LegacyClaim::at(scratch.layout.clone()).unwrap();
        assert!(
            claim.record().unwrap().is_none(),
            "legacy mode wrote a migration record"
        );
        drop(store);
        // backups/ and scratch/ empty or absent.
        for folder in ["backups", "scratch"] {
            let path = scratch.layout.device_dir().join(folder);
            assert!(
                !path.exists() || std::fs::read_dir(&path).unwrap().next().is_none(),
                "{folder}/ is not empty"
            );
        }
        // No lock is held beyond the process holder: after the sign-out's close the owner lock can
        // be taken again, and no claim lock or instance file ever appeared beside the legacy file.
        assert!(
            bir_core::db::try_acquire_live_owner_lock(&scratch.layout.legacy_database()).is_ok(),
            "the owner lock is still held after the room closed"
        );
        let extra_locks: Vec<_> = std::fs::read_dir(scratch.layout.device_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".instance") || name.contains("claim"))
            .collect();
        assert!(extra_locks.is_empty(), "locks left behind: {extra_locks:?}");
    }
}

#[cfg(test)]
mod a3a_tests {
    use super::*;
    use bir_core::db::Database;

    fn turn() -> std::sync::MutexGuard<'static, ()> {
        crate::open_room::room_test_turn()
    }

    fn stored(user: &str) -> StoredAccount {
        StoredAccount {
            server_url: "https://api.buwiz.test".into(),
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            access_expires_at_unix: 0,
            user_id: user.into(),
            email: Some(format!("{user}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        }
    }

    struct Scratch {
        _folder: tempfile::TempDir,
        layout: Layout,
    }

    fn scratch() -> Scratch {
        // Process-wide state the tests before this one left (their order is not fixed): each test
        // starts from the state's nothing-open nothing-offered baseline, whatever ran before.
        crate::open_room::reset_for_tests();
        reset_for_tests();
        bir_core::db::set_open_room_database(None);
        crate::account_gate::set_unlocked(false);
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        Scratch {
            layout: Layout::at(folder.path().to_path_buf()),
            _folder: folder,
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            bir_core::platform::set_device_dir_override(None);
            crate::open_room::close_room();
            bir_core::db::set_open_room_database(None);
        }
    }

    /// A cut claim's record in the scratch device.db, as the library writes it (the `migration`
    /// row `legacy`): step Copied in the link window, the room named, no failure.
    fn sha256_of(path: &std::path::Path) -> String {
        use sha2::Digest as _;
        let mut hash = sha2::Sha256::new();
        hash.update(std::fs::read(path).unwrap());
        format!("{:x}", hash.finalize())
    }

    /// F8: a live connection to the legacy file (a send's Arc outliving the close) refuses Ready —
    /// the claim answers transient and never copies the file out from under the writer.
    #[test]
    fn a_live_connection_to_the_legacy_file_refuses_the_claims_ready() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // The legacy file exists, and a connection to it outlives the close (a send's held Arc).
        Database::open(scratch.layout.legacy_database())
            .unwrap()
            .close()
            .unwrap();
        let (handle, _) = crate::open_room::open_legacy_in_place(
            &scratch.layout,
            "user-a".into(),
            "https://api.buwiz.test".into(),
        )
        .unwrap();
        crate::open_room::close_room();
        let a = stored("user-a");
        let session = open_session_room(&a);
        assert!(
            matches!(session, SessionRoom::BlockedTransient { .. }),
            "the claim ran Ready with a live connection to the legacy file: {session:?}"
        );
        drop(handle);
    }

    /// F11: device.db unreadable (lost, or written by a newer build) — the fallback is the
    /// account's empty room, not a permanent refusal of every sign-in.
    #[test]
    fn a_claim_store_that_cannot_open_still_opens_the_accounts_room() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        std::fs::write(scratch.layout.device_db(), b"not a sqlite database at all").unwrap();
        let a = stored("f11-user");
        let session = open_session_room(&a);
        assert!(
            matches!(
                session,
                SessionRoom::Room { .. } | SessionRoom::PermanentEmptyRoom { .. }
            ),
            "a lost or newer device.db refused the sign-in: {session:?}"
        );
        assert!(
            crate::open_room::current().is_some(),
            "no room is open after the fallback"
        );
    }

    /// F11, the first arm: the claim store unusable WITH a legacy file present — the file opens
    /// in place as the one slot, never an empty room beside the data (§A.3.4).
    #[test]
    fn a_claim_store_that_cannot_open_with_a_legacy_file_opens_it_in_place() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // The data is there: a legacy file with a taxpayer.
        let db = Database::open(&scratch.layout.legacy_database()).unwrap();
        db.create_taxpayer(bir_core::tax_profile::NewTaxpayer {
            tin_root: "123456780".into(),
            name: "Juan Dela Cruz".into(),
            taxpayer_type: bir_core::profile::TaxpayerType::Individual,
            birth_date: None,
            head_office: bir_core::tax_profile::BranchFacts {
                rdo_code: "040".into(),
                registered_address: "1 Example Street".into(),
                zip_code: "1600".into(),
                phone: "0280000000".into(),
                email: "juan@example.com".into(),
            },
        })
        .unwrap();
        db.close().unwrap();
        std::fs::write(scratch.layout.device_db(), b"not a sqlite database at all").unwrap();
        let a = stored("f11-legacy-user");
        let session = open_session_room(&a);
        assert!(
            matches!(session, SessionRoom::LegacyInPlace),
            "with the data present the fallback must not open an empty room beside it: {session:?}"
        );
        // Nothing is written beside the file: no rooms/, no claim artefacts.
        assert!(
            !scratch.layout.device_dir().join("rooms").exists(),
            "a room was created beside the data"
        );
    }

    /// The practice account is never given the legacy file, even in the first arm: its own empty
    /// room, as ever.
    #[test]
    fn a_practice_account_never_gets_the_legacy_file_in_the_fallback() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        Database::open(&scratch.layout.legacy_database())
            .unwrap()
            .close()
            .unwrap();
        std::fs::write(scratch.layout.device_db(), b"not a sqlite database at all").unwrap();
        let mut practice = stored("practice-user");
        practice.practice = true;
        let session = open_session_room(&practice);
        assert!(
            !matches!(session, SessionRoom::LegacyInPlace),
            "the practice account was given the legacy file: {session:?}"
        );
        assert!(
            matches!(
                session,
                SessionRoom::Room { .. } | SessionRoom::PermanentEmptyRoom { .. }
            ),
            "the practice account's own room did not open: {session:?}"
        );
    }

    fn write_cut_record(layout: &Layout, room: &RoomId, who: &Claimant, raw_sha256: String) {
        let store = bir_core::rooms::DeviceStore::open(layout).unwrap();
        drop(store);
        let conn = rusqlite::Connection::open(layout.device_db()).unwrap();
        let record = bir_core::legacy_claim::Record {
            stamp: "test-stamp".into(),
            room: Some(room.as_str().to_owned()),
            origin: Some(bir_core::rooms::room_origin(&who.server_url).unwrap()),
            user_id: Some(who.user_id.clone()),
            email: who.email.clone(),
            unlocked_by: Some(bir_core::legacy_claim::UnlockedBy::NoLock),
            empty_room_aside: false,
            empty_room_file: None,
            os_login_used: false,
            raw_user_version: None,
            raw_sha256: Some(raw_sha256),
            zip: None,
            zip_sha256: None,
            hash_checked: false,
            open_checked: false,
            step: bir_core::legacy_claim::Step::Copied,
            failure: None,
            done_at: None,
            launch_days: vec![],
            room_opened_since: false,
            zip_deleted_at: None,
            legacy_file: None,
            owner_pin_phc: None,
        };
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES ('legacy', ?1)",
            rusqlite::params![serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
    }

    /// A record cut at `step`, as the library writes it — for G1's stopped-at-Moved opens.
    fn write_record_at_step(
        layout: &Layout,
        room: &RoomId,
        who: &Claimant,
        step: bir_core::legacy_claim::Step,
    ) {
        let store = bir_core::rooms::DeviceStore::open(layout).unwrap();
        drop(store);
        let conn = rusqlite::Connection::open(layout.device_db()).unwrap();
        let record = bir_core::legacy_claim::Record {
            stamp: "test-stamp".into(),
            room: Some(room.as_str().to_owned()),
            origin: Some(bir_core::rooms::room_origin(&who.server_url).unwrap()),
            user_id: Some(who.user_id.clone()),
            email: who.email.clone(),
            unlocked_by: Some(bir_core::legacy_claim::UnlockedBy::NoLock),
            empty_room_aside: false,
            empty_room_file: None,
            os_login_used: false,
            raw_user_version: None,
            raw_sha256: None,
            zip: None,
            zip_sha256: None,
            hash_checked: false,
            open_checked: false,
            step,
            failure: None,
            done_at: None,
            launch_days: vec![],
            room_opened_since: false,
            zip_deleted_at: None,
            legacy_file: None,
            owner_pin_phc: None,
        };
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES ('legacy', ?1)",
            rusqlite::params![serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
    }

    /// R1: the pause spans the import's check-to-swap span and lifts on every exit — a failed
    /// import included, which must never leave sending paused.
    #[test]
    fn the_import_pauses_sending_and_a_failed_import_unpauses() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        crate::account_gate::set_unlocked(true);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        assert!(
            !bir_core::background_cron::submission_paused(),
            "paused before the import"
        );
        // Mid-import — inside the checkpoint, after the jobs check and before the swap — sending
        // is paused, so no send can start in the gap.
        crate::bir_module::set_after_restore_checkpoint_hook_for_test(Box::new(|| {
            assert!(
                bir_core::background_cron::submission_paused(),
                "a send could start between the check and the swap"
            );
        }));
        let source = Database::open(scratch.layout.device_dir().join("source.db")).unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.layout.device_dir().join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);
        crate::bir_module::restore_database_archive(&archive, 7).expect("the import runs");
        assert!(
            !bir_core::background_cron::submission_paused(),
            "still paused after the import"
        );
        // A failed import — the zip is not a database — unpauses too.
        let broken = scratch.layout.device_dir().join("broken.zip");
        std::fs::write(&broken, b"not a zip").unwrap();
        let _ = crate::bir_module::restore_database_archive(&broken, 8);
        assert!(
            !bir_core::background_cron::submission_paused(),
            "a failed import left sending paused"
        );
    }

    /// F9: quit releases the room and the device guard — a relaunch (the update's replacement)
    /// opens the same device without meeting the old process's locks.
    #[test]
    fn quit_releases_the_room_and_the_device_guard() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        assert!(
            crate::open_room::current().is_some(),
            "no room open before quit"
        );
        // The quit runs on a thread of its own (its engine thread-locals end with it).
        std::thread::spawn(crate::bir_module::shutdown_for_quit)
            .join()
            .unwrap();
        assert!(
            crate::open_room::current().is_none(),
            "the room stayed in the slot past quit"
        );
        // The relaunch: the same device opens again — no device guard, no room lock held over.
        let b = stored("user-a");
        assert!(
            matches!(open_session_room(&b), SessionRoom::Room { .. }),
            "the relaunch did not open the room"
        );
    }

    /// F12: single-room mode never runs a real claim — the named file is the slot, no record, no rooms.
    #[test]
    fn single_room_mode_opens_no_room_and_runs_no_claim() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // A legacy file with data a claim would move, were one to run.
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        // Single-room mode: the thread names its own file.
        let named = scratch.layout.device_dir().join("single.db");
        bir_core::db::set_database_path_override(Some(named));
        let a = stored("user-a");
        let session = open_session_room(&a);
        assert!(
            matches!(session, SessionRoom::SingleRoom),
            "single-room mode did not answer SingleRoom: {session:?}"
        );
        assert!(
            !scratch.layout.rooms_dir().exists(),
            "a claim ran in single-room mode: rooms/ was made"
        );
        bir_core::db::set_database_path_override(None);
    }

    /// G1: the slot's real path refuses a room whose record is at Moved with no hash check, even
    /// though an earlier claim filled the started set for the folder in this process.
    #[test]
    fn open_room_refuses_a_ready_that_stopped_at_moved() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        // The earlier claim: start() runs and finishes, filling the started set for the folder.
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        crate::open_room::close_room();
        // …and then its move stopped at Moved — the record names the room, no hash check yet.
        let who = Claimant {
            server_url: a.server_url.clone(),
            user_id: a.user_id.clone(),
            email: a.email.clone(),
            practice: false,
        };
        let room = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        write_record_at_step(
            &scratch.layout,
            &room,
            &who,
            bir_core::legacy_claim::Step::Moved,
        );
        // D1: a refusal is an Err, never a panic.
        let refused = crate::open_room::open_room(
            &scratch.layout,
            room.clone(),
            a.user_id.clone(),
            origin_of(&a),
        );
        assert!(
            matches!(&refused, Err(message) if message.contains("not yet hash-checked")),
            "open_room admitted a room at Moved with no hash check"
        );
        // F1's reopen path holds the same rule.
        let reopened = crate::open_room::reopen_fresh(
            &scratch.layout.room_database(&room),
            Some(room),
            a.user_id.clone(),
        );
        assert!(
            matches!(&reopened, Err(message) if message.contains("not yet hash-checked")),
            "reopen_fresh admitted a room at Moved with no hash check"
        );
    }

    /// G1: the record unreadable fails CLOSED while any rooms exist — an unreadable record can
    /// hide exactly the unfinished move the rule protects.
    #[test]
    fn an_unreadable_record_refuses_the_open_while_rooms_exist() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        crate::open_room::close_room();
        let room = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        // device.db damaged beyond reading; the room folder exists.
        std::fs::write(scratch.layout.device_db(), b"not a sqlite database at all").unwrap();
        let refused = crate::open_room::open_room(
            &scratch.layout,
            room.clone(),
            a.user_id.clone(),
            origin_of(&a),
        );
        assert!(
            matches!(&refused, Err(message) if message.contains("cannot be read")),
            "an unreadable record with rooms present did not refuse"
        );
    }

    /// F1': the import refuses while a return is being sent; the job finishes into the real room
    /// — its status and receipt are in the room file, read back from disk — and the refusal holds
    /// while the receipt is still pending, lifting only when it is confirmed.
    #[test]
    fn an_import_is_refused_while_a_return_is_being_sent() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        crate::account_gate::set_unlocked(true);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        let room_id = crate::open_room::room_id().expect("a room id");
        let source = Database::open(scratch.layout.device_dir().join("source.db")).unwrap();
        source
            .create_taxpayer(bir_core::tax_profile::NewTaxpayer {
                tin_root: "987654320".into(),
                name: "Restored Holdings Inc".into(),
                taxpayer_type: bir_core::profile::TaxpayerType::Corporation,
                birth_date: None,
                head_office: bir_core::tax_profile::BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.layout.device_dir().join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);

        // A send in flight: the worker holds its job key, and the return is queued.
        let (handle, _) = crate::open_room::current().expect("a room is open");
        {
            let guard = bir_core::lock_timing::lock(&handle).unwrap();
            guard
                .conn_for_tests()
                .execute(
                    "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)                      VALUES ('12345678900000', '1701Q', 2026, 3, 'Queued', '{}')",
                    [],
                )
                .unwrap();
        }
        let jobs = bir_core::background_cron::get_active_jobs();
        let inner = bir_core::background_cron::submission_job_key(
            "1701Q",
            "12345678900000",
            2026,
            None,
            Some(3),
        );
        jobs.lock()
            .unwrap()
            .insert(bir_core::background_cron::room_job_key(
                &format!("room:{}", room_id.as_str()),
                &inner,
            ));
        let refused = crate::bir_module::restore_database_archive(&archive, 7);
        assert!(
            matches!(&refused, Err(error) if error.message().contains("A return is being sent. Try the import again when it has finished.")),
            "the import ran while a send was in flight"
        );

        // The job finishes into the real room — the import never swapped it out, so the status
        // and the receipt it writes through its held Arc land in the room file.
        {
            let guard = bir_core::lock_timing::lock(&handle).unwrap();
            guard
                .conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET status = 'Submitted' WHERE tin = '12345678900000'",
                    [],
                )
                .unwrap();
            guard
                .conn_for_tests()
                .execute(
                    "INSERT INTO submission_receipts (filename, tin, form_type, period, received_date, received_time, raw_text)                      VALUES ('receipt-1.txt', '12345678900000', '1701Q', 'Q3/26', '2026-11-27', '10:00:00', 'receipt')",
                    [],
                )
                .unwrap();
            guard.checkpoint().unwrap();
        }
        jobs.lock().unwrap().clear();
        // Read back from disk: the room file itself holds them.
        let on_disk = Database::open(scratch.layout.room_database(&room_id)).unwrap();
        let status: String = on_disk
            .conn_for_tests()
            .query_row(
                "SELECT status FROM form_drafts WHERE tin = '12345678900000'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            status, "Submitted",
            "the job's status is not in the room file"
        );
        let receipts: i64 = on_disk
            .conn_for_tests()
            .query_row(
                "SELECT COUNT(*) FROM submission_receipts WHERE filename = 'receipt-1.txt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipts, 1, "the job's receipt is not in the room file");
        drop(on_disk);

        // The key has cleared: the send's writes are already in the real file, so receipt-pending
        // is restore semantics (the backup simply does not hold them) — the import runs.
        crate::bir_module::restore_database_archive(&archive, 7)
            .expect("the import runs once the send finished");

        // F6's narrowing: a job of ANOTHER room writes into that room's own file, so this room's
        // import is not refused for it.
        jobs.lock()
            .unwrap()
            .insert(bir_core::background_cron::room_job_key(
                "room:another-room-entirely",
                &inner,
            ));
        crate::bir_module::restore_database_archive(&archive, 7)
            .expect("another room's send in flight must not refuse this room's import");
        jobs.lock().unwrap().clear();
    }

    /// §A.8, the launch call site: a claim cut mid-move resumes at launch — open_session_room with
    /// a Finished outcome lands the file in the claimer's room.
    #[test]
    fn a_cut_claim_resumes_at_launch() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        // A cut in the link window, built from the files as §A.3.3 reads them: the legacy file is
        // hard-linked into the claimer's room (the move's link before its unlink), and the record
        // says Copied. No test hook of the library — the state is the real one a kill leaves.
        let a = stored("user-a");
        let who = Claimant {
            server_url: a.server_url.clone(),
            user_id: a.user_id.clone(),
            email: a.email.clone(),
            practice: false,
        };
        let room = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        // device.db before rooms/, or the store reads as lost (L2).
        drop(bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap());
        scratch.layout.create_room_dir(&room).unwrap();
        std::fs::hard_link(
            scratch.layout.legacy_database(),
            scratch.layout.room_database(&room),
        )
        .unwrap();
        let raw_sha256 = sha256_of(&scratch.layout.legacy_database());
        write_cut_record(&scratch.layout, &room, &who, raw_sha256);
        // The launch resume: no legacy unlock, no interaction — the files decide.
        let session = open_session_room(&a);
        assert!(
            matches!(session, SessionRoom::Room { .. }),
            "the cut claim did not resume into the room: {session:?}"
        );
        assert!(
            !scratch.layout.legacy_database().exists(),
            "the legacy file is still there after the resumed move"
        );
    }

    /// §A.8, the session_opened call site: an unlocked legacy install is claimed at the first real
    /// account's session_opened — the file is the account's room afterwards.
    #[test]
    fn an_unlocked_install_is_claimed_at_the_first_accounts_session_opened() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let a = stored("user-a");
        let session = open_session_room(&a);
        let SessionRoom::Room { room_id } = session else {
            panic!("not a room: {session:?}")
        };
        assert!(!scratch.layout.legacy_database().exists(), "not moved");
        assert!(
            scratch.layout.room_database(&room_id).exists(),
            "no room file"
        );
        // The claim wrote its record: done.
        let claim = LegacyClaim::at(scratch.layout.clone()).unwrap();
        let record = claim.record().unwrap().expect("no record");
        assert_eq!(record.step, bir_core::legacy_claim::Step::Done);
    }

    /// §A.8: the agent session rebuilds on a room change — the selection of the old room is gone.
    #[test]
    fn agent_selection_cleared_on_room_change() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        let generation_a = crate::open_room::generation().unwrap();
        crate::open_room::close_room();
        let b = stored("user-b");
        assert!(matches!(open_session_room(&b), SessionRoom::Room { .. }));
        let generation_b = crate::open_room::generation().unwrap();
        assert_ne!(generation_a, generation_b, "a new room is a new generation");
        // The session's comparison: a cached generation that is not the open room's rebuilds.
        let mut session = crate::agent::session::Session::headless();
        session.selected_for_test("12345678800000");
        assert!(session.selected_tin_for_test().is_some());
        // A batch under room B sees the session's A-generation cache as stale.
        assert_ne!(Some(generation_a), crate::open_room::generation());
        session.rebuild();
        assert!(
            session.selected_tin_for_test().is_none(),
            "the selection survived the room change"
        );
    }

    /// §A.8: an eBIRForms scan made in room A can never import into room B — the scan records the
    /// database path it was read against, and the close forgets it.
    #[test]
    fn ebirforms_scan_does_not_cross_rooms() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        // A scan of the legacy eBIRForms export folder, held against room A's path.
        let legacy_folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../bir-core/tests/legacy-ebirforms");
        crate::legacy_import_module::scan_folder(&legacy_folder).unwrap();
        assert!(crate::legacy_import_module::holds_scan());
        // The room closes (sign-out); the scan is forgotten before any other room opens.
        crate::open_room::close_room();
        crate::legacy_import_module::forget_scan();
        let b = stored("user-b");
        assert!(matches!(open_session_room(&b), SessionRoom::Room { .. }));
        assert!(
            !crate::legacy_import_module::holds_scan(),
            "a scan of room A is still held in room B"
        );
    }

    /// R1(c): `bir-headless serve` is a separate process that never runs start(). While the claim
    /// record is at Copied — the move unfinished — opening the room file would migrate its bytes and
    /// close the move unfinished, so the serve path refuses it. Once the record is moved, it answers.
    /// (`headless` is its own feature, over `agent`: this test runs under `--features headless`.)
    #[cfg(feature = "headless")]
    #[test]
    fn serve_refuses_a_room_whose_claim_is_cut_at_copied() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        let who = Claimant {
            server_url: a.server_url.clone(),
            user_id: a.user_id.clone(),
            email: a.email.clone(),
            practice: false,
        };
        let room = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        // The room row exists mid-claim when the account's empty room was already the account's
        // (the empty-room-aside flow): step 5 comes after the move, but this row predates the claim.
        store
            .add_room(&bir_core::rooms::RoomRecord {
                room_id: room.clone(),
                origin: bir_core::rooms::room_origin(&a.server_url).unwrap(),
                user_id: a.user_id.clone(),
                email: a.email.clone(),
                created_at: "2026-11-27T00:00:00Z".into(),
                from_legacy: false,
            })
            .unwrap();
        drop(store);
        write_cut_record(&scratch.layout, &room, &who, "00".repeat(32));
        scratch.layout.create_room_dir(&room).unwrap();
        std::fs::write(scratch.layout.room_database(&room), b"copied bytes").unwrap();
        let refused = crate::agent::headless::signed_in_room_database(&scratch.layout, Some(&a));
        assert!(
            refused.is_err(),
            "serve opened a room whose claim is cut at Copied: {refused:?}"
        );
        // The widened guard (the orchestrator's ruling): at Moved but not yet HashChecked step 7's
        // evidence is still at stake, so the room is still refused.
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        let value: String = conn
            .query_row(
                "SELECT value FROM migration WHERE key = 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut record: bir_core::legacy_claim::Record = serde_json::from_str(&value).unwrap();
        record.step = bir_core::legacy_claim::Step::Moved;
        conn.execute(
            "UPDATE migration SET value = ?1 WHERE key = 'legacy'",
            [serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
        drop(conn);
        let still_refused =
            crate::agent::headless::signed_in_room_database(&scratch.layout, Some(&a));
        assert!(
            still_refused.is_err(),
            "serve opened a room whose claim is at Moved with no hash check: {still_refused:?}"
        );
        // Once the record is hash-checked the same path answers the room file.
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        let value: String = conn
            .query_row(
                "SELECT value FROM migration WHERE key = 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut record: bir_core::legacy_claim::Record = serde_json::from_str(&value).unwrap();
        record.step = bir_core::legacy_claim::Step::HashChecked;
        conn.execute(
            "UPDATE migration SET value = ?1 WHERE key = 'legacy'",
            [serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
        drop(conn);
        let answered = crate::agent::headless::signed_in_room_database(&scratch.layout, Some(&a));
        assert_eq!(
            answered.expect("the room answers once the record is hash-checked"),
            scratch.layout.room_database(&room)
        );
    }

    /// X1: the session room opens once per process — a second engine's launch resume does not rerun
    /// the claim, reopen the room or bump the generation while the slot already holds this user id.
    #[test]
    fn the_session_room_opens_once_per_process() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        let first = crate::open_room::generation().expect("a generation");
        // A second engine's boot runs the launch resume again: nothing changes.
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        assert_eq!(
            crate::open_room::generation(),
            Some(first),
            "a second engine's resume reopened the room (a new generation)"
        );
        crate::open_room::close_room();
    }

    /// F2: the server-ended sign-outs close the room too. A's device revoked, B signing in: nothing
    /// of A stays in the slot, however the sign-in ended.
    #[test]
    fn the_server_ended_sign_outs_close_the_room() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        assert!(crate::open_room::current().is_some());
        crate::cloud_account::session_ended_by_server();
        assert!(
            crate::open_room::current().is_none(),
            "the server-ended sign-out left the room in the slot"
        );
        assert!(
            bir_core::db::open_room_id().is_none(),
            "the room path survived the server-ended sign-out"
        );
    }

    /// X2: the offer is the account's it was made to. The practice account (or any other) cannot
    /// pull the device's pre-rooms data into its own room, however it answers.
    #[test]
    fn the_offer_answer_refuses_an_account_it_was_not_made_to() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // A pre-rooms legacy file with data in it (the fixture every legacy test uses).
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../bir-core/tests/legacy-ebirforms");
        if fixture.is_dir() {
            // The claimable file itself: any real database with the legacy shape stands in here.
        }
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let mut practice_account = stored("user-practice");
        practice_account.practice = true;
        assert!(matches!(
            open_session_room(&practice_account),
            SessionRoom::Room { .. }
        ));
        // No offer was made to the practice account: its "add them" must be refused, never imported.
        let arguments = HostArguments::new([HostValue::Bool(true)]);
        let answered = migration_offer_answer(&arguments);
        match answered {
            Err(error) => assert!(
                error.to_string().contains("offer"),
                "the refusal is not the offer's: {error}"
            ),
            Ok(_) => panic!("the practice account answered the offer and it ran"),
        }
    }

    /// F1: an import in rooms mode reopens the room on the restored file — the slot never serves
    /// the ephemeral placeholder past the install.
    #[test]
    fn an_import_in_rooms_mode_reopens_the_room_on_the_restored_file() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        crate::account_gate::set_unlocked(true);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        // The archive: one taxpayer, "Restored Holdings Inc".
        let source = Database::open(scratch.layout.device_dir().join("source.db")).unwrap();
        source
            .create_taxpayer(bir_core::tax_profile::NewTaxpayer {
                tin_root: "987654320".into(),
                name: "Restored Holdings Inc".into(),
                taxpayer_type: bir_core::profile::TaxpayerType::Corporation,
                birth_date: None,
                head_office: bir_core::tax_profile::BranchFacts {
                    rdo_code: "040".into(),
                    registered_address: "1 Example Street, Pasig".into(),
                    zip_code: "1600".into(),
                    phone: "0280000000".into(),
                    email: "office@example.com".into(),
                },
            })
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.layout.device_dir().join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);
        crate::bir_module::restore_database_archive(&archive, 7).expect("the import runs");
        // The room the screens read now is the restored file — its taxpayer is there.
        let (handle, _) = crate::open_room::current().expect("a room is open");
        let guard = bir_core::lock_timing::lock(&handle).unwrap();
        let names: Vec<String> = guard
            .list_profiles()
            .unwrap()
            .iter()
            .map(|profile| profile.full_name.clone())
            .collect();
        assert!(
            names.iter().any(|name| name == "Restored Holdings Inc"),
            "the room does not serve the restored file (the placeholder is still served): {names:?}"
        );
    }

    /// X1: the room path is process-wide, never per-thread. A second engine thread (the print
    /// preview's) reads the open room's id and path; after B signs in it reads B's, never A's.
    #[test]
    fn a_second_engine_reads_the_open_room_not_a_stale_one() {
        let _turn = turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        let room_a = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        // A fresh thread — a second engine booting the host — reads the same room.
        let on_another_engine = std::thread::spawn(|| {
            (
                bir_core::db::open_room_id(),
                bir_core::db::app_database_path(),
            )
        })
        .join()
        .unwrap();
        assert_eq!(
            on_another_engine.0,
            Some(room_a.clone()),
            "a second engine reads no room (the path is per-thread)"
        );
        assert_eq!(
            on_another_engine.1,
            scratch.layout.room_database(&room_a),
            "a second engine's database path is not the room's file"
        );
        // B signs in: the second engine reads B's room, never A's.
        let b = stored("user-b");
        assert!(matches!(open_session_room(&b), SessionRoom::Room { .. }));
        let room_b = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&b.server_url).unwrap(),
            &b.user_id,
        );
        let after_switch = std::thread::spawn(bir_core::db::open_room_id)
            .join()
            .unwrap();
        assert_eq!(
            after_switch,
            Some(room_b),
            "a second engine still reads A's room after B signed in"
        );
        crate::open_room::close_room();
    }

    /// §A.8: after B signs in, no screen and no agent reads room A — the slot serves only the open
    /// room, and a handle of room A still held somewhere is never what new reads go through.
    #[test]
    fn no_screen_or_agent_reads_A_after_B_signs_in() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        // A marker in room A, as the owner's screen would write one.
        let (handle_a, _) = crate::open_room::current().unwrap();
        {
            let guard = bir_core::lock_timing::lock(&handle_a).unwrap();
            guard.set_setting("a3a.marker", "room-a").unwrap();
        }
        crate::open_room::close_room();
        let b = stored("user-b");
        assert!(matches!(open_session_room(&b), SessionRoom::Room { .. }));
        // The screen path: whatever open_database_slot serves now is room B, and it has no A rows.
        let (handle_b, _) = crate::open_room::current().unwrap();
        assert!(
            !std::sync::Arc::ptr_eq(&handle_a, &handle_b),
            "B reads the same handle as A"
        );
        let seen = {
            let guard = bir_core::lock_timing::lock(&handle_b).unwrap();
            guard.get_setting("a3a.marker").unwrap()
        };
        assert_eq!(seen.as_deref(), None, "B's screen reads room A's rows");
    }

    /// #423's review, A3a item 1: the one-copy guard is device-wide now, and an older build — which
    /// only knows the database file's owner lock — still finds the new one, because the new build
    /// holds the open file's owner lock for as long as its connection lives.
    #[test]
    fn older_version_refused_while_the_new_one_runs() {
        let _turn = turn();
        let scratch = scratch();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        db.checkpoint().unwrap();
        drop(db);
        let a = stored("user-a");
        assert!(matches!(open_session_room(&a), SessionRoom::Room { .. }));
        let room = bir_core::rooms::room_id_for(
            &bir_core::rooms::room_origin(&a.server_url).unwrap(),
            &a.user_id,
        );
        let room_file = scratch.layout.room_database(&room);
        // The older build's check: the database file's owner lock. Refused while the new one runs.
        assert!(
            bir_core::db::raw_take_owner_lock_for_test(&room_file).is_err(),
            "an older build opened the room file while the new one runs"
        );
        // A second new copy's check: the device-wide guard. Refused while this one runs.
        assert!(
            bir_core::db::take_device_instance_lock(scratch.layout.device_dir()).is_err(),
            "a second copy took the device guard while this one runs"
        );
        crate::open_room::close_room();
    }
}

// ------------------------------------------------------------------------------------------------ the host verbs

/// TypeScript declarations of the migration verbs (the pages under sign-in look B).
pub const DECLARATIONS: &str = r#"
/** The migration state the sign-in flow shows a page for after session_opened. */
export function migration_state(): {
    kind: "none" | "moving" | "transient" | "permanent" | "moved_not_opened" | "offered";
    copy_saved: boolean;
    detail: string;
};
/** The transient page's Try again: the claim and the room open run again for the saved sign-in. */
export function migration_try_again(): void;
/** The offer page's answer: `add` runs the whole-profile export of the found file and imports it into
 *  the room, then the file is renamed aside; `false` leaves it for the next launch. */
export function migration_offer_answer(add: boolean): void;
/** How many returns of the open room are Queued or sending now — the sign-out confirm's count, no names. */
export function returns_waiting_count(): number;
/** The pre-rooms safety copy of the open room's claim, for Settings › Import / Export: kept-until
 *  date (unix), or the reason it is kept, or none when there is no copy. */
export function safety_copy_status(): { kind: "none" } | { kind: "until"; stamp: number } | { kind: "kept"; reason: string };
"#;

/// Register the migration verbs (the moving and failure pages, the offer, the sign-out count, the
/// Settings safety-copy row).
pub fn register(module: HostModule) -> HostModule {
    module
        .function("migration_state", |_| migration_state_value())
        .function("migration_try_again", |_| migration_try_again())
        .function("migration_offer_answer", migration_offer_answer)
        .function("returns_waiting_count", |_| returns_waiting_count())
        .function("safety_copy_status", |_| safety_copy_status())
}

fn migration_state_value() -> HostResult {
    let (kind, copy_saved, detail) = match migration_state() {
        None => ("none", false, String::new()),
        Some(MigrationState::Moving) => ("moving", false, String::new()),
        Some(MigrationState::Transient { copy_saved, detail }) => ("transient", copy_saved, detail),
        Some(MigrationState::Permanent { detail }) => ("permanent", false, detail),
        Some(MigrationState::MovedNotOpened { detail }) => ("moved_not_opened", false, detail),
        Some(MigrationState::Offered {
            user_id,
            room_id,
            generation,
        }) => {
            let still = crate::open_room::user_id().as_deref() == Some(user_id.as_str())
                && crate::open_room::room_id().as_ref() == Some(&room_id.clone())
                && crate::open_room::generation() == Some(generation);
            if still {
                ("offered", false, String::new())
            } else {
                ("none", false, String::new())
            }
        }
    };
    Ok(HostObject::new()
        .field("kind", kind)
        .field("copy_saved", copy_saved)
        .field("detail", detail)
        .into())
}

/// The transient page's Try again: the launch order runs again for the saved sign-in. The page
/// re-reads migration_state after it.
fn migration_try_again() -> HostResult {
    let stored = bir_core::cloud_account::load_stored_account()
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new("no one is signed in"))?;
    open_session_room(&stored);
    Ok(HostValue::Null)
}

/// §A.3.6: the offer's answer. Add runs the existing whole-profile export on the found file and
/// imports the archive into the open room, then renames the file to `bir_data.offered-<stamp>.db`;
/// Not now leaves it, and the offer shows again at the next launch.
fn migration_offer_answer(arguments: &HostArguments) -> HostResult {
    let add = arguments.boolean(0)?;
    if !add {
        set_migration(None);
        return Ok(HostValue::Null);
    }
    // X2: the answer runs only for the account the offer was made to, in the room it was made in.
    // The slot is read once (the reviewer's note): four separate reads could interleave a sign-out.
    let slot = crate::open_room::snapshot();
    let bound = match (
        &*MIGRATION
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        &slot,
    ) {
        (
            Some(MigrationState::Offered {
                user_id,
                room_id,
                generation,
            }),
            Some((_, open_generation, open_room, open_user)),
        ) => {
            open_user == user_id
                && open_room.as_ref() == Some(room_id)
                && open_generation == generation
        }
        _ => false,
    };
    if !bound {
        return Err(HostError::new(
            "the offer was not made to the account and room open now; the offer is the account's it was made to",
        ));
    }
    let layout = Layout::here();
    let legacy = layout.legacy_database();
    if !legacy.is_file() {
        set_migration(None);
        return Err(HostError::new("the file that was offered is gone"));
    }
    // The probe first (N1), on a scratch copy: a credential means the legacy unlock page, which is
    // A3a-2 — until then the offer cannot run on a locked file.
    if let Ok(bir_core::legacy_claim::LegacyProbe::Locked(_)) =
        bir_core::legacy_claim::probe_legacy(&layout)
    {
        return Err(HostError::new(
            "This file is locked with a TIN and PIN. Moving locked files comes in a later version; choose Not now.",
        ));
    }
    let (handle, ..) = slot.ok_or_else(|| HostError::new("no room is open to add them to"))?;
    let stamp = bir_core::bir_submit::device::now();
    let scratch_dir = layout.device_dir().join("scratch");
    std::fs::create_dir_all(&scratch_dir).map_err(|error| HostError::new(error.to_string()))?;
    let archive = scratch_dir.join(format!("offered-{stamp}.zip"));
    let result = (|| -> Result<(), String> {
        let offered = bir_core::db::Database::open(&legacy).map_err(|error| error.to_string())?;
        bir_core::export::export_all_profiles_data(&offered, &archive)
            .map_err(|error| error.to_string())?;
        drop(offered);
        {
            let room = bir_core::lock_timing::lock(&handle)
                .map_err(|_| "database lock poisoned".to_owned())?;
            bir_core::import::import_profile_data(&room, &archive)
                .map_err(|error| error.to_string())?;
        }
        std::fs::rename(
            &legacy,
            layout
                .device_dir()
                .join(format!("bir_data.offered-{stamp}.db")),
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&archive);
    match result {
        Ok(()) => {
            set_migration(None);
            Ok(HostValue::Null)
        }
        Err(error) => Err(HostError::new(error)),
    }
}

/// The sign-out confirm's count (§A.5): the open room's Queued returns, plus any send in flight —
/// a number, never a name. Zero while no room is open, so the confirm does not show.
fn returns_waiting_count() -> HostResult {
    let count = match crate::open_room::current() {
        None => 0,
        Some((handle, _)) => {
            let guard = match bir_core::lock_timing::lock(&handle) {
                Ok(guard) => guard,
                Err(_) => return Ok(HostValue::Number(0.0)),
            };
            let queued = guard.count_queued_drafts().unwrap_or(0);
            let open_prefix = crate::open_room::job_key_prefix();
            let sending = bir_core::background_cron::get_active_jobs()
                .lock()
                .map(|jobs| {
                    jobs.iter()
                        .filter(|key| {
                            bir_core::background_cron::room_of_job_key(key) == open_prefix
                        })
                        .count() as i64
                })
                .unwrap_or(0);
            queued + sending
        }
    };
    // X6: a plain number, as the declaration says and the page compares — the object it used to
    // return made `waiting > 0` false forever, so the sign-out count and the notice never showed.
    Ok(HostValue::Number(count as f64))
}

/// The Settings row's copy state (§A.3.8): the pre-rooms zip of this room's claim — kept until the
/// date the rule lets it go, or "kept" with the reason when the rule already keeps it for good.
fn safety_copy_status() -> HostResult {
    let layout = Layout::here();
    let claim = match LegacyClaim::at(layout) {
        Ok(claim) => claim,
        Err(_) => return Ok(HostObject::new().field("kind", "none").into()),
    };
    let record = match claim.record() {
        Ok(Some(record)) => record,
        _ => return Ok(HostObject::new().field("kind", "none").into()),
    };
    // X5: the record is the device's one claim, but the row belongs in this account's row only
    // while the move it names IS this room — otherwise the person reads "safety copy kept" for
    // another account's move.
    let open_room = crate::open_room::room_id().map(|room| room.as_str().to_owned());
    if record.room.as_deref() != open_room.as_deref() {
        return Ok(HostObject::new().field("kind", "none").into());
    }
    let Some(zip) = &record.zip else {
        return Ok(HostObject::new().field("kind", "none").into());
    };
    if !std::path::Path::new(zip).is_file() {
        return Ok(HostObject::new().field("kind", "none").into());
    }
    // The rule's keep-until: the claim's done stamp plus ZIP_KEEP_DAYS. Reasons it is kept for good
    // read off the rule itself.
    match claim.zip_due(chrono::Utc::now()) {
        Ok(_) => Ok(HostObject::new().field("kind", "none").into()),
        Err(bir_core::legacy_claim::ZipKept::TooSoon { until }) => Ok(HostObject::new()
            .field("kind", "until")
            .field("stamp", until.timestamp())
            .into()),
        Err(reason) => Ok(HostObject::new()
            .field("kind", "kept")
            .field("reason", format!("{reason:?}"))
            .into()),
    }
}
