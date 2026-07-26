//! Narrow platform primitives used only by validation-rules development tools.
//!
//! The runtime `bir-rules` crate does not depend on this crate. Unsafe Win32
//! FFI is isolated here so the code generator can retain `#![forbid(unsafe_code)]`.

#![deny(unsafe_op_in_unsafe_fn)]

use std::io;
use std::path::Path;

/// Atomically renames a directory without replacing any existing destination.
///
/// On Windows this calls `MoveFileExW` without
/// `MOVEFILE_REPLACE_EXISTING`. A destination that appears concurrently,
/// including an empty directory, therefore makes the operation fail.
#[cfg(windows)]
pub fn rename_directory_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
        let mut encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if encoded.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows path contains an interior NUL",
            ));
        }
        encoded.push(0);
        Ok(encoded)
    }

    let source = wide_path(source)?;
    let target = wide_path(target)?;
    // SAFETY: both buffers are NUL-terminated, remain alive for the call, and
    // contain no interior NUL. Flags deliberately omit replacement authority.
    if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// This helper is intentionally unavailable on non-Windows targets; codegen
/// uses `rustix::renameat_with(..., NOREPLACE)` there.
#[cfg(not(windows))]
pub fn rename_directory_no_replace(_source: &Path, _target: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Win32 no-replace directory rename is unavailable on this target",
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[cfg(windows)]
    #[test]
    fn empty_existing_destination_is_never_replaced() {
        let root = std::env::temp_dir().join(format!(
            "bir-rules-platform-no-replace-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir(&root).expect("create test root");
        fs::create_dir(&source).expect("create source");
        fs::create_dir(&target).expect("create empty target");
        fs::write(source.join("sentinel"), b"source").expect("write source sentinel");

        rename_directory_no_replace(&source, &target)
            .expect_err("empty existing target must block MoveFileExW");
        assert_eq!(
            fs::read(source.join("sentinel")).expect("source survives"),
            b"source"
        );
        assert!(target.is_dir(), "empty target survives");
        assert_eq!(fs::read_dir(&target).expect("read target").count(), 0);
        fs::remove_dir_all(&root).expect("remove test root");
    }
}
