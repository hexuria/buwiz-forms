//! Installs made with the app's own code, in a temporary folder of their own, for the claim's tests and the
//! rehearsal (PLAN "A2 rehearsal"). Nothing here reaches a real data folder.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use super::{Claimant, LegacyClaim};
use crate::db::Database;
use crate::profile::TaxpayerType;
use crate::rooms::{Layout, RoomId};
use crate::tax_profile::{BranchFacts, NewTaxpayer};

/// A base32 authenticator secret (20 bytes).
pub(crate) const TOTP_SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

/// The current 6-digit code of [`TOTP_SECRET`]-style secrets.
pub(crate) fn totp_code(secret: &str) -> String {
    use totp_rs::{Algorithm, Secret, TOTP};
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(secret.to_owned()).to_bytes().unwrap(),
        None,
        String::new(),
    )
    .unwrap()
    .generate_current()
    .unwrap()
}

/// A wrong code for `secret` (never its current one, nor its neighbours).
pub(crate) fn wrong_code(secret: &str) -> String {
    let right: u32 = totp_code(secret).parse().unwrap();
    format!("{:06}", (right + 500_000) % 1_000_000)
}

/// One device folder: the legacy file and everything beside it.
pub(crate) struct Install {
    _dir: tempfile::TempDir,
    pub(crate) layout: Layout,
}

impl Install {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::at(dir.path().join("data"));
        std::fs::create_dir_all(layout.device_dir()).unwrap();
        Self { _dir: dir, layout }
    }

    pub(crate) fn dir(&self) -> &Path {
        self.layout.device_dir()
    }

    pub(crate) fn legacy(&self) -> PathBuf {
        self.layout.legacy_database()
    }

    /// The legacy file opened as the previous release opens it (this build's open: schema 24, as v0.2.0's).
    pub(crate) fn open_legacy(&self) -> Database {
        Database::open(self.legacy()).unwrap()
    }

    pub(crate) fn claim(&self) -> LegacyClaim {
        LegacyClaim::at(self.layout.clone()).unwrap()
    }

    pub(crate) fn room_file(&self, room: &RoomId) -> PathBuf {
        self.layout.room_database(room)
    }

    /// The bytes of `bir_data.db`, `-wal` and `-shm` (absent ones as `None`).
    pub(crate) fn legacy_bytes(&self) -> [Option<Vec<u8>>; 3] {
        ["", "-wal", "-shm"]
            .map(|suffix| std::fs::read(super::files::with_suffix(&self.legacy(), suffix)).ok())
    }

    /// The names in `folder` under the device folder (empty if it is not there).
    pub(crate) fn names(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir().join(folder))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

pub(crate) fn account(user: &str) -> Claimant {
    Claimant {
        server_url: "https://api.buwiz.com".into(),
        user_id: user.into(),
        email: Some(format!("{user}@example.com")),
        practice: false,
    }
}

pub(crate) fn practice_account() -> Claimant {
    Claimant {
        practice: true,
        ..account("practice")
    }
}

pub(crate) fn room_of(claim: &LegacyClaim, who: &Claimant) -> RoomId {
    claim.claim_room(who).unwrap().room_id
}

pub(crate) fn taxpayer(db: &Database, root: &str, name: &str) {
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

pub(crate) fn draft(db: &Database, tin: &str, form: &str, year: u16, quarter: Option<u8>) {
    db.save_form_draft(
        tin,
        form,
        year,
        quarter,
        &crate::forms::FilingStatus::Draft,
        &serde_json::json!({ "note": format!("{form} {year}") }),
    )
    .unwrap();
}

/// A solo install: owner R (`123456788`) with `pin` (or none), two more taxpayers, drafts, a theme.
pub(crate) fn solo(
    install: &Install,
    owner_pin: Option<&str>,
    owner_totp: Option<&str>,
) -> Database {
    let db = install.open_legacy();
    taxpayer(&db, "123456788", "Owner Trading");
    taxpayer(&db, "223456788", "Family Member");
    taxpayer(&db, "323456788", "Another Member");
    db.set_taxpayer_pin_hash(
        "123456788",
        owner_pin.map(crate::crypto::hash_pin).as_deref(),
    )
    .unwrap();
    db.set_taxpayer_totp_secret("123456788", owner_totp)
        .unwrap();
    db.set_taxpayer_pin_hash("223456788", Some(&crate::crypto::hash_pin("2222")))
        .unwrap();
    db.set_setting("access_mode", "solo").unwrap();
    db.set_setting("admin_tin_root", "123456788").unwrap();
    db.set_setting("theme_preference", "Dark").unwrap();
    db.set_setting("buwiz_server_url", "https://old.example")
        .unwrap();
    draft(&db, "12345678800000", "0619E", 2026, Some(1));
    draft(&db, "22345678800000", "1701Q", 2026, Some(2));
    db
}

/// A help-desk install: the admin PIN `admin_pin`, an authenticator, a recovery key; clients with PINs.
pub(crate) fn helpdesk(install: &Install, admin_pin: &str, recovery_key: Option<&str>) -> Database {
    let db = install.open_legacy();
    db.set_setting("access_mode", "helpdesk").unwrap();
    db.set_setting("app_lock_pin_hash", &crate::crypto::hash_pin(admin_pin))
        .unwrap();
    db.set_setting("app_totp_secret", TOTP_SECRET).unwrap();
    if let Some(key) = recovery_key {
        db.set_setting(
            "admin_recovery_key_hash",
            &crate::crypto::hash_recovery_key(key),
        )
        .unwrap();
    }
    for (index, root) in ["423456788", "523456788", "623456788"].iter().enumerate() {
        taxpayer(&db, root, &format!("Client {index}"));
        db.set_taxpayer_pin_hash(
            root,
            Some(&crate::crypto::hash_pin(&format!("{}", 5000 + index))),
        )
        .unwrap();
        draft(&db, &format!("{root}00000"), "0619E", 2026, Some(1));
    }
    db
}

/// An install with data and no lock: no access mode at all (killed between the taxpayer and `setup_solo`).
pub(crate) fn unlocked(install: &Install) -> Database {
    let db = install.open_legacy();
    taxpayer(&db, "123456788", "Owner Trading");
    draft(&db, "12345678800000", "0619E", 2026, Some(1));
    db
}

/// Close `db` the way the app does at quit: the `-wal` folded in and gone.
pub(crate) fn close(db: Database) {
    db.close().unwrap();
}

/// Raw SQL on a closed file (as an older build wrote it), with no migration.
pub(crate) fn raw(path: &Path, sql: &str, values: &[&dyn rusqlite::ToSql]) {
    let conn = Connection::open(path).unwrap();
    conn.execute(sql, values).unwrap();
    conn.close().unwrap();
}

/// Copy `source` (with a live connection holding commits in its `-wal`) to `target` as a crash leaves it: the main
/// file, the `-wal` and the `-shm`, with nothing holding them.
pub(crate) fn crash_copy(source: &Path, target: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let from = super::files::with_suffix(source, suffix);
        if from.exists() {
            std::fs::copy(&from, super::files::with_suffix(target, suffix)).unwrap();
        }
    }
}

/// Row counts of the data tables, for "every row is there".
pub(crate) fn counts(path: &Path) -> Vec<(String, i64)> {
    let conn =
        Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut out = Vec::new();
    for table in ["profiles", "taxpayers", "form_drafts", "settings"] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        out.push((table.to_owned(), count));
    }
    out
}

/// A setting of the file at `path`, read raw.
pub(crate) fn setting_of(path: &Path, key: &str) -> Option<String> {
    let conn =
        Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .unwrap()
}

/// Take the owner lock of `database` as another process would (a separate open file description on Unix, a
/// share-0 open on Windows): `true` when it is free.
pub(crate) fn foreign_lock_free(database: &Path) -> bool {
    let lock_path = crate::db::live_owner_lock_path(database);
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        // SAFETY: a valid descriptor for the call.
        let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if taken {
            // SAFETY: as above.
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        }
        taken
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .share_mode(0)
            .open(&lock_path)
            .is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = lock_path;
        true
    }
}
