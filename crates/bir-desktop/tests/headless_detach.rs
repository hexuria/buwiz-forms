//! End-to-end `bir-headless serve --detach` / `logs` / `status` / `shutdown`.
//!
//! Isolated temp DB + log + pid. Never opens the live app-group / `~/.taxman-ebir`
//! database.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct Harness {
    bin: PathBuf,
    _dir: tempfile::TempDir,
    addr: String,
    token: &'static str,
    db: PathBuf,
    log: PathBuf,
    pid: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let holder = TcpListener::bind("127.0.0.1:0").expect("ephemeral port");
        let addr = holder.local_addr().expect("addr").to_string();
        drop(holder);
        let db = dir.path().join("bir_data.db");
        let log = dir.path().join("logs/bir-headless.log");
        let pid = dir.path().join("bir-headless.pid");
        Self {
            bin: PathBuf::from(env!("CARGO_BIN_EXE_bir-headless")),
            _dir: dir,
            addr,
            token: "detach-e2e-secret",
            db,
            log,
            pid,
        }
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args)
            .env("GPUI_AGENT", "1")
            .env("GPUI_AGENT_TOKEN", self.token)
            .env("GPUI_AGENT_ADDR", &self.addr)
            .env("BIR_DATABASE_PATH", &self.db)
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_HEADLESS_LOG", &self.log)
            .env("BIR_HEADLESS_PID", &self.pid)
            .env("GPUI_AGENT_LOG_REQUESTS", "1")
            .env_remove("GPUI_AGENT_INSECURE_NO_TOKEN");
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args)
            .output()
            .unwrap_or_else(|error| panic!("spawn {} {:?}: {error}", self.bin.display(), args))
    }

    fn wait_ready(&self) {
        let started = Instant::now();
        loop {
            let output = self.run(&["status"]);
            if output.status.success() {
                let body = String::from_utf8_lossy(&output.stdout);
                assert!(
                    body.contains("headless") || body.contains("hello") || body.contains("ok"),
                    "unexpected status stdout: {body}"
                );
                return;
            }
            if started.elapsed() > Duration::from_secs(20) {
                panic!(
                    "daemon never became ready at {}\nstatus stderr: {}\nlog:\n{}",
                    self.addr,
                    String::from_utf8_lossy(&output.stderr),
                    std::fs::read_to_string(&self.log).unwrap_or_default()
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn live_pid(&self) -> Option<u32> {
        let text = std::fs::read_to_string(&self.pid).ok()?;
        let pid: u32 = text.trim().parse().ok()?;
        if pid_alive(pid) { Some(pid) } else { None }
    }

    fn stop_daemon(&self) {
        let _ = self.run(&["shutdown"]);
        if let Some(pid) = self.live_pid() {
            terminate_pid(pid);
            let started = Instant::now();
            while pid_alive(pid) && started.elapsed() < Duration::from_secs(5) {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.stop_daemon();
    }
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

fn terminate_pid(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn wait_log_contains(path: &Path, needle: &str, budget: Duration) -> String {
    let started = Instant::now();
    loop {
        let body = std::fs::read_to_string(path).unwrap_or_default();
        if body.contains(needle) {
            return body;
        }
        if started.elapsed() > budget {
            panic!("log {} never contained {needle:?}:\n{body}", path.display());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn logs_without_file_is_a_clear_error() {
    let harness = Harness::new();
    let output = harness.run(&["logs"]);
    assert!(!output.status.success(), "missing log must fail");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("no bir-headless log"), "{err}");
    assert!(err.contains("serve --detach"), "{err}");
}

#[test]
fn detach_status_logs_follow_shutdown_and_second_detach() {
    let harness = Harness::new();

    let detached = harness.run(&["serve", "--detach"]);
    let stdout = String::from_utf8_lossy(&detached.stdout);
    let stderr = String::from_utf8_lossy(&detached.stderr);
    assert!(
        detached.status.success(),
        "detach failed: status={:?} stdout={stdout} stderr={stderr}",
        detached.status
    );
    assert!(stdout.contains("pid="), "{stdout}");
    assert!(
        stdout.contains(&harness.log.display().to_string()),
        "stdout should print log path\n{stdout}"
    );
    let pid: u32 = stdout
        .lines()
        .find_map(|line| line.strip_prefix("pid=")?.parse().ok())
        .unwrap_or_else(|| panic!("pid line in {stdout}"));
    assert!(
        pid_alive(pid),
        "detached pid {pid} died immediately; log:\n{}",
        std::fs::read_to_string(&harness.log).unwrap_or_default()
    );

    harness.wait_ready();
    wait_log_contains(&harness.log, "gpui-agent listening", Duration::from_secs(5));
    assert_eq!(harness.live_pid(), Some(pid));

    let dumped = harness.run(&["logs"]);
    assert!(
        dumped.status.success(),
        "{}",
        String::from_utf8_lossy(&dumped.stderr)
    );
    let dump_body = String::from_utf8_lossy(&dumped.stdout);
    assert!(
        dump_body.contains("gpui-agent listening"),
        "logs dump missing listen banner:\n{dump_body}"
    );

    let tailed = harness.run(&["logs", "--tail", "2"]);
    assert!(tailed.status.success());

    let mut follow = harness
        .cmd(&["logs", "--follow"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("logs --follow");
    let stdout_pipe = follow.stdout.take().expect("follow stdout");
    let collected = Arc::new(Mutex::new(String::new()));
    let collected_thread = collected.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stdout_pipe);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            collected_thread.lock().expect("lock").push_str(&line);
            collected_thread.lock().expect("lock").push('\n');
        }
    });

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if collected
            .lock()
            .expect("lock")
            .contains("gpui-agent listening")
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        collected
            .lock()
            .expect("lock")
            .contains("gpui-agent listening"),
        "follow never dumped existing listen line: {:?}",
        collected.lock().expect("lock")
    );

    let hello = harness.run(&["status"]);
    assert!(
        hello.status.success(),
        "{}",
        String::from_utf8_lossy(&hello.stderr)
    );

    let started = Instant::now();
    loop {
        let body = collected.lock().expect("lock").clone();
        if body.contains("op=hello") {
            break;
        }
        if started.elapsed() > Duration::from_secs(8) {
            panic!(
                "follow never saw request-log hello line\ncollected:\n{body}\nlog:\n{}",
                std::fs::read_to_string(&harness.log).unwrap_or_default()
            );
        }
        thread::sleep(Duration::from_millis(50));
    }

    let _ = follow.kill();
    let _ = follow.wait();

    let second = harness.run(&["serve", "--detach"]);
    assert!(
        !second.status.success(),
        "second detach must fail while the daemon is running"
    );
    let second_err = String::from_utf8_lossy(&second.stderr);
    assert!(
        second_err.contains("already running") || second_err.contains("in use"),
        "second detach error should mention the live instance:\n{second_err}"
    );

    let stop = harness.run(&["shutdown"]);
    assert!(
        stop.status.success(),
        "shutdown failed: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    let started = Instant::now();
    while pid_alive(pid) && started.elapsed() < Duration::from_secs(8) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!pid_alive(pid), "shutdown left pid {pid} alive");

    let after = harness.run(&["status"]);
    assert!(!after.status.success(), "status must fail after shutdown");
}

#[test]
fn foreground_serve_without_detach_still_refuses_busy_bind() {
    let harness = Harness::new();
    let holder = TcpListener::bind(&harness.addr).expect("hold the test port");
    let output = harness.run(&["serve"]);
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("in use"), "{err}");
    drop(holder);
}
