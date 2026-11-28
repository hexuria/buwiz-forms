//! A3a: the registry of open room connections (the design page, §1). The slot and its readers are
//! gone — the machine in [`crate::session`] is the only source of truth for who owns the open room.
//! What stays here is what the machine uses but does not own: the registry of Weak handles per room
//! file (a room whose in-flight sends still hold it reopens on the same connection — a second
//! `Database::open` of the same file would fail its owner lock), the serialised `open_path` that
//! returns a handle and installs NOTHING, the device-wide one-copy guard, and the job-key prefix
//! the worker's jobs file under.

use std::sync::{Arc, Mutex, Weak};

use bir_core::db::Database;

/// Weak handles per room file, keyed by the room file's path (the legacy file's path in legacy
/// mode). The owner lock rides with the connection: the entry is dropped — and the file's lock
/// released — when the last strong handle of it is gone. While it lives, an older build finds the
/// file locked and refuses, exactly the one-copy rule it knows (#423's review, item 1).
type RegistryEntry = (Weak<Mutex<Database>>, bir_core::db::OwnerLock);
type RegistryMap = std::collections::HashMap<std::path::PathBuf, RegistryEntry>;
static REGISTRY: std::sync::LazyLock<Mutex<RegistryMap>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// The device-wide one-copy guard this process holds, keyed by the device folder (#423's review,
/// item 1): the guard moved off the legacy database file — the databases are many in the rooms
/// world, the app instance is one. Taken ONCE at process start by `session::init` and held until
/// quit releases it last (§1 [D4c]); `open_path` never takes it.
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

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Open `path` and answer its handle — installing NOTHING (the design page §1: the machine's
/// commit is the only place a handle becomes the slot). Reuses the live connection a room's
/// in-flight sends still hold when there is one. The device guard is NOT taken here: init and
/// begin_open hold it (§12.2 item 4). Migrations and the checkpoint run inside `Database::open`,
/// outside SESSION, serialised against other opens on OPEN_SERIAL so the check-insert race stays
/// closed without holding the registry.
pub(crate) fn open_path(
    path: &std::path::Path,
    room_id: Option<&bir_core::rooms::RoomId>,
) -> Result<Arc<Mutex<Database>>, String> {
    // G1 (I4): the whole room-open rule on the room's real path — Database::open below migrates
    // and checkpoints, so a room never opens before start() ran or while its move evidence is
    // unchecked. Legacy mode passes no room id and is not a room.
    if let Some(room) = room_id {
        let device_dir = bir_core::db::room_device_dir(path)
            .ok_or_else(|| format!("no device folder for {}", path.display()))?;
        bir_core::legacy_claim::room_open_allowed(&bir_core::rooms::Layout::at(&device_dir), room)?;
    }
    static OPEN_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _opening = lock(&OPEN_SERIAL);
    let reuse = {
        let mut registry = lock(&REGISTRY);
        registry.retain(|_, (weak, _)| weak.strong_count() > 0);
        registry.get(path).and_then(|(weak, _)| Weak::upgrade(weak))
    };
    match reuse {
        Some(live) => Ok(live),
        None => {
            let owner_lock =
                bir_core::db::hold_owner_lock(path).map_err(|error| error.to_string())?;
            let database = Database::open(path).map_err(|error| error.to_string())?;
            let handle = Arc::new(Mutex::new(database));
            lock(&REGISTRY).insert(path.to_path_buf(), (Arc::downgrade(&handle), owner_lock));
            Ok(handle)
        }
    }
}

/// Reopen `path` fresh — a new connection, never the registry's reuse — installing nothing. For
/// the restore (F1): the shared Arc's contents were swapped for the ephemeral placeholder, so the
/// reuse would serve it again. The stale entry goes first: its owner lock with it, since the swap
/// already dropped the old file's connection. Sends still in flight hold their own Arc and finish
/// against it, as ever.
pub(crate) fn reopen_fresh(path: &std::path::Path) -> Result<Arc<Mutex<Database>>, String> {
    lock(&REGISTRY).remove(path);
    open_path(path, None)
}

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

/// Drop the registry entries whose connection is dead. A closed room's entry goes now, not at
/// the next open: the file's owner lock releases when the last strong handle is gone, so a
/// sign-out frees the file for whatever comes next.
pub(crate) fn sweep_registry() {
    lock(&REGISTRY).retain(|_, (weak, _)| weak.upgrade().is_some());
}

/// The key this room's worker jobs file under (F6): the room's id in rooms mode, the fixed
/// key in single-room mode, "legacy" for the legacy file in place. Reads the machine's snapshot —
/// the worker's source and every reader asking after the open room's jobs build it here, so the
/// two cannot drift.
pub fn job_key_prefix() -> String {
    if bir_core::db::single_room_database().is_some() {
        return bir_core::background_cron::fixed_room_key();
    }
    let snap = crate::session::snapshot();
    match snap.room {
        Some((_, _, Some(room), _)) => format!("room:{}", room.as_str()),
        Some((_, _, None, _)) if matches!(snap.phase_kind, crate::session::PhaseKind::Legacy) => {
            "legacy".to_owned()
        }
        _ => bir_core::background_cron::fixed_room_key(),
    }
}

/// F9: quit releases everything the registry holds — the connections (each file's owner lock
/// rides with them) and the device guards — so an update's replacement process is never kept out
/// by the one that just exited. Sends in flight finish against their own Arcs, as ever; the
/// process is ending either way.
pub fn release_all_for_quit() {
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
    lock(&REGISTRY).clear();
}
