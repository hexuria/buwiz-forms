//! Rooms: one database per Buwiz account, and the device store beside them.
//!
//! Accounts-and-lock PLAN §A.1 and §A.2. A room is the database of one Buwiz account on one server, at
//! `<device folder>/rooms/<room-id>/bir_data.db`. The device folder is [`crate::platform::device_dir`]: on a
//! normal install the app's data folder, where the device-wide files (the saved sign-in, the relay device key,
//! the install id, the pending email sign-in) always were.
//!
//! This module gives:
//! - the room id ([`room_id_for`], [`room_for`]): a hash that keeps the email out of folder names and keeps a
//!   test-server account apart from a production one; the `rooms` table is asked first;
//! - the layout ([`Layout`]): where `device.db`, `rooms/`, `backups/` and `previews/` are;
//! - the device store ([`DeviceStore`]): `device.db` with its whole schema (O5), opened at
//!   `synchronous = FULL`, and never recreated silently (L2);
//! - the device-wide settings with their fallback (O1): [`device_setting_or`] answers `device_settings` first,
//!   then the same key in the database the app has open; [`set_device_setting`] writes `device_settings`.
//!
//! In A1 nothing opens a room: no `rooms/` folder is made by the app, and the legacy file stays the database.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::db::Database;

/// The device store's file, in the device folder.
pub const DEVICE_DB_FILE: &str = "device.db";
/// The folder of the rooms, in the device folder.
pub const ROOMS_DIR: &str = "rooms";
/// The folder of the pre-rooms safety copy (A2), in the device folder.
pub const BACKUPS_DIR: &str = "backups";
/// The folder of the print-preview HTML (A3b), in the device folder.
pub const PREVIEWS_DIR: &str = "previews";
/// A room's database file, in its room folder; also the legacy file's name in the device folder.
pub const ROOM_DB_FILE: &str = "bir_data.db";
/// `device.db`'s own `user_version`.
pub const DEVICE_SCHEMA_VERSION: i32 = 1;
/// The `migration` key that records a device store found missing while rooms were on disk (L2).
pub const DEVICE_STORE_LOST_KEY: &str = "device_store_lost";
/// Hex characters in a room id.
pub const ROOM_ID_LEN: usize = 32;
/// What the room id hash starts with: the id's own version.
const ROOM_ID_DOMAIN: &str = "buwiz-room-v1";
/// How long a write waits for another connection (the app and `bir-headless` may both have it open).
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// `device.db`'s whole schema (PLAN §A.2), fixed in A1 so later pull requests write tables that already exist.
const DEVICE_SCHEMA: &str = "
CREATE TABLE device_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE rooms (
    room_id TEXT PRIMARY KEY, origin TEXT NOT NULL, user_id TEXT NOT NULL,
    email TEXT, created_at TEXT NOT NULL, from_legacy INTEGER NOT NULL DEFAULT 0,
    UNIQUE (origin, user_id));
CREATE TABLE owner_lock (
    room_id TEXT PRIMARY KEY REFERENCES rooms(room_id),
    pin_phc TEXT,
    set_at TEXT,
    biometrics_on INTEGER NOT NULL DEFAULT 0,
    apple_domain_state BLOB);
CREATE TABLE pin_attempts (
    room_id TEXT NOT NULL,
    subject TEXT NOT NULL,
    failures INTEGER NOT NULL DEFAULT 0,
    wait_secs INTEGER NOT NULL DEFAULT 0,
    started_unix INTEGER NOT NULL DEFAULT 0,
    started_boot_secs INTEGER NOT NULL DEFAULT 0,
    started_boot_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (room_id, subject));
CREATE TABLE migration (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

/// The tables [`DEVICE_SCHEMA`] makes, checked on every open of an existing file.
pub const DEVICE_TABLES: [&str; 5] = [
    "device_settings",
    "rooms",
    "owner_lock",
    "pin_attempts",
    "migration",
];

// ------------------------------------------------------------------------------------------------ the room id

/// A room's id: [`ROOM_ID_LEN`] lowercase hex characters. Only such a value can name a folder, so an id can
/// never reach outside `rooms/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomId(String);

impl RoomId {
    /// `raw` when it is a room id, else `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        (raw.len() == ROOM_ID_LEN
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The origin a room is keyed by: the saved sign-in's `server_url` through
/// [`crate::cloud_account::normalize_server_url`], so a trailing slash or a case difference cannot open a
/// second, empty room.
pub fn room_origin(server_url: &str) -> Result<String, crate::cloud_account::ServerUrlError> {
    crate::cloud_account::normalize_server_url(server_url)
}

/// The id a new room of `(origin, user_id)` is named: the first 32 hex characters of SHA-256 of
/// `"buwiz-room-v1\0" + origin + "\0" + user_id`. `origin` must already be normalised ([`room_origin`]).
/// Only for naming a new folder: an existing room is found in `rooms` ([`room_for`]).
pub fn room_id_for(origin: &str, user_id: &str) -> RoomId {
    let mut hash = Sha256::new();
    hash.update(ROOM_ID_DOMAIN.as_bytes());
    hash.update([0]);
    hash.update(origin.as_bytes());
    hash.update([0]);
    hash.update(user_id.as_bytes());
    let mut id = hex::encode(hash.finalize());
    id.truncate(ROOM_ID_LEN);
    RoomId(id)
}

/// What [`room_for`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomLookup {
    /// The `rooms` row of this account on this server.
    Known(RoomRecord),
    /// No room yet: the id and the normalised origin a new one would get.
    New { room_id: RoomId, origin: String },
}

impl RoomLookup {
    pub fn room_id(&self) -> &RoomId {
        match self {
            Self::Known(record) => &record.room_id,
            Self::New { room_id, .. } => room_id,
        }
    }
}

/// Why a room could not be looked up.
#[derive(Debug)]
pub enum RoomError {
    /// The sign-in's server address is not one the app accepts.
    Server(crate::cloud_account::ServerUrlError),
    /// The device store could not be read.
    Store(DeviceStoreError),
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Server(error) => write!(f, "{error}"),
            Self::Store(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RoomError {}

/// The room of `user_id` on `server_url`: the `rooms` row first (by the normalised origin), else the hash a
/// new room would be named. `store` is `None` where there is no device store yet: every account is then new.
pub fn room_for(
    store: Option<&DeviceStore>,
    server_url: &str,
    user_id: &str,
) -> Result<RoomLookup, RoomError> {
    let origin = room_origin(server_url).map_err(RoomError::Server)?;
    if let Some(store) = store
        && let Some(record) = store
            .find_room(&origin, user_id)
            .map_err(RoomError::Store)?
    {
        return Ok(RoomLookup::Known(record));
    }
    Ok(RoomLookup::New {
        room_id: room_id_for(&origin, user_id),
        origin,
    })
}

// --------------------------------------------------------------------------------------------------- the layout

/// Where everything of the rooms layout is, under one device folder (PLAN §A.2):
///
/// ```text
/// <device folder>/
///   device.db
///   device.db.owner.lock                the one-copy guard's lock
///   backups/                             the pre-rooms safety copy (A2)
///   previews/                            print previews (A3b)
///   bir_data.db                          the legacy file until it is claimed
///   rooms/<room-id>/bir_data.db          one per account
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    device_dir: PathBuf,
}

impl Layout {
    /// The layout of this process: under [`crate::platform::device_dir`].
    pub fn here() -> Self {
        Self::at(crate::platform::device_dir())
    }

    /// The layout under `device_dir`.
    pub fn at(device_dir: impl Into<PathBuf>) -> Self {
        Self {
            device_dir: device_dir.into(),
        }
    }

    pub fn device_dir(&self) -> &Path {
        &self.device_dir
    }

    pub fn device_db(&self) -> PathBuf {
        self.device_dir.join(DEVICE_DB_FILE)
    }

    pub fn rooms_dir(&self) -> PathBuf {
        self.device_dir.join(ROOMS_DIR)
    }

    pub fn room_dir(&self, room: &RoomId) -> PathBuf {
        self.rooms_dir().join(room.as_str())
    }

    pub fn room_database(&self, room: &RoomId) -> PathBuf {
        self.room_dir(room).join(ROOM_DB_FILE)
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.device_dir.join(BACKUPS_DIR)
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.device_dir.join(PREVIEWS_DIR)
    }

    /// The legacy database (`<device folder>/bir_data.db`): the file every earlier version opened.
    pub fn legacy_database(&self) -> PathBuf {
        self.device_dir.join(ROOM_DB_FILE)
    }

    /// Whether a `rooms/` folder is on disk: then a missing `device.db` was lost, not never made (L2).
    pub fn has_rooms(&self) -> bool {
        self.rooms_dir().is_dir()
    }

    /// Make the room's folder (and `rooms/`) inside the device folder, and answer it.
    pub fn create_room_dir(&self, room: &RoomId) -> std::io::Result<PathBuf> {
        let dir = self.room_dir(room);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Make `backups/` inside the device folder, and answer it.
    pub fn create_backups_dir(&self) -> std::io::Result<PathBuf> {
        let dir = self.backups_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Make `previews/` inside the device folder, and answer it.
    pub fn create_previews_dir(&self) -> std::io::Result<PathBuf> {
        let dir = self.previews_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}

// --------------------------------------------------------------------------------------------- the device store

/// Why the device store could not be used.
#[derive(Debug)]
pub enum DeviceStoreError {
    /// `device.db` is missing while `rooms/` is on disk: it was lost, and it is never recreated silently (L2).
    /// [`DeviceStore::recreate_after_loss`] makes a new one that records the loss.
    Lost(PathBuf),
    /// `device.db` is there but is not a device store this build can read (not SQLite, damaged, missing a
    /// table). It is left exactly in place.
    Unreadable {
        path: PathBuf,
        reason: String,
    },
    /// `device.db` was written by a newer version of the app.
    Newer {
        path: PathBuf,
        version: i32,
    },
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for DeviceStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lost(path) => write!(
                f,
                "The device store {} is missing while rooms are on this device.",
                path.display()
            ),
            Self::Unreadable { path, reason } => {
                write!(
                    f,
                    "The device store {} cannot be read: {reason}",
                    path.display()
                )
            }
            Self::Newer { path, version } => write!(
                f,
                "The device store {} was written by a newer version of eBIRForms (version {version}).",
                path.display()
            ),
            Self::Sqlite(error) => write!(f, "device store: {error}"),
            Self::Io(error) => write!(f, "device store: {error}"),
        }
    }
}

impl std::error::Error for DeviceStoreError {}

impl DeviceStoreError {
    /// The store cannot be used as it is (lost, unreadable, newer), as against a busy or failing moment: a
    /// device-setting write then goes elsewhere rather than fail ([`set_device_setting_or`]).
    pub fn is_unusable(&self) -> bool {
        matches!(
            self,
            Self::Lost(_) | Self::Unreadable { .. } | Self::Newer { .. }
        )
    }
}

impl From<rusqlite::Error> for DeviceStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::io::Error> for DeviceStoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// One row of `rooms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomRecord {
    pub room_id: RoomId,
    /// The normalised server origin ([`room_origin`]).
    pub origin: String,
    pub user_id: String,
    pub email: Option<String>,
    pub created_at: String,
    /// The room is the claimed legacy file (A2).
    pub from_legacy: bool,
}

/// `device.db`: plain SQLite, device-wide, `journal_mode = WAL` and `synchronous = FULL` (its rows are markers
/// and lock counts, which must survive a power loss). No exclusive owner lock: the app and `bir-headless` both
/// read it.
pub struct DeviceStore {
    pub(crate) conn: Connection,
}

/// Open a room's database file for the one-slot host (A3a). Refuses — an Err in every build —
/// when the legacy claim's `start()` has not run for this device folder in
/// this session (R1): the open runs migrations and a checkpoint, and those writes change bytes the move's
/// hash check still needs as evidence.
pub fn open_room_guarded(
    layout: &Layout,
    room: &RoomId,
) -> Result<crate::db::Database, DeviceStoreError> {
    let ran = crate::legacy_claim::claim_started_this_session(layout.device_dir());
    // D1: a refusal is an Err, never a panic — no debug_assert here.
    if !ran {
        return Err(DeviceStoreError::Io(std::io::Error::other(
            "the legacy claim's start() has not run for this device folder in this session;              a room's database is never opened before it (R1)",
        )));
    }
    crate::db::Database::open(layout.room_database(room)).map_err(|error| match error {
        crate::db::DbError::Sqlite(sqlite) => DeviceStoreError::Sqlite(sqlite),
        other => DeviceStoreError::Io(std::io::Error::other(other.to_string())),
    })
}

impl DeviceStore {
    /// Open the device store of `layout`, making it on a device with no rooms (a new install, or one from
    /// before rooms). A missing `device.db` while `rooms/` is on disk is [`DeviceStoreError::Lost`], never a
    /// fresh file. An existing file is opened with no create and is never replaced.
    pub fn open(layout: &Layout) -> Result<Self, DeviceStoreError> {
        let path = layout.device_db();
        if path.exists() {
            return Self::open_file(&path);
        }
        if layout.has_rooms() {
            return Err(DeviceStoreError::Lost(path));
        }
        Self::create(&path)
    }

    /// Open the device store of `layout` only if `device.db` exists. Never makes a file: readers use this, so
    /// a read never creates `device.db`.
    pub fn open_existing(layout: &Layout) -> Result<Option<Self>, DeviceStoreError> {
        let path = layout.device_db();
        if !path.exists() {
            return Ok(None);
        }
        Self::open_file(&path).map(Some)
    }

    /// The explicit answer to [`DeviceStoreError::Lost`]: make a new `device.db` and record the loss in
    /// `migration` ([`DEVICE_STORE_LOST_KEY`], with the time), so every room goes through the email code before
    /// a PIN is set (§B.6). An existing `device.db` is opened as it is: nothing was lost.
    pub fn recreate_after_loss(layout: &Layout) -> Result<Self, DeviceStoreError> {
        let path = layout.device_db();
        if path.exists() {
            return Self::open_file(&path);
        }
        let store = Self::create(&path)?;
        store.conn.execute(
            "INSERT INTO migration (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO NOTHING",
            params![DEVICE_STORE_LOST_KEY, chrono::Utc::now().to_rfc3339()],
        )?;
        tracing::warn!(
            "device store was missing while rooms are on this device; a new one records the loss"
        );
        Ok(store)
    }

    /// A device store in memory, with the whole schema: for tests and [`use_ephemeral_device_store`].
    pub fn open_in_memory() -> Result<Self, DeviceStoreError> {
        let mut conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;")?;
        initialise(&mut conn)?;
        Ok(Self { conn })
    }

    fn create(path: &Path) -> Result<Self, DeviceStoreError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        configure(&conn).map_err(|error| unreadable(path, error))?;
        check_or_initialise(&mut conn, path)?;
        Ok(Self { conn })
    }

    fn open_file(path: &Path) -> Result<Self, DeviceStoreError> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| unreadable(path, error))?;
        configure(&conn).map_err(|error| unreadable(path, error))?;
        check_or_initialise(&mut conn, path)?;
        Ok(Self { conn })
    }

    /// The store's own connection, for the legacy claim's record (A2), written at `synchronous = FULL`.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// `PRAGMA synchronous` of this connection (2 is FULL).
    pub fn synchronous(&self) -> Result<i64, DeviceStoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))?)
    }

    /// `PRAGMA user_version`.
    pub fn schema_version(&self) -> Result<i32, DeviceStoreError> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    /// The `device_settings` value of `key`. `Some("")` is a value: a row that is there, cleared.
    pub fn setting(&self, key: &str) -> Result<Option<String>, DeviceStoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM device_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DeviceStoreError> {
        self.conn.execute(
            "INSERT INTO device_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), DeviceStoreError> {
        self.conn
            .execute("DELETE FROM device_settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// The room of `user_id` on `origin` (already normalised), if there is one.
    pub fn find_room(
        &self,
        origin: &str,
        user_id: &str,
    ) -> Result<Option<RoomRecord>, DeviceStoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT room_id, origin, user_id, email, created_at, from_legacy
                 FROM rooms WHERE origin = ?1 AND user_id = ?2",
                params![origin, user_id],
                room_row,
            )
            .optional()?
            .flatten())
    }

    /// The `rooms` row of `room_id`, if there is one.
    pub fn room(&self, room_id: &RoomId) -> Result<Option<RoomRecord>, DeviceStoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT room_id, origin, user_id, email, created_at, from_legacy
                 FROM rooms WHERE room_id = ?1",
                params![room_id.as_str()],
                room_row,
            )
            .optional()?
            .flatten())
    }

    /// Every room, oldest first.
    pub fn rooms(&self) -> Result<Vec<RoomRecord>, DeviceStoreError> {
        let mut statement = self.conn.prepare(
            "SELECT room_id, origin, user_id, email, created_at, from_legacy
             FROM rooms ORDER BY created_at, room_id",
        )?;
        let rows = statement.query_map([], room_row)?;
        let mut out = Vec::new();
        for row in rows {
            if let Some(record) = row? {
                out.push(record);
            }
        }
        Ok(out)
    }

    /// Add a room's row. A second row for the same room id or the same `(origin, user_id)` is refused.
    pub fn add_room(&self, record: &RoomRecord) -> Result<(), DeviceStoreError> {
        self.conn.execute(
            "INSERT INTO rooms (room_id, origin, user_id, email, created_at, from_legacy)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.room_id.as_str(),
                record.origin,
                record.user_id,
                record.email,
                record.created_at,
                i64::from(record.from_legacy),
            ],
        )?;
        Ok(())
    }

    /// When the device store was found lost and made again ([`DeviceStore::recreate_after_loss`]), if ever.
    pub fn lost_at(&self) -> Result<Option<String>, DeviceStoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM migration WHERE key = ?1",
                params![DEVICE_STORE_LOST_KEY],
                |row| row.get(0),
            )
            .optional()?)
    }
}

/// A `rooms` row; `None` for a row whose id is not a room id (never written by this code).
fn room_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<RoomRecord>> {
    let raw: String = row.get(0)?;
    let Some(room_id) = RoomId::parse(&raw) else {
        return Ok(None);
    };
    Ok(Some(RoomRecord {
        room_id,
        origin: row.get(1)?,
        user_id: row.get(2)?,
        email: row.get(3)?,
        created_at: row.get(4)?,
        from_legacy: row.get::<_, i64>(5)? != 0,
    }))
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // secure_delete: a PIN hash or count that is overwritten or deleted is zeroed in the file, not left in a free
    // page (F5). Per connection: the held read one and every write one go through here.
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;",
    )?;
    // On Apple systems `fsync` does not reach the disk's platter or flash; F_FULLFSYNC does. The rows here are markers
    // and lock counts that must survive a power loss.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    conn.execute_batch("PRAGMA fullfsync = ON;")?;
    Ok(())
}

/// The file is not a device store this build reads, said in words; busy and locked stay SQLite errors (a
/// transient state, not a bad file).
fn unreadable(path: &Path, error: rusqlite::Error) -> DeviceStoreError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt | ErrorCode::CannotOpen
            ) =>
        {
            DeviceStoreError::Unreadable {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        }
        _ => DeviceStoreError::Sqlite(error),
    }
}

/// Check the schema of an open file, or make it in a file that has none yet (a new file, or one whose first
/// open was cut before its schema was written: `user_version` 0 and no table at all).
fn check_or_initialise(conn: &mut Connection, path: &Path) -> Result<(), DeviceStoreError> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| unreadable(path, error))?;
    match version {
        0 => {
            let tables: i64 = conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )?;
            if tables == 0 {
                initialise(conn)
            } else {
                Err(DeviceStoreError::Unreadable {
                    path: path.to_path_buf(),
                    reason: "it has tables but no device store version".into(),
                })
            }
        }
        DEVICE_SCHEMA_VERSION => {
            for table in DEVICE_TABLES {
                let present: i64 = conn.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )?;
                if present == 0 {
                    return Err(DeviceStoreError::Unreadable {
                        path: path.to_path_buf(),
                        reason: format!("the table {table} is missing"),
                    });
                }
            }
            Ok(())
        }
        newer if newer > DEVICE_SCHEMA_VERSION => Err(DeviceStoreError::Newer {
            path: path.to_path_buf(),
            version: newer,
        }),
        other => Err(DeviceStoreError::Unreadable {
            path: path.to_path_buf(),
            reason: format!("unknown device store version {other}"),
        }),
    }
}

/// Write the schema and `user_version` in one transaction. Another process making the same file at the same
/// moment waits on it, then finds the version set and leaves it.
fn initialise(conn: &mut Connection) -> Result<(), DeviceStoreError> {
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i32 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        transaction.execute_batch(DEVICE_SCHEMA)?;
        transaction.pragma_update(None, "user_version", DEVICE_SCHEMA_VERSION)?;
    }
    transaction.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------------- the device-wide settings

thread_local! {
    /// A device store in memory for this thread ([`use_ephemeral_device_store`]), in place of `device.db`. Shared
    /// (not per connection) so a thread this one starts can be handed it ([`device_store_here`]).
    static EPHEMERAL: RefCell<Option<Arc<Mutex<DeviceStore>>>> = const { RefCell::new(None) };
}

/// Give this thread a device store in memory, in place of the file in the device folder: for a test (or the
/// agent's fixture) whose database is in memory too, so its device settings never reach a real folder. Obeyed
/// only where single-room mode is (a debug build or a test environment, [`crate::db::single_room_allowed`]).
pub fn use_ephemeral_device_store() {
    if !crate::db::single_room_allowed() {
        return;
    }
    let store = match DeviceStore::open_in_memory() {
        Ok(store) => Some(Arc::new(Mutex::new(store))),
        Err(error) => {
            tracing::warn!("in-memory device store not opened: {error}");
            None
        }
    };
    EPHEMERAL.with(|slot| *slot.borrow_mut() = store);
}

/// Forget this thread's in-memory device store ([`use_ephemeral_device_store`]).
pub fn forget_ephemeral_device_store() {
    EPHEMERAL.with(|slot| slot.borrow_mut().take());
}

/// A held read connection and the identity of the file it opened (H1): a `device.db` deleted and made again, or
/// replaced by an older copy, is a different file, and the connection to the old one is dropped before it is used.
struct HeldStore {
    store: DeviceStore,
    id: crate::safe_fs::FileId,
}

/// The device stores this process reads, by `device.db` path: one connection each, so a setting read on the UI thread
/// is a query, not a fresh open with its pragmas and schema checks. A held connection still sees what another process
/// commits. Writes never use it: they open their own connection, outside this lock (H1), so a write that waits out
/// the busy timeout against another process never holds up the readers. Whoever renames `device.db` aside in this
/// process drops the held connection first ([`set_aside_unusable`]): on Windows an open connection refuses the rename.
/// Every connection, held or not, is made through `configure` (WAL, `synchronous = FULL`, `secure_delete`).
static HELD_STORES: Mutex<std::collections::BTreeMap<PathBuf, HeldStore>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Close every device-store connection this process holds, as a fresh process would start with none. Test builds
/// only. On Windows an open SQLite file cannot be deleted, so the device store can only go missing while the app
/// is NOT running; a test that removes `device.db` to stand for a loss closes the held connections first, as the
/// next launch would find them.
#[cfg(any(test, feature = "test-support"))]
pub fn release_held_device_stores() {
    HELD_STORES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

/// Whether this process holds a connection to `layout`'s `device.db`. Test builds only.
#[cfg(any(test, feature = "test-support"))]
pub fn device_store_is_held(layout: &Layout) -> bool {
    HELD_STORES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(&layout.device_db())
}

/// Which device store a thread uses, taken on that thread ([`device_store_here`]) so work it hands to another
/// thread (the PIN hash, PLAN §B.1) reaches the same store: this thread's in-memory one, or `device.db` of
/// its device folder.
#[derive(Clone)]
pub enum DeviceStoreRef {
    /// The in-memory store of [`use_ephemeral_device_store`].
    Ephemeral(Arc<Mutex<DeviceStore>>),
    /// `device.db` of this layout: reads through the process's held connection, writes through their own.
    File(Layout),
}

/// This thread's device store: the in-memory one ([`use_ephemeral_device_store`]), else `device.db` in the
/// device folder ([`Layout::here`]).
pub fn device_store_here() -> DeviceStoreRef {
    match EPHEMERAL.with(|slot| slot.borrow().clone()) {
        Some(store) => DeviceStoreRef::Ephemeral(store),
        None => DeviceStoreRef::File(Layout::here()),
    }
}

impl DeviceStoreRef {
    /// Run `work` on this store.
    /// - `create` (a write): a connection of its own, made where [`DeviceStore::open`] would make the file, and closed
    ///   after; the global lock is not held while it works (H1).
    /// - otherwise (a read): the held connection, opened on first use and opened again whenever the file at the path
    ///   is not the one it opened (deleted, made again, replaced). A missing file answers `Ok(None)` and nothing is
    ///   made, so a lost store is found lost, never written unseen.
    ///
    /// A connection whose file turned out damaged is dropped, and the error says unreadable.
    pub fn with<T>(
        &self,
        create: bool,
        work: impl FnOnce(&DeviceStore) -> Result<T, DeviceStoreError>,
    ) -> Result<Option<T>, DeviceStoreError> {
        let layout = match self {
            Self::Ephemeral(store) => {
                let store = store
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                return work(&store).map(Some);
            }
            Self::File(layout) => layout,
        };
        let path = layout.device_db();
        if create {
            let store = DeviceStore::open(layout)?;
            return work(&store)
                .map(Some)
                .map_err(|error| damaged(&path, error));
        }
        let mut held = HELD_STORES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = match crate::safe_fs::file_id(&path) {
            Ok(id) => id,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                held.remove(&path);
                return Ok(None);
            }
            Err(error) => {
                held.remove(&path);
                return Err(error.into());
            }
        };
        if held.get(&path).is_some_and(|entry| entry.id != current) {
            held.remove(&path);
        }
        if !held.contains_key(&path) {
            let Some(store) = DeviceStore::open_existing(layout)? else {
                return Ok(None);
            };
            #[cfg(test)]
            AFTER_HELD_OPEN.with(|hook| hook.borrow_mut().take().map(|hook| hook()));
            // Recorded with the identity read before the open (H1'): a swap in between makes the next read find
            // another file and open again. An identity read after the open could pair this connection to the old
            // file with the new file's identity, and every later read would stay on the old one.
            held.insert(path.clone(), HeldStore { store, id: current });
        }
        let Some(entry) = held.get(&path) else {
            return Ok(None);
        };
        match work(&entry.store) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                let error = damaged(&path, error);
                if error.is_unusable() {
                    held.remove(&path);
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Runs once right after a held read connection is opened, before it is recorded: a test swaps the file there.
    static AFTER_HELD_OPEN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

/// A SQLite error that says the file itself is not a store (not a database, corrupt) as [`DeviceStoreError::Unreadable`].
fn damaged(path: &Path, error: DeviceStoreError) -> DeviceStoreError {
    match error {
        DeviceStoreError::Sqlite(sqlite) => unreadable(path, sqlite),
        other => other,
    }
}

/// [`DeviceStoreRef::with`] on this thread's store ([`device_store_here`]).
fn with_store_here<T>(
    create: bool,
    work: impl FnOnce(&DeviceStore) -> Result<T, DeviceStoreError>,
) -> Result<Option<T>, DeviceStoreError> {
    device_store_here().with(create, work)
}

/// The `device_settings` value of `key` here, without the fallback. Never makes `device.db`: no file, a lost
/// or unreadable one answer `None` (the last two logged).
pub fn device_setting(key: &str) -> Option<String> {
    match with_store_here(false, |store| store.setting(key)) {
        Ok(value) => value.flatten(),
        Err(error) => {
            tracing::warn!(key, "device setting not read: {error}");
            None
        }
    }
}

/// A device-wide setting (PLAN §A.2, O1): the `device_settings` value first; until one is there, the same key
/// in the database the app has open (`open`: the legacy file before A3a, the room after). So a value a person
/// set before rooms still answers, and A1 changes nothing they see. `Some("")` is a value.
pub fn device_setting_or(key: &str, open: Option<&Database>) -> Option<String> {
    device_setting(key).or_else(|| open.and_then(|db| db.get_setting(key).ok().flatten()))
}

/// Write a device-wide setting to `device_settings`, made on a new install.
///
/// A store that cannot be used (#423's review, item 5) never stays in the way where nothing depends on it: with no
/// `rooms/` on disk, an unreadable `device.db` that a `quick_check` shows is not a store is renamed aside
/// ([`set_aside_unusable`]) and a fresh store takes the write. Otherwise the error is returned; it says
/// [`DeviceStoreError::is_unusable`] when the store is lost, unreadable or newer, and [`set_device_setting_or`] then
/// writes the open database instead. A lost store with rooms on disk is never made again here (L2, the N6 path).
pub fn set_device_setting(key: &str, value: &str) -> Result<(), DeviceStoreError> {
    let write = |store: &DeviceStore| store.set_setting(key, value);
    let here = device_store_here();
    match here.with(true, write) {
        Ok(_) => Ok(()),
        Err(error @ DeviceStoreError::Unreadable { .. }) => {
            let DeviceStoreRef::File(layout) = &here else {
                return Err(error);
            };
            if layout.has_rooms() {
                return Err(error);
            }
            match set_aside_unusable(layout) {
                Ok(Some(aside)) => {
                    tracing::warn!(
                        aside = %aside.display(),
                        "device store could not be read ({error}); kept under a new name, and a new one was made"
                    );
                    here.with(true, write).map(|_| ())
                }
                Ok(None) => Err(error),
                Err(set_aside) => {
                    tracing::warn!("device store not set aside: {set_aside}");
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

/// Where [`set_device_setting_or`] wrote a device-wide setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingWritten {
    /// To `device_settings`.
    DeviceStore,
    /// To the same key in the database the app has open: the device store could not be used.
    OpenDatabase,
}

/// Write a device-wide setting, never blocked by a device store that cannot be used (PLAN §A.7, A2's first commit):
/// [`set_device_setting`], and where that answers an unusable store (lost, unreadable, newer), the same key in the
/// database the app has open (`open`: the legacy file before A3a, the room after), which the readers fall back to
/// (O1). With no open database there is nowhere else, and the store's own error is returned.
pub fn set_device_setting_or(
    key: &str,
    value: &str,
    open: Option<&Database>,
) -> Result<SettingWritten, DeviceStoreError> {
    match set_device_setting(key, value) {
        Ok(()) => Ok(SettingWritten::DeviceStore),
        Err(error) if error.is_unusable() => {
            let Some(db) = open else {
                return Err(error);
            };
            tracing::warn!(key, "device setting written to the open database: {error}");
            db.set_setting(key, value)
                .map_err(|db_error| match db_error {
                    crate::db::DbError::Sqlite(sqlite) => DeviceStoreError::Sqlite(sqlite),
                    other => DeviceStoreError::Io(std::io::Error::other(other.to_string())),
                })?;
            Ok(SettingWritten::OpenDatabase)
        }
        Err(error) => Err(error),
    }
}

/// Delete a device-wide setting from `device_settings`. A device with no `device.db` has nothing to delete,
/// and none is made for it; nor has a store that cannot be used (its value is read by nobody): the caller deletes
/// the open database's key too.
pub fn delete_device_setting(key: &str) -> Result<(), DeviceStoreError> {
    match with_store_here(false, |store| store.delete_setting(key)) {
        Ok(_) => Ok(()),
        Err(error) if error.is_unusable() => {
            tracing::warn!(
                key,
                "device setting not deleted from an unusable store: {error}"
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Whether the file at `path` is shown not to be a device store this build reads: it is not SQLite, `quick_check`
/// finds damage, or its shape is not a device store (tables with no version, a table missing, an unknown version).
/// `None` when that cannot be shown (it opens fine, it is a newer store, or it cannot be opened at all, which may be
/// a passing state such as file protection): then it is never set aside.
fn unusable_reason(path: &Path) -> Option<String> {
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(error) => {
            return matches!(&error, rusqlite::Error::SqliteFailure(failure, _) if failure.code == ErrorCode::NotADatabase)
                .then(|| format!("not a database ({error})"));
        }
    };
    let check: Result<String, _> = conn.query_row("PRAGMA quick_check", [], |row| row.get(0));
    drop(conn);
    match check {
        Ok(answer) if answer == "ok" => {}
        Ok(answer) => return Some(format!("quick_check: {answer}")),
        Err(rusqlite::Error::SqliteFailure(failure, message))
            if matches!(
                failure.code,
                ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt
            ) =>
        {
            return Some(format!(
                "quick_check: {failure} {}",
                message.unwrap_or_default()
            ));
        }
        Err(_) => return None,
    }
    match DeviceStore::open_file(path) {
        Err(DeviceStoreError::Unreadable { reason, .. }) => Some(reason),
        _ => None,
    }
}

/// Rename an unusable `device.db` (and its `-wal` and `-shm`) aside as `device.db.unusable-<stamp>`, never replacing,
/// and make the rename durable; answers the new name, or `None` when [`unusable_reason`] cannot show the file is
/// unusable (nothing is renamed). It is never deleted. The held connection to it is dropped first: on Windows an open
/// connection would refuse the rename (H1).
pub fn set_aside_unusable(layout: &Layout) -> Result<Option<PathBuf>, DeviceStoreError> {
    let path = layout.device_db();
    let mut held = HELD_STORES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    held.remove(&path);
    let Some(reason) = unusable_reason(&path) else {
        return Ok(None);
    };
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%9fZ");
    let aside = layout
        .device_dir()
        .join(format!("{DEVICE_DB_FILE}.unusable-{stamp}"));
    // The side files first (R3): a log left at the old name, if the process stopped between the renames, would be
    // read into the fresh store as if it were its own.
    for suffix in ["-wal", "-shm"] {
        let mut side = path.as_os_str().to_os_string();
        side.push(suffix);
        let mut side_aside = aside.as_os_str().to_os_string();
        side_aside.push(suffix);
        match crate::safe_fs::rename_no_replace(Path::new(&side), Path::new(&side_aside)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    // Make the side-file renames durable before the main one: after a power cut the folder then shows either all
    // of them moved or none, never a fresh device.db with a stray -wal or -shm at the old name.
    crate::safe_fs::sync_dir(layout.device_dir())?;
    crate::safe_fs::rename_no_replace(&path, &aside)?;
    crate::safe_fs::sync_dir(layout.device_dir())?;
    tracing::warn!(aside = %aside.display(), "device store set aside: {reason}");
    Ok(Some(aside))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(store_origin: &str, user_id: &str, email: &str) -> RoomRecord {
        RoomRecord {
            room_id: room_id_for(store_origin, user_id),
            origin: store_origin.to_owned(),
            user_id: user_id.to_owned(),
            email: Some(email.to_owned()),
            created_at: "2026-11-26T00:00:00+00:00".to_owned(),
            from_legacy: false,
        }
    }

    #[test]
    fn room_id_normalises_the_origin() {
        let a = room_for(None, "https://api.buwiz.com/", "user-1").unwrap();
        let b = room_for(None, "https://API.buwiz.com", "user-1").unwrap();
        let c = room_for(None, "  https://api.buwiz.com  ", "user-1").unwrap();
        assert_eq!(a.room_id(), b.room_id());
        assert_eq!(a.room_id(), c.room_id());
        assert_eq!(
            a,
            RoomLookup::New {
                room_id: room_id_for("https://api.buwiz.com", "user-1"),
                origin: "https://api.buwiz.com".to_owned(),
            }
        );

        // And the row is found under either spelling once it is there.
        let store = DeviceStore::open_in_memory().unwrap();
        store
            .add_room(&record("https://api.buwiz.com", "user-1", "jane@buwiz.com"))
            .unwrap();
        for spelling in ["https://api.buwiz.com/", "https://API.buwiz.com"] {
            assert!(matches!(
                room_for(Some(&store), spelling, "user-1").unwrap(),
                RoomLookup::Known(_)
            ));
        }
    }

    #[test]
    fn room_id_hides_the_email_and_differs_per_server() {
        let email = "jane@buwiz.com";
        let production = room_id_for("https://api.buwiz.com", email);
        let test_server = room_id_for("http://127.0.0.1:3008", email);
        assert_ne!(
            production, test_server,
            "a test-server account is its own room"
        );
        for id in [&production, &test_server] {
            assert_eq!(id.as_str().len(), ROOM_ID_LEN);
            assert!(RoomId::parse(id.as_str()).is_some());
            assert!(!id.as_str().contains("jane"), "{id}");
            assert!(!id.as_str().contains('@'), "{id}");
        }
        // The folder name carries the id only.
        let layout = Layout::at("/device");
        let folder = layout.room_dir(&production);
        assert!(!folder.to_string_lossy().contains("jane"));
        // Two accounts on one server differ; the same account is stable.
        assert_ne!(
            room_id_for("https://api.buwiz.com", "user-1"),
            room_id_for("https://api.buwiz.com", "user-2")
        );
        assert_eq!(
            room_id_for("https://api.buwiz.com", "user-1"),
            room_id_for("https://api.buwiz.com", "user-1")
        );
        // The separator keeps "a" + "bc" apart from "ab" + "c".
        assert_ne!(room_id_for("ab", "c"), room_id_for("a", "bc"));
    }

    #[test]
    fn the_rooms_table_is_asked_before_the_hash() {
        let store = DeviceStore::open_in_memory().unwrap();
        let mut kept = record("https://api.buwiz.com", "user-1", "jane@buwiz.com");
        kept.room_id = RoomId::parse("0123456789abcdef0123456789abcdef").unwrap();
        store.add_room(&kept).unwrap();
        let found = room_for(Some(&store), "https://api.buwiz.com", "user-1").unwrap();
        assert_eq!(found, RoomLookup::Known(kept.clone()));
        assert_eq!(store.rooms().unwrap(), vec![kept]);
        // The same account again is refused (UNIQUE (origin, user_id)).
        assert!(
            store
                .add_room(&record("https://api.buwiz.com", "user-1", "jane@buwiz.com"))
                .is_err()
        );
    }

    #[test]
    fn a_room_id_is_only_hex_of_its_length() {
        assert!(RoomId::parse("0123456789abcdef0123456789abcdef").is_some());
        for refused in [
            "",
            "../../etc",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789abcdef/123456789abcdef",
        ] {
            assert!(RoomId::parse(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn the_device_store_has_its_whole_schema_at_synchronous_full() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let store = DeviceStore::open(&layout).unwrap();
        assert_eq!(store.schema_version().unwrap(), DEVICE_SCHEMA_VERSION);
        assert_eq!(store.synchronous().unwrap(), 2, "synchronous = FULL");
        let journal: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal.to_ascii_lowercase(), "wal");
        for table in DEVICE_TABLES {
            let present: i64 = store
                .conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, 1, "{table}");
        }
        // The columns later pull requests write.
        let columns = |table: &str| -> Vec<String> {
            let mut statement = store
                .conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(columns("device_settings"), ["key", "value"]);
        assert_eq!(
            columns("rooms"),
            [
                "room_id",
                "origin",
                "user_id",
                "email",
                "created_at",
                "from_legacy"
            ]
        );
        assert_eq!(
            columns("owner_lock"),
            [
                "room_id",
                "pin_phc",
                "set_at",
                "biometrics_on",
                "apple_domain_state"
            ]
        );
        assert_eq!(
            columns("pin_attempts"),
            [
                "room_id",
                "subject",
                "failures",
                "wait_secs",
                "started_unix",
                "started_boot_secs",
                "started_boot_id"
            ]
        );
        assert_eq!(columns("migration"), ["key", "value"]);
        // An owner_lock row needs its room (the foreign key is on).
        assert!(
            store
                .conn
                .execute("INSERT INTO owner_lock (room_id) VALUES ('nope')", [])
                .is_err()
        );
        drop(store);
        // Opened again: the same file, the same rows.
        let again = DeviceStore::open(&layout).unwrap();
        again.set_setting("k", "v").unwrap();
        drop(again);
        assert_eq!(
            DeviceStore::open(&layout)
                .unwrap()
                .setting("k")
                .unwrap()
                .as_deref(),
            Some("v")
        );
    }

    /// F5: every device.db connection, made, reopened, the held one and the in-memory one, has secure_delete on.
    #[test]
    fn device_db_connections_use_secure_delete() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let secure = |store: &DeviceStore| -> i64 {
            store
                .conn
                .query_row("PRAGMA secure_delete", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(secure(&DeviceStore::open(&layout).unwrap()), 1, "made");
        assert_eq!(secure(&DeviceStore::open(&layout).unwrap()), 1, "reopened");
        assert_eq!(
            secure(&DeviceStore::open_existing(&layout).unwrap().unwrap()),
            1,
            "read"
        );
        assert_eq!(
            secure(&DeviceStore::open_in_memory().unwrap()),
            1,
            "in memory"
        );
        let held = DeviceStoreRef::File(layout.clone())
            .with(false, |store| Ok(secure(store)))
            .unwrap();
        assert_eq!(held, Some(1), "the held connection");
    }

    /// On Apple systems the device store asks for F_FULLFSYNC, so a committed marker is on the disk, not in its cache.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn device_db_uses_fullfsync_on_apple() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        for store in [
            DeviceStore::open(&layout).unwrap(),
            DeviceStore::open(&layout).unwrap(),
        ] {
            let fullfsync: i64 = store
                .conn
                .query_row("PRAGMA fullfsync", [], |row| row.get(0))
                .unwrap();
            assert_eq!(fullfsync, 1, "created and reopened alike");
        }
    }

    #[test]
    fn device_db_is_not_recreated_when_rooms_exist() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        let room = room_id_for("https://api.buwiz.com", "user-1");
        layout.create_room_dir(&room).unwrap();
        std::fs::write(layout.room_database(&room), b"").unwrap();

        // Missing, with rooms on disk: lost, and no file is made, by an open or by a read.
        assert!(matches!(
            DeviceStore::open(&layout),
            Err(DeviceStoreError::Lost(_))
        ));
        assert!(DeviceStore::open_existing(&layout).unwrap().is_none());
        assert!(!layout.device_db().exists(), "never recreated silently");

        // The explicit answer makes it and records the loss.
        let store = DeviceStore::recreate_after_loss(&layout).unwrap();
        assert!(store.lost_at().unwrap().is_some());
        assert!(store.rooms().unwrap().is_empty());
        drop(store);
        assert!(
            DeviceStore::open(&layout)
                .unwrap()
                .lost_at()
                .unwrap()
                .is_some()
        );

        // An unreadable one is an error, and is left exactly in place.
        let other = tempfile::tempdir().unwrap();
        let layout = Layout::at(other.path());
        std::fs::write(
            layout.device_db(),
            b"this is not a SQLite file, it is long enough",
        )
        .unwrap();
        let before = std::fs::read(layout.device_db()).unwrap();
        assert!(matches!(
            DeviceStore::open(&layout),
            Err(DeviceStoreError::Unreadable { .. })
        ));
        assert!(DeviceStore::open_existing(&layout).is_err());
        assert_eq!(std::fs::read(layout.device_db()).unwrap(), before);
    }

    #[test]
    fn a_device_db_with_no_rooms_is_made_as_on_a_new_install() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path().join("new"));
        assert!(!layout.has_rooms());
        let store = DeviceStore::open(&layout).unwrap();
        assert!(store.lost_at().unwrap().is_none());
        assert!(layout.device_db().is_file());
        assert!(!layout.rooms_dir().exists(), "A1 makes no rooms folder");
    }

    #[test]
    fn a_newer_device_db_is_refused() {
        let folder = tempfile::tempdir().unwrap();
        let layout = Layout::at(folder.path());
        drop(DeviceStore::open(&layout).unwrap());
        {
            let conn = Connection::open(layout.device_db()).unwrap();
            conn.pragma_update(None, "user_version", DEVICE_SCHEMA_VERSION + 1)
                .unwrap();
        }
        assert!(matches!(
            DeviceStore::open(&layout),
            Err(DeviceStoreError::Newer { version, .. }) if version == DEVICE_SCHEMA_VERSION + 1
        ));
    }

    /// Lead's review point 1: on iOS the whole data folder is excluded from backup with "complete" protection
    /// (platform/ios.rs `prepare_ios_storage`, through `platform::backup_excluded_dir`). Every new folder of the
    /// rooms layout is made under it, never beside it. Runs on every system (a home folder of its own), so the
    /// rule is checked where the iOS build cannot run its tests.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn the_new_folders_are_created_under_the_backup_excluded_folder() {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_str().unwrap().to_owned();
        temp_env::with_vars(
            [
                ("HOME", Some(home_path.as_str())),
                ("XDG_DATA_HOME", None),
                ("LOCALAPPDATA", Some(home_path.as_str())),
                ("BIR_DATABASE_PATH", None),
            ],
            || {
                crate::db::set_database_path_override(None);
                let excluded = crate::platform::backup_excluded_dir()
                    .canonicalize()
                    .unwrap();
                assert!(excluded.starts_with(home.path().canonicalize().unwrap()));
                let layout = Layout::here();
                let room = room_id_for("https://api.buwiz.com", "user-1");
                // Made first, as on a new install (with rooms/ on disk it would be "lost").
                drop(DeviceStore::open(&layout).unwrap());
                let made = [
                    layout.create_room_dir(&room).unwrap(),
                    layout.create_backups_dir().unwrap(),
                    layout.create_previews_dir().unwrap(),
                    layout.rooms_dir(),
                ];
                let device_db = layout.device_db();
                for path in made.iter().chain([&device_db]) {
                    let path = path.canonicalize().unwrap();
                    assert!(
                        path.starts_with(&excluded) && path != excluded,
                        "{} is not inside {}",
                        path.display(),
                        excluded.display()
                    );
                }
            },
        );
    }

    /// The device files and the rooms layout use the same folder (`platform::device_dir`), and opening a room
    /// moves none of them: before and after, the sign-in, the relay device key, the install id and the pending
    /// sign-in are in the data folder.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn device_files_do_not_move_when_a_room_opens() {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_str().unwrap().to_owned();
        temp_env::with_vars(
            [
                ("HOME", Some(home_path.as_str())),
                ("XDG_DATA_HOME", None),
                ("LOCALAPPDATA", Some(home_path.as_str())),
                ("BIR_DATABASE_PATH", None),
                (crate::cloud_account::ENV_ACCOUNT_FILE, None),
                (crate::email_sign_in::ENV_PENDING_FILE, None),
            ],
            || {
                crate::db::set_database_path_override(None);
                let data = crate::platform::data_dir();
                let files = || {
                    use crate::install_id::Sources as _;
                    (
                        crate::cloud_account::account_file_path(),
                        crate::relay::secrets_dir(),
                        crate::install_id::OsSources.folder(),
                        crate::email_sign_in::pending_file_path(),
                    )
                };
                let before = files();
                assert_eq!(before.0.parent(), Some(data.as_path()));
                assert_eq!(before.1, data);
                assert_eq!(before.2, data);
                assert_eq!(before.3.parent(), Some(data.as_path()));
                assert_eq!(crate::platform::device_dir(), data);

                let layout = Layout::here();
                let room = room_id_for("https://api.buwiz.com", "user-1");
                layout.create_room_dir(&room).unwrap();
                let _turn = crate::db::room_path_test_turn();
                crate::db::set_open_room_database(Some(layout.room_database(&room)));
                let opened = Database::open(crate::db::app_database_path()).unwrap();
                let during = files();
                drop(opened);
                crate::db::set_open_room_database(None);

                assert_eq!(during, before, "a device file followed the room");
                assert_eq!(crate::platform::device_dir(), data);
                assert_eq!(crate::db::app_database_path(), layout.legacy_database());
            },
        );
    }

    #[test]
    fn device_settings_fall_back_to_the_open_database() {
        let legacy = Database::open_ephemeral().unwrap();
        // A person turned auto-download off before A1: it reads off after A1.
        legacy.set_setting("auto_update_download", "0").unwrap();
        use_ephemeral_device_store();
        assert_eq!(
            device_setting_or("auto_update_download", Some(&legacy)).as_deref(),
            Some("0")
        );
        assert_eq!(device_setting_or("auto_update_download", None), None);
        // A write goes to device_settings and wins from then on, whatever the open database says.
        set_device_setting("auto_update_download", "1").unwrap();
        assert_eq!(
            device_setting_or("auto_update_download", Some(&legacy)).as_deref(),
            Some("1")
        );
        assert_eq!(
            legacy
                .get_setting("auto_update_download")
                .unwrap()
                .as_deref(),
            Some("0"),
            "the open database is not written"
        );
        // A cleared value is a value: no fallback.
        set_device_setting("global_hotkey_key", "").unwrap();
        legacy
            .set_setting("global_hotkey_key", "control+alt+E")
            .unwrap();
        assert_eq!(
            device_setting_or("global_hotkey_key", Some(&legacy)).as_deref(),
            Some("")
        );
        delete_device_setting("global_hotkey_key").unwrap();
        assert_eq!(
            device_setting_or("global_hotkey_key", Some(&legacy)).as_deref(),
            Some("control+alt+E")
        );
        forget_ephemeral_device_store();
    }

    /// Reads go through one held connection per `device.db` (no open per read), still see another connection's
    /// commits, and a file removed under it is never written unseen: the next write finds it missing and makes it again
    /// (no rooms here) rather than writing into the removed file. Writes open their own connection (H1).
    #[test]
    fn device_setting_reads_use_one_held_connection() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("bir_data.db");
        crate::db::set_database_path_override(Some(database));
        forget_ephemeral_device_store();
        let layout = Layout::here();
        set_device_setting("theme_preference", "Dark").unwrap();
        let held = || {
            HELD_STORES
                .lock()
                .unwrap()
                .contains_key(&layout.device_db())
        };
        assert!(!held(), "a write opens its own connection (H1)");
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Dark"));
        assert!(held(), "the read's connection is kept");
        // Another connection (another process, as headless is) commits: the held one sees it.
        DeviceStore::open(&layout)
            .unwrap()
            .set_setting("theme_preference", "Light")
            .unwrap();
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Light"));
        // The file goes (Unix lets an open file be removed): the read answers nothing and makes nothing; the write
        // makes a new one.
        if cfg!(unix) {
            std::fs::remove_file(layout.device_db()).unwrap();
            let _ = std::fs::remove_file(folder.path().join("device.db-wal"));
            let _ = std::fs::remove_file(folder.path().join("device.db-shm"));
            assert_eq!(device_setting("theme_preference"), None);
            assert!(!layout.device_db().exists());
            set_device_setting("theme_preference", "Dark").unwrap();
            assert!(layout.device_db().is_file());
            assert_eq!(
                DeviceStore::open(&layout)
                    .unwrap()
                    .setting("theme_preference")
                    .unwrap()
                    .as_deref(),
                Some("Dark")
            );
        }
        crate::db::set_database_path_override(None);
    }

    /// The `device.db.unusable-*` files in `folder`, and their bytes.
    fn set_aside(folder: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("device.db.unusable-")
                            && !name.ends_with("-wal")
                            && !name.ends_with("-shm")
                    })
            })
            .map(|path| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&path).unwrap(),
                )
            })
            .collect();
        out.sort();
        out
    }

    /// PLAN §A.7, A2's first commit (#423's review, item 5): a `device.db` that cannot be used (unreadable, newer,
    /// lost) never makes a device-setting write fail.
    /// - With no `rooms/`, an unreadable file is renamed aside (`device.db.unusable-<stamp>`, never replacing, its
    ///   bytes kept) once a `quick_check` shows it is not a store, and a fresh store takes the write.
    /// - With `rooms/` on disk, the write goes to the same key in the open database, and the store is left exactly as
    ///   it is (its loss is the N6 path's, never recreated silently).
    /// - A newer store is never renamed: its own version may still need it. The write goes to the open database.
    #[test]
    fn an_unusable_device_db_never_blocks_a_setting_write() {
        let folder = tempfile::tempdir().unwrap();
        let legacy_path = folder.path().join("bir_data.db");
        crate::db::set_database_path_override(Some(legacy_path.clone()));
        forget_ephemeral_device_store();
        let layout = Layout::here();
        assert_eq!(layout.device_dir(), folder.path());
        let open = Database::open(&legacy_path).unwrap();
        let garbage = b"this is not a SQLite file, and it is long enough to be read".to_vec();

        // 1. No rooms/: the unreadable file goes aside, kept byte for byte, and a fresh store takes the write.
        std::fs::write(layout.device_db(), &garbage).unwrap();
        assert_eq!(
            set_device_setting_or("theme_preference", "Dark", Some(&open)).unwrap(),
            SettingWritten::DeviceStore
        );
        let aside = set_aside(folder.path());
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(aside[0].1, garbage, "the unusable file is kept as it was");
        assert_eq!(
            DeviceStore::open(&layout)
                .unwrap()
                .setting("theme_preference")
                .unwrap()
                .as_deref(),
            Some("Dark")
        );
        assert_eq!(open.get_setting("theme_preference").unwrap(), None);

        // 2. rooms/ on disk and the store unreadable: the open database takes the write; nothing is renamed or made.
        let room = room_id_for("https://api.buwiz.com", "user-1");
        layout.create_room_dir(&room).unwrap();
        HELD_STORES.lock().unwrap().clear();
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(folder.path().join(format!("device.db{suffix}")));
        }
        std::fs::write(layout.device_db(), &garbage).unwrap();
        assert_eq!(
            set_device_setting_or("theme_preference", "Light", Some(&open)).unwrap(),
            SettingWritten::OpenDatabase
        );
        assert_eq!(
            open.get_setting("theme_preference").unwrap().as_deref(),
            Some("Light")
        );
        assert_eq!(std::fs::read(layout.device_db()).unwrap(), garbage);
        assert_eq!(set_aside(folder.path()).len(), 1, "nothing more set aside");
        assert_eq!(
            device_setting_or("theme_preference", Some(&open)).as_deref(),
            Some("Light"),
            "the read follows the write"
        );

        // 3. rooms/ on disk and device.db gone (lost): the open database takes the write; no device.db is made.
        std::fs::remove_file(layout.device_db()).unwrap();
        assert_eq!(
            set_device_setting_or("global_hotkey_key", "control+alt+E", Some(&open)).unwrap(),
            SettingWritten::OpenDatabase
        );
        assert!(!layout.device_db().exists(), "never recreated silently");
        assert_eq!(
            open.get_setting("global_hotkey_key").unwrap().as_deref(),
            Some("control+alt+E")
        );

        // 4. No rooms/ and a store of a newer version: never renamed; the open database takes the write.
        std::fs::remove_dir_all(layout.rooms_dir()).unwrap();
        drop(DeviceStore::open(&layout).unwrap());
        {
            let conn = Connection::open(layout.device_db()).unwrap();
            conn.pragma_update(None, "user_version", DEVICE_SCHEMA_VERSION + 1)
                .unwrap();
        }
        HELD_STORES.lock().unwrap().clear();
        let newer = std::fs::read(layout.device_db()).unwrap();
        assert_eq!(
            set_device_setting_or("log_retention_days", "14", Some(&open)).unwrap(),
            SettingWritten::OpenDatabase
        );
        assert_eq!(std::fs::read(layout.device_db()).unwrap(), newer);
        assert_eq!(set_aside(folder.path()).len(), 1);
        assert_eq!(
            open.get_setting("log_retention_days").unwrap().as_deref(),
            Some("14")
        );

        // With no open database there is nowhere else to write: the store's own error.
        assert!(set_device_setting_or("log_retention_days", "7", None).is_err());

        HELD_STORES.lock().unwrap().clear();
        crate::db::set_database_path_override(None);
    }

    /// H1 (#423's delta read): `device.db` deleted and made again (another process, or a person restoring an old copy)
    /// is a different file. The held read connection is never used on the old one: the next read sees the new file, and
    /// the next write reaches it (on Unix the old connection would write into the unlinked file, and the write would be
    /// lost). Windows cannot delete a file a connection holds; its own test is the rename aside below.
    #[cfg(unix)]
    #[test]
    fn a_recreated_device_db_gets_the_next_write() {
        let folder = tempfile::tempdir().unwrap();
        crate::db::set_database_path_override(Some(folder.path().join("bir_data.db")));
        forget_ephemeral_device_store();
        let layout = Layout::here();
        set_device_setting("theme_preference", "Dark").unwrap();
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Dark"));
        assert!(
            HELD_STORES
                .lock()
                .unwrap()
                .contains_key(&layout.device_db()),
            "a read holds its connection"
        );

        // Another process deletes device.db and makes a new one.
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(folder.path().join(format!("device.db{suffix}")));
        }
        drop(DeviceStore::open(&layout).unwrap());
        assert_eq!(
            device_setting("theme_preference"),
            None,
            "the read is of the new file"
        );
        let secure_delete = DeviceStoreRef::File(layout.clone())
            .with(false, |store| {
                Ok(store
                    .conn
                    .query_row("PRAGMA secure_delete", [], |row| row.get::<_, i64>(0))?)
            })
            .unwrap();
        assert_eq!(
            secure_delete,
            Some(1),
            "the reopened connection keeps secure_delete (B1)"
        );
        set_device_setting("theme_preference", "Light").unwrap();
        assert_eq!(
            DeviceStore::open(&layout)
                .unwrap()
                .setting("theme_preference")
                .unwrap()
                .as_deref(),
            Some("Light"),
            "the write reached the new file"
        );
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Light"));

        // A copy put over it (a restore of an older device.db) is a new file too.
        let older = folder.path().join("older.db");
        {
            let store = DeviceStore::open(&Layout::at(folder.path().join("elsewhere"))).unwrap();
            store.set_setting("theme_preference", "Older").unwrap();
        }
        std::fs::copy(folder.path().join("elsewhere").join("device.db"), &older).unwrap();
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(folder.path().join(format!("device.db{suffix}")));
        }
        std::fs::rename(&older, layout.device_db()).unwrap();
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Older"));

        HELD_STORES.lock().unwrap().clear();
        crate::db::set_database_path_override(None);
    }

    /// H1' (review of A2): a `device.db` swapped while the held read connection is being opened is the swapped-in
    /// file at the next read: the connection is recorded with the identity read before its open, never one read
    /// after it, which would pair a connection to the old file with the new file's identity for good.
    #[cfg(unix)]
    #[test]
    fn a_swap_during_the_held_open_is_seen_at_the_next_read() {
        let folder = tempfile::tempdir().unwrap();
        crate::db::set_database_path_override(Some(folder.path().join("bir_data.db")));
        forget_ephemeral_device_store();
        let layout = Layout::here();
        set_device_setting("theme_preference", "Old").unwrap();
        HELD_STORES.lock().unwrap().clear();
        // The file another process puts in its place while this one opens its read connection.
        let other = Layout::at(folder.path().join("other"));
        DeviceStore::open(&other)
            .unwrap()
            .set_setting("theme_preference", "New")
            .unwrap();
        let (from, to) = (other.device_db(), layout.device_db());
        AFTER_HELD_OPEN.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                std::fs::rename(&from, &to).unwrap();
            }))
        });
        // This read is of the file it opened (either answer is fair at that instant)...
        let _ = device_setting("theme_preference");
        // ...and every later one is of the file now at the path.
        assert_eq!(device_setting("theme_preference").as_deref(), Some("New"));
        HELD_STORES.lock().unwrap().clear();
        crate::db::set_database_path_override(None);
    }

    /// H1 on Windows: an open connection there refuses a rename of its file (os error 32), so the in-process rename aside
    /// of an unusable `device.db` (item 5) drops the held read connection first, and the write that needed it succeeds.
    #[cfg(windows)]
    #[test]
    fn the_held_connection_never_blocks_a_rename_aside() {
        let folder = tempfile::tempdir().unwrap();
        crate::db::set_database_path_override(Some(folder.path().join("bir_data.db")));
        forget_ephemeral_device_store();
        let layout = Layout::here();
        set_device_setting("theme_preference", "Dark").unwrap();
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Dark"));
        assert!(
            HELD_STORES
                .lock()
                .unwrap()
                .contains_key(&layout.device_db()),
            "a read holds its connection"
        );

        // Another program writes over the store while the connection is held.
        let garbage = b"this is not a SQLite file, and it is long enough to be read".to_vec();
        std::fs::write(layout.device_db(), &garbage).unwrap();
        set_device_setting("theme_preference", "Light").unwrap();
        let aside = set_aside(folder.path());
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(aside[0].1, garbage);
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Light"));

        HELD_STORES.lock().unwrap().clear();
        crate::db::set_database_path_override(None);
    }

    /// The file-backed path of the same rule, in single-room mode (the device folder is the named database's).
    #[test]
    fn device_settings_are_written_to_device_db_in_the_device_folder() {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("bir_data.db");
        crate::db::set_database_path_override(Some(database.clone()));
        forget_ephemeral_device_store();
        assert_eq!(device_setting("theme_preference"), None);
        assert!(
            !folder.path().join(DEVICE_DB_FILE).exists(),
            "a read makes no device.db"
        );
        set_device_setting("theme_preference", "Dark").unwrap();
        assert!(folder.path().join(DEVICE_DB_FILE).is_file());
        assert_eq!(device_setting("theme_preference").as_deref(), Some("Dark"));
        delete_device_setting("theme_preference").unwrap();
        assert_eq!(device_setting("theme_preference"), None);
        crate::db::set_database_path_override(None);
    }
}
