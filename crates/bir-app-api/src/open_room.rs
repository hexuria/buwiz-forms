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
    pub generation: u64,
    pub handle: Arc<Mutex<Database>>,
}

static SLOT: Mutex<Option<OpenRoom>> = Mutex::new(None);
/// Weak handles per room file, so a room whose in-flight sends still hold it reopens on the same
/// connection. Keyed by the room file's path (the legacy file's path in legacy mode).
static REGISTRY: std::sync::LazyLock<
    Mutex<std::collections::HashMap<std::path::PathBuf, Weak<Mutex<Database>>>>,
> = std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The open room's handle and generation, if one is open — the worker's tick source (§A.4).
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
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    let handle = {
        let mut registry = lock(&REGISTRY);
        registry.retain(|_, weak| weak.strong_count() > 0);
        match registry.get(path).and_then(Weak::upgrade) {
            Some(live) => live,
            None => {
                let database = Database::open(path).map_err(|error| error.to_string())?;
                let handle = Arc::new(Mutex::new(database));
                registry.insert(path.to_path_buf(), Arc::downgrade(&handle));
                handle
            }
        }
    };
    let generation = NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    *lock(&SLOT) = Some(OpenRoom {
        room_id,
        user_id,
        generation,
        handle: handle.clone(),
    });
    Ok((handle, generation))
}

/// Open the account's room as the one slot. The open goes through [`bir_core::rooms::open_room_guarded`]
/// in the end — R1: never before the claim's `start()` ran in this session. Until the guard's inner
/// open is wired here, the path open above is the same file.
pub fn open_room(
    layout: &Layout,
    room_id: RoomId,
    user_id: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    if !bir_core::legacy_claim::claim_started_this_session(layout.device_dir()) {
        debug_assert!(
            false,
            "a room opened before the claim's start() ran in this session (R1)"
        );
        return Err(
            "the legacy claim's start() has not run for this device folder in this session (R1)"
                .into(),
        );
    }
    open_path(&layout.room_database(&room_id), Some(room_id), user_id)
}

/// Open the legacy file in place as the one slot (§A.3.7 legacy mode): no room, no record, no zip,
/// nothing beside it. The owner lock of the open is the process holder (X1).
pub fn open_legacy_in_place(
    layout: &Layout,
    user_id: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    open_path(&layout.legacy_database(), None, user_id)
}

/// Close the room (§A.5 step 6): the slot's Arc and generation go. Sends already in flight finish
/// against their own Arc; the registry reopens the same connection if the room is opened again
/// before they finish. The room's file and its PIN stay.
pub fn close_room() {
    lock(&SLOT).take();
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    lock(&SLOT).take();
    lock(&REGISTRY).clear();
}
