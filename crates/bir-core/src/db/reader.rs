//! A second, read-only connection for reads that must not wait on the
//! writer.
//!
//! The desktop app shares one `Mutex<Database>` between its UI thread and
//! the submission worker; a read behind that mutex waits for whatever the
//! worker holds it for. In WAL mode a reader on its own connection does not
//! wait for a writer: it reads the last committed state while the writer
//! works.

use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroizing;

use super::{Database, DbError};

/// A reader rarely sees `SQLITE_BUSY` in WAL mode (recovery after a crash, a
/// truncating checkpoint). It errors after this rather than stalling the UI
/// for the writer's five seconds.
const READER_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

impl Database {
    /// Open a read-only connection to this database's file, keyed with the
    /// key [`Database::open`] already read: no second keychain read, no
    /// prompt. `Ok(None)` for an in-memory database, which has no file a
    /// second connection could open.
    ///
    /// The connection is opened read-only and `PRAGMA query_only = ON`, so a
    /// write sent to it fails with an error rather than writing. It runs no
    /// migration and holds no owner lock: the writer did and does.
    ///
    /// The key is kept only until a reader opens; this drops it.
    pub fn open_reader(&mut self) -> Result<Option<Database>, DbError> {
        let Some(path) = self
            .conn
            .path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
        else {
            return Ok(None);
        };
        let Some(key) = self.reader_key.as_ref() else {
            return Err(DbError::Other(
                "a read-only connection needs the key the database was opened with; \
                 it was already used or this database was not opened with Database::open"
                    .into(),
            ));
        };
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )?;
        conn.execute_batch(&Zeroizing::new(format!(
            "PRAGMA key = \"x'{}'\";",
            key.as_str()
        )))?;
        conn.busy_timeout(READER_BUSY_TIMEOUT)?;
        conn.execute_batch("PRAGMA query_only = ON;")?;
        let _: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
            .map_err(|error| match &error {
                rusqlite::Error::SqliteFailure(failure, _)
                    if failure.code == rusqlite::ErrorCode::NotADatabase =>
                {
                    DbError::Encryption
                }
                _ => DbError::Sqlite(error),
            })?;
        self.reader_key = None;
        Ok(Some(Database {
            conn,
            _owner_lock: None,
            reader_key: None,
        }))
    }

    /// Run `read` inside one read transaction, so a host call that makes
    /// several queries sees one committed state, as it did when it held the
    /// writer's lock for its whole run. The transaction only reads; it is
    /// rolled back when `read` returns.
    pub fn read_snapshot<R>(&self, read: impl FnOnce(&Database) -> R) -> Result<R, DbError> {
        let snapshot = self.conn.unchecked_transaction()?;
        let result = read(self);
        snapshot.rollback()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    use super::*;

    /// Well under the two-second hold: a read that waited cannot pass.
    const BOUND: Duration = Duration::from_millis(250);
    const HOLD: Duration = Duration::from_secs(2);

    /// A file-backed database (the unit-test key, never the keychain) with
    /// one committed setting, and its reader.
    fn writer_and_reader(dir: &tempfile::TempDir) -> (Arc<Mutex<Database>>, Database) {
        let mut writer = Database::open(dir.path().join("bir_data.db")).unwrap();
        writer.set_setting("t1.committed", "Juana Santos").unwrap();
        let reader = writer.open_reader().unwrap().expect("a file has a reader");
        (Arc::new(Mutex::new(writer)), reader)
    }

    /// Hold the writer's mutex on another thread, with `write_open` an
    /// uncommitted `BEGIN IMMEDIATE` write under it, until `release` or
    /// [`HOLD`].
    fn hold(
        writer: &Arc<Mutex<Database>>,
        write_open: bool,
    ) -> (mpsc::Sender<()>, std::thread::JoinHandle<()>) {
        let writer = writer.clone();
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let guard = writer.lock().unwrap();
            if write_open {
                guard
                    .conn
                    .execute_batch(
                        "BEGIN IMMEDIATE;
                         INSERT INTO settings (key, value) VALUES ('t1.uncommitted', '1');",
                    )
                    .unwrap();
            }
            held_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(HOLD);
            if write_open {
                guard.conn.execute_batch("ROLLBACK").unwrap();
            }
        });
        held_rx.recv().unwrap();
        (release_tx, thread)
    }

    /// T1: a read through the reader returns the committed rows in bounded
    /// time while another thread holds the writer: its mutex, and an open
    /// write transaction.
    #[test]
    fn a_reader_answers_while_the_writer_is_held() {
        for write_open in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let (writer, reader) = writer_and_reader(&dir);
            let (release, holder) = hold(&writer, write_open);
            let start = Instant::now();
            let (committed, uncommitted) = reader
                .read_snapshot(|db| {
                    (
                        db.get_setting("t1.committed").unwrap(),
                        db.get_setting("t1.uncommitted").unwrap(),
                    )
                })
                .unwrap();
            let took = start.elapsed();
            release.send(()).unwrap();
            holder.join().unwrap();
            eprintln!("T1 (write open: {write_open}): the reader answered in {took:?}");
            assert_eq!(committed.as_deref(), Some("Juana Santos"));
            assert_eq!(uncommitted, None, "an uncommitted write is not read");
            assert!(
                took < BOUND,
                "the read waited {took:?} on the writer (write open: {write_open})"
            );
        }
    }

    /// A write sent to the reader fails loudly; nothing reaches the file.
    #[test]
    fn a_reader_refuses_writes() {
        let dir = tempfile::tempdir().unwrap();
        let (writer, reader) = writer_and_reader(&dir);
        assert!(reader.set_setting("t1.written", "1").is_err());
        assert_eq!(
            writer.lock().unwrap().get_setting("t1.written").unwrap(),
            None
        );
        let query_only: i64 = reader
            .conn
            .query_row("PRAGMA query_only", [], |row| row.get(0))
            .unwrap();
        assert_eq!(query_only, 1);
    }

    /// The reader sees what the writer committed after it opened.
    #[test]
    fn a_reader_sees_later_commits() {
        let dir = tempfile::tempdir().unwrap();
        let (writer, reader) = writer_and_reader(&dir);
        writer.lock().unwrap().set_setting("t1.later", "2").unwrap();
        let later = reader
            .read_snapshot(|db| db.get_setting("t1.later"))
            .unwrap()
            .unwrap();
        assert_eq!(later.as_deref(), Some("2"));
    }

    #[test]
    fn the_key_opens_one_reader_and_is_then_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = Database::open(dir.path().join("bir_data.db")).unwrap();
        assert!(writer.open_reader().unwrap().is_some());
        assert!(writer.reader_key.is_none());
        assert!(writer.open_reader().is_err());
    }

    #[test]
    fn an_in_memory_database_has_no_reader() {
        let mut db = Database::open_ephemeral().unwrap();
        assert!(db.open_reader().unwrap().is_none());
    }
}
