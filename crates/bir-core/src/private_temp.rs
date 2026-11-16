//! A private folder in the temp folder, for a plain (unencrypted) copy of the
//! database while a backup is made or restored.
//!
//! The folder is new for each operation (made with `create`, never reused: a
//! name someone else made first is an error, not a folder we write into), it
//! is this user's only (0700 on unix, its files 0600), and dropping it removes
//! it with everything in it: every return of the operation, success or error,
//! and a panic that unwinds, leave nothing behind. An abort (the release
//! profile's `panic = "abort"`), a kill or a power loss do leave it; the next
//! folder made with the same prefix sweeps such leftovers once nothing in them
//! changed for [`STALE_AFTER`] (never one this process still holds).
//!
//! Windows has no modes: `std::env::temp_dir()` is the per-user `%TEMP%`,
//! whose default ACL lets in only this user, SYSTEM and the administrators.
//! That is accepted as it is; no ACL is set here.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A leftover folder whose contents nobody changed for this long is swept.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The folders this process holds now: never swept, however old.
static HELD: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);

fn held(update: impl FnOnce(&mut HashSet<PathBuf>)) {
    let mut held = HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    update(held.get_or_insert_with(HashSet::new));
}

/// A name no other call of this process, and no other process, uses.
pub(crate) fn unique_name() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!(
        "{}-{nanos}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::SeqCst)
    )
}

#[cfg(test)]
thread_local! {
    /// Where this thread's private folders are made instead of the temp
    /// folder, so a test can see that none is left behind.
    static ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Run `body` with this thread's private folders made under `root`.
#[cfg(test)]
pub(crate) fn with_root<T>(root: &Path, body: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(ROOT.with(|slot| slot.borrow_mut().replace(root.to_path_buf())));
    body()
}

fn root() -> PathBuf {
    #[cfg(test)]
    if let Some(root) = ROOT.with(|slot| slot.borrow().clone()) {
        return root;
    }
    std::env::temp_dir()
}

/// A folder only this user can open, removed with its contents on drop.
#[derive(Debug)]
pub(crate) struct PrivateTempDir {
    path: PathBuf,
}

impl PrivateTempDir {
    /// Make a new folder `<temp>/<prefix>-<unique>`.
    /// Leftovers of the same prefix are swept first (see the module notes).
    pub(crate) fn new(prefix: &str) -> io::Result<Self> {
        let root = root();
        sweep_stale(&root, prefix, STALE_AFTER);
        let path = root.join(format!("{prefix}-{}", unique_name()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(&path)?;
            held(|held| {
                held.insert(path.clone());
            });
            let folder = Self { path };
            // The umask can only take bits away; this says exactly 0700.
            fs::set_permissions(&folder.path, fs::Permissions::from_mode(0o700))?;
            Ok(folder)
        }
        #[cfg(not(unix))]
        {
            fs::DirBuilder::new().create(&path)?;
            held(|held| {
                held.insert(path.clone());
            });
            Ok(Self { path })
        }
    }

    /// Only the SQLCipher build's restore stages here by path.
    #[cfg_attr(not(feature = "sqlcipher"), allow(dead_code))]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Create the new, empty file `name` in the folder (0600 on unix) and
    /// answer it with its path. SQLite writes into an empty file it is given
    /// and keeps its mode.
    pub(crate) fn create_file(&self, name: &str) -> io::Result<(PathBuf, fs::File)> {
        let path = self.path.join(name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        Ok((path, file))
    }
}

impl Drop for PrivateTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        held(|held| {
            held.remove(&self.path);
        });
    }
}

/// Remove the folders `<root>/<prefix>-*` that an abort, a kill or a power
/// loss left: real folders (not links), this user's on unix, not held by this
/// process, and with nothing inside changed for `stale_after`. Best effort.
fn sweep_stale(root: &Path, prefix: &str, stale_after: std::time::Duration) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let start = format!("{prefix}-");
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_name().to_string_lossy().starts_with(&start) {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_dir() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // SAFETY: geteuid only reads the process's own id.
            if meta.uid() != unsafe { libc::geteuid() } {
                continue;
            }
        }
        let mut is_held = false;
        held(|held| is_held = held.contains(&path));
        if is_held {
            continue;
        }
        let stale = newest_change(&path)
            .and_then(|changed| changed.elapsed().ok())
            .is_some_and(|age| age > stale_after);
        if stale {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

/// The latest modified time of `folder` and of what is directly in it (the
/// folders made here hold files only). `None` when something cannot be read,
/// which counts as "changed just now".
fn newest_change(folder: &Path) -> Option<std::time::SystemTime> {
    let mut newest = fs::symlink_metadata(folder).ok()?.modified().ok()?;
    for entry in fs::read_dir(folder).ok()? {
        newest = newest.max(entry.ok()?.metadata().ok()?.modified().ok()?);
    }
    Some(newest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn age(path: &Path, by: std::time::Duration) {
        let when = std::time::SystemTime::now() - by;
        let mut options = fs::OpenOptions::new();
        #[cfg(not(windows))]
        options.read(true);
        // Windows sets a time only through a handle opened with
        // FILE_WRITE_ATTRIBUTES, and opens a folder only with
        // FILE_FLAG_BACKUP_SEMANTICS (a plain open answers "Access is denied").
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
            options
                .access_mode(FILE_WRITE_ATTRIBUTES)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
        }
        options.open(path).unwrap().set_modified(when).unwrap();
    }

    /// A leftover folder (what an abort leaves: the folder and the plain
    /// database in it) is swept by the next folder of its prefix once it is
    /// older than the bound, and only then; a folder this process holds, a
    /// folder of another prefix, a file of the prefix and a recent leftover
    /// are left alone.
    #[test]
    fn the_next_folder_sweeps_what_an_abort_left_behind() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let two_hours = std::time::Duration::from_secs(2 * 60 * 60);
        let leftover = |name: &str, by: Option<std::time::Duration>| {
            let folder = root.join(name);
            fs::create_dir(&folder).unwrap();
            let file = folder.join("bir_data.db");
            fs::write(&file, b"plain pages").unwrap();
            if let Some(by) = by {
                age(&file, by);
                age(&folder, by);
            }
            folder
        };
        let stale = leftover("bir_unencrypted-1-2-3", Some(two_hours));
        let recent = leftover("bir_unencrypted-1-2-4", None);
        let other = leftover("bir_read_only_export-1-2-5", Some(two_hours));
        let stray = root.join("bir_unencrypted-1-2-6");
        fs::write(&stray, b"a file, not a folder").unwrap();
        age(&stray, two_hours);
        // Old folder, but a file inside was written a minute ago: in use.
        let busy = leftover("bir_unencrypted-1-2-7", Some(two_hours));
        fs::write(busy.join("bir_data.db-journal"), b"now").unwrap();
        age(&busy, two_hours);

        with_root(root, || {
            let held = PrivateTempDir::new("bir_unencrypted").unwrap();
            fs::write(held.path().join("bir_data.db"), b"in use").unwrap();
            age(&held.path().join("bir_data.db"), two_hours);
            age(held.path(), two_hours);
            assert!(!stale.exists(), "an old leftover is swept");
            for kept in [&recent, &other, &stray, &busy] {
                assert!(kept.exists(), "{}", kept.display());
            }

            let next = PrivateTempDir::new("bir_unencrypted").unwrap();
            assert!(
                held.path().exists(),
                "a folder this process holds is never swept"
            );
            let held_path = held.path().to_path_buf();
            drop(held);
            assert!(!held_path.exists(), "dropping removes it");
            drop(next);
        });
    }
}
