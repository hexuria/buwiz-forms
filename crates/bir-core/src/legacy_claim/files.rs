//! The claim's file work: the raw safety zip (step 3), the move (step 4), the offer's "Add", the sweep.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

#[cfg(test)]
use super::{
    Added, ClaimRoom, Failure, OfferConsumptionMatch, OfferConsumptionState, Step, UnlockedBy,
};
use super::{
    Cut, Permanent, PermanentReason, SCRATCH_DIR, Transient, TransientCause, ZIP_PREFIX, cut,
};
use crate::rooms::{Layout, ROOM_DB_FILE, RoomId};
#[cfg(test)]
use crate::safe_fs::FileId;
use crate::safe_fs::Placed;

/// The tables whose rows make a room "not empty" (PLAN §A.3.5: no taxpayer, no draft, no filed return).
const DATA_TABLES: [&str; 6] = [
    "profiles",
    "taxpayers",
    "form_drafts",
    "submissions",
    "submission_receipts",
    "legacy_filed_returns",
];

pub(super) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

/// Whether `path` names anything (a broken link included). Only a clear "not found" is `false`.
pub(super) fn exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn is_disk_full(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::StorageFull {
        return true;
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ENOSPC)
    }
    #[cfg(windows)]
    {
        // ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL
        matches!(error.raw_os_error(), Some(39) | Some(112))
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

pub(super) fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut HashWriter(&mut hash))?;
    Ok(hex::encode(hash.finalize()))
}

struct HashWriter<'a>(&'a mut Sha256);

impl Write for HashWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A reader that hashes what passes through it.
struct HashReader<R> {
    inner: R,
    hash: Sha256,
}

impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hash.update(&buf[..read]);
        Ok(read)
    }
}

/// Remove `path` and make the removal durable in `dir`.
pub(super) fn remove_and_sync(path: &Path, dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    crate::safe_fs::sync_dir(dir)
}

fn transient(cause: TransientCause, copy_saved: bool, detail: impl Into<String>) -> Transient {
    Transient {
        cause,
        copy_saved,
        record_room: false,
        detail: detail.into(),
    }
}

fn io_transient(what: &str, error: &std::io::Error, copy_saved: bool) -> Transient {
    tracing::warn!(what, "legacy claim file operation failed: {error}");
    let cause = if is_disk_full(error) {
        TransientCause::DiskFull
    } else {
        TransientCause::Io
    };
    transient(
        cause,
        copy_saved,
        "the device cannot complete the file operation",
    )
}

fn sqlite_transient(what: &str, error: &rusqlite::Error, copy_saved: bool) -> Transient {
    tracing::warn!(what, "legacy claim database operation failed: {error}");
    let cause = match error {
        rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
            rusqlite::ErrorCode::DiskFull => TransientCause::DiskFull,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                TransientCause::WalReturned
            }
            _ => TransientCause::Io,
        },
        _ => TransientCause::Io,
    };
    transient(cause, copy_saved, "the room database cannot be used")
}

/// A `-wal` or `-shm` beside `database`.
pub(super) fn sidecars_present(database: &Path) -> std::io::Result<bool> {
    Ok(exists(&with_suffix(database, "-wal"))? || exists(&with_suffix(database, "-shm"))?)
}

/// A bare connection with no create and no migrations folds the `-wal` into the file and closes, so the main file
/// alone holds every commit. Answers the file's `user_version`. Something else holding the file is a `-wal` that came
/// back (transient, with its cause).
fn checkpoint_and_close(database: &Path, copy_saved: bool) -> Result<i32, Transient> {
    let conn = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| sqlite_transient("open", &error, copy_saved))?;
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|error| sqlite_transient("busy timeout", &error, copy_saved))?;
    let (busy, _, _): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|error| sqlite_transient("checkpoint", &error, copy_saved))?;
    if busy != 0 {
        return Err(transient(
            TransientCause::WalReturned,
            copy_saved,
            "the checkpoint was held back by another connection",
        ));
    }
    let user_version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| sqlite_transient("user_version", &error, copy_saved))?;
    conn.close()
        .map_err(|(_, error)| sqlite_transient("close", &error, copy_saved))?;
    if sidecars_present(database).map_err(|error| io_transient("sidecars", &error, copy_saved))? {
        return Err(transient(
            TransientCause::WalReturned,
            copy_saved,
            "a -wal or -shm is still beside the file after its checkpoint",
        ));
    }
    Ok(user_version)
}

/// Step 3's result.
#[derive(Debug, Clone)]
pub(super) struct SafetyCopy {
    pub(super) user_version: i32,
    pub(super) raw_sha256: String,
    pub(super) zip_name: String,
    pub(super) zip_sha256: String,
}

pub(super) enum CopyError {
    Permanent(Permanent),
    Transient(Transient),
}

/// Step 3 (PLAN §A.3.2, M2): checkpoint with a bare connection, record the version, hash the raw file, and keep one
/// zip per raw file: an existing `…-<sha16>.zip` whose entry is this file is reused once re-checked; otherwise the
/// bytes are written as the single top-level `bir_data.db` of a `.partial`, fsynced, renamed without replacing, the
/// folder fsynced, and read back.
pub(super) fn raw_safety_copy(layout: &Layout, legacy: &Path) -> Result<SafetyCopy, CopyError> {
    match crate::db::database_file_kind(legacy) {
        Ok(crate::db::DatabaseFileKind::Other) => {
            return Err(CopyError::Permanent(Permanent {
                reason: PermanentReason::NotPlainSqlite,
                detail: "the legacy file is not a plain SQLite database".into(),
            }));
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!("legacy claim file kind not read: {error}");
            return Err(CopyError::Transient(transient(
                TransientCause::Io,
                false,
                "the legacy file could not be read",
            )));
        }
    }
    let user_version = checkpoint_and_close(legacy, false).map_err(CopyError::Transient)?;
    let supported = crate::db::current_schema_version().map_err(|error| {
        tracing::warn!("legacy claim schema version not read: {error}");
        CopyError::Transient(transient(
            TransientCause::Io,
            false,
            "the device cannot complete the file operation",
        ))
    })?;
    if user_version > supported {
        return Err(CopyError::Permanent(Permanent {
            reason: PermanentReason::NewerSchema,
            detail: format!("schema {user_version}, this build opens up to {supported}"),
        }));
    }
    let io = |what: &str| {
        let what = what.to_owned();
        move |error: std::io::Error| CopyError::Transient(io_transient(&what, &error, false))
    };
    let raw_sha256 = sha256_file(legacy).map_err(io("hash"))?;
    let backups = layout.backups_dir();
    let made_backups = !exists(&backups).map_err(io("backups"))?;
    std::fs::create_dir_all(&backups).map_err(io("backups"))?;
    if made_backups {
        crate::safe_fs::sync_dir(layout.device_dir()).map_err(io("fsync"))?;
    }
    remove_partials(&backups);

    let short = &raw_sha256[..16];
    for existing in pre_rooms_zips(layout) {
        let name = existing
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.ends_with(&format!("-{short}.zip"))
            && zip_entry_sha256(&existing).ok().as_deref() == Some(&*raw_sha256)
        {
            let zip_sha256 = sha256_file(&existing).map_err(io("zip hash"))?;
            tracing::info!(
                zip = name,
                "legacy claim: the safety copy of this file is already saved; reused"
            );
            return Ok(SafetyCopy {
                user_version,
                raw_sha256,
                zip_name: name.to_owned(),
                zip_sha256,
            });
        }
    }

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let zip_name = format!("{ZIP_PREFIX}{stamp}-{short}.zip");
    let final_path = backups.join(&zip_name);
    let partial = with_suffix(&final_path, ".partial");
    if let Some(error) = super::injected(super::Inject::Zip) {
        return Err(CopyError::Transient(io_transient("zip", &error, false)));
    }
    let entry_sha256 = write_zip(legacy, &partial).map_err(|error| {
        let _ = std::fs::remove_file(&partial);
        CopyError::Transient(io_transient("zip", &error, false))
    })?;
    if entry_sha256 != raw_sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(CopyError::Transient(transient(
            TransientCause::FileChanged,
            false,
            "the legacy file changed while it was copied",
        )));
    }
    crate::safe_fs::rename_no_replace(&partial, &final_path).map_err(|error| {
        let _ = std::fs::remove_file(&partial);
        CopyError::Transient(io_transient("zip rename", &error, false))
    })?;
    crate::safe_fs::sync_dir(&backups).map_err(io("fsync"))?;
    cut(Cut::AfterZipWritten);
    // Read back: the entry is the raw file.
    let read_back = zip_entry_sha256(&final_path).map_err(io("zip read-back"))?;
    if read_back != raw_sha256 {
        return Err(CopyError::Transient(transient(
            TransientCause::Io,
            false,
            "the safety copy did not read back as the file",
        )));
    }
    let zip_sha256 = sha256_file(&final_path).map_err(io("zip hash"))?;
    Ok(SafetyCopy {
        user_version,
        raw_sha256,
        zip_name,
        zip_sha256,
    })
}

/// Write `database` as the single top-level `bir_data.db` of a zip at `out` (the format the previous version's
/// Settings › Import restores), fsync it, and answer the entry's SHA-256.
fn write_zip(database: &Path, out: &Path) -> std::io::Result<String> {
    let size = std::fs::metadata(database)?.len();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(size >= u64::from(u32::MAX) / 2);
    zip.start_file(ROOM_DB_FILE, options)
        .map_err(std::io::Error::other)?;
    let mut reader = HashReader {
        inner: std::fs::File::open(database)?,
        hash: Sha256::new(),
    };
    std::io::copy(&mut reader, &mut zip)?;
    let file = zip.finish().map_err(std::io::Error::other)?;
    file.sync_all()?;
    Ok(hex::encode(reader.hash.finalize()))
}

/// The SHA-256 of the zip's one entry, `bir_data.db`. A zip with any other entry is refused.
pub(super) fn zip_entry_sha256(zip_path: &Path) -> std::io::Result<String> {
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(zip_path)?).map_err(std::io::Error::other)?;
    if archive.len() != 1 {
        return Err(std::io::Error::other(
            "the safety copy has more than one entry",
        ));
    }
    let mut entry = archive
        .by_name(ROOM_DB_FILE)
        .map_err(std::io::Error::other)?;
    let mut hash = Sha256::new();
    std::io::copy(&mut entry, &mut HashWriter(&mut hash))?;
    Ok(hex::encode(hash.finalize()))
}

/// The pre-rooms zips in `backups/`, by name.
pub(super) fn pre_rooms_zips(layout: &Layout) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(layout.backups_dir()) else {
        return Vec::new();
    };
    let mut zips: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(ZIP_PREFIX) && name.ends_with(".zip"))
        })
        .collect();
    zips.sort();
    zips
}

fn remove_partials(backups: &Path) {
    let Ok(entries) = std::fs::read_dir(backups) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(ZIP_PREFIX) && name.ends_with(".zip.partial"))
            && let Err(error) = std::fs::remove_file(&path)
        {
            tracing::warn!(zip = %path.display(), "partial safety copy not removed: {error}");
        }
    }
}

/// The launch sweep: every probe copy under `scratch/`, and partial zips.
pub(super) fn sweep_scratch(layout: &Layout) {
    let scratch = layout.device_dir().join(SCRATCH_DIR);
    if let Ok(entries) = std::fs::read_dir(&scratch) {
        for path in entries.flatten().map(|entry| entry.path()) {
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(error) = removed {
                tracing::warn!(path = %path.display(), "scratch copy not swept: {error}");
            }
        }
    }
    remove_partials(&layout.backups_dir());
}

/// Whether the room file at `path` has no taxpayer, no draft and no filed return (PLAN §A.3.5). Opened with no create;
/// a file that cannot be read is never "empty", so it is never set aside.
pub(super) fn room_is_empty(path: &Path) -> bool {
    let Ok(conn) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    for table in DATA_TABLES {
        let present: Result<i64, _> = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        );
        match present {
            Ok(0) => continue,
            Ok(_) => {}
            Err(_) => return false,
        }
        let rows: Result<i64, _> =
            conn.query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |row| {
                row.get(0)
            });
        if !matches!(rows, Ok(0)) {
            return false;
        }
    }
    true
}

/// Step 4 (PLAN §A.3.2): the zip re-checked, the room folder made, an empty room set aside, the legacy file linked
/// into the room (a no-replace rename where links are not supported) and unlinked, the safety files moved along, each
/// act followed by its folder's fsync. Answers the name an empty room's file was set aside under.
pub(super) fn move_into_room(
    layout: &Layout,
    legacy: &Path,
    room: &RoomId,
    copy: &SafetyCopy,
    empty_room_aside: bool,
) -> Result<Option<String>, Transient> {
    let io = |what: &'static str| move |error: std::io::Error| io_transient(what, &error, true);
    // M1: the zip is there and is the one checked.
    let zip = layout.backups_dir().join(&copy.zip_name);
    match sha256_file(&zip) {
        Ok(hash) if hash == copy.zip_sha256 => {}
        Ok(_) | Err(_) => {
            return Err(transient(
                TransientCause::FileChanged,
                false,
                "the safety copy is missing or changed since it was checked",
            ));
        }
    }
    if sidecars_present(legacy).map_err(io("sidecars"))? {
        return Err(transient(
            TransientCause::WalReturned,
            true,
            "a -wal or -shm came back beside the legacy file after its checkpoint",
        ));
    }

    let rooms = layout.rooms_dir();
    let made_rooms = !exists(&rooms).map_err(io("rooms"))?;
    let room_dir = layout.room_dir(room);
    std::fs::create_dir_all(&room_dir).map_err(io("room folder"))?;
    if made_rooms {
        crate::safe_fs::sync_dir(layout.device_dir()).map_err(io("fsync"))?;
    }
    crate::safe_fs::sync_dir(&rooms).map_err(io("fsync"))?;
    cut(Cut::AfterRoomDir);

    let room_file = layout.room_database(room);
    let mut aside = None;
    let mut linked_already = false;
    if exists(&room_file).map_err(io("room file"))? {
        if crate::safe_fs::file_id(&room_file)
            .ok()
            .is_some_and(|id| crate::safe_fs::file_id(legacy).ok() == Some(id))
        {
            linked_already = true;
        } else if empty_room_aside {
            aside = Some(set_empty_room_aside(&room_file, &room_dir)?);
            cut(Cut::AfterEmptyAside);
        } else {
            return Err(transient(
                TransientCause::RoomFileTaken,
                true,
                "a room file with data is already where the legacy file would go",
            ));
        }
    }

    let placed = if linked_already {
        Placed::Linked
    } else {
        if let Some(error) = super::injected(super::Inject::Link) {
            return Err(io_transient("link", &error, true));
        }
        match crate::safe_fs::link_no_replace(legacy, &room_file) {
            Ok(placed) => placed,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(transient(
                    TransientCause::RoomFileTaken,
                    true,
                    "a file appeared where the legacy file would go",
                ));
            }
            Err(error) => return Err(io_transient("link", &error, true)),
        }
    };
    crate::safe_fs::sync_dir(&room_dir).map_err(io("fsync"))?;
    cut(Cut::AfterLink);
    if placed == Placed::Linked {
        remove_and_sync(legacy, layout.device_dir()).map_err(io("unlink"))?;
    } else {
        crate::safe_fs::sync_dir(layout.device_dir()).map_err(io("fsync"))?;
    }
    cut(Cut::AfterUnlink);

    move_safety_files(layout, &room_dir);
    cut(Cut::AfterSafetyFiles);
    Ok(aside)
}

/// The empty room's file goes to `bir_data.empty-<stamp>.db` beside it, never replacing; it is checkpointed with a
/// bare connection first, under the room's own owner lock, so no `-wal` is left for the next file to read.
fn set_empty_room_aside(room_file: &Path, room_dir: &Path) -> Result<String, Transient> {
    let lock = crate::db::hold_owner_lock(room_file).map_err(|error| {
        tracing::warn!("empty room owner lock not taken: {error}");
        transient(
            TransientCause::OwnerLockHeld,
            true,
            "the empty room is open elsewhere",
        )
    })?;
    if !room_is_empty(room_file) {
        return Err(transient(
            TransientCause::RoomFileTaken,
            true,
            "the room is no longer empty",
        ));
    }
    checkpoint_and_close(room_file, true)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let name = format!("bir_data.empty-{stamp}.db");
    crate::safe_fs::rename_no_replace(room_file, &room_dir.join(&name))
        .map_err(|error| io_transient("empty room aside", &error, true))?;
    crate::safe_fs::sync_dir(room_dir).map_err(|error| io_transient("fsync", &error, true))?;
    drop(lock);
    Ok(name)
}

/// Move the `bir_data.safety-*.db` import safety copies beside the legacy file into the room folder, the same way:
/// never replacing. One that cannot be moved stays where it is, and is logged.
fn move_safety_files(layout: &Layout, room_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(layout.device_dir()) else {
        return;
    };
    let mut moved = false;
    for path in entries.flatten().map(|entry| entry.path()) {
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if !(name.starts_with("bir_data.safety-") && name.ends_with(".db")) {
            continue;
        }
        let target = room_dir.join(&name);
        let result = match crate::safe_fs::link_no_replace(&path, &target) {
            Ok(Placed::Linked) => std::fs::remove_file(&path),
            Ok(Placed::Renamed) => Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::AlreadyExists
                    && crate::safe_fs::file_id(&path).ok()
                        == crate::safe_fs::file_id(&target).ok() =>
            {
                // Linked by a run that was cut before its unlink.
                std::fs::remove_file(&path)
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => moved = true,
            Err(error) => {
                tracing::warn!(file = name, "safety copy not moved into the room: {error}")
            }
        }
    }
    if moved {
        for dir in [room_dir, layout.device_dir()] {
            if let Err(error) = crate::safe_fs::sync_dir(dir) {
                tracing::warn!(folder = %dir.display(), "fsync after the safety copies: {error}");
            }
        }
    }
}

#[cfg(any(test, debug_assertions))]
static FAIL_NEXT_OFFER_RENAME: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Test seam, debug builds only: the next offered-file rename answers as a sharing/AV failure.
#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
pub fn fail_next_offer_rename_for_test() {
    FAIL_NEXT_OFFER_RENAME.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(any(test, debug_assertions))]
static CUT_AFTER_IMPORTING_MARK: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Test seam, debug builds only: the next offer answers as though the process was cut immediately
/// after its `importing` mark and before the import could commit.
#[cfg(any(test, debug_assertions))]
#[doc(hidden)]
pub fn cut_next_offer_after_importing_mark_for_test() {
    CUT_AFTER_IMPORTING_MARK.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Test-only remnant of the removed Add path (§A.3.6): the whole-profile export imported into `room`,
/// then the legacy file renamed aside. Production callers never reach it.
#[cfg(test)]
pub(super) fn add_offered(
    claim: &super::LegacyClaim,
    room: &crate::db::Database,
    claim_room: &ClaimRoom,
    probed: Option<FileId>,
) -> Result<Added, Failure> {
    let layout = &claim.layout;
    let legacy = layout.legacy_database();
    let fail = |transient: Transient| Failure::Transient(transient);
    if crate::safe_fs::file_id(&legacy).ok() != probed || probed.is_none() {
        return Err(fail(transient(
            TransientCause::FileChanged,
            false,
            "the file offered is not the one probed",
        )));
    }
    checkpoint_and_close(&legacy, false).map_err(fail)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let scratch = layout
        .device_dir()
        .join(SCRATCH_DIR)
        .join(format!("offer-{stamp}-{}", std::process::id()));
    let io =
        |what: &'static str| move |error: std::io::Error| fail(io_transient(what, &error, false));
    std::fs::create_dir_all(&scratch).map_err(io("scratch"))?;
    let result = (|| -> Result<Added, Failure> {
        let copy = scratch.join(ROOM_DB_FILE);
        std::fs::copy(&legacy, &copy).map_err(io("copy"))?;
        // The mark keys these exact bytes. Opening the copy may migrate it, so hash before the open.
        let content_sha256 = sha256_file(&copy).map_err(io("hash"))?;
        let source = crate::db::Database::open(&copy).map_err(|error| {
            Failure::Permanent(Permanent {
                reason: PermanentReason::CannotOpen,
                detail: error.to_string(),
            })
        })?;
        let profiles = source
            .list_profiles()
            .map_err(|error| {
                tracing::warn!("offered profiles not read: {error}");
                fail(transient(
                    TransientCause::Io,
                    false,
                    "the offered data could not be read",
                ))
            })?
            .len();
        let archive = scratch.join("profiles.zip");
        crate::export::export_all_profiles_data(&source, &archive).map_err(|error| {
            tracing::warn!("offered profiles not exported: {error}");
            fail(transient(
                TransientCause::Io,
                false,
                "the offered data could not be exported",
            ))
        })?;
        drop(source);
        let probed = probed.expect("the identity was checked above");
        let mut imported = false;
        match claim
            .offer_consumption_match(&claim_room.room_id, &content_sha256)
            .map_err(super::store_failure)?
        {
            OfferConsumptionMatch::OtherRoom => {
                return Err(fail(transient(
                    TransientCause::FileChanged,
                    false,
                    "the file offered was already added to another account's room",
                )));
            }
            OfferConsumptionMatch::Own(OfferConsumptionState::Imported) => {}
            OfferConsumptionMatch::None
            | OfferConsumptionMatch::Own(OfferConsumptionState::Importing) => {
                claim
                    .write_offer_consumption(
                        &claim_room.room_id,
                        &content_sha256,
                        OfferConsumptionState::Importing,
                    )
                    .map_err(super::store_failure)?;
                #[cfg(any(test, debug_assertions))]
                if CUT_AFTER_IMPORTING_MARK.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    return Err(fail(transient(
                        TransientCause::Io,
                        false,
                        "injected offer-import cut after the importing mark",
                    )));
                }
                crate::import::import_profile_data(room, &archive).map_err(|error| {
                    tracing::warn!("offered profiles not imported: {error}");
                    fail(transient(
                        TransientCause::Io,
                        false,
                        "the offered data could not be imported",
                    ))
                })?;
                claim
                    .write_offer_consumption(
                        &claim_room.room_id,
                        &content_sha256,
                        OfferConsumptionState::Imported,
                    )
                    .map_err(super::store_failure)?;
                imported = true;
            }
        }

        // B2b: the consumed offer leaves a real claim record. Even when the rename is refused, a
        // later sign-in can see which room the file belongs to; the content mark is what stops the
        // offer for every other room.
        let mut record = claim
            .record()
            .map_err(super::store_failure)?
            .unwrap_or_default();
        record.room = Some(claim_room.room_id.as_str().to_owned());
        record.origin = Some(claim_room.origin.clone());
        record.user_id = Some(claim_room.user_id.clone());
        record.email = claim_room.email.clone();
        record.unlocked_by = record.unlocked_by.or(Some(UnlockedBy::NoLock));
        record.legacy_file = Some(probed);
        record.raw_sha256 = Some(content_sha256.clone());
        record.failure = None;
        record.step = Step::Done;
        record.done_at = Some(chrono::Utc::now().to_rfc3339());
        claim.write_record(&record).map_err(super::store_failure)?;

        let offered = layout
            .device_dir()
            .join(format!("bir_data.offered-{stamp}.db"));
        #[cfg(any(test, debug_assertions))]
        if FAIL_NEXT_OFFER_RENAME.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Err(io("rename")(std::io::Error::other(
                "injected offered-file rename failure",
            )));
        }
        crate::safe_fs::rename_no_replace(&legacy, &offered).map_err(io("rename"))?;
        crate::safe_fs::sync_dir(layout.device_dir()).map_err(io("fsync"))?;
        tracing::info!(
            profiles,
            "an older version's tax profiles were added to this account"
        );
        Ok(Added {
            profiles: if imported { profiles } else { 0 },
            offered_file: offered,
        })
    })();
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        tracing::warn!(folder = %scratch.display(), "offer scratch not removed (the next launch sweeps it): {error}");
    }
    result
}
