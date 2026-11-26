//! Windows facts the accounts-and-lock plan relies on, checked on a real machine before the code that needs them.
//!
//! - `fsync <dir>`: a folder fsync (PLAN.md A.3.2 step 4): `FlushFileBuffers` on a handle opened with
//!   `FILE_FLAG_BACKUP_SEMANTICS`, read-only and with write access.
//! - `move <dir>`: the step-4 move in that folder: hard link (never replacing), file identity (volume serial and
//!   file index, N2), unlink, folder fsyncs, and `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`.
//! - `boot`: the clock readings of B.2 (L4, X3): wall clock, `GetTickCount64`, "now minus ticks", and the boot
//!   GUID from `NtQuerySystemInformation(SystemBootEnvironmentInformation)`. The registry BootId is read by the
//!   script. Run before and after the clock is set forward within one boot.
//! - `holder <a> <b>`: the owner-lock holder of A.2 (X1): one map keyed by path, each file opened the way
//!   `try_acquire_live_owner_lock` opens it today (crates/bir-core/src/db/mod.rs, `share_mode(0)`), and another
//!   process refused.
//! - `try-open <path>`: the other process of `holder`: one `share_mode(0)` open. Exit 0 opened, 3 refused
//!   (sharing violation), 4 any other error.
//!
//! Every line printed is one JSON object.

fn main() {
    #[cfg(windows)]
    std::process::exit(probes::run(std::env::args().skip(1).collect()));
    #[cfg(not(windows))]
    {
        eprintln!("Windows only");
        std::process::exit(2);
    }
}

#[cfg(windows)]
mod probes {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle, MOVEFILE_WRITE_THROUGH,
        MoveFileExW,
    };
    use windows_sys::Win32::System::SystemInformation::GetTickCount64;

    #[link(name = "ntdll", kind = "raw-dylib")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(class: u32, buffer: *mut c_void, length: u32, returned: *mut u32) -> i32;
    }

    const SYSTEM_BOOT_ENVIRONMENT_INFORMATION: u32 = 90;

    pub fn run(args: Vec<String>) -> i32 {
        match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["fsync", dir] => fsync(Path::new(dir)),
            ["move", dir] => move_steps(Path::new(dir)),
            ["boot"] => boot(),
            ["holder", a, b] => holder(Path::new(a), Path::new(b)),
            ["try-open", path] => try_open(Path::new(path)),
            _ => {
                eprintln!("usage: fsync <dir> | move <dir> | boot | holder <a> <b> | try-open <path>");
                2
            }
        }
    }

    fn line(fields: &[(&str, String)]) {
        let body: Vec<String> = fields.iter().map(|(key, value)| format!("{key:?}:{value}")).collect();
        println!("{{{}}}", body.join(","));
    }

    fn text(value: impl std::fmt::Display) -> String {
        format!("{:?}", value.to_string())
    }

    fn outcome(result: &std::io::Result<()>) -> (String, String) {
        match result {
            Ok(()) => (text("ok"), "null".to_owned()),
            Err(error) => (text(error), error.raw_os_error().map_or("null".to_owned(), |code| code.to_string())),
        }
    }

    /// A folder fsync as A.3.2 would write it: open the folder with backup semantics, then `sync_all`
    /// (FlushFileBuffers).
    fn flush_dir(dir: &Path, write: bool) -> std::io::Result<()> {
        OpenOptions::new()
            .read(true)
            .write(write)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)?
            .sync_all()
    }

    fn fsync(dir: &Path) -> i32 {
        std::fs::create_dir_all(dir).expect("the folder");
        for write in [false, true] {
            let result = flush_dir(dir, write);
            let (result_text, code) = outcome(&result);
            line(&[
                ("probe", text("fsync")),
                ("dir", text(dir.display())),
                ("handle", text(if write { "read+write" } else { "read" })),
                ("result", result_text),
                ("os_error", code),
            ]);
        }
        0
    }

    fn identity(path: &Path) -> std::io::Result<(u32, u64, u32)> {
        let file = OpenOptions::new().read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: a valid handle and a writable struct.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
        Ok((info.dwVolumeSerialNumber, index, info.nNumberOfLinks))
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
        // SAFETY: two NUL-terminated paths. No MOVEFILE_REPLACE_EXISTING: an existing target fails.
        if unsafe { MoveFileExW(wide(from).as_ptr(), wide(to).as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn step(name: &str, result: std::io::Result<()>) {
        let (result_text, code) = outcome(&result);
        line(&[("probe", text("move")), ("step", text(name)), ("result", result_text), ("os_error", code)]);
    }

    fn move_steps(dir: &Path) -> i32 {
        let _ = std::fs::remove_dir_all(dir);
        let rooms = dir.join("rooms");
        let room = rooms.join("r1");
        let legacy = dir.join("bir_data.db");
        let room_file = room.join("bir_data.db");
        std::fs::create_dir_all(dir).expect("the data folder");
        File::create(&legacy).and_then(|mut file| file.write_all(b"legacy bytes").and_then(|()| file.sync_all())).expect("legacy");
        step("create rooms/r1", std::fs::create_dir_all(&room));
        step("fsync rooms (read+write)", flush_dir(&rooms, true));
        step("hard_link legacy -> room file", std::fs::hard_link(&legacy, &room_file));
        match (identity(&legacy), identity(&room_file)) {
            (Ok(a), Ok(b)) => line(&[
                ("probe", text("move")),
                ("step", text("identity of both names")),
                ("legacy", text(format!("volume {:08x} index {:016x} links {}", a.0, a.1, a.2))),
                ("room", text(format!("volume {:08x} index {:016x} links {}", b.0, b.1, b.2))),
                ("same_file", (a.0 == b.0 && a.1 == b.1).to_string()),
            ]),
            (a, b) => line(&[("probe", text("move")), ("step", text("identity")), ("error", text(format!("{a:?} {b:?}")))]),
        }
        step("hard_link again (must fail AlreadyExists, 183)", std::fs::hard_link(&legacy, &room_file));
        step("fsync room folder", flush_dir(&room, true));
        step("remove legacy name", std::fs::remove_file(&legacy));
        step("fsync data folder", flush_dir(dir, true));
        if let Ok((volume, index, links)) = identity(&room_file) {
            line(&[
                ("probe", text("move")),
                ("step", text("room file after unlink")),
                ("identity", text(format!("volume {volume:08x} index {index:016x} links {links}"))),
            ]);
        }
        let aside = room.join("bir_data.empty-1.db");
        File::create(&aside).expect("aside");
        step("MoveFileExW onto an existing file (must fail, 183)", move_no_replace(&aside, &room_file));
        step("MoveFileExW to a free name", move_no_replace(&aside, &room.join("bir_data.empty-2.db")));
        let read_back = std::fs::read(&room_file).map(|bytes| bytes == b"legacy bytes");
        line(&[("probe", text("move")), ("step", text("room file holds the legacy bytes")), ("result", text(format!("{read_back:?}")))]);
        0
    }

    fn boot_guid() -> String {
        let mut buffer = [0u8; 64];
        let mut returned = 0u32;
        // SAFETY: a writable buffer of the given length.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_BOOT_ENVIRONMENT_INFORMATION,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut returned,
            )
        };
        if status != 0 {
            return format!("NTSTATUS {status:#x}");
        }
        buffer[..16].iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn boot() -> i32 {
        let wall_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
        // SAFETY: a plain query.
        let tick_ms = unsafe { GetTickCount64() };
        line(&[
            ("probe", text("boot")),
            ("wall_unix_ms", wall_ms.to_string()),
            ("tick64_ms", tick_ms.to_string()),
            ("wall_minus_tick_ms", (wall_ms.saturating_sub(tick_ms)).to_string()),
            ("boot_guid", text(boot_guid())),
        ]);
        0
    }

    /// The owner-lock file opened exactly as `open_live_owner_lock_file` does on Windows today.
    fn open_lock(path: &Path) -> std::io::Result<File> {
        OpenOptions::new().create(true).read(true).write(true).truncate(false).share_mode(0).open(path)
    }

    /// The process-wide holder of A.2 (X1): one handle per path, kept after its first take.
    static HOLDER: Mutex<Option<HashMap<PathBuf, File>>> = Mutex::new(None);

    fn take(path: &Path) -> std::io::Result<usize> {
        let mut held = HOLDER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let held = held.get_or_insert_with(HashMap::new);
        if let Some(file) = held.get(path) {
            return Ok(file.as_raw_handle() as usize);
        }
        let file = open_lock(path)?;
        let handle = file.as_raw_handle() as usize;
        held.insert(path.to_path_buf(), file);
        Ok(handle)
    }

    fn release(path: &Path) {
        let mut held = HOLDER.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(held) = held.as_mut() {
            held.remove(path);
        }
    }

    fn other_process(path: &Path) -> i32 {
        let exe = std::env::current_exe().expect("this program");
        Command::new(exe).arg("try-open").arg(path).status().map_or(-1, |status| status.code().unwrap_or(-1))
    }

    fn report(name: &str, value: String) {
        line(&[("probe", text("holder")), ("step", text(name)), ("result", value)]);
    }

    fn handle_or_error(result: std::io::Result<usize>) -> String {
        match result {
            Ok(handle) => text(format!("held, handle {handle:#x}")),
            Err(error) => text(format!("refused: {error} (os error {:?})", error.raw_os_error())),
        }
    }

    fn holder(a: &Path, b: &Path) -> i32 {
        for path in [a, b] {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        // Today's bug: a second plain take of the same lock in the same process.
        let first = open_lock(a);
        let second = open_lock(a);
        let describe = |result: &std::io::Result<File>| match result {
            Ok(file) => text(format!("opened, handle {:#x}", file.as_raw_handle() as usize)),
            Err(error) => text(format!("refused: {error} (os error {:?})", error.raw_os_error())),
        };
        report("raw: first open of A in this process", describe(&first));
        report("raw: second open of A in the same process (today's code)", describe(&second));
        drop((first, second));

        // The holder: two paths side by side, and the same path asked again.
        let a1 = take(a);
        let b1 = take(b);
        let a2 = take(a);
        let same = matches!((&a1, &a2), (Ok(x), Ok(y)) if x == y);
        report("holder: take A", handle_or_error(a1));
        report("holder: take B while A is held", handle_or_error(b1));
        report("holder: take A again (same handle, no new open)", handle_or_error(a2));
        report("holder: A's second take returned the first handle", same.to_string());

        // Another process: refused on both while held.
        report("another process opens A while held (3 = refused)", other_process(a).to_string());
        report("another process opens B while held (3 = refused)", other_process(b).to_string());

        // Released: another process gets it.
        release(a);
        report("another process opens A after release (0 = opened)", other_process(a).to_string());
        report("another process opens B, still held (3 = refused)", other_process(b).to_string());
        release(b);
        0
    }

    fn try_open(path: &Path) -> i32 {
        match open_lock(path) {
            Ok(_file) => 0,
            Err(error) if error.raw_os_error() == Some(32) => 3,
            Err(error) => {
                eprintln!("try-open: {error}");
                4
            }
        }
    }
}
