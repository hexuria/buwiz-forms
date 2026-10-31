//! Several processes on one SQLCipher file.
//!
//! bir-shell, bir-mini-app, painted bir and bir-headless may all have
//! `bir_data.db` open at once. SQLite's WAL mode and the 5 s busy timeout
//! let them read and write side by side; the sidecar files here cover what
//! SQLite does not:
//!
//! - `bir_data.db.maintenance.lock`, exclusive and brief: held only while the
//!   file is created, the master key is created, migrations run, or an
//!   unreadable file is moved aside. Waits up to [`MAINTENANCE_LOCK_WAIT`].
//! - `bir_data.db.worker.lock`, exclusive while one process runs the
//!   background jobs (filing queue, cron, calendar sync). The first process
//!   to take it keeps it until it quits; the others retry.
//! - `bir_data.db.open.lock`, shared by every open connection, so an
//!   unreadable-looking file is never moved aside while another process has
//!   it open (with a key that works for it).
//!
//! All three use std's `File::lock` family: `flock` on macOS and Linux,
//! `LockFileEx` on Windows. The OS drops them when a process dies.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::DbError;

/// How long a process waits for another one's create / migrate / repair.
pub const MAINTENANCE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// How often a process without the worker lease tries to take it.
pub const WORKER_LEASE_RETRY: Duration = Duration::from_secs(30);

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut raw = db_path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

/// `bir_data.db.maintenance.lock`.
pub fn maintenance_lock_path(db_path: &Path) -> PathBuf {
    sidecar(db_path, ".maintenance.lock")
}

/// `bir_data.db.worker.lock`.
pub fn worker_lock_path(db_path: &Path) -> PathBuf {
    sidecar(db_path, ".worker.lock")
}

/// `bir_data.db.open.lock`.
pub fn open_lock_path(db_path: &Path) -> PathBuf {
    sidecar(db_path, ".open.lock")
}

/// `bir_data.db.owner.lock`: builds before this change held it exclusively
/// for their whole life. Only read now, to tell such a build is running.
fn legacy_owner_lock_path(db_path: &Path) -> PathBuf {
    sidecar(db_path, ".owner.lock")
}

fn open_sidecar(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
}

thread_local! {
    /// Maintenance locks this thread holds, so a nested
    /// [`with_maintenance_lock`] on the same file runs instead of waiting on
    /// itself (open-or-recreate re-opens, and an open creates the key, both
    /// under the lock).
    static HELD: RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());
}

/// Run `f` holding the database's maintenance lock.
///
/// Brief and blocking: waits up to [`MAINTENANCE_LOCK_WAIT`] for another
/// process's create, migration, key creation or repair, then gives up with
/// [`DbError::MaintenanceBusy`]. Never held for a process's life.
pub fn with_maintenance_lock<T>(
    db_path: &Path,
    f: impl FnOnce() -> Result<T, DbError>,
) -> Result<T, DbError> {
    with_maintenance_lock_waiting(db_path, MAINTENANCE_LOCK_WAIT, f)
}

pub(crate) fn with_maintenance_lock_waiting<T>(
    db_path: &Path,
    wait: Duration,
    f: impl FnOnce() -> Result<T, DbError>,
) -> Result<T, DbError> {
    let lock_path = maintenance_lock_path(db_path);
    if HELD.with(|held| held.borrow().contains(&lock_path)) {
        return f();
    }
    let file = open_sidecar(&lock_path)?;
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(DbError::MaintenanceBusy(db_path.to_path_buf()));
            }
            Err(TryLockError::Error(error)) => return Err(DbError::Io(error)),
        }
    }
    HELD.with(|held| held.borrow_mut().insert(lock_path.clone()));
    let result = f();
    HELD.with(|held| held.borrow_mut().remove(&lock_path));
    let _ = file.unlock();
    result
}

/// The shared lock an open connection holds on `bir_data.db.open.lock`.
/// `None` when the lock file cannot be opened (a read-only folder): the
/// connection still works, it just does not stop a repair elsewhere.
pub(crate) fn hold_open_lock(db_path: &Path) -> Option<File> {
    let file = open_sidecar(&open_lock_path(db_path)).ok()?;
    match file.lock_shared() {
        Ok(()) => Some(file),
        Err(error) => {
            tracing::warn!(%error, "database open lock not taken");
            None
        }
    }
}

/// Whether some other connection (this process or another) has the file
/// open: it holds the shared open lock, or an older build holds the
/// life-long owner lock. Moving or replacing the file then would pull it out
/// from under a process that reads it fine.
pub fn database_in_use_elsewhere(db_path: &Path) -> bool {
    sole_use(db_path).is_none()
}

/// The open lock held exclusively, when no connection has the file open and
/// no older build holds its owner lock. While it is held, a new open waits
/// (it takes the shared lock before opening the file), so the file can be
/// moved aside without anyone opening the old one.
pub(crate) fn sole_use(db_path: &Path) -> Option<File> {
    let legacy = legacy_owner_lock_path(db_path);
    if legacy.exists() {
        // Unknown counts as held: then nothing is moved aside.
        let file = open_sidecar(&legacy).ok()?;
        file.try_lock().ok()?;
        let _ = file.unlock();
    }
    let file = open_sidecar(&open_lock_path(db_path)).ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// The right to run the background jobs for one database file.
///
/// Held by at most one process at a time (`bir_data.db.worker.lock`); the
/// OS releases it when the process quits or crashes, and the next process
/// that tries takes it over.
#[derive(Debug)]
pub struct WorkerLease {
    file: File,
}

impl WorkerLease {
    /// Take the lease when nobody holds it; `Ok(None)` when another
    /// process (or another lease in this one) does.
    pub fn try_acquire(db_path: &Path) -> Result<Option<Self>, DbError> {
        let file = open_sidecar(&worker_lock_path(db_path))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(DbError::Io(error)),
        }
    }
}

impl Drop for WorkerLease {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// The master key, created at most once across processes: read it; when
/// that fails, take the maintenance lock and read again (another process
/// may just have created it) before `create`.
pub(crate) fn master_key_once(
    db_path: &Path,
    existing: impl Fn() -> Result<String, DbError>,
    create: impl FnOnce() -> Result<String, DbError>,
) -> Result<String, DbError> {
    if let Ok(key) = existing() {
        return Ok(key);
    }
    with_maintenance_lock(db_path, || existing().or_else(|_| create()))
}

/// SQLITE_BUSY / SQLITE_LOCKED: another connection holds a lock. Transient,
/// never a reason to move the file aside.
pub fn is_busy_or_locked(error: &DbError) -> bool {
    is_sqlite_busy(error) || matches!(error, DbError::MaintenanceBusy(_))
}

fn is_sqlite_busy(error: &DbError) -> bool {
    matches!(
        error,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(failure, _))
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

/// Run an open again when it failed only because SQLite found the file busy
/// (a maintenance-lock timeout has already waited 30 s and is not retried):
/// up to four tries in all (0.25 s, 0.5 s, then 1 s apart), each one
/// already waiting out SQLite's busy timeout where SQLite allows it.
pub(crate) fn retry_when_busy<T>(
    mut attempt: impl FnMut() -> Result<T, DbError>,
) -> Result<T, DbError> {
    let mut tries: u32 = 0;
    loop {
        match attempt() {
            Err(error) if tries < 3 && is_sqlite_busy(&error) => {
                tries += 1;
                tracing::warn!(%error, tries, "database busy while opening; trying again");
                std::thread::sleep(Duration::from_millis(250 << (tries - 1)));
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Barrier};

    /// Environment variables a child copy of this test binary reads.
    const CHILD_ROLE: &str = "BIR_SHARED_DB_TEST_ROLE";
    const CHILD_DB: &str = "BIR_SHARED_DB_TEST_PATH";

    /// The longest any child step may take before the parent kills it and
    /// fails, so a deadlock fails fast instead of hanging the run.
    const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

    /// What a child prints to say where it is. The test harness writes
    /// `test <name> ... ` with no newline before running the test body
    /// (`--nocapture`), so a marker can land mid-line: match it anywhere.
    fn marker(step: &str) -> String {
        format!("@@bir-shared-db-child:{step}@@")
    }

    /// A child copy of this test binary, killed if the parent gives up.
    struct Kid {
        child: std::process::Child,
        lines: std::sync::mpsc::Receiver<String>,
    }

    impl Kid {
        /// Run one `#[test]` of this binary as a separate process.
        fn spawn(test: &str, role: &str, db: &Path) -> Self {
            let mut child = Command::new(std::env::current_exe().expect("test binary"))
                .args(["--exact", test, "--nocapture", "--test-threads=1"])
                .env(CHILD_ROLE, role)
                .env(CHILD_DB, db)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("spawn child test process");
            let stdout = child.stdout.take().expect("child stdout");
            let (sender, lines) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stdout).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            Self { child, lines }
        }

        /// Wait (at most [`CHILD_TIMEOUT`]) for the child to print `step`.
        fn wait_for(&mut self, step: &str) {
            let wanted = marker(step);
            let deadline = Instant::now() + CHILD_TIMEOUT;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.lines.recv_timeout(left) {
                    Ok(line) if line.contains(&wanted) => return,
                    Ok(_) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        let _ = self.child.kill();
                        panic!("child did not reach {step:?} within {CHILD_TIMEOUT:?}");
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        panic!("child exited before reaching {step:?}");
                    }
                }
            }
        }

        /// Close the child's stdin (its signal to finish).
        fn close_stdin(&mut self) {
            drop(self.child.stdin.take());
        }

        /// Wait (at most [`CHILD_TIMEOUT`]) for the child to exit.
        fn finish(&mut self) -> std::process::ExitStatus {
            let deadline = Instant::now() + CHILD_TIMEOUT;
            loop {
                if let Some(status) = self.child.try_wait().expect("child status") {
                    return status;
                }
                if Instant::now() >= deadline {
                    let _ = self.child.kill();
                    panic!("child did not exit within {CHILD_TIMEOUT:?}");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    impl Drop for Kid {
        fn drop(&mut self) {
            if matches!(self.child.try_wait(), Ok(None)) {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    fn child_role() -> Option<(String, PathBuf)> {
        let role = std::env::var(CHILD_ROLE).ok()?;
        let db = std::env::var_os(CHILD_DB)?;
        Some((role, PathBuf::from(db)))
    }

    /// The child half of the process tests. Does nothing in a normal run.
    #[test]
    fn shared_db_child_process() {
        let Some((role, db)) = child_role() else {
            return;
        };
        match role.as_str() {
            "writer" => {
                let database = Database::open(&db).expect("child opens the shared file");
                println!("{}", marker("open"));
                for index in 0..50 {
                    database
                        .set_setting(&format!("child-{index}"), "written by the child")
                        .expect("child write while the parent writes");
                }
                println!("{}", marker("done"));
            }
            "lease-holder" => {
                let lease = WorkerLease::try_acquire(&db)
                    .expect("lease file")
                    .expect("the child is first: it holds the lease");
                println!("{}", marker("holding"));
                // Hold until the parent closes our stdin.
                let mut sink = String::new();
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut sink);
                drop(lease);
            }
            other => panic!("unknown child role {other}"),
        }
    }

    const CHILD_TEST: &str = "db::sharing::tests::shared_db_child_process";

    #[test]
    fn two_processes_write_the_same_database() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let parent = Database::open(&path).expect("parent opens the file");
        let mut child = Kid::spawn(CHILD_TEST, "writer", &path);
        child.wait_for("open");
        for index in 0..50 {
            parent
                .set_setting(&format!("parent-{index}"), "written by the parent")
                .expect("parent write while the child writes");
        }
        child.wait_for("done");
        let status = child.finish();
        assert!(status.success(), "child writer failed: {status}");
        let count: i64 = parent
            .conn
            .query_row(
                "SELECT count(*) FROM settings WHERE key LIKE 'parent-%' OR key LIKE 'child-%'",
                [],
                |row| row.get(0),
            )
            .expect("count both writers' rows");
        assert_eq!(count, 100, "the parent sees its 50 rows and the child's 50");
    }

    #[test]
    fn only_one_process_holds_the_worker_lease() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let mut child = Kid::spawn(CHILD_TEST, "lease-holder", &path);
        child.wait_for("holding");
        assert!(
            WorkerLease::try_acquire(&path)
                .expect("lease file")
                .is_none(),
            "a second process must be a follower while the first holds the lease"
        );
        child.close_stdin();
        let status = child.finish();
        assert!(status.success(), "lease holder failed: {status}");
        let lease = WorkerLease::try_acquire(&path)
            .expect("lease file")
            .expect("the follower takes over once the holder quit");
        assert!(
            WorkerLease::try_acquire(&path)
                .expect("lease file")
                .is_none(),
            "the new holder keeps it"
        );
        drop(lease);
        assert!(
            WorkerLease::try_acquire(&path)
                .expect("lease file")
                .is_some()
        );
    }

    #[test]
    fn a_busy_database_is_never_quarantined() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        {
            let db = Database::open(&path).expect("create");
            db.set_setting("probe", "keep").expect("write");
        }
        // Another process holding the file in exclusive locking mode: every
        // other connection gets SQLITE_BUSY until it lets go.
        let holder = rusqlite::Connection::open(&path).expect("raw connection");
        holder
            .execute_batch(&format!(
                "PRAGMA key = \"x'{}'\"; PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE;
                 INSERT INTO settings (key, value) VALUES ('holder', 'x')
                 ON CONFLICT(key) DO UPDATE SET value = 'x';",
                Database::get_or_create_master_key().unwrap()
            ))
            .expect("hold the file");
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1000));
            holder.execute_batch("COMMIT;").expect("commit");
            drop(holder);
        });
        let (db, backup) =
            Database::open_or_recreate(&path).expect("opens once the holder lets go");
        release.join().unwrap();
        assert!(backup.is_none(), "a busy file must not be moved aside");
        assert_eq!(db.get_setting("probe").unwrap(), Some("keep".into()));
        let corrupt = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().contains("corrupt"));
        assert!(!corrupt);

        let busy = DbError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            None,
        ));
        assert!(is_busy_or_locked(&busy));
        assert!(!Database::should_recreate_after_open_error(&busy));
    }

    #[test]
    fn an_unreadable_file_another_process_has_open_is_not_moved_aside() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        std::fs::write(&path, b"not-a-sqlite-database").unwrap();
        // Another process has it open (with a key that works for it).
        let other = hold_open_lock(&path).expect("shared open lock");
        assert!(database_in_use_elsewhere(&path));
        let error = match Database::open_or_recreate(&path) {
            Ok(_) => panic!("must not recreate a file in use elsewhere"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("open in another"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"not-a-sqlite-database");
        drop(other);
        assert!(!database_in_use_elsewhere(&path));
        let (_db, backup) = Database::open_or_recreate(&path).expect("repair once nobody has it");
        assert!(backup.expect("moved aside").exists());
    }

    #[test]
    fn an_older_build_holding_the_owner_lock_blocks_a_repair() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        std::fs::write(&path, b"not-a-sqlite-database").unwrap();
        let legacy = open_sidecar(&legacy_owner_lock_path(&path)).unwrap();
        legacy.try_lock().expect("older build's owner lock");
        assert!(database_in_use_elsewhere(&path));
        assert!(Database::open_or_recreate(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not-a-sqlite-database");
    }

    #[test]
    fn two_first_runs_create_one_key() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let store = directory.path().join("bir_key.txt");
        let racers = 6;
        let barrier = Arc::new(Barrier::new(racers));
        let threads: Vec<_> = (0..racers)
            .map(|_| {
                let (path, store, barrier) = (path.clone(), store.clone(), barrier.clone());
                std::thread::spawn(move || {
                    let existing = || match std::fs::read_to_string(&store) {
                        Ok(key) if key.len() == 64 => Ok(key),
                        _ => Err(DbError::Other("no key yet".into())),
                    };
                    let create = || {
                        // Slow store: the window two first runs would race in.
                        std::thread::sleep(Duration::from_millis(50));
                        let key = hex::encode(rand::random::<[u8; 32]>());
                        std::fs::write(&store, &key)?;
                        Ok(key)
                    };
                    barrier.wait();
                    master_key_once(&path, existing, create).expect("key")
                })
            })
            .collect();
        let keys: HashSet<String> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(keys.len(), 1, "every first run reads the same key");
        assert_eq!(
            std::fs::read_to_string(&store).unwrap(),
            keys.into_iter().next().unwrap()
        );
    }

    #[test]
    fn migrations_run_once_under_concurrency() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let racers = 4;
        let barrier = Arc::new(Barrier::new(racers));
        let threads: Vec<_> = (0..racers)
            .map(|_| {
                let (path, barrier) = (path.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    Database::open(&path).expect("concurrent first open")
                })
            })
            .collect();
        let opened: Vec<Database> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        let canonical = std::fs::canonicalize(&path).unwrap();
        let fresh = crate::db::migrations::fresh_migrations_of(&canonical);
        assert_eq!(fresh, 1, "exactly one open built the schema");
        let version: i32 = opened[0]
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, crate::db::migrations::current_version());
    }

    #[test]
    fn close_succeeds_with_another_connection_open() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let first = Database::open(&path).expect("first");
        let second = Database::open(&path).expect("second");
        first.set_setting("from-first", "1").unwrap();
        // The second connection is mid-read: a TRUNCATE checkpoint would
        // have to wait for it.
        second
            .conn
            .execute_batch("BEGIN; SELECT count(*) FROM settings;")
            .unwrap();
        let started = Instant::now();
        first
            .close()
            .expect("close never fails because others are open");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "close must not wait on readers"
        );
        second.conn.execute_batch("COMMIT;").unwrap();
        second.set_setting("from-second", "2").unwrap();
        assert_eq!(second.get_setting("from-first").unwrap(), Some("1".into()));
    }

    #[test]
    fn a_maintenance_lock_held_elsewhere_times_out_clearly() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("bir_data.db");
        let other = open_sidecar(&maintenance_lock_path(&path)).unwrap();
        other.try_lock().unwrap();
        let error = with_maintenance_lock_waiting(&path, Duration::from_millis(100), || Ok(()))
            .unwrap_err();
        assert!(matches!(error, DbError::MaintenanceBusy(_)), "{error}");
        drop(other);
        // Nested on one thread: runs instead of waiting on itself.
        let nested = with_maintenance_lock(&path, || with_maintenance_lock(&path, || Ok(7)));
        assert_eq!(nested.unwrap(), 7);
    }
}
