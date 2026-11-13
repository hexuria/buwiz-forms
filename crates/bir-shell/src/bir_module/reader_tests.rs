//! The reads the page polls must not wait on the submission worker.
//!
//! Every host call runs on the GPUI main thread. While the worker holds the
//! database (its mutex, or a write transaction under it) a read that needs
//! the same lock freezes the window. These tests stand in for the worker
//! with a thread that holds the writer and time the reads the page makes
//! every 30 s and on each render.
//!
//! A second connection needs a database file, so each test re-runs itself
//! as a child test process on a scratch `BIR_DATABASE_PATH`, with
//! `EBIR_TEST_ENV` keeping the key off the keychain: never the live database.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bir_core::db::{Database, Job};
use gpui_shell::{HostArguments, HostModule};
use serde_json::{Value, json};

use super::{database_handle, db_error, import_archive_from, module, to_json, with_database};

/// What a polled read may take while the writer is held: well under
/// [`HOLD`], so a read that waited for the lock cannot pass.
const BOUND: Duration = Duration::from_millis(250);
/// How long the worker stand-in holds the writer unless released sooner.
const HOLD: Duration = Duration::from_secs(2);

const HEAD: &str = "00012345300000";
const YEAR: i64 = 2026;
const RESTORED_HEAD: &str = "98765432000000";

/// Run `body` in a child test process on a scratch database (see the module
/// doc). In the child, refuse to run anywhere but the scratch file.
fn in_scratch_child(test: &str, body: fn(&Path)) {
    const SCRATCH: &str = "BIR_SHELL_READER_SCRATCH";
    if let Some(scratch) = std::env::var_os(SCRATCH) {
        let scratch = PathBuf::from(scratch);
        assert_eq!(
            bir_core::db::app_database_path(),
            scratch.join("bir_data.db"),
            "the scratch database"
        );
        assert!(
            std::env::var_os("EBIR_TEST_ENV").is_some(),
            "the key stays off the keychain"
        );
        body(&scratch);
        std::fs::write(scratch.join("child-done"), "ok").unwrap();
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("bir_module::reader_tests::{test}"),
            "--nocapture",
        ])
        .current_dir(scratch.path())
        .env(SCRATCH, scratch.path())
        .env("BIR_DATABASE_PATH", scratch.path().join("bir_data.db"))
        .env("EBIR_TEST_ENV", "1")
        .env("BIR_AUTH_BYPASS", "1")
        .env("BIR_SFTP_DRY_RUN", "1")
        .status()
        .unwrap();
    assert!(status.success(), "{test}: the child test failed");
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("child-done"))
            .ok()
            .as_deref(),
        Some("ok"),
        "{test}: the child ran"
    );
}

/// A thread standing in for the submission worker: it holds the writer's
/// mutex, and with `write_open` an uncommitted `BEGIN IMMEDIATE` write
/// under it, until [`Holder::release`] or [`HOLD`] passes.
struct Holder {
    release: mpsc::Sender<()>,
    thread: std::thread::JoinHandle<()>,
}

fn hold_writer(write_open: bool) -> Holder {
    let handle = database_handle().unwrap();
    let (held_tx, held_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let guard = handle.lock().unwrap();
        if write_open {
            guard
                .conn_for_tests()
                .execute_batch(
                    "BEGIN IMMEDIATE;
                     INSERT INTO settings (key, value) VALUES ('reader_tests.uncommitted', '1');",
                )
                .unwrap();
        }
        held_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(HOLD);
        if write_open {
            guard.conn_for_tests().execute_batch("ROLLBACK").unwrap();
        }
    });
    held_rx.recv().unwrap();
    Holder { release, thread }
}

impl Holder {
    fn release(self) {
        let _ = self.release.send(());
        self.thread.join().unwrap();
    }
}

fn call(bir: &HostModule, name: &str, values: &[Value]) -> Result<Value, String> {
    let arguments = HostArguments::new(values.iter().cloned().map(super::json));
    bir.call(name, &arguments)
        .map(|value| to_json(&value))
        .map_err(|error| error.message().to_string())
}

fn ok(bir: &HostModule, name: &str, values: &[Value]) -> Value {
    call(bir, name, values).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// `name` called while the worker stand-in holds the writer, and how long
/// the call took.
fn while_held(
    bir: &HostModule,
    write_open: bool,
    name: &str,
    values: &[Value],
) -> (Value, Duration) {
    let holder = hold_writer(write_open);
    let start = Instant::now();
    let result = call(bir, name, values);
    let took = start.elapsed();
    holder.release();
    eprintln!("{name} (write open: {write_open}) took {took:?} while the writer was held");
    (
        result.unwrap_or_else(|error| panic!("{name}: {error}")),
        took,
    )
}

fn create_juana(bir: &HostModule) {
    ok(
        bir,
        "create_taxpayer",
        &[json!({
            "tin_root": "000-123-453",
            "name": "Juana Santos",
            "taxpayer_type": "Individual",
            "head_office": {
                "rdo_code": "040",
                "registered_address": "1 Example Street, Pasig",
                "zip_code": "1600",
                "phone": "0280000000",
                "email": "juana@example.com"
            }
        })],
    );
}

/// A saved Draft 2551Q for the first quarter, written on this thread.
fn save_2551q() -> i64 {
    with_database(|db| {
        let profile = db.get_profile(HEAD).map_err(db_error)?.unwrap();
        let mut draft = bir_core::forms::Form2551QDraft::new_from_profile(&profile, 2026, 1);
        draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
        db.save_2551q_draft(&draft).map_err(db_error)
    })
    .unwrap()
}

/// The reads the page makes on every render and every 30 s poll.
fn polled_reads() -> Vec<(&'static str, Vec<Value>)> {
    vec![
        ("list_returns", vec![json!("all")]),
        (
            "deadlines_between",
            vec![json!("2026-01-01"), json!("2026-12-31")],
        ),
        ("as_needed_forms", vec![json!(YEAR)]),
        ("list_form_templates", vec![json!(HEAD)]),
        ("list_background_jobs", vec![]),
        ("list_taxpayers", vec![]),
    ]
}

/// T2: each polled read answers in bounded time, with the rows it answers
/// uncontended, while the worker holds the writer's mutex and while it
/// holds an open write transaction.
#[test]
fn polled_reads_answer_while_the_worker_holds_the_writer() {
    in_scratch_child(
        "polled_reads_answer_while_the_worker_holds_the_writer",
        |_| {
            let bir = module();
            create_juana(&bir);
            ok(
                &bir,
                "set_yearly_forms",
                &[json!(HEAD), json!(YEAR), json!(["2551Q", "1600WP"])],
            );
            save_2551q();
            ok(
                &bir,
                "create_template_from_latest",
                &[json!(HEAD), json!("2551Q")],
            );
            with_database(|db| {
                db.save_job(Job {
                    id: None,
                    name: "Reader test job".into(),
                    job_type: "Custom".into(),
                    cron_expr: None,
                    command: None,
                    status: "Completed".into(),
                    retries: 0,
                    last_run_at: None,
                    next_run_at: None,
                    created_at: String::new(),
                    output_log: None,
                })
                .map_err(db_error)
            })
            .unwrap();

            let mut slow = Vec::new();
            for write_open in [false, true] {
                for (name, values) in polled_reads() {
                    let uncontended = ok(&bir, name, &values);
                    assert_ne!(uncontended, json!([]), "{name}: the fixture gives it rows");
                    let (held, took) = while_held(&bir, write_open, name, &values);
                    assert_eq!(held, uncontended, "{name}: the same rows");
                    if took >= BOUND {
                        slow.push(format!("{name} (write open: {write_open}): {took:?}"));
                    }
                }
            }
            assert!(slow.is_empty(), "reads that waited on the writer: {slow:?}");
        },
    );
}

/// T3: a read that follows a write on the main thread sees the write, even
/// while the worker holds the writer. After a database import replaces the
/// file, reads see the imported database, not the file it replaced.
#[test]
fn reads_see_the_main_threads_writes() {
    in_scratch_child("reads_see_the_main_threads_writes", |scratch| {
        let bir = module();
        let mut slow = Vec::new();
        let mut check = |name: &str, took: Duration| {
            if took >= BOUND {
                slow.push(format!("{name}: {took:?}"));
            }
        };
        create_juana(&bir);
        let (taxpayers, took) = while_held(&bir, true, "list_taxpayers", &[]);
        assert_eq!(taxpayers[0]["name"], "Juana Santos", "the new taxpayer");
        check("list_taxpayers", took);

        ok(
            &bir,
            "set_yearly_forms",
            &[json!(HEAD), json!(YEAR), json!(["2551Q", "1600WP"])],
        );
        let (forms, took) = while_held(&bir, false, "as_needed_forms", &[json!(YEAR)]);
        assert_eq!(forms[0]["form_code"], "1600WP", "the new yearly form");
        check("as_needed_forms", took);

        let id = save_2551q();
        let (saved, took) = while_held(&bir, true, "list_returns", &[json!("saved")]);
        assert!(
            saved
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == json!(id)),
            "the saved return is listed: {saved}"
        );
        check("list_returns", took);

        ok(
            &bir,
            "create_template_from_latest",
            &[json!(HEAD), json!("2551Q")],
        );
        let (templates, took) = while_held(&bir, false, "list_form_templates", &[json!(HEAD)]);
        assert_eq!(templates[0]["form_code"], "2551Q", "the new template");
        check("list_form_templates", took);

        let source = Database::open(scratch.join("source.db")).unwrap();
        source
            .create_taxpayer(bir_core::tax_profile::NewTaxpayer {
                tin_root: "987654320".into(),
                name: "Restored Holdings Inc".into(),
                taxpayer_type: bir_core::profile::TaxpayerType::Corporation,
                birth_date: None,
                head_office: bir_core::tax_profile::BranchFacts {
                    rdo_code: "043".into(),
                    registered_address: "2 Sample Road, Makati".into(),
                    zip_code: "1200".into(),
                    phone: "0280000001".into(),
                    email: "books@example.com".into(),
                },
            })
            .unwrap();
        source
            .set_yearly_forms(RESTORED_HEAD, 2026, &["1600WP"])
            .unwrap();
        source.checkpoint().unwrap();
        let archive = scratch.join("backup.zip");
        bir_core::export::export_database_zip(&source, &archive).unwrap();
        drop(source);
        assert_eq!(
            to_json(&import_archive_from(&archive).unwrap()),
            json!({ "kind": "database" })
        );
        let (forms, took) = while_held(&bir, false, "as_needed_forms", &[json!(YEAR)]);
        let tins: Vec<&Value> = forms
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["tin"])
            .collect();
        assert_eq!(tins, [&json!(RESTORED_HEAD)], "the imported database");
        check("as_needed_forms after import", took);
        let (taxpayers, took) = while_held(&bir, false, "list_taxpayers", &[]);
        let names: Vec<&Value> = taxpayers
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["name"])
            .collect();
        assert_eq!(
            names,
            [&json!("Restored Holdings Inc")],
            "the imported taxpayers"
        );
        check("list_taxpayers after import", took);

        assert!(slow.is_empty(), "reads that waited on the writer: {slow:?}");
    });
}
