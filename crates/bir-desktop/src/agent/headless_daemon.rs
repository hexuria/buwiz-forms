//! Pid file, logfile, and Docker-like `serve --detach` / `logs` helpers.
//!
//! Compiled without `--features agent` so path/pid/follow tests run in default
//! CI. The `bir-headless` CLI (agent feature) is the only production caller.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Override the daemon log file (tests / CI). Unset → app data `logs/` dir.
pub const LOG_ENV: &str = "BIR_HEADLESS_LOG";
/// Override the pid file path.
pub const PID_ENV: &str = "BIR_HEADLESS_PID";
/// Set on the detached child so it knows stdio already goes to the log.
pub const DETACHED_ENV: &str = "BIR_HEADLESS_DETACHED";

const EARLY_EXIT_BUDGET: Duration = Duration::from_millis(400);
const FOLLOW_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetachInfo {
    pub pid: u32,
    pub log_path: PathBuf,
    pub pid_path: PathBuf,
}

pub fn log_path() -> PathBuf {
    if let Some(path) = env_path(LOG_ENV) {
        return path;
    }
    default_log_dir().join("bir-headless.log")
}

pub fn pid_path() -> PathBuf {
    if let Some(path) = env_path(PID_ENV) {
        return path;
    }
    match std::env::var_os("BIR_DATABASE_PATH") {
        Some(db) if !db.is_empty() => {
            let db_path = PathBuf::from(db);
            let mut name = db_path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("bir_data.db"))
                .to_os_string();
            name.push(".headless.pid");
            db_path.with_file_name(name)
        }
        _ => bir_core::platform::data_dir().join("bir-headless.pid"),
    }
}

fn default_log_dir() -> PathBuf {
    match std::env::var_os("BIR_DATABASE_PATH") {
        Some(db) if !db.is_empty() => PathBuf::from(db)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("logs"),
        _ => bir_core::platform::data_dir().join("logs"),
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn missing_log_message(path: &Path) -> String {
    format!(
        "no bir-headless log at {}. Start a daemon with `bir-headless serve --detach` \
         (or a launchd unit that writes the same path). If the process is already \
         running in the foreground, its output is on that terminal, not this file.",
        path.display()
    )
}

pub fn already_running_message(pid: u32) -> String {
    format!(
        "bir-headless already running (pid {pid}). Use `bir-headless status`, \
         `bir-headless logs --follow`, or `bir-headless shutdown`. \
         Pass `--wait` to start a background waiter that takes over after the \
         current owner releases the bind and live-DB lock."
    )
}

pub fn format_detach_report(info: &DetachInfo) -> String {
    format!("pid={}\nlog={}\n", info.pid, info.log_path.display())
}

pub fn live_pid(path: &Path) -> Option<u32> {
    let pid: u32 = fs::read_to_string(path).ok()?.trim().parse().ok()?;
    if pid != 0 && process_is_alive(pid) {
        Some(pid)
    } else {
        None
    }
}

pub fn write_pid_file(path: &Path, pid: u32) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create pid directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path, format!("{pid}\n"))
        .map_err(|error| format!("failed to write pid file {}: {error}", path.display()))
}

pub fn write_current_pid() -> Result<(), String> {
    write_pid_file(&pid_path(), std::process::id())
}

pub fn remove_pid_file_if_current(path: &Path) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    if text.trim() == std::process::id().to_string() {
        let _ = fs::remove_file(path);
    }
}

pub fn ensure_log_file(path: &Path) -> Result<File, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create log directory {}: {error}",
                parent.display()
            )
        })?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("failed to open log file {}: {error}", path.display()))
}

pub fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        unix_pid_alive(pid)
    }
    #[cfg(windows)]
    {
        windows_pid_alive(pid)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

#[cfg(unix)]
fn unix_pid_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status")) {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("State:") {
                    let state = rest.trim().chars().next();
                    // Zombie / dead: the pid still exists until wait, but the
                    // daemon is not running.
                    return !matches!(state, Some('Z' | 'X'));
                }
            }
            return false;
        }
    }
    let pid = match i32::try_from(pid) {
        Ok(pid) => pid,
        Err(_) => return false,
    };
    // SAFETY: signal 0 never delivers; it only checks existence / permissions.
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn windows_pid_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
    }
    // SAFETY: Win32 process-query handles; we close before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        ok && code == STILL_ACTIVE
    }
}

/// Spawn `exe serve` (plus `--wait`) with stdio on the log file, then return.
pub fn spawn_detached_serve(exe: &Path, wait: bool) -> Result<DetachInfo, String> {
    let mut args = vec!["serve".to_string()];
    if wait {
        args.push("--wait".into());
    }
    spawn_detached_command(exe, &args)
}

pub fn spawn_detached_command(exe: &Path, args: &[String]) -> Result<DetachInfo, String> {
    let log_path = log_path();
    let pid_file = pid_path();
    let log = ensure_log_file(&log_path)?;
    let stdout = log
        .try_clone()
        .map_err(|error| format!("failed to clone log handle {}: {error}", log_path.display()))?;

    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(log))
        .env(DETACHED_ENV, "1");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: runs in the child after fork, before exec. `setsid` has no
        // memory-safety requirements; failure becomes an io::Error.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|error| format!("failed to spawn {}: {error}", exe.display()))?;
    let pid = child.id();
    write_pid_file(&pid_file, pid)?;

    if let Some(status) = wait_for_early_exit(&mut child, EARLY_EXIT_BUDGET) {
        remove_pid_file_if_pid(&pid_file, pid);
        let detail = fs::read_to_string(&log_path).unwrap_or_default();
        let mut message = format!(
            "bir-headless failed to stay running (pid {pid}, {status}). See {}",
            log_path.display()
        );
        let tail = log_tail_text(&detail, 40);
        if !tail.is_empty() {
            message.push('\n');
            message.push_str(&tail);
        }
        return Err(message);
    }

    Ok(DetachInfo {
        pid,
        log_path,
        pid_path: pid_file,
    })
}

fn remove_pid_file_if_pid(path: &Path, pid: u32) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    if text.trim() == pid.to_string() {
        let _ = fs::remove_file(path);
    }
}

fn wait_for_early_exit(child: &mut Child, budget: Duration) -> Option<std::process::ExitStatus> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if started.elapsed() >= budget => return None,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

fn log_tail_text(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// Dump the log (optional `--tail N`) and optionally follow new bytes.
///
/// `stop` is checked on idle polls so tests can exit; the CLI passes a flag
/// that stays false until the process receives Ctrl-C.
pub fn dump_or_follow(
    path: &Path,
    follow: bool,
    tail: Option<usize>,
    stop: &AtomicBool,
    out: &mut impl Write,
) -> io::Result<()> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            missing_log_message(path),
        ));
    }

    let mut pos = match tail {
        Some(n) => {
            let text = fs::read_to_string(path)?;
            byte_offset_of_last_n_lines(&text, n)
        }
        None => 0,
    };

    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        if len < pos {
            pos = 0;
        }
        file.seek(SeekFrom::Start(pos))?;
        let mut added = String::new();
        file.read_to_string(&mut added)?;
        pos = file.stream_position()?;
        if !added.is_empty() {
            out.write_all(added.as_bytes())?;
            out.flush()?;
        } else if !follow {
            return Ok(());
        } else {
            std::thread::sleep(FOLLOW_POLL);
        }
    }
}

fn byte_offset_of_last_n_lines(text: &str, n: usize) -> u64 {
    if n == 0 {
        return text.len() as u64;
    }
    let pieces: Vec<&str> = text.split_inclusive('\n').collect();
    let skip = pieces.len().saturating_sub(n);
    pieces[..skip]
        .iter()
        .map(|piece| piece.len())
        .sum::<usize>() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn isolate_paths(dir: &Path) -> (PathBuf, PathBuf) {
        let log = dir.join("logs/bir-headless.log");
        let pid = dir.join("bir-headless.pid");
        (log, pid)
    }

    #[test]
    fn env_overrides_win_over_data_dir() {
        let directory = tempfile::tempdir().expect("temp dir");
        let log = directory.path().join("custom.log");
        let pid = directory.path().join("custom.pid");
        temp_env::with_vars(
            [
                (LOG_ENV, Some(log.to_str().expect("utf8"))),
                (PID_ENV, Some(pid.to_str().expect("utf8"))),
                ("BIR_DATABASE_PATH", Some("/tmp/unused.db")),
            ],
            || {
                assert_eq!(log_path(), log);
                assert_eq!(pid_path(), pid);
            },
        );
    }

    #[test]
    fn database_path_override_keeps_pid_and_log_off_the_live_data_dir() {
        let directory = tempfile::tempdir().expect("temp dir");
        let db = directory.path().join("demo.db");
        temp_env::with_vars(
            [
                (LOG_ENV, None::<&str>),
                (PID_ENV, None::<&str>),
                ("BIR_DATABASE_PATH", Some(db.to_str().expect("utf8"))),
            ],
            || {
                assert_eq!(log_path(), directory.path().join("logs/bir-headless.log"));
                let mut expected_pid = db.clone().into_os_string();
                expected_pid.push(".headless.pid");
                assert_eq!(pid_path(), PathBuf::from(expected_pid));
            },
        );
    }

    #[test]
    fn ensure_log_file_creates_parent_and_appends() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested/out.log");
        {
            let mut file = ensure_log_file(&path).expect("create");
            writeln!(file, "first").unwrap();
        }
        {
            let mut file = ensure_log_file(&path).expect("append");
            writeln!(file, "second").unwrap();
        }
        let body = fs::read_to_string(&path).unwrap();
        assert_eq!(body, "first\nsecond\n");
    }

    #[test]
    fn dump_tail_and_missing_log() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("serve.log");
        let stop = AtomicBool::new(false);
        let missing = dump_or_follow(&path, false, None, &stop, &mut Vec::new());
        assert_eq!(missing.unwrap_err().kind(), io::ErrorKind::NotFound);

        fs::write(&path, "a\nb\nc\nd\n").unwrap();
        let mut all = Vec::new();
        dump_or_follow(&path, false, None, &stop, &mut all).unwrap();
        assert_eq!(String::from_utf8(all).unwrap(), "a\nb\nc\nd\n");

        let mut last_two = Vec::new();
        dump_or_follow(&path, false, Some(2), &stop, &mut last_two).unwrap();
        assert_eq!(String::from_utf8(last_two).unwrap(), "c\nd\n");
    }

    #[test]
    fn follow_reads_lines_appended_after_start() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("follow.log");
        fs::write(&path, "boot\n").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let collected = Arc::new(std::sync::Mutex::new(Vec::new()));
        let path_for_thread = path.clone();
        let stop_for_thread = stop.clone();
        let collected_for_thread = collected.clone();
        let worker = thread::spawn(move || {
            let mut out = SharedWriter(collected_for_thread);
            dump_or_follow(&path_for_thread, true, None, &stop_for_thread, &mut out)
        });

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let body = String::from_utf8(collected.lock().unwrap().clone()).unwrap();
            if body.contains("boot") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "follow never dumped existing line: {body:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }

        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"serve-ready\n")
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let body = String::from_utf8(collected.lock().unwrap().clone()).unwrap();
            if body.contains("serve-ready") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "follow never saw appended line: {body:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }

        stop.store(true, Ordering::Relaxed);
        worker.join().expect("follow thread").expect("follow io");
        let body = String::from_utf8(collected.lock().unwrap().clone()).unwrap();
        assert!(body.contains("boot"), "{body:?}");
        assert!(body.contains("serve-ready"), "{body:?}");
    }

    struct SharedWriter(Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("log buffer").extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn pid_file_round_trip_and_stale_pid_is_not_live() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("bir-headless.pid");
        write_pid_file(&path, std::process::id()).unwrap();
        assert_eq!(live_pid(&path), Some(std::process::id()));
        write_pid_file(&path, 1_000_000_007).unwrap();
        assert_eq!(live_pid(&path), None, "must not treat a dead pid as live");
        remove_pid_file_if_current(&path);
        assert!(path.exists(), "foreign pid must not be deleted");
        write_pid_file(&path, std::process::id()).unwrap();
        remove_pid_file_if_current(&path);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_detached_writes_pid_log_and_second_live_pid_is_detectable() {
        let directory = tempfile::tempdir().expect("temp dir");
        let (log, pid) = isolate_paths(directory.path());
        temp_env::with_vars(
            [
                (LOG_ENV, Some(log.to_str().expect("utf8"))),
                (PID_ENV, Some(pid.to_str().expect("utf8"))),
            ],
            || {
                let info = spawn_detached_command(
                    Path::new("/bin/sh"),
                    &["-c".into(), "echo listening; exec sleep 30".into()],
                )
                .expect("spawn sleep");
                assert_eq!(info.pid_path, pid);
                assert_eq!(info.log_path, log);
                assert!(process_is_alive(info.pid));
                assert_eq!(live_pid(&pid), Some(info.pid));
                let report = format_detach_report(&info);
                assert!(report.contains(&format!("pid={}", info.pid)), "{report}");
                assert!(report.contains(&log.display().to_string()), "{report}");

                let started = Instant::now();
                let mut log_body = String::new();
                while started.elapsed() < Duration::from_secs(2) {
                    log_body = fs::read_to_string(&log).unwrap_or_default();
                    if log_body.contains("listening") {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                assert!(log_body.contains("listening"), "{log_body:?}");
                assert!(live_pid(&pid).is_some());

                // Negative pid = process group created by setsid in spawn_detached.
                unsafe {
                    libc::kill(-(info.pid as i32), libc::SIGTERM);
                    let mut status = 0;
                    let stop_at = Instant::now();
                    while stop_at.elapsed() < Duration::from_secs(3) {
                        let waited = libc::waitpid(info.pid as i32, &mut status, libc::WNOHANG);
                        if waited == info.pid as i32 || waited < 0 {
                            break;
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    libc::kill(info.pid as i32, libc::SIGKILL);
                    libc::waitpid(info.pid as i32, &mut status, libc::WNOHANG);
                }
                assert!(
                    !process_is_alive(info.pid),
                    "detached child {} still alive after SIGTERM",
                    info.pid
                );
            },
        );
    }

    #[test]
    fn already_running_message_points_at_status_logs_and_wait() {
        let message = already_running_message(42);
        assert!(message.contains("pid 42"), "{message}");
        assert!(message.contains("status"), "{message}");
        assert!(message.contains("logs --follow"), "{message}");
        assert!(message.contains("shutdown"), "{message}");
        assert!(message.contains("--wait"), "{message}");
    }
}
