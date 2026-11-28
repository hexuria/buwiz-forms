//! The Rust side of the NSH app: what TSX imports from `ebirforms/native`.
//!
//! * `Bridge.ping()`: proof that TSX reaches this crate.
//! * `Door`: the host functions of the Rust brain (`crates/bir-app-api`). Screens never import it: they use
//!   the typed wrapper in `nsh/bir`. The plain-Rust half (boot, guards, dispatch) is in `door.rs` and
//!   `guards.rs`; the hooks the host functions call for what needs a UI are in `hooks.rs`.
//!
//! Its TypeScript declarations are generated from these types into `.nsh/types/ebirforms/native.d.ts`.

pub mod agent;
mod auth;
mod desktop;
#[cfg(any(test, target_os = "android"))]
mod dev_command;
#[cfg(any(test, target_os = "android"))]
mod dev_env;
pub mod door;
#[cfg(target_os = "android")]
mod export_android;
#[cfg(any(test, target_os = "android", target_os = "ios"))]
mod export_archive;
#[cfg(target_os = "ios")]
mod export_ios;
pub mod guards;
mod hooks;
// Inert until A3a-2 registers its host verbs (features/accounts-and-lock/OS-LOGIN-INTERFACE.md).
#[allow(dead_code)]
mod os_login;
#[cfg(target_os = "android")]
mod phone_android;
#[cfg(target_os = "ios")]
mod phone_ios;
mod print;
#[cfg(target_os = "android")]
mod print_android;
#[cfg(any(test, target_os = "android"))]
mod print_android_policy;
#[cfg(any(windows, target_os = "linux"))]
mod sheets;

use nsh_core::JsonValue;
use nsh_core::prelude::*;

/// The crate's UI set (`nsh.json` `"rust"`): everything TSX can import from `ebirforms/native`.
#[native_module("ebirforms/native")]
fn native(registry: &mut Registry) {
    // Desktop runners install NSH's own log facade logger only after native
    // modules register (the pin says so in nsh-linux-app/src/lib.rs): take the
    // facade first so kit log records become tracing events and pass through
    // bir-core's redacting file writer. Android's JNI_OnLoad installs its
    // logger before this crate can run, so this cannot replace that one at
    // the pin; those OS-log lines never enter the app's daily file.
    // The kit's describe program prints the sets as JSON on stdout: nothing
    // may run there that can write a line, so registration installs no
    // logging in it at all. This exe-name check is only a second layer — the
    // real fix is that the app's logging never takes a stdout layer at all
    // (logging_options in door.rs), so no host tool's protocol can be
    // disturbed, whatever the binary is called.
    #[cfg(not(target_os = "android"))]
    let describe = door::is_describe_program();
    #[cfg(not(target_os = "android"))]
    if !describe {
        door::install_log_facade_bridge();
    }
    // Install logging here, at registration, not on the first door call: a record made before that call
    // (the launch storage's own lines included) must reach the daily file too. install_logging is a Once,
    // so the first door call's install is a no-op after this.
    #[cfg(not(target_os = "android"))]
    if !describe {
        door::install_logging();
    }
    // The plug's window control plane: `native()` runs before the host opens any surface, so this is
    // where the platform crate's surface hook is installed (see agent.rs, "Agent remote control").
    agent::install_window_plane();
    // Desktop hosts register before NSH starts. Android waits for injected System.data_dir and
    // uses in-process configuration rather than changing the environment (see `door::process`).
    #[cfg(not(target_os = "android"))]
    door::prepare_process();
    // One copy per database: a second launch asks the first to come forward and ends here (`desktop.rs`).
    #[cfg(not(target_os = "android"))]
    desktop::prepare_process();
    // Only after the one-copy guard: a second launch exits above and must not
    // sweep a preview the first copy is writing. In a test environment whose
    // database is not named yet, the door sweeps after it names its scratch
    // database; sweeping here would touch the real installed folder instead.
    #[cfg(not(target_os = "android"))]
    if !door::scratch_database_wanted() {
        bir_core::platform::prepare_print_previews();
    }
    registry
        .module::<Bridge>()
        .module::<Door>()
        .module::<agent::Agent>()
        .module::<desktop::Desktop>()
        .module::<print::Print>();
}

/// Proof that TSX reaches this crate.
#[module]
pub struct Bridge;

// A module: `Bridge.ping()`. Not `async`, no `cx`: it runs inside the TSX call and returns its string.
#[module]
impl Bridge {
    /// Answers `"pong"`, so a screen or a test can tell the native module is linked.
    pub fn ping() -> String {
        "pong".to_owned()
    }
}

/// Whether a host function answers at once or with a promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PropEnum)]
pub enum HostFunctionKind {
    /// Answers at once: call it with `Door.call`.
    Sync,
    /// Answers with a promise: call it with `Door.callAsync`.
    Async,
}

/// One host function of the `bir` module.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct HostFunction {
    /// Its name, as the Rust module registers it (`system_info`).
    pub name: String,
    /// Whether it is `sync` or `async`.
    pub kind: HostFunctionKind,
}

/// How the door stands.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct DoorStatus {
    /// `dry_run` (nothing is sent anywhere), `mock` (the local mock BIR server), `relay` (the local mock relay,
    /// docs/relay.md) or `refused` (the submission worker did not start).
    pub sending: String,
    /// A sentence about `sending` when there is something to say (the mock's address, why it was refused).
    pub sending_note: Option<String>,
    /// The database file this process opens, as the environment names it now.
    pub database_path: String,
    /// `EBIR_TEST_ENV` is on: a dev, test or agent run.
    pub test_environment: bool,
    /// `BIR_SHELL_POLL=1` in a test environment: the frame's polls run there as they do in the shipped app.
    /// Read here because a script under `nsh test` does not see the environment.
    pub shell_poll: bool,
    /// The opt-in Android test mailbox may be polled, even while the app is not focused.
    pub dev_command_enabled: bool,
}

/// The door to the Rust brain: the host functions the screens call, with the guards of the migration plan
/// (a forced dry run, no database inside the owner's folders). Screens use `nsh/bir`, which types each call.
#[module]
pub struct Door;

fn dev_command_enabled() -> bool {
    #[cfg(target_os = "android")]
    {
        crate::dev_command::enabled(
            true,
            crate::dev_env::ENABLED,
            bir_core::config::var("EBIR_TEST_ENV").ok().as_deref(),
        )
    }
    #[cfg(not(target_os = "android"))]
    false
}

/// Before a door call from the engine `system` belongs to: refused once that engine quit (`door::enter`).
fn prepare_door(system: System) -> Result<(), String> {
    door::enter(&system.locals)?;
    #[cfg(target_os = "android")]
    door::prepare_android(system.data_dir)?;
    #[cfg(not(target_os = "android"))]
    door::install_logging();
    Ok(())
}

#[module]
impl Door {
    /// Calls the synchronous host function `name` with `args` and answers its value. A host error is thrown
    /// as an Error with the host's message; an async function, or an unknown name, throws too.
    pub fn call(name: String, args: Vec<JsonValue>, system: System) -> Result<JsonValue, String> {
        prepare_door(system)?;
        door::call(&name, args)
    }

    /// Calls the asynchronous host function `name` with `args`: a promise of its value, rejected with the
    /// host's message on a host error. A sync function, or an unknown name, rejects too.
    pub async fn call_async(
        name: String,
        args: Vec<JsonValue>,
        system: System,
    ) -> Result<JsonValue, String> {
        prepare_door(system)?;
        // On the engine thread: the UI thread's job pipe (the sign-in's browser, the screen kept on during a send).
        #[cfg(target_os = "android")]
        phone_android::install();
        door::call_async(name, args).await
    }

    /// Every host function with its kind, sorted by name.
    pub fn functions() -> Vec<HostFunction> {
        door::functions()
            .into_iter()
            .map(|(name, kind)| HostFunction {
                name,
                kind: match kind {
                    bir_app_api::host::FunctionKind::Sync => HostFunctionKind::Sync,
                    bir_app_api::host::FunctionKind::Async => HostFunctionKind::Async,
                },
            })
            .collect()
    }

    /// An opaque database change token; null when changes cannot be determined.
    pub fn data_change_token(system: System) -> Result<Option<String>, String> {
        prepare_door(system)?;
        door::data_change_token()
    }

    /// How sending stands and which database file is named, without opening it.
    pub fn status(_system: System) -> Result<DoorStatus, String> {
        #[cfg(target_os = "android")]
        door::prepare_android(_system.data_dir)?;
        let mode = door::send_mode();
        Ok(DoorStatus {
            sending: mode.label().to_owned(),
            sending_note: mode.note(),
            database_path: door::database_path().display().to_string(),
            test_environment: door::test_environment(),
            shell_poll: door::shell_poll(),
            dev_command_enabled: dev_command_enabled(),
        })
    }

    /// Consumes one opt-in Android test command from the private data directory.
    pub fn dev_command(_system: System) -> Result<Option<String>, String> {
        #[cfg(target_os = "android")]
        {
            if !crate::dev_env::ENABLED {
                Ok(None)
            } else {
                door::prepare_android(_system.data_dir)?;
                crate::dev_command::consume(&bir_core::platform::data_dir(), dev_command_enabled())
                    .map(|command| command.map(str::to_owned))
            }
        }
        #[cfg(not(target_os = "android"))]
        Ok(None)
    }

    /// Names the database file this thread opens, before anything else is called (test environments only:
    /// `EBIR_TEST_ENV=1`). Answers the path. The database guard applies.
    pub fn use_database(path: String, system: System) -> Result<String, String> {
        door::enter(&system.locals)?;
        #[cfg(target_os = "android")]
        door::prepare_android(system.data_dir)?;
        door::use_database(&path)
    }

    /// The quit path: stops the submission worker and closes the database. Call it once, when the app quits; every
    /// door call from this engine after it is refused. A later engine boots again, on any thread.
    pub fn shutdown(system: System) {
        door::shutdown(&system.locals);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::Bridge;

    #[test]
    fn ping_answers_pong() {
        assert_eq!(Bridge::ping(), "pong");
    }

    /// The kit's describe program prints the crate's UI sets as one JSON
    /// document on stdout, and the tool parses exactly that. This test runs
    /// the same steps the generated describe main runs — the crate's
    /// registration, then one JSON document on stdout — in-process, with
    /// file descriptor 1 captured into a pipe for the duration. Anything
    /// registration writes to stdout (a log line at any level, a bare
    /// println) lands in the capture and fails the parse exactly as it
    /// fails the kit's. Running in-process means the test always exercises
    /// the code of the build it belongs to: no binary left from an older
    /// build can give a false green or red, and no nested cargo build can
    /// block on the outer run's lock.
    ///
    /// Warn-level probes through both facades follow the document: the
    /// registration installs the process's logging, so if any stdout layer
    /// exists at all, the probes reach the capture whether or not this
    /// test's own registration call was the one to install it.
    #[cfg(unix)]
    #[test]
    fn the_describe_programs_stdout_parses_as_json() {
        // The capture is process-wide, and this test binary runs other tests
        // whose re-executed children share fd 1 — so the capture runs in a
        // dedicated child process of this same binary: no concurrent test
        // can write into it, the binary is always the build under test, and
        // no nested cargo build can block on the outer run's lock.
        const CHILD: &str = "NSH_DESCRIBE_STDOUT_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let captured = capture_stdout_fd(|| {
                let sets = vec![crate::__nsh_native_module()];
                let json = nsh_core::element::description::Description::of(&sets).to_json();
                write_fd1(&format!("{json}\n"));
                tracing::warn!("describe probe");
                log::warn!("describe probe");
            });
            if let Err(error) = serde_json::from_str::<serde_json::Value>(&captured) {
                write_fd2(&format!(
                    "the describe program's stdout is not JSON ({error}): {}\n",
                    &captured[..captured.len().min(300)]
                ));
                std::process::exit(1);
            }
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::the_describe_programs_stdout_parses_as_json",
            ])
            .env(CHILD, "1")
            .env("BUWIZ_NO_KEYCHAIN", "1")
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_AUTH_BYPASS", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .env_remove("BIR_SFTP_LIVE")
            .output()
            .expect("the describe child process");
        assert!(
            output.status.success(),
            "the describe child's stdout did not parse as JSON: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The capture cannot stall: more than any system's pipe buffer passes
    /// through it whole, because the draining thread reads while the write
    /// side is still writing. In a dedicated child process, as the describe
    /// capture: the capture is process-wide.
    #[cfg(unix)]
    #[test]
    fn the_stdout_capture_passes_more_than_a_pipe_buffer() {
        const CHILD: &str = "NSH_PIPE_CAPACITY_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let line = "x".repeat(1024);
            let captured = capture_stdout_fd(|| {
                for _ in 0..256 {
                    write_fd1(&format!("{line}\n"));
                }
            });
            std::process::exit(if captured.len() == 256 * 1025 { 0 } else { 1 });
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::the_stdout_capture_passes_more_than_a_pipe_buffer",
            ])
            .env(CHILD, "1")
            .status()
            .expect("the pipe-capacity child process");
        assert!(status.success(), "the capture stalled or lost bytes");
    }

    /// Writes `text` to file descriptor 1 itself, bypassing the test
    /// harness's output capture: the describe program's stdout is a real
    /// file descriptor write, and the capture below measures exactly that.
    #[cfg(unix)]
    fn write_fd1(text: &str) {
        use std::io::Write as _;
        use std::os::unix::io::FromRawFd as _;
        let mut out = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(1) });
        out.write_all(text.as_bytes()).expect("write fd 1");
        out.flush().expect("flush fd 1");
    }

    /// The same direct write for file descriptor 2: the child reports its
    /// red line where the test harness's output capture cannot swallow it.
    #[cfg(unix)]
    fn write_fd2(text: &str) {
        use std::io::Write as _;
        use std::os::unix::io::FromRawFd as _;
        let mut out = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(2) });
        out.write_all(text.as_bytes()).expect("write fd 2");
        out.flush().expect("flush fd 2");
    }

    /// Runs `run` with the process's file descriptor 1 redirected into a
    /// pipe and returns everything written to it. Logging layers write to
    /// stdout as a file descriptor, so the capture is at the same level.
    /// The pipe drains on a thread while `run` writes: a pipe holds only
    /// 16–64 KB, and reading only after `run` returned would block the
    /// write side forever once the buffer fills.
    #[cfg(unix)]
    fn capture_stdout_fd(run: impl FnOnce()) -> String {
        use std::io::Write as _;
        unsafe {
            let mut ends = [0i32; 2];
            assert_eq!(libc::pipe(ends.as_mut_ptr()), 0, "pipe");
            let (reader, writer) = (ends[0], ends[1]);
            let draining = std::thread::spawn(move || {
                let mut captured = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = libc::read(reader, buf.as_mut_ptr().cast(), buf.len());
                    if n <= 0 {
                        break;
                    }
                    captured.extend_from_slice(&buf[..n as usize]);
                }
                libc::close(reader);
                captured
            });
            let saved = libc::dup(1);
            assert!(saved >= 0, "dup stdout");
            let _ = std::io::stdout().flush();
            assert_eq!(libc::dup2(writer, 1), 1, "redirect stdout");
            libc::close(writer);
            run();
            let _ = std::io::stdout().flush();
            assert_eq!(libc::dup2(saved, 1), 1, "restore stdout");
            libc::close(saved);
            let captured = draining.join().expect("the draining thread");
            String::from_utf8_lossy(&captured).into_owned()
        }
    }

    #[cfg(target_os = "android")]
    #[test]
    fn android_status_uses_injected_directory_without_a_reactive_runtime() {
        use super::{Door, System};

        assert!(
            Door::status(System::default())
                .unwrap_err()
                .contains("no private data directory")
        );
        let mut system = System::default();
        system.data_dir = Some("relative".into());
        assert!(
            Door::status(system.clone())
                .unwrap_err()
                .contains("must be absolute")
        );

        let folder = tempfile::tempdir().expect("private data folder");
        system.data_dir = Some(folder.path().to_path_buf());
        temp_env::with_vars(
            [("BIR_RELAY_URL", None), ("BIR_SFTP_DRY_RUN", Some("1"))],
            || {
                let first = Door::status(system.clone()).expect("status needs no reactive runtime");
                let second =
                    Door::status(system).expect("repeated status needs no reactive runtime");
                assert_eq!(first.sending, "dry_run");
                assert_eq!(second.database_path, first.database_path);
                assert_eq!(
                    crate::guards::account_home(),
                    Some(folder.path().canonicalize().unwrap())
                );
            },
        );
    }

    /// The NSH app is plain SQLite: no database key, no keychain, its own data folder. Nothing else pins
    /// that: a `bir-core` dependency that gained the `sqlcipher` feature (here or through another crate of
    /// the graph) would compile, run, and write encrypted files the app cannot read back. This fails then:
    /// a new database starts with the plain SQLite header, and the data folder is the NSH app's own.
    #[test]
    fn bir_core_is_built_as_plain_sqlite_with_the_nsh_data_folder() {
        let folder = tempfile::tempdir().expect("temp folder");
        let path = folder.path().join("probe.db");
        temp_env::with_vars(
            [
                ("EBIR_TEST_ENV", Some("1")),
                ("BUWIZ_NO_KEYCHAIN", Some("1")),
            ],
            || {
                let db = bir_core::db::Database::open(&path).expect("a plain database opens");
                db.set_setting("probe", "plain").expect("a write");
                db.close().expect("closes");
            },
        );
        let mut head = [0u8; 16];
        std::fs::File::open(&path)
            .expect("the database file")
            .read_exact(&mut head)
            .expect("its first bytes");
        assert_eq!(
            &head,
            bir_core::db::SQLITE_PLAIN_HEADER,
            "a new database is encrypted: bir-core was built with the sqlcipher feature"
        );

        let home = tempfile::tempdir().expect("temp home");
        temp_env::with_vars(
            [
                ("HOME", Some(home.path().as_os_str())),
                ("XDG_DATA_HOME", None),
                // Windows reads the data folder from LOCALAPPDATA: point it at the temp home too, so bir-core's
                // test-build rule keeps this temp folder instead of its own scratch one.
                ("LOCALAPPDATA", Some(home.path().as_os_str())),
                ("BIR_DATABASE_PATH", None),
            ],
            || {
                let data = bir_core::platform::data_dir();
                assert!(
                    data.ends_with("dev.goldcoders.bir"),
                    "the data folder is the old app's group container: bir-core was built with sqlcipher ({})",
                    data.display()
                );
                #[cfg(unix)]
                assert!(
                    data.starts_with(home.path()),
                    "the data folder left the temp home ({})",
                    data.display()
                );
            },
        );
    }
}
