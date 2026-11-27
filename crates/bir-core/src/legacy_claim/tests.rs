//! PLAN §A.8, A2: the claim's tests. Every one runs in a temporary device folder of its own.

use std::panic::AssertUnwindSafe;

use super::fixtures::*;
use super::*;

/// The claim of `who` from start to end, for a file with no lock.
fn claim_unlocked(install: &Install, who: &Claimant) -> Outcome {
    let claim = install.claim();
    match claim.start(who) {
        Start::Ready(ready) => ready.claim(&claim),
        other => panic!("not ready: {other:?}"),
    }
}

/// Unlock `who`'s locked claim with `credential` and run it.
fn claim_locked(install: &Install, who: &Claimant, credential: LegacyCredential<'_>) -> Outcome {
    let claim = install.claim();
    match claim.start(who) {
        Start::Locked(locked) => match locked.unlock(&claim, credential) {
            Ok(ready) => ready.claim(&claim),
            Err(_) => panic!("refused"),
        },
        other => panic!("not locked: {other:?}"),
    }
}

/// Run a claim for `who` and cut it at `point` (a kill there). The locks go with the "process".
fn claim_cut_at(install: &Install, who: &Claimant, point: Cut) {
    let claim = install.claim();
    let Start::Ready(ready) = claim.start(who) else {
        panic!("not ready");
    };
    CUT_AT.with(|at| at.set(Some(point)));
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| ready.claim(&claim)));
    CUT_AT.with(|at| at.set(None));
    assert!(result.is_err(), "the claim was not cut at {point:?}");
}

/// Run a locked claim for `who` (unlocked with `pin`) and cut it at `point`, as a kill there.
fn locked_claim_cut_at(install: &Install, who: &Claimant, pin: &str, point: Cut) {
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(who) else {
        panic!("not locked")
    };
    let ready = match locked.unlock(&claim, LegacyCredential::Pin(pin)) {
        Ok(ready) => ready,
        Err(_) => panic!("refused"),
    };
    CUT_AT.with(|at| at.set(Some(point)));
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| ready.claim(&claim)));
    CUT_AT.with(|at| at.set(None));
    assert!(result.is_err(), "the claim was not cut at {point:?}");
}

/// Run `observe` when the claim reaches `point`.
fn at_point(point: Cut, observe: impl FnOnce() + 'static) {
    AT_POINT.with(|slot| *slot.borrow_mut() = Some((point, Box::new(observe))));
}

fn done_room(outcome: &Outcome) -> RoomId {
    match outcome {
        Outcome::Done { room, .. } => room.clone(),
        other => panic!("not done: {other:?}"),
    }
}

fn zips(install: &Install) -> Vec<String> {
    install
        .names("backups")
        .into_iter()
        .filter(|name| name.ends_with(".zip"))
        .collect()
}

// ---------------------------------------------------------------------------------------------- step 3

/// M2: the zip's one entry is the raw file, before any migration of this build, and the record keeps its old
/// `user_version`.
#[test]
fn the_safety_zip_is_the_raw_file() {
    let install = Install::new();
    close(unlocked(&install));
    // An older schema's version, as the raw file says it: the claim must record it, not this build's.
    raw(&install.legacy(), "PRAGMA user_version = 23", &[]);
    let raw_sha = files::sha256_file(&install.legacy()).unwrap();

    let who = account("a");
    let outcome = claim_unlocked(&install, &who);
    done_room(&outcome);
    let record = install.claim().record().unwrap().unwrap();
    assert_eq!(record.raw_user_version, Some(23));
    assert_eq!(record.raw_sha256.as_deref(), Some(raw_sha.as_str()));
    let zip = install
        .dir()
        .join("backups")
        .join(record.zip.as_ref().unwrap());
    assert_eq!(
        files::zip_entry_sha256(&zip).unwrap(),
        raw_sha,
        "the entry is the raw file"
    );
    assert_eq!(record.zip_sha256, Some(files::sha256_file(&zip).unwrap()));
    // The format the previous version's Import restores: one top-level bir_data.db, which its check accepts.
    let restore = tempfile::tempdir().unwrap();
    let staged = crate::import::stage_database_zip(&zip, &restore.path().join("bir_data.db"));
    assert!(staged.is_ok(), "{:?}", staged.err());
}

// ---------------------------------------------------------------------------------------------- resume

/// M1: a cut after the link and before the unlink, and after the unlink and before the record: the files decide, no
/// legacy file is ever made again, and the room keeps every row.
#[test]
fn resume_never_creates_a_legacy_file() {
    for point in [Cut::AfterLink, Cut::AfterUnlink, Cut::AfterSafetyFiles] {
        let install = Install::new();
        close(unlocked(&install));
        let before = counts(&install.legacy());
        let who = account("a");
        claim_cut_at(&install, &who, point);

        // Any account's next launch finishes it.
        let claim = install.claim();
        match claim.start(&account("someone-else")) {
            Start::Finished(Outcome::Done { room, .. }) => {
                assert_eq!(room, room_of(&claim, &who), "{point:?}");
                assert_eq!(
                    counts(&install.room_file(&room))[..3],
                    before[..3],
                    "{point:?}: every row"
                );
            }
            other => panic!("{point:?}: {other:?}"),
        }
        assert!(!install.legacy().exists(), "{point:?}: no legacy file made");
        assert!(matches!(claim.start(&who), Start::Nothing), "{point:?}");
        assert!(!install.legacy().exists(), "{point:?}");
    }
}

/// M1: the move never puts the legacy file over a room file that has data (it is offered instead), and the low-level
/// move refuses a taken name.
#[test]
fn the_move_never_replaces_a_room_file() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    let claim = install.claim();
    let room = room_of(&claim, &who);
    // This account's room already has data.
    install.layout.create_room_dir(&room).unwrap();
    let room_db = Database::open(install.room_file(&room)).unwrap();
    taxpayer(&room_db, "923456788", "Already Here");
    close(room_db);
    let room_before = std::fs::read(install.room_file(&room)).unwrap();
    assert!(matches!(
        claim.start(&who),
        Start::Offer(OfferStart::Open(_))
    ));
    assert_eq!(
        std::fs::read(install.room_file(&room)).unwrap(),
        room_before
    );
    assert!(install.legacy().exists());

    // The link itself refuses a taken name.
    let error =
        crate::safe_fs::link_no_replace(&install.legacy(), &install.room_file(&room)).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(install.room_file(&room)).unwrap(),
        room_before
    );
}

/// M1: the zip is checked again just before the move; one changed since stops the move, and the next round copies again.
#[test]
fn zip_rechecked_before_the_move() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    let claim = install.claim();
    let Start::Ready(ready) = claim.start(&who) else {
        panic!("not ready")
    };
    let backups = install.dir().join("backups");
    at_point(Cut::AfterCopyRecorded, move || {
        for entry in std::fs::read_dir(&backups).unwrap().flatten() {
            std::fs::write(entry.path(), b"not the copy any more").unwrap();
        }
    });
    match ready.claim(&claim) {
        Outcome::Transient(transient) => {
            assert_eq!(transient.cause, TransientCause::FileChanged);
            assert!(!transient.copy_saved, "a bad copy is not a saved copy");
        }
        other => panic!("{other:?}"),
    }
    assert!(install.legacy().exists(), "not moved");
    assert!(
        !install
            .layout
            .rooms_dir()
            .join(room_of(&claim, &who).as_str())
            .join("bir_data.db")
            .exists()
    );
    // The next round writes a good copy, and finishes.
    let room = done_room(&claim_unlocked(&install, &who));
    assert!(install.room_file(&room).exists());
}

/// M4: the owner lock is held from the start to the move, so another opener is refused; it goes with the move.
#[test]
fn claim_holds_the_owner_lock() {
    let install = Install::new();
    close(unlocked(&install));
    let legacy = install.legacy();
    assert!(foreign_lock_free(&legacy));
    let claim = install.claim();
    let Start::Ready(ready) = claim.start(&account("a")) else {
        panic!("not ready")
    };
    assert!(!foreign_lock_free(&legacy), "held from step 1");
    let observed = std::rc::Rc::new(std::cell::Cell::new(None));
    let seen = observed.clone();
    let probe_path = legacy.clone();
    at_point(Cut::AfterLink, move || {
        seen.set(Some(foreign_lock_free(&probe_path)))
    });
    done_room(&ready.claim(&claim));
    assert_eq!(observed.get(), Some(false), "held during step 4");
    assert!(foreign_lock_free(&legacy), "released after the move");
}

// ---------------------------------------------------------------------------------------------- the offer

/// M4, N1: a legacy file seen after the claim is done is offered, never merged by itself; with a credential the
/// unlock comes first; "Add" imports its profiles and renames it, never replacing.
#[test]
fn a_reappearing_legacy_file_is_offered() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    let room = done_room(&claim_unlocked(&install, &who));

    // The old version ran again: an empty file, then a new profile in it.
    let old = install.open_legacy();
    taxpayer(&old, "723456788", "Typed In The Old Version");
    close(old);
    let claim = install.claim();
    let Start::Offer(OfferStart::Open(offer)) = claim.start(&who) else {
        panic!("not offered")
    };
    assert!(install.legacy().exists(), "never merged by itself");
    assert_eq!(
        Offer::MESSAGE,
        "Tax profiles from an older version of eBIRForms were found on this device."
    );
    // Not now: dropped, and offered again next time.
    drop(offer);
    let Start::Offer(OfferStart::Open(offer)) = claim.start(&who) else {
        panic!("not offered again")
    };
    let room_db = Database::open(install.room_file(&room)).unwrap();
    let added = offer.add(&claim, &room_db).unwrap();
    assert_eq!(added.profiles, 1);
    assert!(
        room_db
            .get_profile_by_tin("72345678800000")
            .unwrap()
            .is_some()
    );
    assert!(
        room_db
            .get_profile_by_tin("12345678800000")
            .unwrap()
            .is_some(),
        "the room's own data stays"
    );
    assert!(!install.legacy().exists());
    assert!(
        added
            .offered_file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("bir_data.offered-")
    );
    drop(room_db);
    assert!(matches!(claim.start(&who), Start::Nothing));

    // A reappeared file the old version locked asks its PIN before Add.
    let old = install.open_legacy();
    taxpayer(&old, "823456788", "Locked Again");
    old.set_setting("access_mode", "helpdesk").unwrap();
    old.set_setting("app_lock_pin_hash", &crate::crypto::hash_pin("4321"))
        .unwrap();
    close(old);
    let Start::Offer(OfferStart::Locked(locked)) = claim.start(&who) else {
        panic!("the unlock does not come first")
    };
    let locked = locked
        .unlock(&claim, LegacyCredential::Pin("0000"))
        .unwrap_err()
        .into_inner();
    assert!(locked.unlock(&claim, LegacyCredential::Pin("4321")).is_ok());
}

/// N1 (§A.3.5): an account whose room has data meets a locked legacy file only through the unlock, then the offer.
#[test]
fn an_account_with_data_gets_the_offer_through_the_unlock() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let who = account("b");
    let claim = install.claim();
    let room = room_of(&claim, &who);
    install.layout.create_room_dir(&room).unwrap();
    let room_db = Database::open(install.room_file(&room)).unwrap();
    taxpayer(&room_db, "923456788", "Added After Not Mine");
    match claim.start(&who) {
        Start::Offer(OfferStart::Locked(_)) => {}
        other => panic!("{other:?}"),
    }
}

/// §A.3.6: what a whole-profile archive carries into the room (the offer's wording follows it). It carries each
/// profile, its submissions, drafts, receipts, data providers' names, yearly forms and year facts; it does not carry
/// settings, credentials (PINs, authenticators), eBIRForms filed returns (`legacy_filed_returns`), templates or alerts.
#[test]
fn what_the_offer_carries() {
    let install = Install::new();
    let db = unlocked(&install);
    db.save_submission(crate::db::Submission {
        id: None,
        tin: "12345678800000".into(),
        form_type: "1601C".into(),
        period: "2026-01".into(),
        status: "Submitted".into(),
        form_data: Default::default(),
        submitted_at: None,
        filename: Some("123456788000-1601C-012026.xml".into()),
        created_at: None,
        updated_at: None,
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
        source_filename: "old.xml".into(),
        raw_file: b"<xml/>".to_vec(),
        fields_json: "{}".into(),
        imported_at: None,
    })
    .unwrap();
    db.set_taxpayer_pin_hash("123456788", Some(&crate::crypto::hash_pin("1111")))
        .unwrap();
    db.set_setting("deadline_overrides", "[]").unwrap();
    close(db);

    let room_dir = tempfile::tempdir().unwrap();
    let room = Database::open(room_dir.path().join("bir_data.db")).unwrap();
    // Through the offer's own code path (a client PIN is not an admin lock, so no unlock comes first).
    let lock = crate::db::hold_owner_lock(&install.legacy()).unwrap();
    let added = files::add_offered(
        &install.layout,
        &room,
        crate::safe_fs::file_id(&install.legacy()).ok(),
    )
    .unwrap();
    drop(lock);
    assert_eq!(added.profiles, 1);
    let count = |table: &str| -> i64 {
        room.conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count("profiles"), 1);
    assert_eq!(count("form_drafts"), 1, "drafts are carried");
    assert_eq!(count("submissions"), 1, "submissions are carried");
    assert_eq!(
        count("legacy_filed_returns"),
        0,
        "eBIRForms filed returns are not carried"
    );
    assert_eq!(
        room.taxpayer_credentials("123456788").unwrap().pin_hash,
        None,
        "no credential is carried"
    );
    assert_eq!(
        room.get_setting("deadline_overrides").unwrap(),
        None,
        "no setting is carried"
    );
}

// ---------------------------------------------------------------------------------------------- the unlock

/// L1: a locked file opens only with its old credential, of each kind.
#[test]
fn locked_legacy_needs_its_old_credential() {
    struct Case {
        name: &'static str,
        make: fn(&Install),
        right: fn() -> LegacyCredential<'static>,
        by: UnlockedBy,
    }
    fn helpdesk_install(install: &Install) {
        close(helpdesk(install, "1357", Some("ABCD-EFGH-JKMN-PQRS")));
    }
    let totp: fn() -> LegacyCredential<'static> =
        || LegacyCredential::AuthenticatorCode(Box::leak(totp_code(TOTP_SECRET).into_boxed_str()));
    let cases = [
        Case {
            name: "admin PIN",
            make: helpdesk_install,
            right: || LegacyCredential::Pin("1357"),
            by: UnlockedBy::Pin,
        },
        Case {
            name: "admin authenticator",
            make: helpdesk_install,
            right: totp,
            by: UnlockedBy::Authenticator,
        },
        Case {
            name: "recovery key",
            make: helpdesk_install,
            right: || LegacyCredential::RecoveryKey("abcd efgh jkmn pqrs"),
            by: UnlockedBy::RecoveryKey,
        },
        Case {
            name: "solo PIN",
            make: |install| close(solo(install, Some("2468"), None)),
            right: || LegacyCredential::Pin("2468"),
            by: UnlockedBy::Pin,
        },
        Case {
            name: "solo authenticator, no PIN",
            make: |install| close(solo(install, None, Some(TOTP_SECRET))),
            right: totp,
            by: UnlockedBy::Authenticator,
        },
        Case {
            name: "branch profile_pin_hash",
            make: |install| {
                close(solo(install, Some("2468"), None));
                // An older build left a different PIN on the owner's branch profile; the v22 heal would clear it.
                raw(
                    &install.legacy(),
                    "UPDATE profiles SET data_json = json_set(data_json, '$.profile_pin_hash', ?1)
                     WHERE tin = '12345678800000'",
                    &[&crate::crypto::hash_pin("8642")],
                );
            },
            right: || LegacyCredential::Pin("8642"),
            by: UnlockedBy::Pin,
        },
    ];
    for case in cases {
        let install = Install::new();
        (case.make)(&install);
        let who = account("owner");
        let claim = install.claim();
        let Start::Locked(locked) = claim.start(&who) else {
            panic!("{}: not locked", case.name)
        };
        // Wrong ones first: each is refused and nothing moves.
        let locked = locked
            .unlock(&claim, LegacyCredential::Pin("0000"))
            .unwrap_err()
            .into_inner();
        let locked = locked
            .unlock(
                &claim,
                LegacyCredential::AuthenticatorCode(&wrong_code(TOTP_SECRET)),
            )
            .unwrap_err()
            .into_inner();
        let locked = locked
            .unlock(&claim, LegacyCredential::RecoveryKey("ZZZZ-ZZZZ-ZZZZ-ZZZZ"))
            .unwrap_err()
            .into_inner();
        assert!(install.legacy().exists());
        assert!(
            claim.record().unwrap().is_none(),
            "{}: no record before the unlock (N3)",
            case.name
        );
        let ready = locked
            .unlock(&claim, (case.right)())
            .unwrap_or_else(|_| panic!("{}: the right credential was refused", case.name));
        assert_eq!(ready.unlocked_by(), case.by, "{}", case.name);
        let room = done_room(&ready.claim(&claim));
        assert!(install.room_file(&room).exists(), "{}", case.name);
        assert!(!install.legacy().exists(), "{}", case.name);
    }
}

/// L1: a client taxpayer's PIN opened only that client, so it never opens the whole file.
#[test]
fn a_client_pin_does_not_open_the_legacy_file() {
    for (name, make, client_pin) in [
        (
            "help desk client",
            (|install: &Install| close(helpdesk(install, "1357", None))) as fn(&Install),
            "5001",
        ),
        (
            "solo family member",
            |install| close(solo(install, Some("2468"), None)),
            "2222",
        ),
    ] {
        let install = Install::new();
        make(&install);
        let claim = install.claim();
        let Start::Locked(locked) = claim.start(&account("a")) else {
            panic!("{name}: not locked")
        };
        assert!(
            locked
                .unlock(&claim, LegacyCredential::Pin(client_pin))
                .is_err(),
            "{name}"
        );
        assert!(install.legacy().exists(), "{name}");
    }
}

/// Ruling 1 (the library's half; the page is A3a-2's): the computer's login opens the claim once, and only once.
#[test]
fn computer_login_is_accepted_once_per_claim() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&account("a")) else {
        panic!("not locked")
    };
    assert!(locked.offers_computer_login(&claim));
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .unwrap();
    assert_eq!(ready.unlocked_by(), UnlockedBy::ComputerLogin);
    // Cut mid-move (after the copy's record): the resume needs a new unlock, and L1's ruling offers the button
    // again — a person must never lose their fallback because the app was closed at the wrong second.
    CUT_AT.with(|at| at.set(Some(Cut::AfterCopyRecorded)));
    let cut = std::panic::catch_unwind(AssertUnwindSafe(|| ready.claim(&claim)));
    CUT_AT.with(|at| at.set(None));
    assert!(cut.is_err(), "the claim was not cut");
    let Start::Locked(locked) = claim.start(&account("a")) else {
        panic!("not locked")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "offered again for the resume (L1)"
    );
    assert!(
        locked
            .unlock(&claim, LegacyCredential::ComputerLogin)
            .is_ok(),
        "the computer login opens the resume"
    );
}

/// Ruling 1, the clearances: the computer login is spent once per claim, never once forever. Usable once, refused
/// within the same claim, and usable again when that claim ends — at Done, and after a failure before the move (a
/// pre-move resume needs a new unlock: the person is never left without their fallback).
#[test]
fn computer_login_is_usable_again_when_the_claim_ends() {
    // Usable once, refused within the same claim, then the claim runs to Done.
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(locked.offers_computer_login(&claim), "offered first");
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .unwrap();
    // Cut mid-move (after the copy's record): the resume needs a new unlock, and L1's ruling offers the button
    // again — a kill before the move never strands the fallback. The refusal that remains is within one live
    // claim, no start() between the uses (a_second_use_within_one_live_claim_is_refused).
    CUT_AT.with(|at| at.set(Some(Cut::AfterCopyRecorded)));
    let cut = std::panic::catch_unwind(AssertUnwindSafe(|| ready.claim(&claim)));
    CUT_AT.with(|at| at.set(None));
    assert!(cut.is_err(), "the claim was not cut");
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "offered again for the resume (L1)"
    );
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .expect("the computer login opens the resume");
    done_room(&ready.claim(&claim));
    assert!(
        !install.claim().os_login_used().unwrap(),
        "the used mark is cleared at Done"
    );

    // A locked legacy file turning up after Done is a new round (offered, §A.3.6): the computer login opens it
    // again, because the first claim's end cleared the used mark.
    close(helpdesk(&install, "1357", None));
    let Start::Offer(OfferStart::Locked(locked_offer)) = claim.start(&a) else {
        panic!("not a locked offer for the new round")
    };
    locked_offer
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .expect("usable again in a new round after Done");

    // A failure before the move ends the claim: the computer login is not left spent.
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .unwrap();
    INJECT_AT.with(|at| at.set(Some(Inject::Zip)));
    match ready.claim(&claim) {
        Outcome::Transient(transient) => assert_eq!(transient.cause, TransientCause::DiskFull),
        other => panic!("{other:?}"),
    }
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked after the pre-move failure")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "offered again after a pre-move failure"
    );
    locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .expect("usable again after a pre-move failure");
}

// ---------------------------------------------------------------------------------------------- Not mine

/// Ruled: "Not mine" gives that account an empty room (its own, the caller's) and leaves the file for the next account.
#[test]
fn not_mine_gives_an_empty_room_and_leaves_the_file() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    locked.not_mine(&claim).unwrap();
    assert!(install.legacy().exists());
    assert!(
        !install.layout.rooms_dir().exists(),
        "nothing of the claim's in rooms/"
    );
    assert!(
        matches!(claim.start(&a), Start::DeclinedHere),
        "not offered to A again"
    );
    // The next account is offered the page, and the right one claims.
    let b = account("b");
    let room = done_room(&claim_locked(&install, &b, LegacyCredential::Pin("1357")));
    assert_eq!(room, room_of(&claim, &b));
    assert!(matches!(claim.start(&a), Start::Nothing));
}

/// Ruled: "Not mine" leaves `bir_data.db`, `-wal` and `-shm` byte for byte, and makes no `backups/` entry.
#[test]
fn not_mine_leaves_the_file_byte_for_byte() {
    let install = Install::new();
    // A crash image: commits still in the -wal, the -shm beside it, nothing holding them.
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("bir_data.db");
    let live = Database::open(&source).unwrap();
    live.conn
        .execute_batch("PRAGMA wal_autocheckpoint = 0;")
        .unwrap();
    live.set_setting("access_mode", "helpdesk").unwrap();
    live.set_setting("app_lock_pin_hash", &crate::crypto::hash_pin("1357"))
        .unwrap();
    taxpayer(&live, "423456788", "Client In The Wal");
    crash_copy(&source, &install.legacy());
    drop(live);
    let before = install.legacy_bytes();
    assert!(
        before[1].as_ref().is_some_and(|wal| !wal.is_empty()),
        "a -wal with commits"
    );
    assert!(before[2].is_some(), "a -shm");

    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&account("a")) else {
        panic!("not locked: the probe must read the -wal's commits")
    };
    locked.not_mine(&claim).unwrap();
    assert_eq!(install.legacy_bytes(), before, "byte for byte");
    assert!(zips(&install).is_empty(), "no zip in a Not mine round");
    assert!(install.names("backups").is_empty());
    assert!(
        install.names(SCRATCH_DIR).is_empty(),
        "the probe copy is gone"
    );
}

// ---------------------------------------------------------------------------------------------- who

/// Decision 12: the store-review account never claims; the file waits for a real account.
#[test]
fn a_practice_account_never_claims() {
    let install = Install::new();
    close(unlocked(&install));
    let before = install.legacy_bytes();
    let claim = install.claim();
    assert!(matches!(
        claim.start(&practice_account()),
        Start::NotHere(NotHere::Practice)
    ));
    assert_eq!(install.legacy_bytes(), before);
    assert!(claim.record().unwrap().is_none());
    // A real account then claims it.
    done_room(&claim_unlocked(&install, &account("real")));
}

/// N3: A reaches the unlock and leaves; B types the admin PIN; the file goes to B's room and A's stays empty. Also with
/// a record left by an earlier round of A's that got as far as the copy.
#[test]
fn second_account_mid_claim_moves_nothing_into_the_first_room() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let (a, b) = (account("a"), account("b"));
    let claim = install.claim();
    let a_room = room_of(&claim, &a);
    // A reaches the page and leaves (the app is closed).
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    drop(locked);
    assert!(
        claim.record().unwrap().is_none(),
        "no room named before a credential passed"
    );
    // An earlier round by A that unlocked and was cut after its copy.
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    let ready = locked
        .unlock(&claim, LegacyCredential::Pin("1357"))
        .unwrap();
    CUT_AT.with(|at| at.set(Some(Cut::AfterCopyRecorded)));
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| ready.claim(&claim))).is_err());
    assert_eq!(
        claim.record().unwrap().unwrap().room.as_deref(),
        Some(a_room.as_str())
    );

    // B signs in: B must unlock for itself, and the file goes to B's room.
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&b) else {
        panic!("B is not asked")
    };
    let b_room = done_room(
        &locked
            .unlock(&claim, LegacyCredential::Pin("1357"))
            .unwrap()
            .claim(&claim),
    );
    assert_eq!(b_room, room_of(&claim, &b));
    assert!(!install.room_file(&a_room).exists(), "nothing in A's room");
    assert_eq!(
        claim.record().unwrap().unwrap().room.as_deref(),
        Some(b_room.as_str())
    );
}

// ---------------------------------------------------------------------------------------------- N2 windows

/// N2: cut between the link and the unlink; the resume sees one file under two names and finishes, with no offer.
#[test]
fn hard_link_window_finishes_the_unlink() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    claim_cut_at(&install, &who, Cut::AfterLink);
    let claim = install.claim();
    let room = room_of(&claim, &who);
    assert!(
        install.legacy().exists() && install.room_file(&room).exists(),
        "both names"
    );
    if crate::safe_fs::file_id(&install.legacy()).is_ok() {
        assert_eq!(
            crate::safe_fs::file_id(&install.legacy()).unwrap(),
            crate::safe_fs::file_id(&install.room_file(&room)).unwrap()
        );
    }
    match claim.start(&who) {
        Start::Finished(Outcome::Done { room: done, .. }) => assert_eq!(done, room),
        other => panic!("{other:?}"),
    }
    assert!(!install.legacy().exists());
    assert!(matches!(claim.start(&who), Start::Nothing), "no offer");
}

/// N2: an account with an empty room, cut before and after the empty file is set aside: either way the empty file
/// ends set aside and the data is linked in.
#[test]
fn empty_room_aside_window_moves_it_then_links() {
    for point in [Cut::AfterRoomDir, Cut::AfterEmptyAside] {
        let install = Install::new();
        close(unlocked(&install));
        let who = account("b");
        let claim = install.claim();
        let room = room_of(&claim, &who);
        install.layout.create_room_dir(&room).unwrap();
        close(Database::open(install.room_file(&room)).unwrap());
        claim_cut_at(&install, &who, point);
        let claim = install.claim();
        match claim.start(&who) {
            Start::Ready(ready) => done_room(&ready.claim(&claim)),
            Start::Finished(outcome) => done_room(&outcome),
            other => panic!("{point:?}: {other:?}"),
        };
        let names = install.names(&format!("rooms/{room}"));
        assert!(
            names.iter().any(|name| name.starts_with("bir_data.empty-")),
            "{point:?}: {names:?}"
        );
        assert!(
            !files::room_is_empty(&install.room_file(&room)),
            "{point:?}: the data is in"
        );
        assert!(!install.legacy().exists());
    }
}

// ---------------------------------------------------------------------------------------------- run-time checks

/// Ruling 2: a room file whose hash is not the raw one is "moved, not opened": it is kept as it is (no fixups), the
/// zip is kept, and no empty room is offered.
#[test]
fn hash_check_failure_keeps_the_file_and_the_zip() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    claim_cut_at(&install, &who, Cut::AfterRoomRow);
    let claim = install.claim();
    let room = room_of(&claim, &who);
    // Something wrote the moved file before the check.
    raw(
        &install.room_file(&room),
        "INSERT INTO settings (key, value) VALUES ('written', 'outside')",
        &[],
    );
    let changed = std::fs::read(install.room_file(&room)).unwrap();
    let zip_before = zips(&install);
    match claim.start(&who) {
        Start::Finished(Outcome::MovedNotOpened(moved)) => {
            assert_eq!(moved.reason, MovedNotOpenedReason::HashMismatch);
            assert!(
                moved
                    .message("<folder>")
                    .contains("a copy is saved in <folder>/backups")
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        std::fs::read(install.room_file(&room)).unwrap(),
        changed,
        "no fixup ran"
    );
    assert_eq!(zips(&install), zip_before);
    // Still that state at the next launch, and the zip still kept.
    assert!(matches!(
        claim.start(&who),
        Start::Finished(Outcome::MovedNotOpened(_))
    ));
    assert_eq!(
        claim.zip_due(Utc::now() + chrono::Duration::days(400)),
        Err(ZipKept::NotDone)
    );
}

/// Ruling 2: a failure at step 6 or its open check is "moved, not opened", not an empty room beside the data.
#[test]
fn step_6_failure_is_moved_not_opened_and_opens_no_empty_room() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    claim_cut_at(&install, &who, Cut::AfterHashChecked);
    let claim = install.claim();
    let room = room_of(&claim, &who);
    // The moved file is damaged before step 6.
    let path = install.room_file(&room);
    let mut bytes = std::fs::read(&path).unwrap();
    let page = 4096.min(bytes.len() / 2);
    for byte in &mut bytes[page..] {
        *byte = 0xA5;
    }
    std::fs::write(&path, &bytes).unwrap();
    match claim.start(&who) {
        Start::Finished(Outcome::MovedNotOpened(moved)) => {
            assert_ne!(moved.reason, MovedNotOpenedReason::HashMismatch)
        }
        other => panic!("{other:?}"),
    }
    assert!(path.exists(), "the file stays in the room");
    assert_eq!(
        install.names(&format!("rooms/{room}")),
        ["bir_data.db", "bir_data.db.owner.lock"]
    );
    assert_eq!(zips(&install).len(), 1, "the zip is kept");
    assert!(
        !install.legacy().exists(),
        "no legacy file and no empty room beside the data"
    );
}

// ---------------------------------------------------------------------------------------------- failures

/// M3: a transient failure says a copy is saved only once step 3 passed its check.
#[test]
fn transient_failure_says_copy_saved_only_after_step_3() {
    // Before step 3: the owner lock is held by another copy of the app.
    let install = Install::new();
    close(unlocked(&install));
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(crate::db::live_owner_lock_path(&install.legacy()))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a valid descriptor.
        assert_eq!(
            unsafe { libc::flock(foreign.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        let claim = install.claim();
        match claim.start(&account("a")) {
            Start::Failed(Failure::Transient(transient)) => {
                assert_eq!(transient.cause, TransientCause::OwnerLockHeld);
                assert!(!transient.copy_saved);
                let words = transient.message("/data");
                assert_eq!(
                    words,
                    "Your tax profiles could not be moved to this account yet. Nothing was changed: they are still \
                     where they were."
                );
            }
            other => panic!("{other:?}"),
        }
    }
    drop(foreign);

    // After step 3: something opened the file between the copy and the move.
    let claim = install.claim();
    let Start::Ready(ready) = claim.start(&account("a")) else {
        panic!("not ready")
    };
    let legacy = install.legacy();
    let holder = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = holder.clone();
    at_point(Cut::AfterCopyRecorded, move || {
        let conn = Connection::open(&legacy).unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL; SELECT count(*) FROM settings;")
            .unwrap();
        *keep.borrow_mut() = Some(conn);
    });
    match ready.claim(&claim) {
        Outcome::Transient(transient) => {
            assert_eq!(transient.cause, TransientCause::WalReturned);
            assert!(transient.copy_saved);
            assert!(
                transient
                    .message("this app's storage")
                    .contains("and a copy is saved in this app's storage/backups.")
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(install.legacy().exists(), "nothing moved");
    drop(holder);
}

/// A `-wal` that keeps coming back stays transient, with its cause named, however often; never an empty room.
#[test]
fn recurring_wal_stays_transient_with_its_cause() {
    let install = Install::new();
    close(unlocked(&install));
    // Another program keeps the file open with its own connection.
    let other = Connection::open(install.legacy()).unwrap();
    other
        .execute_batch("PRAGMA journal_mode = WAL; SELECT count(*) FROM settings;")
        .unwrap();
    for round in 0..4 {
        let claim = install.claim();
        let outcome = match claim.start(&account("a")) {
            Start::Ready(ready) => ready.claim(&claim),
            Start::Failed(failure) => failure.into(),
            other => panic!("round {round}: {other:?}"),
        };
        match outcome {
            Outcome::Transient(transient) => {
                assert_eq!(
                    transient.cause,
                    TransientCause::WalReturned,
                    "round {round}"
                );
                assert!(
                    transient
                        .message("/data")
                        .contains("Another program has your tax profiles open")
                );
            }
            other => panic!("round {round}: {other:?}"),
        }
        assert!(install.legacy().exists());
        assert!(!install.layout.rooms_dir().exists(), "no room is opened");
    }
    drop(other);
    done_room(&claim_unlocked(&install, &account("a")));
}

/// M3: a permanent failure leaves the file exactly in place, records it, and does not run again by itself; the
/// account gets an empty room (the caller's) and the note on Home.
#[test]
fn permanent_failure_opens_an_empty_room_and_leaves_the_file() {
    let install = Install::new();
    close(unlocked(&install));
    // A schema newer than this build.
    raw(&install.legacy(), "PRAGMA user_version = 999", &[]);
    let before = install.legacy_bytes();
    let claim = install.claim();
    for _ in 0..2 {
        match claim.start(&account("a")) {
            Start::Failed(Failure::Permanent(permanent)) => {
                assert_eq!(permanent.reason, PermanentReason::NewerSchema);
                assert!(permanent.message().ends_with(
                    "They are still on this device, unchanged. Write to support@buwiz.com."
                ));
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(install.legacy_bytes(), before, "left exactly in place");
    assert!(!install.layout.rooms_dir().exists());
    assert!(zips(&install).is_empty());
    assert!(matches!(
        claim.record().unwrap().unwrap().failure,
        Some(RecordedFailure::Permanent {
            reason: PermanentReason::NewerSchema,
            ..
        })
    ));
}

/// A painted-era encrypted file is permanent; an unversioned painted-era file claims.
#[test]
fn encrypted_painted_file_is_permanent() {
    let install = Install::new();
    let mut noise = vec![0u8; 8192];
    for (index, byte) in noise.iter_mut().enumerate() {
        *byte = (index * 7919 % 251) as u8;
    }
    std::fs::write(install.legacy(), &noise).unwrap();
    let claim = install.claim();
    match claim.start(&account("a")) {
        Start::Failed(Failure::Permanent(permanent)) => {
            assert_eq!(permanent.reason, PermanentReason::NotPlainSqlite)
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read(install.legacy()).unwrap(), noise);

    // Unversioned (user_version 0) with the app's tables: it claims.
    let install = Install::new();
    close(unlocked(&install));
    raw(&install.legacy(), "PRAGMA user_version = 0", &[]);
    let room = done_room(&claim_unlocked(&install, &account("a")));
    let db = Database::open(install.room_file(&room)).unwrap();
    assert!(db.get_profile_by_tin("12345678800000").unwrap().is_some());
}

// ---------------------------------------------------------------------------------------------- the zip

/// §A.3.8: one zip per raw file. A resume reuses it once re-checked, "Not mine" makes none, and a changed file gets
/// its own beside the first.
#[test]
fn one_zip_per_raw_file() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    claim_cut_at(&install, &who, Cut::AfterCopyRecorded);
    assert_eq!(zips(&install).len(), 1);
    claim_cut_at(&install, &who, Cut::AfterCopyRecorded);
    assert_eq!(zips(&install).len(), 1, "the resume reuses it");
    // The file changes (the old version ran): its own zip, kept beside the first.
    let db = install.open_legacy();
    taxpayer(&db, "723456788", "Changed");
    close(db);
    done_room(&claim_unlocked(&install, &who));
    assert_eq!(zips(&install).len(), 2);
    for name in zips(&install) {
        assert!(name.starts_with(ZIP_PREFIX), "{name}");
    }
}

fn done_install() -> (Install, Claimant, RoomId) {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    let room = done_room(&claim_unlocked(&install, &who));
    (install, who, room)
}

fn launch_on_days(claim: &LegacyClaim, from: NaiveDate, days: i64) {
    for day in 1..=days {
        claim
            .note_launch(from + chrono::Duration::days(day))
            .unwrap();
    }
}

/// §A.3.8, D42: the zip goes only after 30 days by the wall clock and launches on 5 distinct days.
#[test]
fn zip_needs_30_days_and_5_distinct_launch_days() {
    let (install, _who, room) = done_install();
    let claim = install.claim();
    claim.note_room_opened(&room).unwrap();
    let now = Utc::now();
    let today = chrono::Local::now().date_naive();
    // 31 days later, but only 4 launch days (the same day again does not count).
    launch_on_days(&claim, today, 4);
    claim
        .note_launch(today + chrono::Duration::days(2))
        .unwrap();
    assert_eq!(
        claim.zip_due(now + chrono::Duration::days(31)),
        Err(ZipKept::TooFewLaunchDays(4))
    );
    // A clock set forward: 5 days, but not 30 days.
    claim
        .note_launch(today + chrono::Duration::days(40))
        .unwrap();
    assert!(matches!(
        claim.zip_due(now + chrono::Duration::days(5)),
        Err(ZipKept::TooSoon { .. })
    ));
    // Both: it goes, and the deletion is recorded.
    let deleted = claim
        .delete_zip_if_due(now + chrono::Duration::days(31))
        .unwrap();
    assert_eq!(deleted.len(), 1);
    assert!(zips(&install).is_empty());
    assert!(claim.record().unwrap().unwrap().zip_deleted_at.is_some());
}

/// Ruling 2 and §A.3.8: the zip needs both run-time checks recorded, no failure state, no offer waiting, and the room
/// opened since.
#[test]
fn zip_deleted_only_after_both_runtime_checks_and_no_failure_state() {
    let later = Utc::now() + chrono::Duration::days(31);
    let today = chrono::Local::now().date_naive();

    // The room not opened since.
    let (install, _who, room) = done_install();
    let claim = install.claim();
    launch_on_days(&claim, today, 5);
    assert_eq!(claim.zip_due(later), Err(ZipKept::RoomNotOpened));
    claim.note_room_opened(&room).unwrap();
    assert!(claim.zip_due(later).is_ok());

    // A check missing.
    let mut record = claim.record().unwrap().unwrap();
    record.open_checked = false;
    claim.write_record(&record).unwrap();
    assert_eq!(claim.zip_due(later), Err(ZipKept::ChecksMissing));
    record.open_checked = true;
    record.hash_checked = false;
    claim.write_record(&record).unwrap();
    assert_eq!(claim.zip_due(later), Err(ZipKept::ChecksMissing));
    record.hash_checked = true;

    // A failure state.
    record.failure = Some(RecordedFailure::Transient {
        cause: TransientCause::Io,
        step: Step::Done,
        at: now_text(),
    });
    claim.write_record(&record).unwrap();
    assert_eq!(claim.zip_due(later), Err(ZipKept::FailureState));
    record.failure = None;
    claim.write_record(&record).unwrap();

    // An offered file waiting.
    close(install.open_legacy());
    assert_eq!(claim.zip_due(later), Err(ZipKept::OfferWaiting));
    std::fs::remove_file(install.legacy()).unwrap();
    assert_eq!(claim.delete_zip_if_due(later).unwrap().len(), 1);

    // And a claim that never finished keeps its zip for good.
    let install = Install::new();
    close(unlocked(&install));
    claim_cut_at(&install, &account("a"), Cut::AfterCopyRecorded);
    assert_eq!(install.claim().zip_due(later), Err(ZipKept::NotDone));
}

// ---------------------------------------------------------------------------------------------- the probe

/// The probe copies into `scratch/` under the device folder, never beside the file or in the OS temp folder, removes
/// it, and a copy left by a cut is swept at the next launch.
#[test]
fn probe_scratch_is_under_data_dir_and_swept() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let scratch = install.dir().join(SCRATCH_DIR);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let during = seen.clone();
    let folder = scratch.clone();
    at_point(Cut::ProbeCopied, move || {
        for entry in std::fs::read_dir(&folder).unwrap().flatten() {
            during.borrow_mut().push(entry.path());
        }
    });
    let claim = install.claim();
    assert!(matches!(claim.start(&account("a")), Start::Locked(_)));
    let seen = seen.borrow().clone();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!(
        seen[0].starts_with(install.dir()),
        "under the device folder"
    );
    assert!(seen[0].join("bir_data.db").exists() || !seen[0].exists());
    assert!(
        install.names(SCRATCH_DIR).is_empty(),
        "removed after the probe"
    );

    // A probe cut short leaves its copy; the launch sweep removes it.
    std::fs::create_dir_all(scratch.join("probe-1-1")).unwrap();
    std::fs::write(scratch.join("probe-1-1").join("bir_data.db"), b"copy").unwrap();
    claim.sweep();
    assert!(install.names(SCRATCH_DIR).is_empty());
}

/// X2: A3a's legacy mode stops at the probe: a locked file answers Locked with its kinds, and nothing is written.
#[test]
fn the_probe_alone_writes_nothing() {
    let install = Install::new();
    close(helpdesk(&install, "1357", Some("ABCD-EFGH-JKMN-PQRS")));
    let before = install.legacy_bytes();
    assert_eq!(
        probe_legacy(&install.layout).unwrap(),
        LegacyProbe::Locked(CredentialKinds {
            pin: true,
            authenticator: true,
            recovery_key: true
        })
    );
    assert_eq!(install.legacy_bytes(), before);
    assert!(install.claim().record().unwrap().is_none());
    let empty = Install::new();
    assert_eq!(probe_legacy(&empty.layout).unwrap(), LegacyProbe::NoFile);
    close(unlocked(&empty));
    assert_eq!(probe_legacy(&empty.layout).unwrap(), LegacyProbe::Unlocked);
}

// ---------------------------------------------------------------------------------------------- step 6

/// Step 6: the fixups in the room file and in `device.db`, and the room row.
#[test]
fn the_fixups_and_the_room_row() {
    let install = Install::new();
    let db = helpdesk(&install, "1357", None);
    db.set_setting("buwiz_server_url", "https://old.example")
        .unwrap();
    db.set_setting("theme_preference", "Dark").unwrap();
    db.set_setting("log_retention_days", "30").unwrap();
    close(db);
    std::fs::write(
        install.dir().join("bir_data.safety-1700000000.db"),
        b"an import safety copy",
    )
    .unwrap();
    let who = account("owner");
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&who) else {
        panic!("not locked")
    };
    let ready = locked
        .unlock(&claim, LegacyCredential::Pin("1357"))
        .unwrap()
        .with_owner_pin_phc("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".into());
    let room = done_room(&ready.claim(&claim));
    let room_file = install.room_file(&room);
    assert_eq!(setting_of(&room_file, "buwiz_server_url"), None);
    assert_eq!(
        setting_of(&room_file, HAND_TO_CLIENTS_SETTING).as_deref(),
        Some("on")
    );
    assert!(
        setting_of(&room_file, "app_lock_pin_hash").is_some(),
        "old lock keys stay"
    );
    let store = claim.store();
    assert_eq!(
        store.setting("theme_preference").unwrap().as_deref(),
        Some("Dark")
    );
    assert_eq!(
        store.setting("log_retention_days").unwrap().as_deref(),
        Some("30")
    );
    let pin: Option<String> = store
        .conn()
        .query_row(
            "SELECT pin_phc FROM owner_lock WHERE room_id = ?1",
            [room.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(pin.unwrap().starts_with("$argon2id$"));
    let rows = store.rooms().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].from_legacy);
    assert_eq!(rows[0].user_id, "owner");
    assert!(
        install
            .dir()
            .join("rooms")
            .join(room.as_str())
            .join("bir_data.safety-1700000000.db")
            .exists()
    );
    assert!(!install.dir().join("bir_data.safety-1700000000.db").exists());
    let record = claim.record().unwrap().unwrap();
    assert!(record.hash_checked && record.open_checked);
    assert_eq!(record.step, Step::Done);
    assert_eq!(
        record.owner_pin_phc, None,
        "the PIN's hash lives in owner_lock only"
    );

    // Solo: the switch is off; no typed PIN: owner_lock with no PIN, so Set PIN follows.
    let install = Install::new();
    close(solo(&install, None, None));
    let claim = install.claim();
    let room = done_room(&claim_unlocked(&install, &account("solo")));
    assert_eq!(
        setting_of(&install.room_file(&room), HAND_TO_CLIENTS_SETTING).as_deref(),
        Some("off")
    );
    let pin: Option<String> = claim
        .store()
        .conn()
        .query_row(
            "SELECT pin_phc FROM owner_lock WHERE room_id = ?1",
            [room.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pin, None);
}

/// Every cut point, as a kill or a power cut after its fsync: the next launch reaches the same end state.
#[test]
fn a_cut_at_every_step_resumes_to_the_same_end() {
    let reference = {
        let install = Install::new();
        close(solo(&install, None, None));
        let room = done_room(&claim_unlocked(&install, &account("a")));
        counts(&install.room_file(&room))
    };
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
        close(solo(&install, None, None));
        let who = account("a");
        claim_cut_at(&install, &who, point);
        let claim = install.claim();
        let room = match claim.start(&who) {
            Start::Ready(ready) => done_room(&ready.claim(&claim)),
            Start::Finished(outcome) => done_room(&outcome),
            other => panic!("{point:?}: {other:?}"),
        };
        assert_eq!(counts(&install.room_file(&room)), reference, "{point:?}");
        assert!(!install.legacy().exists(), "{point:?}");
        assert_eq!(zips(&install).len(), 1, "{point:?}: one zip");
        assert_eq!(
            claim.record().unwrap().unwrap().step,
            Step::Done,
            "{point:?}"
        );
    }
}

use rusqlite::Connection;

// ---------------------------------------------------------------------------------------------- review 1

/// C1 (review of A2): a record that names a room but never moved its file never finishes on that room's own later
/// file. A's claim is cut after its copy, the legacy file goes, and A then has a room of its own with data: the next
/// launches leave that room exactly as it is (no from_legacy row, no "moved, not opened"), and the record is closed.
#[test]
fn a_stale_record_never_finishes_on_the_accounts_own_room() {
    for closed_first in [true, false] {
        let install = Install::new();
        close(unlocked(&install));
        let a = account("a");
        claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
        // The legacy file goes: moved by hand, or taken by another account's offer.
        std::fs::rename(install.legacy(), install.dir().join("moved-away.db")).unwrap();
        let claim = install.claim();
        if closed_first {
            assert!(matches!(claim.start(&a), Start::Nothing));
            assert_eq!(claim.record().unwrap().unwrap().step, Step::Closed);
        }
        // A gets a room of its own and adds data to it.
        let room = room_of(&claim, &a);
        install.layout.create_room_dir(&room).unwrap();
        let own = Database::open(install.room_file(&room)).unwrap();
        taxpayer(&own, "923456788", "A's own");
        close(own);
        let before = std::fs::read(install.room_file(&room)).unwrap();
        for _ in 0..2 {
            match claim.start(&a) {
                Start::Nothing => {}
                other => panic!("closed first {closed_first}: {other:?}"),
            }
        }
        assert_eq!(
            std::fs::read(install.room_file(&room)).unwrap(),
            before,
            "closed first {closed_first}"
        );
        let record = claim.record().unwrap().unwrap();
        assert_eq!(record.step, Step::Closed, "closed first {closed_first}");
        assert_eq!(record.failure, None, "closed first {closed_first}");
        assert!(
            claim.store().rooms().unwrap().is_empty(),
            "no from_legacy row"
        );
    }
}

/// C2 (review of A2): after a cut between the link and the unlink, another process (an older eBIRForms,
/// bir-headless) may open bir_data.db before this one starts. The resume unlinks the legacy name only under the
/// legacy lock and with no -wal or -shm beside it; otherwise it waits (transient), with both names kept.
#[test]
fn the_link_window_resume_waits_for_another_opener() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    claim_cut_at(&install, &who, Cut::AfterLink);
    let claim = install.claim();
    let room = room_of(&claim, &who);

    // Another program has the file open through its own connection.
    let other = Connection::open(install.legacy()).unwrap();
    other
        .execute_batch("PRAGMA journal_mode = WAL; SELECT count(*) FROM settings;")
        .unwrap();
    match claim.start(&who) {
        Start::Failed(Failure::Transient(transient)) => {
            assert_eq!(transient.cause, TransientCause::WalReturned)
        }
        other => panic!("{other:?}"),
    }
    assert!(
        install.legacy().exists() && install.room_file(&room).exists(),
        "both names kept"
    );
    drop(other);

    // Another process holds the legacy lock.
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let foreign = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(crate::db::live_owner_lock_path(&install.legacy()))
            .unwrap();
        // SAFETY: a valid descriptor.
        assert_eq!(
            unsafe { libc::flock(foreign.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        match claim.start(&who) {
            Start::Failed(Failure::Transient(transient)) => {
                assert_eq!(transient.cause, TransientCause::OwnerLockHeld)
            }
            other => panic!("{other:?}"),
        }
        assert!(install.legacy().exists() && install.room_file(&room).exists());
    }

    // With nobody else on it, the resume finishes.
    match claim.start(&who) {
        Start::Finished(Outcome::Done { room: done, .. }) => assert_eq!(done, room),
        other => panic!("{other:?}"),
    }
    assert!(!install.legacy().exists());
}

/// C3 (review of A2): a full disk at any step is transient (Try again), never "moved, not opened" for this build; the
/// next launch finishes the claim with every row.
#[test]
fn a_full_disk_at_each_step_is_transient() {
    for point in [
        Inject::Zip,
        Inject::Link,
        Inject::Open,
        Inject::Fixups,
        Inject::DeviceRows,
        Inject::OpenCheck,
    ] {
        let install = Install::new();
        close(unlocked(&install));
        let before = counts(&install.legacy());
        let who = account("a");
        let claim = install.claim();
        let Start::Ready(ready) = claim.start(&who) else {
            panic!("{point:?}: not ready")
        };
        INJECT_AT.with(|at| at.set(Some(point)));
        let outcome = ready.claim(&claim);
        INJECT_AT.with(|at| at.set(None));
        match outcome {
            Outcome::Transient(transient) => {
                assert_eq!(transient.cause, TransientCause::DiskFull, "{point:?}")
            }
            other => panic!("{point:?}: {other:?}"),
        }
        let room = match claim.start(&who) {
            Start::Ready(ready) => done_room(&ready.claim(&claim)),
            Start::Finished(outcome) => done_room(&outcome),
            other => panic!("{point:?}: the next launch: {other:?}"),
        };
        assert_eq!(
            counts(&install.room_file(&room))[..3],
            before[..3],
            "{point:?}"
        );
        assert!(!install.legacy().exists(), "{point:?}");
    }
}

/// C4 (review of A2): the owner lock is re-entrant within a process, so it keeps out only other processes. Two claims
/// in one process (the launch resume and the sign-in call) never overlap: the second waits (transient), and the launch
/// sweep leaves a running claim's files alone.
#[test]
fn two_claims_in_one_process_never_overlap() {
    let install = Install::new();
    close(unlocked(&install));
    let who = account("a");
    let first = install.claim();
    let Start::Ready(ready) = first.start(&who) else {
        panic!("not ready")
    };
    let second = install.claim();
    match second.start(&who) {
        Start::Failed(Failure::Transient(transient)) => {
            assert_eq!(transient.cause, TransientCause::ClaimInProgress)
        }
        other => panic!("the second claim ran beside the first: {other:?}"),
    }
    let scratch = install
        .dir()
        .join(SCRATCH_DIR)
        .join("probe-of-the-running-claim");
    std::fs::create_dir_all(&scratch).unwrap();
    second.sweep();
    assert!(scratch.exists(), "the sweep waits for the running claim");
    done_room(&ready.claim(&first));
    assert!(matches!(second.start(&who), Start::Nothing));
    second.sweep();
    assert!(!scratch.exists());
}

/// C5 (review of A2): every try at the legacy unlock is counted in device.db (`pin_attempts('', 'legacy')`, B1's
/// limiter): after 6 wrong ones a wait runs, during which even the right PIN does not open the file, and the count
/// survives a relaunch.
#[test]
fn the_legacy_unlock_is_limited() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let who = account("a");
    let claim = install.claim();
    let Start::Locked(mut locked) = claim.start(&who) else {
        panic!("not locked")
    };
    for (index, wrong) in ["0000", "0001", "0002", "0003", "0004", "0005"]
        .iter()
        .enumerate()
    {
        let refused = locked
            .unlock(&claim, LegacyCredential::Pin(wrong))
            .unwrap_err();
        let wait_started = if index == 5 {
            Some(std::time::Duration::from_secs(60))
        } else {
            None
        };
        assert_eq!(refused.reason(), &RefusedReason::Wrong { wait_started });
        locked = refused.into_inner();
    }
    let refused = locked
        .unlock(&claim, LegacyCredential::Pin("1357"))
        .expect_err("the right PIN during the wait");
    assert!(matches!(refused.reason(), RefusedReason::Waiting(_)));
    // The authenticator code and the recovery key count against the same wait.
    let refused = refused
        .into_inner()
        .unlock(
            &claim,
            LegacyCredential::AuthenticatorCode(&totp_code(TOTP_SECRET)),
        )
        .expect_err("a right code during the wait");
    assert!(matches!(refused.reason(), RefusedReason::Waiting(_)));
    drop(refused);
    assert!(install.legacy().exists());
    // Another launch: the wait is still running.
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&who) else {
        panic!("not locked")
    };
    let refused = locked
        .unlock(&claim, LegacyCredential::Pin("1357"))
        .expect_err("the count survives a relaunch");
    assert!(matches!(refused.reason(), RefusedReason::Waiting(_)));
}

/// C8 (review of A2): a symlinked bir_data.db is a permanent failure at the probe; the link and its target are left
/// as they are.
#[cfg(unix)]
#[test]
fn a_symlinked_legacy_file_is_never_claimed() {
    let install = Install::new();
    let elsewhere = install.dir().join("elsewhere.db");
    let db = Database::open(&elsewhere).unwrap();
    taxpayer(&db, "123456788", "Behind A Link");
    close(db);
    std::os::unix::fs::symlink("elsewhere.db", install.legacy()).unwrap();
    let target = std::fs::read(&elsewhere).unwrap();
    let claim = install.claim();
    match claim.start(&account("a")) {
        Start::Failed(Failure::Permanent(permanent)) => {
            assert_eq!(permanent.reason, PermanentReason::Symlink)
        }
        other => panic!("{other:?}"),
    }
    assert!(
        std::fs::symlink_metadata(install.legacy())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&elsewhere).unwrap(), target);
    assert!(!install.layout.rooms_dir().exists());
}

/// C9 (review of A2): the owner PIN's hash is never in a `Debug` print of the token or the record.
#[test]
fn the_owner_pin_hash_is_never_printed() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&account("a")) else {
        panic!("not locked")
    };
    let phc = "$argon2id$v=19$m=19456,t=2,p=1$c2VjcmV0c2FsdA$c2VjcmV0aGFzaA";
    let ready = locked
        .unlock(&claim, LegacyCredential::Pin("1357"))
        .unwrap()
        .with_owner_pin_phc(phc.into());
    assert!(!format!("{ready:?}").contains("c2VjcmV0"), "{ready:?}");
    let record = Record {
        owner_pin_phc: Some(PinPhc(phc.into())),
        ..Record::default()
    };
    assert!(!format!("{record:?}").contains("c2VjcmV0"));
    assert!(
        serde_json::to_string(&record).unwrap().contains(phc),
        "stored as it is"
    );
}

/// N1 (delta review of A2): ext4 and f2fs reuse freed inode numbers. A's claim is cut after its copy, the legacy file
/// is deleted, and A's new room file gets the freed inode, so its identity matches the recorded one: the room is still
/// never finished on, because it is not byte for byte the raw file.
#[test]
fn a_reused_inode_never_finishes_on_the_accounts_own_room() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
    std::fs::remove_file(install.legacy()).unwrap();
    let claim = install.claim();
    let room = room_of(&claim, &a);
    install.layout.create_room_dir(&room).unwrap();
    let own = Database::open(install.room_file(&room)).unwrap();
    taxpayer(&own, "923456788", "A's own");
    close(own);
    let before = std::fs::read(install.room_file(&room)).unwrap();
    SAME_ID.with(|same| same.set(true));
    let started = claim.start(&a);
    SAME_ID.with(|same| same.set(false));
    assert!(matches!(started, Start::Nothing), "{started:?}");
    assert_eq!(std::fs::read(install.room_file(&room)).unwrap(), before);
    let record = claim.record().unwrap().unwrap();
    assert_eq!(record.step, Step::Closed);
    assert_eq!(record.failure, None);
}

/// PLAN §A.8 reviewer pass, M1: a read error in the Copied window (an EIO, a sharing violation) is transient,
/// never a permanent close: the record stays at Copied, and a later run with the error gone finishes the move.
#[test]
fn a_read_error_at_the_room_check_is_transient_not_closed() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
    let claim = install.claim();
    let room = room_of(&claim, &a);
    install.layout.create_room_dir(&room).unwrap();
    // A move the record never learned of: the legacy file is the room file now.
    std::fs::rename(install.legacy(), install.room_file(&room)).unwrap();
    ROOM_CHECK_ERR.with(|err| err.set(true));
    let started = claim.start(&a);
    assert!(
        matches!(started, Start::Failed(Failure::Transient(_))),
        "not transient: {started:?}"
    );
    let record = claim.record().unwrap().unwrap();
    assert_eq!(record.step, Step::Copied, "the record must stay at Copied");
    assert!(
        !matches!(record.step, Step::Closed),
        "a read error must never close the claim"
    );
    let finished = claim.start(&a);
    assert!(
        matches!(finished, Start::Finished(Outcome::Done { .. })),
        "the move does not finish once the error is gone: {finished:?}"
    );
}

/// PLAN §A.8 reviewer pass, L1: a kill or a dropped ReadyClaim between the used mark's write and the Unlocked
/// record must not hide the computer login for the rest of that claim: the next start() offers it again.
#[test]
fn computer_login_offered_again_when_the_unlock_died_before_the_record() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(locked.offers_computer_login(&claim), "offered first");
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .unwrap();
    // The kill window: the mark is written, the Unlocked record never is.
    drop(ready);
    assert!(
        claim.os_login_used().unwrap(),
        "the mark was written at the unlock"
    );
    let claim = install.claim();
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "the computer login must be offered again: the claim it was spent in never began"
    );
}

// ------------------------------------------------------------------------- T1: the used-mark clears, one test each

fn write_used_mark(claim: &LegacyClaim) {
    claim
        .set_migration_value(OS_LOGIN_USED_KEY, &now_text())
        .unwrap();
    assert!(claim.os_login_used().unwrap(), "the mark is set");
}

/// T1: the used mark is gone after the claim closes (nothing to claim).
#[test]
fn the_used_mark_is_cleared_at_closed() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
    std::fs::remove_file(install.legacy()).unwrap();
    let claim = install.claim();
    write_used_mark(&claim);
    let started = claim.start(&a);
    assert!(matches!(started, Start::Nothing), "{started:?}");
    assert_eq!(claim.record().unwrap().unwrap().step, Step::Closed);
    assert!(
        !claim.os_login_used().unwrap(),
        "the used mark is cleared at Closed"
    );
}

/// T1: the used mark is gone after clear_permanent.
#[test]
fn the_used_mark_is_cleared_in_clear_permanent() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
    let claim = install.claim();
    claim
        .record_permanent(&Permanent {
            reason: PermanentReason::NotPlainSqlite,
            detail: "a test's permanent failure".into(),
        })
        .unwrap();
    write_used_mark(&claim);
    claim.clear_permanent().unwrap();
    assert!(
        !claim.os_login_used().unwrap(),
        "the used mark is cleared in clear_permanent"
    );
}

/// T1: the used mark is gone after record_permanent.
#[test]
fn the_used_mark_is_cleared_in_record_permanent() {
    let install = Install::new();
    close(unlocked(&install));
    let claim = install.claim();
    write_used_mark(&claim);
    claim
        .record_permanent(&Permanent {
            reason: PermanentReason::NotPlainSqlite,
            detail: "a test's permanent failure".into(),
        })
        .unwrap();
    assert!(
        !claim.os_login_used().unwrap(),
        "the used mark is cleared in record_permanent"
    );
}

/// R1: a room's database is never opened before the claim's start() ran in this session — a migration or a
/// checkpoint would change the bytes the move's hash check still needs. The open is refused before start(),
/// the claim then still finishes, and the open is allowed after.
#[test]
fn a_room_open_is_refused_before_start_and_the_claim_still_finishes() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    let claim = install.claim();
    let room = room_of(&claim, &a);
    install.layout.create_room_dir(&room).unwrap();
    let before = crate::rooms::open_room_guarded(&install.layout, &room);
    assert!(
        before.is_err(),
        "the room opened before the claim's start() ran"
    );
    let outcome = claim_unlocked(&install, &a);
    assert_eq!(done_room(&outcome), room);
    crate::rooms::open_room_guarded(&install.layout, &room)
        .expect("the room opens once the claim ran");
}

// ------------------------------------------------------------------------- L1, the orchestrator's ruling

/// The mark protects nothing and can strand a person: after a kill during the copy (the record is
/// at Unlocked, no failure), the resume needs a new unlock — and the button must be offered.
#[test]
fn a_cut_at_unlocked_offers_the_computer_login_again() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    // A kill during the copy: the record got to Unlocked, no failure, the legacy file in place.
    locked_claim_cut_at(&install, &a, "1357", Cut::AfterCopyRecorded);
    let mut record = claim.record().unwrap().unwrap();
    record.step = Step::Unlocked;
    claim.write_record(&record).unwrap();
    write_used_mark(&claim);
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "the button stayed hidden after a kill during the copy"
    );
    locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .expect("the computer login opens the resume");
}

/// After a kill before the move (the record at Copied, the legacy file present), the resume needs
/// a new unlock — and the button must be offered.
#[test]
fn a_cut_at_copied_with_the_legacy_file_present_offers_the_computer_login_again() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    locked_claim_cut_at(&install, &a, "1357", Cut::AfterCopyRecorded);
    assert!(install.legacy().exists());
    write_used_mark(&claim);
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(
        locked.offers_computer_login(&claim),
        "the button stayed hidden after a kill before the move"
    );
    locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .expect("the computer login opens the resume");
}

/// The refusal that remains: a second use within one live claim, no start() between the uses. The
/// mark is written at the first use, and until the claim ends the button is not offered again.
#[test]
fn a_second_use_within_one_live_claim_is_refused() {
    let install = Install::new();
    close(helpdesk(&install, "1357", None));
    let claim = install.claim();
    let a = account("a");
    let Start::Locked(locked) = claim.start(&a) else {
        panic!("not locked")
    };
    assert!(locked.offers_computer_login(&claim), "offered first");
    let ready = locked
        .unlock(&claim, LegacyCredential::ComputerLogin)
        .unwrap();
    // The claim is live: no start() since the use. The mark stands, so the page does not offer the
    // button again and the credential is refused while this claim lives.
    assert!(
        claim.os_login_used().unwrap(),
        "the mark must stand within the live claim"
    );
    assert!(
        !claim.offers_computer_login_standing(),
        "a second use within one live claim must be refused"
    );
    drop(ready);
}

// ------------------------------------------------------------------------- R1, the reviewer's holes

/// R1(a): a start() that answers a transient failure never opens the room of the record. In M1's
/// own case — the record at Copied, the legacy file gone, the room read failing — opening the room
/// would migrate its bytes and close the move unfinished: the open is refused.
#[test]
fn a_transient_room_check_does_not_open_the_room() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterCopyRecorded);
    let claim = install.claim();
    let room = room_of(&claim, &a);
    install.layout.create_room_dir(&room).unwrap();
    std::fs::rename(install.legacy(), install.room_file(&room)).unwrap();
    // The cut claim ran in a previous session — a kill: this session has answered nothing yet.
    crate::legacy_claim::clear_started_for_test(install.layout.device_dir());
    ROOM_CHECK_ERR.with(|err| err.set(true));
    let started = claim.start(&a);
    assert!(
        matches!(started, Start::Failed(Failure::Transient(_))),
        "not transient: {started:?}"
    );
    assert!(
        !crate::legacy_claim::claim_started_this_session(install.layout.device_dir()),
        "a transient answer must not open the room"
    );
    let refused =
        std::panic::catch_unwind(|| crate::rooms::open_room_guarded(&install.layout, &room));
    match refused {
        Ok(result) => assert!(result.is_err(), "the room opened after a transient answer"),
        Err(_) => {} // the debug_assert fired: refused either way
    }
    // The error gone, the claim finishes — and now the room opens.
    let finished = claim.start(&a);
    assert!(
        matches!(finished, Start::Finished(Outcome::Done { .. })),
        "{finished:?}"
    );
    crate::rooms::open_room_guarded(&install.layout, &room)
        .expect("the room opens once the claim has a definitive answer");
}

/// R1(d): the set's key is canonical — a device folder reached through a symlinked parent is the
/// same folder, so the owner is never refused their own room.
#[test]
fn a_symlinked_device_folder_is_the_same_for_the_guard() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_unlocked(&install, &a);
    assert!(crate::legacy_claim::claim_started_this_session(
        install.layout.device_dir()
    ));
    // The same folder through a symlinked parent.
    let link = install.dir().parent().unwrap().join("a3a-linked-data");
    std::os::unix::fs::symlink(install.dir(), &link).unwrap();
    assert!(
        crate::legacy_claim::claim_started_this_session(&link),
        "the symlinked path is not the same folder for the guard"
    );
    std::fs::remove_file(&link).unwrap();
}

/// R1(b): the guard sits at the choke point every room open in the app passes — `open_live_database`
/// behind `app_database_path` — not only in `open_room_guarded`. An open through the app's real path
/// before start() is refused; once the claim has run, the same open goes through.
#[test]
fn the_apps_real_open_path_is_refused_before_start() {
    let _turn = crate::db::room_path_test_turn();
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    let claim = install.claim();
    let room = room_of(&claim, &a);
    install.layout.create_room_dir(&room).unwrap();
    crate::db::set_open_room_database(Some(install.room_file(&room)));
    let before = std::panic::catch_unwind(crate::db::open_live_database);
    crate::db::set_open_room_database(None);
    match before {
        Ok(result) => assert!(
            result.is_err(),
            "the room opened through the app's real path before start() ran"
        ),
        Err(_) => {} // the debug_assert fired: refused either way
    }
    claim_unlocked(&install, &a);
    crate::db::set_open_room_database(Some(install.room_file(&room)));
    let after = crate::db::open_live_database();
    crate::db::set_open_room_database(None);
    after.expect("the room opens through the app's real path once the claim ran");
}

/// The widened guard (the orchestrator's ruling): a record at Moved but not yet HashChecked — step 7's
/// hash evidence is still at stake — refuses the room open through the app's real path, even though
/// start() ran to a definitive answer in this session.
#[test]
fn a_room_cut_at_moved_refuses_the_open_through_the_apps_path() {
    let _turn = crate::db::room_path_test_turn();
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    let claim = install.claim();
    let room = room_of(&claim, &a);
    claim_cut_at(&install, &a, Cut::AfterMoveRecorded);
    let record = claim.record().unwrap().expect("a record");
    assert_eq!(record.step, Step::Moved, "the cut did not land at Moved");
    // The started guard passes: start() answered Ready — definitive — in this session.
    assert!(crate::legacy_claim::claim_started_this_session(
        install.layout.device_dir()
    ));
    crate::db::set_open_room_database(Some(install.room_file(&room)));
    let refused = crate::db::open_live_database();
    crate::db::set_open_room_database(None);
    assert!(
        refused.is_err(),
        "the room opened while the record is at Moved with no hash check"
    );
}

/// Item 6 (the ruling): a start() that finishes into a transient outcome after the move is not a
/// definitive answer — the folder must not open rooms on its word alone.
#[test]
fn a_transient_finish_after_the_move_does_not_fill_the_started_set() {
    let install = Install::new();
    close(unlocked(&install));
    let a = account("a");
    claim_cut_at(&install, &a, Cut::AfterMoveRecorded);
    // The resume is a new session (the cut was the kill): nothing answered yet.
    crate::legacy_claim::clear_started_for_test(install.layout.device_dir());
    let claim = install.claim();
    INJECT_AT.with(|at| at.set(Some(Inject::DeviceRows)));
    let resumed = claim.start(&a);
    assert!(
        matches!(resumed, Start::Finished(Outcome::Transient(_))),
        "not a transient finish: {resumed:?}"
    );
    assert!(
        !crate::legacy_claim::claim_started_this_session(install.layout.device_dir()),
        "a transient finish after the move must not fill the set"
    );
}
