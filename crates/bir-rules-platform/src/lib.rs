//! Narrow platform primitives used only by validation-rules development tools.
//!
//! The runtime `bir-rules` crate does not depend on this crate. Unsafe Win32
//! FFI is isolated here so the code generator can retain `#![forbid(unsafe_code)]`.

#![deny(unsafe_op_in_unsafe_fn)]

use std::io;
use std::path::Path;

#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandleEx(
        file: *mut c_void,
        information_class: i32,
        information: *mut c_void,
        buffer_size: u32,
    ) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct WindowsFileStandardInfo {
    allocation_size: i64,
    end_of_file: i64,
    number_of_links: u32,
    delete_pending: u8,
    directory: u8,
}

#[cfg(windows)]
#[repr(C)]
struct WindowsFileIdInfo {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

/// Stable Windows identity returned by `FILE_ID_INFO` for an open file.
///
/// The 128-bit file ID is interpreted only together with its volume serial
/// number. Fields remain private so callers can compare complete identities
/// but cannot accidentally compare only part of one.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsFileIdentity {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

#[cfg(windows)]
fn validated_file_identity(information: WindowsFileIdInfo) -> io::Result<WindowsFileIdentity> {
    // Remote providers can legitimately report volume serial zero. It remains
    // part of the complete identity; only sentinel 128-bit file IDs are
    // rejected as invalid.
    if information.file_id == [0; 16] || information.file_id == [0xff; 16] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FILE_ID_INFO returned an invalid all-zero or all-ones 128-bit file identifier",
        ));
    }
    Ok(WindowsFileIdentity {
        volume_serial_number: information.volume_serial_number,
        file_id: information.file_id,
    })
}

#[cfg(windows)]
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt as _;

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

/// Returns the hard-link count from `FILE_STANDARD_INFO` for an open file.
///
/// This uses `GetFileInformationByHandleEx`, which is independent of the
/// legacy `BY_HANDLE_FILE_INFORMATION` query and is documented for ordinary,
/// Transparent Failover, and Scale-out SMB 3 shares. Providers can still
/// return an error or an invalid zero; callers must not interpret either as a
/// single-link result.
#[cfg(windows)]
pub fn standard_link_count(file: &std::fs::File) -> io::Result<u64> {
    use std::mem::{MaybeUninit, size_of};
    use std::os::windows::io::AsRawHandle as _;

    const FILE_STANDARD_INFO_CLASS: i32 = 1;

    let mut information = MaybeUninit::<WindowsFileStandardInfo>::uninit();
    // SAFETY: `file` owns a valid handle, `information` points to writable
    // storage of the exact FILE_STANDARD_INFO layout and size, and class 1
    // selects that structure.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FILE_STANDARD_INFO_CLASS,
            information.as_mut_ptr().cast(),
            u32::try_from(size_of::<WindowsFileStandardInfo>())
                .expect("FILE_STANDARD_INFO size fits in u32"),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful call initialized the full FILE_STANDARD_INFO.
    Ok(unsafe { information.assume_init() }.number_of_links as u64)
}

/// Returns the volume-scoped 128-bit `FILE_ID_INFO` identity of an open file.
///
/// Unsupported providers and invalid all-zero file identifiers are errors.
/// Callers must fail closed rather than fall back to a narrower identity.
#[cfg(windows)]
pub fn file_identity(file: &std::fs::File) -> io::Result<WindowsFileIdentity> {
    use std::mem::{MaybeUninit, size_of};
    use std::os::windows::io::AsRawHandle as _;

    const FILE_ID_INFO_CLASS: i32 = 0x12;

    let mut information = MaybeUninit::<WindowsFileIdInfo>::uninit();
    // SAFETY: `file` owns a valid handle, `information` points to writable
    // storage of the exact FILE_ID_INFO layout and size, and class 0x12
    // selects that structure.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FILE_ID_INFO_CLASS,
            information.as_mut_ptr().cast(),
            u32::try_from(size_of::<WindowsFileIdInfo>()).expect("FILE_ID_INFO size fits in u32"),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: a successful call initialized the full FILE_ID_INFO.
    validated_file_identity(unsafe { information.assume_init() })
}

/// Atomically renames a directory without replacing any existing destination.
///
/// On Windows this calls `MoveFileExW` without
/// `MOVEFILE_REPLACE_EXISTING`. A destination that appears concurrently,
/// including an empty directory, therefore makes the operation fail.
#[cfg(windows)]
pub fn rename_directory_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
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
    fn sentinel_file_ids_are_invalid() {
        for file_id in [[0; 16], [0xff; 16]] {
            let error = validated_file_identity(WindowsFileIdInfo {
                volume_serial_number: 7,
                file_id,
            })
            .expect_err("sentinel FILE_ID_INFO must be rejected");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }

        let zero_volume = validated_file_identity(WindowsFileIdInfo {
            volume_serial_number: 0,
            file_id: [0x5a; 16],
        })
        .expect("volume serial zero with a non-sentinel file ID remains valid");
        assert_eq!(zero_volume.volume_serial_number, 0);
        assert_eq!(zero_volume.file_id, [0x5a; 16]);
    }

    #[cfg(windows)]
    #[test]
    fn win32_information_layouts_match_the_documented_abi() {
        use std::mem::{align_of, offset_of, size_of};

        assert_eq!(size_of::<WindowsFileStandardInfo>(), 24);
        assert_eq!(align_of::<WindowsFileStandardInfo>(), 8);
        assert_eq!(offset_of!(WindowsFileStandardInfo, allocation_size), 0);
        assert_eq!(offset_of!(WindowsFileStandardInfo, end_of_file), 8);
        assert_eq!(offset_of!(WindowsFileStandardInfo, number_of_links), 16);
        assert_eq!(offset_of!(WindowsFileStandardInfo, delete_pending), 20);
        assert_eq!(offset_of!(WindowsFileStandardInfo, directory), 21);

        assert_eq!(size_of::<WindowsFileIdInfo>(), 24);
        assert_eq!(align_of::<WindowsFileIdInfo>(), 8);
        assert_eq!(offset_of!(WindowsFileIdInfo, volume_serial_number), 0);
        assert_eq!(offset_of!(WindowsFileIdInfo, file_id), 8);
    }

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

    #[cfg(windows)]
    #[test]
    fn file_id_info_distinguishes_files_and_matches_a_true_alias() {
        let root = std::env::temp_dir().join(format!(
            "bir-rules-platform-file-id-info-{}-{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let original = root.join("original.bin");
        let distinct = root.join("distinct.bin");
        let alias = root.join("alias.bin");
        fs::create_dir(&root).expect("create test root");
        fs::write(&original, b"official-bytes").expect("write original");
        fs::write(&distinct, b"other-bytes").expect("write distinct file");

        let original_file = fs::File::open(&original).expect("open original file");
        let reopened_file = fs::File::open(&original).expect("reopen original file");
        let distinct_file = fs::File::open(&distinct).expect("open distinct file");
        let original_identity = file_identity(&original_file).expect("query original FILE_ID_INFO");
        assert_eq!(
            original_identity,
            file_identity(&reopened_file).expect("query reopened FILE_ID_INFO")
        );
        assert_ne!(
            original_identity,
            file_identity(&distinct_file).expect("query distinct FILE_ID_INFO")
        );
        assert_eq!(
            standard_link_count(&original_file).expect("query ordinary FILE_STANDARD_INFO"),
            1
        );

        fs::hard_link(&original, &alias).expect("create true hard-link alias");
        let alias_file = fs::File::open(&alias).expect("open true hard-link alias");
        assert_eq!(
            original_identity,
            file_identity(&alias_file).expect("query alias FILE_ID_INFO")
        );
        assert_eq!(
            standard_link_count(&original_file).expect("query aliased FILE_STANDARD_INFO"),
            2
        );

        fs::remove_dir_all(&root).expect("remove test root");
    }
}
