//! File moves that never replace a file, and folder fsyncs (accounts-and-lock PLAN §A.3.2 step 4).
//!
//! `std::fs::rename` replaces its target on every system, so nothing here uses it: a move that finds its target
//! taken fails with [`std::io::ErrorKind::AlreadyExists`] and changes nothing.

use std::io;
use std::path::Path;

/// Rename `from` to `to` only if `to` does not exist, atomically: `renamex_np(RENAME_EXCL)` on Apple,
/// `renameat2(RENAME_NOREPLACE)` on Linux and Android, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on
/// Windows. A taken `to` is [`io::ErrorKind::AlreadyExists`] (Windows: os error 183, or 80 for a file), and
/// nothing moved. A system without such a call answers [`io::ErrorKind::Unsupported`]: there is no fallback that
/// could replace a file.
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    imp::rename_no_replace(from, to)
}

/// Which file a path names: the device and inode on Unix, the volume serial number and file index on Windows (equal
/// for two names of one file, as a hard link makes). Read without opening the file for data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileId {
    volume: u64,
    index: u64,
}

/// The [`FileId`] of the file at `path`.
pub fn file_id(path: &Path) -> io::Result<FileId> {
    imp::file_id(path)
}

/// How [`link_no_replace`] put the file at its new name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// A hard link: both names are the file until the caller unlinks the old one.
    Linked,
    /// Hard links are not supported there: the file was renamed, never replacing ([`rename_no_replace`]).
    Renamed,
}

/// Give the file at `from` the name `to`, never replacing a file: `hard_link` (which fails with `AlreadyExists` on a
/// taken name), or, where the file system has no hard links, [`rename_no_replace`]. "Not supported" is, on Windows,
/// the raw os error 1 (`ERROR_INVALID_FUNCTION`, what exFAT answers) or 50 (`ERROR_NOT_SUPPORTED`); on Unix `EPERM`,
/// `ENOTSUP`/`EOPNOTSUPP` or `ENOSYS` (and `EACCES` on Android, where the policy may refuse links). Any other error
/// stops: nothing was moved.
pub fn link_no_replace(from: &Path, to: &Path) -> io::Result<Placed> {
    match std::fs::hard_link(from, to) {
        Ok(()) => Ok(Placed::Linked),
        Err(error) if links_unsupported(&error) => {
            tracing::info!("hard links not supported here ({error}); renaming without replacing");
            rename_no_replace(from, to).map(|()| Placed::Renamed)
        }
        Err(error) => Err(error),
    }
}

fn links_unsupported(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::Unsupported {
        return true;
    }
    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(1) | Some(50))
    }
    #[cfg(unix)]
    {
        let code = error.raw_os_error();
        // A slice, not a pattern: ENOTSUP and EOPNOTSUPP are one value on Linux, so a pattern is unreachable there.
        [libc::EPERM, libc::ENOTSUP, libc::EOPNOTSUPP, libc::ENOSYS]
            .into_iter()
            .any(|errno| code == Some(errno))
            || (cfg!(target_os = "android") && code == Some(libc::EACCES))
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// Make a folder's entries durable: the rename, link or unlink just done in it survives a power loss. On Windows the
/// handle is opened with write access and `FILE_FLAG_BACKUP_SEMANTICS` (a read-only handle fails with os error 5 on
/// NTFS, ReFS and exFAT); where it still fails, or where a file system has no folder fsync (`EINVAL`), the failure
/// is logged and the caller goes on, as PLAN §A.3.2 says. Any other error is returned.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    match imp::sync_dir(dir) {
        Ok(()) => Ok(()),
        Err(error) if tolerated_sync_error(&error) => {
            tracing::warn!(folder = %dir.display(), "folder fsync not done: {error}");
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// The folder fsync failures that are logged, not returned: every one on Windows (PLAN §A.3.2), and the "not on this
/// file system" answers elsewhere.
fn tolerated_sync_error(error: &io::Error) -> bool {
    if cfg!(windows) {
        return true;
    }
    #[cfg(unix)]
    {
        // A slice, not a pattern: ENOTSUP and EOPNOTSUPP are one value on Linux, so a pattern is unreachable there.
        let code = error.raw_os_error();
        [libc::EINVAL, libc::ENOTSUP, libc::EOPNOTSUPP]
            .into_iter()
            .any(|errno| code == Some(errno))
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod imp {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn c_path(path: &Path) -> io::Result<CString> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path with a NUL byte"))
    }

    pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (c_path(from)?, c_path(to)?);
        // SAFETY: both are NUL-terminated paths that live across the call.
        let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn sync_dir(dir: &Path) -> io::Result<()> {
        // `sync_all` is F_FULLFSYNC on Apple systems.
        std::fs::File::open(dir)?.sync_all()
    }

    pub fn file_id(path: &Path) -> io::Result<super::FileId> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path)?;
        Ok(super::FileId {
            volume: metadata.dev(),
            index: metadata.ino(),
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod imp {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// `RENAME_NOREPLACE` from `linux/fs.h`.
    const RENAME_NOREPLACE: libc::c_uint = 1;

    fn c_path(path: &Path) -> io::Result<CString> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path with a NUL byte"))
    }

    pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (c_path(from)?, c_path(to)?);
        // The system call itself: bionic's `renameat2` wrapper exists only from API 30.
        // SAFETY: both are NUL-terminated paths that live across the call; AT_FDCWD resolves them as given.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                RENAME_NOREPLACE,
            )
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn sync_dir(dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }

    pub fn file_id(path: &Path) -> io::Result<super::FileId> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(path)?;
        Ok(super::FileId {
            volume: metadata.dev(),
            index: metadata.ino(),
        })
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::Path;

    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    /// FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE: reading the identity never stands in another open's way.
    const SHARE_ALL: u32 = 0x7;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    /// `BY_HANDLE_FILE_INFORMATION`.
    #[repr(C)]
    #[derive(Default)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation: FileTime,
        last_access: FileTime,
        last_write: FileTime,
        volume_serial_number: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        fn GetFileInformationByHandle(
            file: *mut core::ffi::c_void,
            info: *mut ByHandleFileInformation,
        ) -> i32;
    }

    pub fn file_id(path: &Path) -> io::Result<super::FileId> {
        use std::os::windows::io::AsRawHandle;
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(SHARE_ALL)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)?;
        let mut info = ByHandleFileInformation::default();
        // SAFETY: the handle is open for the call, and `info` is a properly laid out out-parameter.
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(super::FileId {
            volume: u64::from(info.volume_serial_number),
            index: (u64::from(info.index_high) << 32) | u64::from(info.index_low),
        })
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (wide(from), wide(to));
        // No MOVEFILE_REPLACE_EXISTING: a taken name fails with ERROR_ALREADY_EXISTS (183).
        // SAFETY: both are NUL-terminated UTF-16 paths that live across the call.
        let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if ok != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn sync_dir(dir: &Path) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)?
            .sync_all()
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    use std::io;
    use std::path::Path;

    pub fn rename_no_replace(_from: &Path, _to: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no rename that refuses to replace on this system",
        ))
    }

    pub fn sync_dir(_dir: &Path) -> io::Result<()> {
        Ok(())
    }

    pub fn file_id(_path: &Path) -> io::Result<super::FileId> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no file identity on this system",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_never_replaces_its_target() {
        let folder = tempfile::tempdir().unwrap();
        let (from, to) = (folder.path().join("a"), folder.path().join("b"));
        std::fs::write(&from, b"from").unwrap();
        std::fs::write(&to, b"kept").unwrap();
        let error = rename_no_replace(&from, &to).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists, "{error}");
        assert_eq!(std::fs::read(&from).unwrap(), b"from");
        assert_eq!(std::fs::read(&to).unwrap(), b"kept");

        std::fs::remove_file(&to).unwrap();
        rename_no_replace(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(std::fs::read(&to).unwrap(), b"from");
        sync_dir(folder.path()).unwrap();
    }
}
