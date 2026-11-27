//! PLAN "A2 rehearsal on copies of real-shaped installs": the harness.
//!
//! Each shape is a whole device folder made with the app's own code (this build's `Database`, whose schema is v0.2.0's,
//! 24), in a temporary folder of its own; never the owner's data. For each:
//! 1. **Before.** The legacy file, copied with its `-wal` and `-shm`, is opened once by this build (the previous
//!    release's open, whose migrations are this build's at schema 24) and digested per table by
//!    `scripts/a2_table_digests.py`, which belongs to neither build.
//! 2. **The claim** runs on the install, with the shape's unlock.
//! 3. **After.** The room file's per-table digests must equal the "before" ones, with the step-6 keys
//!    (`buwiz_server_url`, `hand_to_clients`) listed apart. Any other difference fails the run.
//! 4. **Undo.** The safety zip is restored into a third copy by this build's own `import::extract_database_zip`; its
//!    raw SHA-256 must equal the recorded `raw_sha256`. With `A2_REHEARSAL_ZIPS` set, each zip is kept there for
//!    `scripts/a2_old_release_restore.sh`, which restores them with a previous release's own code (a test built from
//!    its tag).
//!
//! Run it with `cargo test -p bir-core --lib a2_rehearsal -- --ignored --nocapture`; `A2_REHEARSAL_OUT` names a file
//! to write the proof table to. What it cannot do here (the v0.2.0 binary's own open and UI, the mock relay sends, the
//! VM power cut, the phones) is listed in the proof's last section.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::fixtures::*;
use super::*;

/// The settings keys step 6 changes: listed apart, not compared.
const STEP_6_KEYS: [&str; 2] = [OBSOLETE_SERVER_URL_SETTING, HAND_TO_CLIENTS_SETTING];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The digest script's answer for the file at `path`.
fn digest(path: &Path) -> serde_json::Value {
    let script = repo_root().join("scripts").join("a2_table_digests.py");
    let output = std::process::Command::new("python3")
        .arg("-I")
        .arg(&script)
        .arg(path)
        .arg("--skip-settings")
        .arg(STEP_6_KEYS.join(","))
        .output()
        .expect("python3 runs the digest script");
    assert!(
        output.status.success(),
        "digest script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// "Before": a copy of the legacy file (with its sidecars), opened once by this build, then digested.
fn before_digest(install: &Install) -> serde_json::Value {
    let folder = tempfile::tempdir().unwrap();
    let copy = folder.path().join("bir_data.db");
    crash_copy(&install.legacy(), &copy);
    close(Database::open(&copy).unwrap());
    digest(&copy)
}

struct ShapeResult {
    number: &'static str,
    name: &'static str,
    rows: Vec<(String, String, String, bool)>,
    extras: Vec<String>,
    pass: bool,
}

fn short(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(|text| text[..12.min(text.len())].to_owned())
        .unwrap_or_else(|| "-".into())
}

/// Compare "before" and "after" table by table.
fn compare(
    before: &serde_json::Value,
    after: &serde_json::Value,
) -> (Vec<(String, String, String, bool)>, bool) {
    let tables = |value: &serde_json::Value| -> BTreeMap<String, serde_json::Value> {
        value["tables"]
            .as_object()
            .unwrap()
            .clone()
            .into_iter()
            .collect()
    };
    let (before_tables, after_tables) = (tables(before), tables(after));
    let mut names: Vec<&String> = before_tables.keys().chain(after_tables.keys()).collect();
    names.sort();
    names.dedup();
    let mut rows = Vec::new();
    let mut pass = true;
    for name in names {
        let b = before_tables
            .get(name)
            .map(|t| &t["sha256"])
            .cloned()
            .unwrap_or_default();
        let a = after_tables
            .get(name)
            .map(|t| &t["sha256"])
            .cloned()
            .unwrap_or_default();
        let equal = b == a && !b.is_null();
        pass &= equal;
        rows.push((name.clone(), short(&b), short(&a), equal));
    }
    (rows, pass)
}

/// The claim's room, run to its end with `unlock` (None: no lock).
fn run_claim(
    install: &Install,
    who: &Claimant,
    unlock: Option<LegacyCredential<'static>>,
) -> Result<RoomId, String> {
    let claim = install.claim();
    let outcome = match (claim.start(who), unlock) {
        (Start::Ready(ready), None) => ready.claim(&claim),
        (Start::Locked(locked), Some(credential)) => match locked.unlock(&claim, credential) {
            Ok(ready) => ready.claim(&claim),
            Err(_) => return Err("the credential was refused".into()),
        },
        (other, _) => return Err(format!("start: {other:?}")),
    };
    match outcome {
        Outcome::Done { room, .. } => Ok(room),
        other => Err(format!("{other:?}")),
    }
}

/// The undo: the zip restored by this build's own Import code into a third copy; its raw hash is the recorded one.
fn undo(install: &Install, shape: &str) -> Result<String, String> {
    let claim = install.claim();
    let record = claim.record().unwrap().ok_or("no record")?;
    let zip = install
        .dir()
        .join("backups")
        .join(record.zip.as_ref().ok_or("no zip")?);
    keep_for_the_old_releases(
        shape,
        &zip,
        record.raw_sha256.as_deref().unwrap_or_default(),
    );
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("bir_data.db");
    std::fs::write(&target, b"").unwrap();
    crate::import::extract_database_zip(&zip, &target).map_err(|error| error.to_string())?;
    let restored = files::sha256_file(&target).unwrap();
    if Some(&restored) == record.raw_sha256.as_ref() {
        Ok(format!(
            "restored by this build's Import: raw SHA-256 {} equal",
            &restored[..12]
        ))
    } else {
        Err(format!(
            "restored {restored} != raw {:?}",
            record.raw_sha256
        ))
    }
}

/// With `A2_REHEARSAL_ZIPS` set, each shape's safety zip and its raw SHA-256 are kept there, for the restore by the
/// previous release's own code (`scripts/a2_old_release_restore.sh`).
fn keep_for_the_old_releases(shape: &str, zip: &Path, raw_sha256: &str) {
    let Ok(dir) = std::env::var("A2_REHEARSAL_ZIPS") else {
        return;
    };
    if dir.is_empty() {
        return;
    }
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let name = format!("shape-{shape:0>3}");
    std::fs::copy(zip, dir.join(format!("{name}.zip"))).unwrap();
    std::fs::write(dir.join(format!("{name}.sha256")), raw_sha256).unwrap();
}

fn extras(label: &str, value: &serde_json::Value) -> String {
    let rows: i64 = value["tables"]
        .as_object()
        .unwrap()
        .values()
        .map(|table| table["rows"].as_i64().unwrap_or(0))
        .sum();
    format!(
        "{label}: integrity_check {}, user_version {}, rows {rows}, raw SHA-256 {}",
        value["integrity_check"],
        value["user_version"],
        short(&value["raw_sha256"])
    )
}

/// One shape: build, digest, claim, digest, undo.
fn rehearse(
    number: &'static str,
    name: &'static str,
    build: impl FnOnce(&Install),
    run: impl FnOnce(&Install) -> Result<RoomId, String>,
) -> ShapeResult {
    let install = Install::new();
    build(&install);
    let before = before_digest(&install);
    let mut result = ShapeResult {
        number,
        name,
        rows: Vec::new(),
        extras: vec![extras("before", &before)],
        pass: true,
    };
    match run(&install) {
        Ok(room) => {
            let after = digest(&install.room_file(&room));
            let (rows, pass) = compare(&before, &after);
            result.rows = rows;
            result.pass = pass;
            result.extras.push(extras("after", &after));
            result.extras.push(format!(
                "step-6 keys, before: {}, after: {}",
                before["skipped_settings"], after["skipped_settings"]
            ));
            match undo(&install, number) {
                Ok(line) => result.extras.push(line),
                Err(error) => {
                    result.pass = false;
                    result.extras.push(format!("UNDO FAILED: {error}"));
                }
            }
        }
        Err(error) => {
            result.pass = false;
            result.extras.push(format!("CLAIM FAILED: {error}"));
        }
    }
    result
}

/// Shape 1's data: a solo owner with a PIN, three more taxpayers, drafts of four forms, a filed return with its
/// receipt, an imported eBIRForms filed return (a BLOB), a tax year with facts, deadline overrides, the dark theme.
fn solo_full(install: &Install) {
    let db = solo(install, Some("2468"), None);
    taxpayer(&db, "423456788", "Third Taxpayer");
    draft(&db, "12345678800000", "0619F", 2026, Some(1));
    draft(&db, "12345678800000", "1701Q", 2026, Some(1));
    draft(&db, "12345678800000", "1702RT", 2025, None);
    db.save_submission(crate::db::Submission {
        id: None,
        tin: "12345678800000".into(),
        form_type: "1601C".into(),
        period: "2026-01".into(),
        status: "Submitted".into(),
        form_data: [("item".to_owned(), "1".to_owned())].into_iter().collect(),
        submitted_at: Some("2026-02-10T08:00:00+08:00".into()),
        filename: Some("123456788000-1601C-012026.xml".into()),
        created_at: None,
        updated_at: None,
    })
    .unwrap();
    db.save_submission_receipt(&crate::receipt::BirReceiptConfirmation {
        filename: "123456788000-1601C-012026.xml".into(),
        date_received: chrono::NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
        time_received: chrono::NaiveTime::from_hms_opt(8, 1, 2).unwrap(),
        source_from: Some("eFPS".into()),
        raw_text: "Your return was received.".into(),
        raw_html: Some("<p>Your return was received.</p>".into()),
    })
    .unwrap();
    db.insert_legacy_filed_return(&crate::db::LegacyFiledReturn {
        id: None,
        sha256: "0".repeat(64),
        tin: "12345678800000".into(),
        form_identity: "1601C".into(),
        form_id: None,
        period: "2025-12".into(),
        amendment_version: None,
        email_copy: None,
        encrypted: false,
        source_filename: "123456788000-1601C-122025.xml".into(),
        raw_file: (0u8..=255).cycle().take(40_000).collect(),
        fields_json: "{\"item\":\"x\"}".into(),
        imported_at: None,
    })
    .unwrap();
    db.add_tax_year("12345678800000", 2025, false).unwrap();
    db.set_year_facts(
        "12345678800000",
        2025,
        &crate::tax_profile::YearFactsPatch {
            line_of_business: Some(Some("Retail".into())),
            ..Default::default()
        },
    )
    .unwrap();
    db.set_setting(
        "deadline_overrides",
        "[{\"id\":\"x\",\"title\":\"Moved\",\"source_reference\":\"RMC\",\"affected_form_codes\":[\"1601C\"],\"original_deadline\":\"2026-02-10\",\"adjusted_deadline\":\"2026-02-12\",\"affected_regions\":[],\"affected_taxpayer_types\":[],\"effective_from\":null,\"effective_until\":null,\"expires_at\":null}]",
    )
    .unwrap();
    close(db);
}

/// Shape 2: the help desk with six clients, five with PINs, one with an authenticator only.
fn helpdesk_full(install: &Install) {
    let db = helpdesk(install, "1357", Some("ABCD-EFGH-JKMN-PQRS"));
    for (index, root) in ["723456788", "823456788", "923456788"].iter().enumerate() {
        taxpayer(&db, root, &format!("Client {}", index + 3));
        if index < 2 {
            db.set_taxpayer_pin_hash(
                root,
                Some(&crate::crypto::hash_pin(&format!("{}", 6000 + index))),
            )
            .unwrap();
        } else {
            db.set_taxpayer_totp_secret(root, Some(TOTP_SECRET))
                .unwrap();
        }
        draft(&db, &format!("{root}00000"), "1701Q", 2026, Some(1));
    }
    close(db);
}

/// Shape 4's data: three Queued returns and one Submitted awaiting its BIR email (the queue is
/// `form_drafts.status`; the statuses are set raw, as the reviewed queue paths would leave them).
fn queued(install: &Install) {
    let db = solo(install, None, None);
    for (form, quarter) in [("0619F", 2), ("1701Q", 1), ("0619E", 2)] {
        draft(&db, "12345678800000", form, 2026, Some(quarter));
    }
    draft(&db, "22345678800000", "0619E", 2026, Some(3));
    db.conn
        .execute_batch(
            "UPDATE form_drafts SET status = 'Queued'
             WHERE tin = '12345678800000' AND taxable_year = 2026 AND form_code IN ('0619F', '1701Q')
                OR (tin = '12345678800000' AND form_code = '0619E' AND quarter = 2);
             UPDATE form_drafts SET status = 'Submitted' WHERE tin = '22345678800000' AND quarter = 3;",
        )
        .unwrap();
    close(db);
}

fn zips_in(install: &Install) -> usize {
    install
        .names("backups")
        .iter()
        .filter(|name| name.ends_with(".zip"))
        .count()
}

/// The proof (PLAN "The proof"): a table per install, the extras, the undo, then what remains to run elsewhere.
fn report(results: &[ShapeResult], extra_lines: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# A2 rehearsal: per-table content digests (M5)\n");
    let _ = writeln!(
        out,
        "Made by `cargo test -p bir-core --lib a2_rehearsal -- --ignored` at {}. Digests by `scripts/a2_table_digests.py` \
         ({}, its own sqlite3). Fixtures made by this build's own code (schema 24, v0.2.0's), in temporary folders.\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        String::from_utf8_lossy(
            &std::process::Command::new("python3")
                .arg("--version")
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default()
        )
        .trim()
    );
    for result in results {
        let _ = writeln!(
            out,
            "## Shape {}: {} ({})\n",
            result.number,
            result.name,
            if result.pass { "PASS" } else { "FAIL" }
        );
        if !result.rows.is_empty() {
            let _ = writeln!(out, "| table | before | after | equal |\n|---|---|---|---|");
            for (table, before, after, equal) in &result.rows {
                let _ = writeln!(
                    out,
                    "| {table} | {before} | {after} | {} |",
                    if *equal { "yes" } else { "NO" }
                );
            }
            let _ = writeln!(out);
        }
        for line in &result.extras {
            let _ = writeln!(out, "- {line}");
        }
        let _ = writeln!(out);
    }
    for line in extra_lines {
        let _ = writeln!(out, "{line}");
    }
    out
}

#[test]
#[ignore = "the rehearsal: needs python3; run with --ignored"]
fn a2_rehearsal() {
    let mut results = Vec::new();

    results.push(rehearse("1", "solo (owner PIN, four forms, filed + receipt, eBIRForms BLOB, year facts, overrides, dark theme)", solo_full, |install| {
        run_claim(install, &account("owner"), Some(LegacyCredential::Pin("2468")))
    }));
    results.push(rehearse(
        "2",
        "help desk (admin PIN, authenticator, recovery key, six clients)",
        helpdesk_full,
        |install| {
            run_claim(
                install,
                &account("admin"),
                Some(LegacyCredential::Pin("1357")),
            )
        },
    ));
    results.push(rehearse(
        "3",
        "no saved sign-in, then A (Not mine) and B (admin PIN)",
        helpdesk_full,
        |install| {
            let claim = install.claim();
            let Start::Locked(locked) = claim.start(&account("a")) else {
                return Err("A was not asked".into());
            };
            let locked = locked
                .unlock(&claim, LegacyCredential::Pin("0000"))
                .map(|_| ())
                .err()
                .ok_or("a wrong PIN opened it")?
                .into_inner();
            locked.not_mine(&claim).map_err(|error| error.to_string())?;
            let room = run_claim(install, &account("b"), Some(LegacyCredential::Pin("1357")))?;
            if room != room_of(&claim, &account("b")) {
                return Err("the data went to the wrong room".into());
            }
            Ok(room)
        },
    ));
    results.push(rehearse(
        "4",
        "queued returns (three Queued, one Submitted awaiting its email)",
        queued,
        |install| run_claim(install, &account("owner"), None),
    ));
    results.push(rehearse(
        "6",
        "practice account first, then a real account opens the old lock",
        helpdesk_full,
        |install| {
            let claim = install.claim();
            if !matches!(
                claim.start(&practice_account()),
                Start::NotHere(NotHere::Practice)
            ) {
                return Err("the practice account claimed".into());
            }
            run_claim(
                install,
                &account("real"),
                Some(LegacyCredential::Pin("1357")),
            )
        },
    ));
    results.push(rehearse(
        "7",
        "a v22-shaped file (this build's file with the v23/v24 tables dropped and user_version 22)",
        |install| {
            close(solo(install, Some("2468"), None));
            raw(&install.legacy(), "DROP TABLE setups", &[]);
            raw(&install.legacy(), "DROP TABLE year_facts", &[]);
            raw(&install.legacy(), "PRAGMA user_version = 22", &[]);
        },
        |install| {
            run_claim(
                install,
                &account("owner"),
                Some(LegacyCredential::Pin("2468")),
            )
        },
    ));
    results.push(rehearse(
        "8",
        "solo with an authenticator and no PIN",
        |install| close(solo(install, None, Some(TOTP_SECRET))),
        |install| {
            run_claim(
                install,
                &account("owner"),
                Some(LegacyCredential::AuthenticatorCode(Box::leak(
                    totp_code(TOTP_SECRET).into_boxed_str(),
                ))),
            )
        },
    ));
    results.push(rehearse(
        "9a",
        "a taxpayer with no access_mode",
        |install| close(unlocked(install)),
        |install| run_claim(install, &account("owner"), None),
    ));
    results.push(rehearse(
        "9b",
        "solo with no admin_tin_root",
        |install| {
            let db = solo(install, Some("2468"), None);
            db.delete_setting("admin_tin_root").unwrap();
            close(db);
        },
        |install| run_claim(install, &account("owner"), None),
    ));
    results.push(rehearse(
        "10",
        "a file from an archive restore (foreign help-desk mode and PINs) with two safety files beside it",
        |install| {
            close(helpdesk(install, "9753", None));
            for stamp in ["1700000000", "1700000100"] {
                std::fs::write(install.dir().join(format!("bir_data.safety-{stamp}.db")), stamp.as_bytes()).unwrap();
            }
        },
        |install| {
            let room = run_claim(install, &account("owner"), Some(LegacyCredential::Pin("9753")))?;
            let names = install.names(&format!("rooms/{room}"));
            if names.iter().filter(|name| name.starts_with("bir_data.safety-")).count() != 2 {
                return Err(format!("safety files not moved: {names:?}"));
            }
            Ok(room)
        },
    ));
    results.push(rehearse(
        "11a",
        "an unversioned painted-era file (user_version 0)",
        |install| {
            close(unlocked(install));
            raw(&install.legacy(), "PRAGMA user_version = 0", &[]);
        },
        |install| run_claim(install, &account("owner"), None),
    ));
    results.push(rehearse(
        "12",
        "a crash -wal with real commits in it",
        |install| {
            let source_dir = tempfile::tempdir().unwrap();
            let source = source_dir.path().join("bir_data.db");
            let live = Database::open(&source).unwrap();
            live.conn
                .execute_batch("PRAGMA wal_autocheckpoint = 0;")
                .unwrap();
            taxpayer(&live, "123456788", "Only In The Wal");
            draft(&live, "12345678800000", "0619E", 2026, Some(1));
            crash_copy(&source, &install.legacy());
            drop(live);
        },
        |install| run_claim(install, &account("owner"), None),
    ));
    results.push(rehearse(
        "13",
        "a debug build's bir_data.dev.db beside the real file",
        |install| {
            close(unlocked(install));
            let dev = Database::open(install.dir().join("bir_data.dev.db")).unwrap();
            taxpayer(&dev, "923456788", "Dev Only");
            close(dev);
        },
        |install| {
            let dev = install.dir().join("bir_data.dev.db");
            let dev_before = files::sha256_file(&dev).unwrap();
            let mut claim = install.claim();
            claim.mode = Some(NotHere::DevDatabase);
            if !matches!(
                claim.start(&account("owner")),
                Start::NotHere(NotHere::DevDatabase)
            ) {
                return Err("the claim ran while the dev database was chosen".into());
            }
            let room = run_claim(install, &account("owner"), None)?;
            if files::sha256_file(&dev).unwrap() != dev_before {
                return Err("bir_data.dev.db changed".into());
            }
            Ok(room)
        },
    ));

    // Shape 11b: the encrypted painted-era file is a permanent failure, left in place.
    let mut extra = Vec::new();
    {
        let install = Install::new();
        let noise: Vec<u8> = (0..16384u32)
            .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        std::fs::write(install.legacy(), &noise).unwrap();
        let claim = install.claim();
        let permanent = matches!(
            claim.start(&account("owner")),
            Start::Failed(Failure::Permanent(Permanent {
                reason: PermanentReason::NotPlainSqlite,
                ..
            }))
        );
        let untouched = std::fs::read(install.legacy()).unwrap() == noise;
        let pass = permanent && untouched && !install.layout.rooms_dir().exists();
        results.push(ShapeResult {
            number: "11b",
            name: "an encrypted painted-era file",
            rows: Vec::new(),
            extras: vec![format!(
                "permanent (not plain SQLite): {permanent}; file byte for byte: {untouched}; no room made by the claim; the note: {}",
                Permanent { reason: PermanentReason::NotPlainSqlite, detail: String::new() }.message()
            )],
            pass,
        });
    }

    // Shape 14 (in process): shape 1 cut after each durable step, resumed: the same end state, and the room's tables
    // equal that install's own "before" (timestamps differ between installs, so each is its own reference).
    {
        let mut lines = Vec::new();
        let mut pass = true;
        for point in [
            Cut::AfterUnlockRecorded,
            Cut::AfterZipWritten,
            Cut::AfterCopyRecorded,
            Cut::AfterRoomDir,
            Cut::AfterLink,
            Cut::AfterUnlink,
            Cut::AfterSafetyFiles,
            Cut::AfterMoveRecorded,
            Cut::AfterRoomRow,
            Cut::AfterHashChecked,
            Cut::AfterFixups,
            Cut::AfterOpenChecked,
        ] {
            let install = Install::new();
            solo_full(&install);
            let before = before_digest(&install);
            let claim = install.claim();
            let who = account("owner");
            let Start::Locked(locked) = claim.start(&who) else {
                panic!("not locked")
            };
            let ready = locked
                .unlock(&claim, LegacyCredential::Pin("2468"))
                .unwrap();
            CUT_AT.with(|at| at.set(Some(point)));
            let cut =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ready.claim(&claim)))
                    .is_err();
            CUT_AT.with(|at| at.set(None));
            drop(claim);
            let claim = install.claim();
            let room = match claim.start(&who) {
                Start::Finished(Outcome::Done { room, .. }) => Ok(room),
                Start::Locked(locked) => match locked
                    .unlock(&claim, LegacyCredential::Pin("2468"))
                    .unwrap()
                    .claim(&claim)
                {
                    Outcome::Done { room, .. } => Ok(room),
                    other => Err(format!("{other:?}")),
                },
                other => Err(format!("{other:?}")),
            };
            let (ok, unequal) = match &room {
                Ok(room) => {
                    let after = digest(&install.room_file(room));
                    let (rows, equal) = compare(&before, &after);
                    let unequal: Vec<String> = rows
                        .into_iter()
                        .filter(|row| !row.3)
                        .map(|row| row.0)
                        .collect();
                    (
                        equal && !install.legacy().exists() && zips_in(&install) == 1,
                        unequal,
                    )
                }
                Err(error) => (false, vec![error.clone()]),
            };
            pass &= ok && cut;
            lines.push(format!(
                "cut after {point:?}: {}; resumed to Done with every table equal to its before, one zip: {ok}{}",
                if cut { "cut" } else { "NOT CUT" },
                if unequal.is_empty() {
                    String::new()
                } else {
                    format!(" (unequal: {unequal:?})")
                }
            ));
        }
        results.push(ShapeResult {
            number: "14",
            name: "power loss at each step (in-process cut after each fsync boundary, shape 1)",
            rows: Vec::new(),
            extras: lines,
            pass,
        });
    }

    extra.push("## Not run here, and where it runs\n".into());
    for line in [
        "- **Done here: the undo by v0.2.0's own code.** `scripts/a2_old_release_restore.sh v0.2.0 <zips> <work>` builds \
         a test from the v0.2.0 tag that restores every zip this run kept (`A2_REHEARSAL_ZIPS`) with that release's own \
         `import::stage_database_zip` (which runs its `check_restorable_database`) and `StagedDatabase::install`, the \
         two calls its `restore_staged_database` makes: each restored file's raw SHA-256 equals the recorded \
         `raw_sha256`, and v0.2.0 opens it. Output: `a2-rehearsal-v0.2.0-restore.txt` beside this file.",
        "- **Still for a Devin Mac with the v0.2.0 build.** \"Before\" here is this build's open of the copy (the same \
         schema, 24, and the same migrations as v0.2.0); the plan's column taken by the v0.2.0 binary's own open, and \
         the restore through v0.2.0's Settings › Import screen.",
        "- **v0.1.0 (shape 7).** v0.1.0 was the SQLCipher build: its file is encrypted and lives in another folder \
         (macOS `~/Library/Group Containers/group.dev.goldcoders.bir`, Windows `%LOCALAPPDATA%\\Taxman\\eBIRForms`, \
         Linux `~/.taxman-ebir`; crates/bir-core/src/platform/*.rs, `cfg(feature = \"sqlcipher\")`), never the plain \
         build's `dev.goldcoders.bir` folder that the claim reads. So no v0.1.0 install reaches the claim as a schema-22 \
         file; shape 7 here is a v22-shaped plain file made by this build. A v0.1.0 backup zip restored by v0.2.0 is \
         already schema 24 by the time the claim sees it.",
        "- **Shapes 4 and 5 (sends).** The three Queued returns going out exactly once each under their own account, and \
         a send killed mid-pipe at the upgrade, need the running app, the worker and the mock relay's log: the A.9 \
         device proof (and A3b's binding), after A3a adds the call sites.",
        "- **Shape 14 on a real disk.** The VM power cut (`echo b > /proc/sysrq-trigger`) at each step runs on the \
         Linux sandbox.",
        "- **Phones.** Shape 1 on the iOS simulator and the Android emulator (their `data_dir()` paths).",
    ] {
        extra.push(line.to_owned());
    }
    let text = report(&results, &extra);
    println!("{text}");
    if let Ok(out) = std::env::var("A2_REHEARSAL_OUT")
        && !out.is_empty()
    {
        std::fs::write(&out, &text).unwrap();
    }
    let failed: Vec<&str> = results
        .iter()
        .filter(|result| !result.pass)
        .map(|result| result.number)
        .collect();
    assert!(failed.is_empty(), "rehearsal shapes failed: {failed:?}");
}
