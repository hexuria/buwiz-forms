//! A3a: the one open room of this device (PLAN §A.4). The slot is process-wide so the submission
//! worker — on threads of its own — reads the same room the page does. Opening a room makes a new
//! Arc and a new generation; it never swaps inside an old one. A registry of Weak handles per room
//! lets a room whose in-flight sends still hold it be reopened on the same connection (a second
//! `Database::open` of the same file would fail its owner lock). §A.3.7 legacy mode: the legacy file
//! opened in place is the one slot (`room_id: None`), under its old lock, writing nothing beside it.

use std::sync::{Arc, Mutex, Weak};

use bir_core::db::Database;
use bir_core::rooms::{Layout, RoomId};

/// The one open room: whose it is, which generation, and the handle (§A.4). `room_id` is `None` in
/// legacy mode (§A.3.7): the legacy file in place, never a room; B2 counts the slot as a room open.
pub struct OpenRoom {
    pub room_id: Option<RoomId>,
    pub user_id: String,
    /// The normalised server origin the room is keyed by, with the user id (X1's note).
    pub origin: String,
    pub generation: u64,
    pub handle: Arc<Mutex<Database>>,
}

static SLOT: Mutex<Option<OpenRoom>> = Mutex::new(None);
/// Weak handles per room file, so a room whose in-flight sends still hold it reopens on the same
/// connection. Keyed by the room file's path (the legacy file's path in legacy mode).
/// The owner lock rides with the connection: the tuple is dropped — and the file's lock released —
/// when the last strong handle of it is gone (the registry sweep in `open_path` and `close_room`).
/// While it lives, an older build finds the file locked and refuses, exactly the one-copy rule it
/// knows (#423's review, item 1).
type RegistryEntry = (Weak<Mutex<Database>>, bir_core::db::OwnerLock);
type RegistryMap = std::collections::HashMap<std::path::PathBuf, RegistryEntry>;
static REGISTRY: std::sync::LazyLock<Mutex<RegistryMap>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// The device-wide one-copy guard this process holds, keyed by the device folder (#423's review,
/// item 1): the guard moved off the legacy database file — the databases are many in the rooms
/// world, the app instance is one. Held for the whole run once taken.
static DEVICE_GUARDS: std::sync::LazyLock<
    Mutex<std::collections::HashMap<std::path::PathBuf, bir_core::db::OwnerLock>>,
> = std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Take the device-wide one-copy guard for `device_dir`, or re-answer the one this process already
/// holds. `Err` while another copy — a new one or an older one running — holds it.
pub(crate) fn hold_device_guard(device_dir: &std::path::Path) -> Result<(), String> {
    let key = device_dir
        .canonicalize()
        .unwrap_or_else(|_| device_dir.to_path_buf());
    let mut guards = lock(&DEVICE_GUARDS);
    if guards.contains_key(&key) {
        return Ok(());
    }
    let guard = bir_core::db::take_device_instance_lock(device_dir)
        .map_err(|error| format!("another copy of the app has this device's data open: {error}"))?;
    guards.insert(key, guard);
    Ok(())
}
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The open room's handle and generation, if one is open — the worker's tick source (§A.4).
/// The whole slot in one read (X2's note): the handle, the generation, the room and the user,
/// never four reads a sign-out could interleave.
pub type SlotSnapshot = (Arc<Mutex<Database>>, u64, Option<RoomId>, String);

pub fn snapshot() -> Option<SlotSnapshot> {
    lock(&SLOT).as_ref().map(|room| {
        (
            room.handle.clone(),
            room.generation,
            room.room_id.clone(),
            room.user_id.clone(),
        )
    })
}

pub fn current() -> Option<(Arc<Mutex<Database>>, u64)> {
    lock(&SLOT)
        .as_ref()
        .map(|room| (room.handle.clone(), room.generation))
}

/// The open room's generation, or none while no room is open. Threads started for a room compare
/// theirs against this; a result of a generation that is gone is dropped and logged (§A.4).
pub fn generation() -> Option<u64> {
    lock(&SLOT).as_ref().map(|room| room.generation)
}

/// Whose room is open (the user id of the saved sign-in it was opened for).
pub fn user_id() -> Option<String> {
    lock(&SLOT).as_ref().map(|room| room.user_id.clone())
}

/// The normalised origin the open room is keyed by (X1's note), `None` while no room is open.
pub fn origin() -> Option<String> {
    lock(&SLOT).as_ref().map(|room| room.origin.clone())
}

/// The open room's id, `None` in legacy mode and while no room is open. Distinguish with
/// [`legacy_mode`]: legacy mode is a slot open with no room id.
pub fn room_id() -> Option<RoomId> {
    lock(&SLOT).as_ref().and_then(|room| room.room_id.clone())
}

/// Whether the slot is legacy mode (§A.3.7): the legacy file in place, writing nothing beside it.
pub fn legacy_mode() -> bool {
    lock(&SLOT)
        .as_ref()
        .is_some_and(|room| room.room_id.is_none())
}

/// Open `path` as the one slot for `user_id`, reusing the live connection a room's in-flight sends
/// still hold when there is one. Every open is a new generation, even of the room just closed.
fn open_path(
    path: &std::path::Path,
    room_id: Option<RoomId>,
    user_id: String,
    origin: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    // The one-copy guard (item 1): device-wide for the run, and the file's owner lock for as long
    // as its connection lives. The device folder is the room file's great-grandparent — or the
    // legacy file's parent in legacy mode.
    let device_dir = bir_core::db::room_device_dir(path)
        .or_else(|| path.parent().map(std::path::Path::to_path_buf))
        .ok_or_else(|| format!("no device folder for {}", path.display()))?;
    // G1: the whole room-open rule on the slot's real path — Database::open below migrates and
    // checkpoints, so a room never opens before start() ran or while its move evidence is
    // unchecked. Legacy mode passes no room id and is not a room. reopen_fresh comes through
    // here too (the import's reopen).
    if let Some(room) = &room_id {
        bir_core::legacy_claim::room_open_allowed(&bir_core::rooms::Layout::at(&device_dir), room)?;
    }
    hold_device_guard(&device_dir)?;
    // F10: REGISTRY is never held across Database::open — its migrations and checkpoint can take
    // real time, and every close/open of any room would wait behind them. Opens serialize on
    // OPEN_SERIAL instead, so the check-insert race stays closed without holding the registry.
    static OPEN_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _opening = lock(&OPEN_SERIAL);
    let handle = {
        let reuse = {
            let mut registry = lock(&REGISTRY);
            registry.retain(|_, (weak, _)| weak.strong_count() > 0);
            registry.get(path).and_then(|(weak, _)| Weak::upgrade(weak))
        };
        match reuse {
            Some(live) => live,
            None => {
                let owner_lock =
                    bir_core::db::hold_owner_lock(path).map_err(|error| error.to_string())?;
                let database = Database::open(path).map_err(|error| error.to_string())?;
                let handle = Arc::new(Mutex::new(database));
                lock(&REGISTRY).insert(path.to_path_buf(), (Arc::downgrade(&handle), owner_lock));
                handle
            }
        }
    };
    let generation = NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dropped = lock(&SLOT).replace(OpenRoom {
        room_id,
        user_id,
        origin,
        generation,
        handle: handle.clone(),
    });
    drop(dropped); // F17: outside the lock, as in close_room.
    Ok((handle, generation))
}

/// Reopen `path` fresh — a new connection, a new Arc, a new generation — never the registry's reuse.
/// For the restore (F1): the shared Arc's contents were swapped for the ephemeral placeholder, so the
/// reuse would serve it again. The stale entry goes first: its owner lock with it, since the swap
/// already dropped the old file's connection. Sends still in flight hold their own Arc and finish
/// against it, as ever.
pub(crate) fn reopen_fresh(
    path: &std::path::Path,
    room_id: Option<RoomId>,
    user_id: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    {
        let mut registry = lock(&REGISTRY);
        registry.remove(path);
    }
    // The reopen is the same account's room: the origin stands.
    let origin = origin().unwrap_or_default();
    open_path(path, room_id, user_id, origin)
}

/// Open the account's room as the one slot. The open goes through [`bir_core::rooms::open_room_guarded`]
/// in the end — R1: never before the claim's `start()` ran in this session. Until the guard's inner
/// open is wired here, the path open above is the same file.
pub fn open_room(
    layout: &Layout,
    room_id: RoomId,
    user_id: String,
    origin: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    open_path(
        &layout.room_database(&room_id),
        Some(room_id),
        user_id,
        origin,
    )
}

/// Open the legacy file in place as the one slot (§A.3.7 legacy mode): no room, no record, no zip,
/// nothing beside it. The owner lock of the open is the process holder (X1).
pub fn open_legacy_in_place(
    layout: &Layout,
    user_id: String,
    origin: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    open_path(&layout.legacy_database(), None, user_id, origin)
}

/// The open room's generation, or `None` while no room is open — and in single-room mode, where the
/// slot is never populated. Threads started for a room carry it (§A.4): their result lives only while
/// the open room's generation is still theirs.
pub fn current_generation() -> Option<u64> {
    lock(&SLOT).as_ref().map(|room| room.generation)
}

/// Close the room (§A.5 step 6): the slot's Arc and generation go. Sends already in flight finish
/// against their own Arc; the registry reopens the same connection if the room is opened again
/// before they finish. The room's file and its PIN stay.
/// Whether the registry holds a live connection for `path` (F8): a send's or screen's Arc of the
/// legacy file outliving its close means the file is still being read and written — a claim that
/// copies and moves it now would race those writes.
pub(crate) fn live_entry_for(path: &std::path::Path) -> bool {
    let mut registry = lock(&REGISTRY);
    registry.retain(|_, (weak, _)| weak.strong_count() > 0);
    registry
        .get(path)
        .and_then(|(weak, _)| Weak::upgrade(weak))
        .is_some()
}

/// The key this room's worker jobs file under (F6): the room's id in rooms mode, the fixed
/// key in single-room mode, "legacy" for the legacy file in place. The worker's source and every
/// reader asking after the open room's jobs build it here, so the two cannot drift.
pub fn job_key_prefix() -> String {
    if bir_core::db::single_room_database().is_some() {
        return bir_core::background_cron::fixed_room_key();
    }
    match lock(&SLOT).as_ref() {
        Some(open) => match &open.room_id {
            Some(room) => format!("room:{}", room.as_str()),
            None => "legacy".to_owned(),
        },
        None => bir_core::background_cron::fixed_room_key(),
    }
}

pub fn close_room() {
    // F4: a consent started in this room must not complete into whatever signs in next — its
    // loopback listener answers Err and accepts nothing more.
    bir_core::email::cancel_oauth_listeners();
    // F17: the taken room drops AFTER the lock lets go — its Arc's last drop runs the database's
    // close and checkpoint, real work no other open or close should wait behind.
    let dropped = lock(&SLOT).take();
    drop(dropped);
    // A dead connection's entry goes now, not at the next open: its file's owner lock releases when
    // the last strong handle is gone, so a sign-out frees the file for whatever comes next.
    lock(&REGISTRY).retain(|_, (weak, _)| weak.strong_count() > 0);
}

/// F9: quit releases everything the slot holds — the room's Arc and generation, the registry's
/// connections (each file's owner lock rides with them) and the device guards — so an update's
/// replacement process is never kept out by the one that just exited. Sends in flight finish
/// against their own Arcs, as ever; the process is ending either way.
pub fn release_all_for_quit() {
    lock(&SLOT).take();
    lock(&REGISTRY).clear();
    lock(&DEVICE_GUARDS).clear();
}

/// One turn for every test that opens a room in this process (F4: ONE turn for everything —
/// the machine, the slot's successors, the account gate and the owner-lock state are all
/// process-wide): tests in different modules would otherwise write them over each other. The
/// turn is bir-core's room test turn, so bir-core's room tests serialise with these and no two
/// turns can deadlock.
#[cfg(test)]
pub(crate) fn room_test_turn() -> std::sync::MutexGuard<'static, ()> {
    bir_core::db::room_path_test_turn()
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    lock(&SLOT).take();
    lock(&REGISTRY).clear();
}
