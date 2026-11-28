//! The credential probe (PLAN §A.3.1): does the legacy file hold a credential, and which?
//!
//! The legacy file is never opened in place. `bir_data.db`, with its `-wal` and `-shm` when present, is copied byte for
//! byte into `<device folder>/scratch/probe-<stamp>/`, checked and migrated there by this build's own open (so the v22
//! heal and the access-mode inference read every schema from the painted era on), read, then deleted. Every launch
//! sweeps `scratch/` ([`super::LegacyClaim::sweep`]), so a probe cut short leaves no copy behind.
//!
//! "Locked" is what today's app locks on: the admin has a PIN or an authenticator under the access mode
//! (`auth_state`'s `has_admin_pin || has_admin_totp`, nsh/shell/app.ts `readAccess`). The values that open it:
//! - help desk: the admin PIN (`app_lock_pin_hash`), the admin authenticator (`app_totp_secret`), the recovery key;
//! - solo, owner R: R's PIN or authenticator (`taxpayers`), a PIN or authenticator still on one of R's branch profiles
//!   (read from the copy before the v22 heal clears them), the recovery key;
//! - no access mode, or `solo` with no owner: no lock (today's first-launch question opened it to anyone).
//!
//! A client taxpayer's PIN is never one of them: it opened only that client.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::{Failure, Permanent, PermanentReason, SCRATCH_DIR, Transient, TransientCause};
use crate::rooms::Layout;
use crate::safe_fs::FileId;

/// Which ways in the legacy file has: what the legacy unlock page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CredentialKinds {
    pub pin: bool,
    pub authenticator: bool,
    pub recovery_key: bool,
}

/// The access mode of the file, as today's `access_mode` reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Solo(String),
    Helpdesk,
    None,
}

/// What the probe read. The values are hashes and authenticator secrets of the old lock, kept in memory only for the
/// one check at the legacy unlock; `Debug` names none of them.
pub(crate) struct Probe {
    identity: Option<FileId>,
    user_version: i32,
    pin_hashes: Vec<String>,
    totp_secrets: Vec<String>,
    recovery_key_hash: Option<String>,
}

impl std::fmt::Debug for Probe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probe")
            .field("user_version", &self.user_version)
            .field("kinds", &self.kinds())
            .finish_non_exhaustive()
    }
}

impl Probe {
    /// Today's lock: the admin has a PIN or an authenticator.
    pub(crate) fn locked(&self) -> bool {
        !self.pin_hashes.is_empty() || !self.totp_secrets.is_empty()
    }

    pub(crate) fn kinds(&self) -> CredentialKinds {
        CredentialKinds {
            pin: !self.pin_hashes.is_empty(),
            authenticator: !self.totp_secrets.is_empty(),
            recovery_key: self.locked() && self.recovery_key_hash.is_some(),
        }
    }

    /// The legacy file's identity when it was copied.
    pub(crate) fn identity(&self) -> Option<FileId> {
        self.identity
    }

    pub(crate) fn pin_matches(&self, pin: &str) -> bool {
        let valid = pin.len() == 4 && pin.bytes().all(|byte| byte.is_ascii_digit());
        valid && {
            let typed = crate::crypto::hash_pin(pin);
            self.pin_hashes.contains(&typed)
        }
    }

    pub(crate) fn code_matches(&self, code: &str) -> bool {
        self.totp_secrets
            .iter()
            .any(|secret| crate::crypto::validate_totp(secret, code))
    }

    pub(crate) fn recovery_key_matches(&self, key: &str) -> bool {
        self.locked()
            && self
                .recovery_key_hash
                .as_deref()
                .is_some_and(|stored| stored == crate::crypto::hash_recovery_key(key))
    }
}

/// What [`probe_legacy`] found: the answer A3a's legacy mode stops at (§A.3.7, X2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProbe {
    /// No legacy file.
    NoFile,
    /// The file has no lock: the first real account claims it.
    Unlocked,
    /// The file holds a credential: legacy mode until A3a-2, then the legacy unlock.
    Locked(CredentialKinds),
}

/// Does the legacy file under `layout` hold a credential? Writes no record and nothing beside the file; the owner
/// lock is held through the process holder only while the copy is taken.
pub fn probe_legacy(layout: &Layout) -> Result<LegacyProbe, Failure> {
    let legacy = layout.legacy_database();
    if !super::files::exists(&legacy).map_err(super::io_failure)? {
        return Ok(LegacyProbe::NoFile);
    }
    let lock = super::hold_legacy_lock(&legacy)?;
    let probe = probe(layout, &legacy);
    drop(lock);
    let probe = probe?;
    Ok(if probe.locked() {
        LegacyProbe::Locked(probe.kinds())
    } else {
        LegacyProbe::Unlocked
    })
}

/// The probe on a scratch copy of `legacy` (the caller holds its owner lock).
pub(crate) fn probe(layout: &Layout, legacy: &Path) -> Result<Probe, Failure> {
    // A link is never claimed (C8): the identity and the hash would follow it while the link moves the link itself,
    // and a relative one would dangle in the room folder.
    if std::fs::symlink_metadata(legacy).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(permanent(
            PermanentReason::Symlink,
            "the legacy file is a symbolic link",
        ));
    }
    // A file the plain build cannot open is permanent here already (PLAN §A.3.2 step 3).
    match crate::db::database_file_kind(legacy) {
        Ok(crate::db::DatabaseFileKind::Other) => {
            return Err(permanent(
                PermanentReason::NotPlainSqlite,
                "the legacy file is not a plain SQLite database (encrypted, or not a database)",
            ));
        }
        Ok(_) => {}
        Err(crate::db::DbError::Io(error)) => return Err(super::io_failure(error)),
        Err(error) => {
            tracing::warn!("legacy file kind not read: {error}");
            return Err(transient(
                TransientCause::Io,
                "the legacy file could not be read".into(),
            ));
        }
    }
    let scratch = Scratch::new(layout).map_err(super::io_failure)?;
    let identity = crate::safe_fs::file_id(legacy).ok();
    let copy = scratch.copy_database(legacy).map_err(super::io_failure)?;
    super::cut(super::Cut::ProbeCopied);
    if crate::safe_fs::file_id(legacy).ok() != identity {
        return Err(transient(
            TransientCause::FileChanged,
            "the legacy file was replaced while it was copied".into(),
        ));
    }
    let read = read_copy(&copy);
    drop(scratch);
    let mut probe = read?;
    probe.identity = identity;
    Ok(probe)
}

/// Check, migrate and read the copy.
fn read_copy(copy: &Path) -> Result<Probe, Failure> {
    let supported = crate::db::current_schema_version().map_err(|error| {
        tracing::warn!("schema version not read: {error}");
        transient(
            TransientCause::Io,
            "the device cannot complete the file operation".into(),
        )
    })?;
    // 1. Before this build touches it: its version, its health, and the PINs and authenticators still on profiles
    //    (the v22 heal lifts one per taxpayer and clears the rest).
    let (user_version, branch) = {
        let conn = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|error| sqlite_failure("open", error))?;
        let check: String = conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| sqlite_failure("quick_check", error))?;
        if check != "ok" {
            return Err(permanent(
                PermanentReason::Damaged,
                &format!("quick_check: {check}"),
            ));
        }
        let user_version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| sqlite_failure("user_version", error))?;
        if user_version > supported {
            return Err(permanent(
                PermanentReason::NewerSchema,
                &format!("schema {user_version}, this build opens up to {supported}"),
            ));
        }
        (user_version, branch_credentials(&conn))
    };
    // 2. This build's own open of the copy: its migrations and heals, as the person's app ran them.
    let db = crate::db::Database::open(copy).map_err(open_failure)?;
    let conn = &db.conn;
    let setting = |key: &str| -> Option<String> {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
    };
    let mode = match setting("access_mode").as_deref() {
        Some("solo") => setting("admin_tin_root").map_or(Mode::None, Mode::Solo),
        Some("helpdesk") => Mode::Helpdesk,
        _ => Mode::None,
    };
    let mut pin_hashes = Vec::new();
    let mut totp_secrets = Vec::new();
    match &mode {
        Mode::Helpdesk => {
            pin_hashes.extend(setting("app_lock_pin_hash"));
            totp_secrets.extend(setting("app_totp_secret"));
        }
        Mode::Solo(root) => {
            if let Ok(credentials) = db.taxpayer_credentials(root) {
                pin_hashes.extend(credentials.pin_hash.filter(|value| !value.is_empty()));
                totp_secrets.extend(credentials.totp_secret.filter(|value| !value.is_empty()));
            }
            if let Some((pins, secrets)) = branch.get(root) {
                pin_hashes.extend(pins.iter().cloned());
                totp_secrets.extend(secrets.iter().cloned());
            }
        }
        Mode::None => {}
    }
    pin_hashes.sort();
    pin_hashes.dedup();
    totp_secrets.sort();
    totp_secrets.dedup();
    Ok(Probe {
        identity: None,
        user_version,
        pin_hashes,
        totp_secrets,
        recovery_key_hash: setting("admin_recovery_key_hash"),
    })
}

/// The PIN hashes and authenticator secrets on profile rows, by TIN root, read before any migration. Empty where the
/// schema has no such data (a missing table or column is not an error here).
fn branch_credentials(conn: &Connection) -> BTreeMap<String, (Vec<String>, Vec<String>)> {
    let mut out: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    let rows = conn.prepare(
        "SELECT substr(tin, 1, 9),
                CASE WHEN json_valid(data_json) THEN json_extract(data_json, '$.profile_pin_hash') END,
                CASE WHEN json_valid(data_json) THEN json_extract(data_json, '$.totp_secret') END
         FROM profiles WHERE length(tin) >= 9",
    );
    let Ok(mut statement) = rows else {
        return out;
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1).ok().flatten(),
            row.get::<_, Option<String>>(2).ok().flatten(),
        ))
    }) else {
        return out;
    };
    for (root, pin, secret) in rows.flatten() {
        let entry = out.entry(root).or_default();
        entry.0.extend(pin.filter(|value| !value.is_empty()));
        entry.1.extend(secret.filter(|value| !value.is_empty()));
    }
    out
}

/// A probe folder under `<device folder>/scratch/`, removed when dropped.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(layout: &Layout) -> std::io::Result<Self> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir = layout
            .device_dir()
            .join(SCRATCH_DIR)
            .join(format!("probe-{stamp}-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Copy `legacy` and its `-wal` and `-shm` byte for byte; answer the copy's path.
    fn copy_database(&self, legacy: &Path) -> std::io::Result<PathBuf> {
        let copy = self.dir.join(crate::rooms::ROOM_DB_FILE);
        std::fs::copy(legacy, &copy)?;
        for suffix in ["-wal", "-shm"] {
            let side = super::files::with_suffix(legacy, suffix);
            match std::fs::copy(&side, super::files::with_suffix(&copy, suffix)) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(copy)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            tracing::warn!(folder = %self.dir.display(), "probe copy not removed (the next launch sweeps it): {error}");
        }
    }
}

fn permanent(reason: PermanentReason, detail: &str) -> Failure {
    Failure::Permanent(Permanent {
        reason,
        detail: detail.to_owned(),
    })
}

fn transient(cause: TransientCause, detail: String) -> Failure {
    Failure::Transient(Transient {
        cause,
        copy_saved: false,
        record_room: false,
        detail,
    })
}

/// This build's open of the copy failed: the file's own state is permanent; a full disk or an I/O error is not.
fn open_failure(error: crate::db::DbError) -> Failure {
    match error {
        crate::db::DbError::Io(io) => super::io_failure(io),
        crate::db::DbError::NotPlainSqlite(_) | crate::db::DbError::Encryption => {
            permanent(PermanentReason::NotPlainSqlite, &error.to_string())
        }
        crate::db::DbError::Sqlite(sqlite) => match &sqlite {
            rusqlite::Error::SqliteFailure(failure, _)
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::DiskFull
                        | rusqlite::ErrorCode::SystemIoFailure
                        | rusqlite::ErrorCode::DatabaseBusy
                        | rusqlite::ErrorCode::DatabaseLocked
                        | rusqlite::ErrorCode::OutOfMemory
                ) =>
            {
                sqlite_failure("open", sqlite)
            }
            _ => permanent(PermanentReason::CannotOpen, &sqlite.to_string()),
        },
        other => permanent(PermanentReason::CannotOpen, &other.to_string()),
    }
}

/// A SQLite error on the copy: "not a database" and corruption are the file's (permanent); anything else (a full disk,
/// I/O) is transient.
fn sqlite_failure(what: &str, error: rusqlite::Error) -> Failure {
    use rusqlite::ErrorCode;
    match &error {
        rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
            ErrorCode::NotADatabase => {
                permanent(PermanentReason::NotPlainSqlite, &format!("{what}: {error}"))
            }
            ErrorCode::DatabaseCorrupt => {
                permanent(PermanentReason::Damaged, &format!("{what}: {error}"))
            }
            ErrorCode::DiskFull => {
                tracing::warn!(what, "legacy copy not opened: {error}");
                transient(
                    TransientCause::DiskFull,
                    "the device cannot complete the file operation".into(),
                )
            }
            _ => {
                tracing::warn!(what, "legacy copy not opened: {error}");
                transient(
                    TransientCause::Io,
                    "the legacy file could not be read".into(),
                )
            }
        },
        _ => {
            tracing::warn!(what, "legacy copy not opened: {error}");
            transient(
                TransientCause::Io,
                "the legacy file could not be read".into(),
            )
        }
    }
}
