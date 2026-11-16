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
pub mod door;
pub mod guards;
mod hooks;
mod print;

use nsh_core::JsonValue;
use nsh_core::prelude::*;

/// The crate's UI set (`nsh.json` `"rust"`): everything TSX can import from `ebirforms/native`.
#[native_module("ebirforms/native")]
fn native(registry: &mut Registry) {
    // The earliest single point: the generated hosts build the registry before NSH starts, so the door's
    // one change to the process environment happens before any other thread exists (see `door::process`).
    door::prepare_process();
    // One copy per database: a second launch asks the first to come forward and ends here (`desktop.rs`).
    desktop::prepare_process();
    registry.module::<Bridge>().module::<Door>().module::<agent::Agent>().module::<desktop::Desktop>().module::<print::Print>();
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
    /// `dry_run` (nothing is sent anywhere), `mock` (the local mock BIR server) or `refused` (the submission
    /// worker did not start).
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
}

/// The door to the Rust brain: the host functions the screens call, with the guards of the migration plan
/// (a forced dry run, no database inside the owner's folders). Screens use `nsh/bir`, which types each call.
#[module]
pub struct Door;

#[module]
impl Door {
    /// Calls the synchronous host function `name` with `args` and answers its value. A host error is thrown
    /// as an Error with the host's message; an async function, or an unknown name, throws too.
    pub fn call(name: String, args: Vec<JsonValue>) -> Result<JsonValue, String> {
        door::call(&name, args)
    }

    /// Calls the asynchronous host function `name` with `args`: a promise of its value, rejected with the
    /// host's message on a host error. A sync function, or an unknown name, rejects too.
    pub async fn call_async(name: String, args: Vec<JsonValue>) -> Result<JsonValue, String> {
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

    /// How sending stands and which database file is named, without opening it.
    pub fn status() -> DoorStatus {
        let mode = door::send_mode();
        DoorStatus {
            sending: mode.label().to_owned(),
            sending_note: mode.note(),
            database_path: door::database_path().display().to_string(),
            test_environment: door::test_environment(),
            shell_poll: door::shell_poll(),
        }
    }

    /// Names the database file this thread opens, before anything else is called (test environments only:
    /// `EBIR_TEST_ENV=1`). Answers the path. The database guard applies.
    pub fn use_database(path: String) -> Result<String, String> {
        door::use_database(&path)
    }

    /// The quit path: stops the submission worker and checkpoints the database. Call it once, when the app quits;
    /// the door is not for use afterwards.
    pub fn shutdown() {
        door::shutdown();
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
                ("BIR_DATABASE_PATH", None),
            ],
            || {
                let data = bir_core::platform::data_dir();
                assert!(
                    data.ends_with("dev.goldcoders.bir.nsh"),
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
