//! A3a: the migration verbs the page calls (PLAN §A.4). The machine in [`crate::session`] owns
//! the room, the claim and the migration state — what lives here is only the TSX-facing surface:
//! the state read, Try again, the offer's answer, the sign-out confirm's count and the Settings
//! safety-copy row, each reading the machine's snapshot or driving its verbs.

use bir_core::legacy_claim::LegacyClaim;
use bir_core::rooms::Layout;

use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

pub const DECLARATIONS: &str = r#"
/** The migration state the sign-in flow shows a page for after session_opened. */
export function migration_state(): {
    kind: "none" | "opening" | "moving" | "transient" | "permanent" | "moved_not_opened" | "offered";
    copy_saved: boolean;
    detail: string;
};
/** The transient page's Try again: the claim and the room open run again for the saved sign-in. */
export function migration_try_again(): void;
/** The offer page's answer: `add` runs the whole-profile export of the found file and imports it into
 *  the room, then the file is renamed aside; `false` leaves it for the next launch. */
export function migration_offer_answer(add: boolean): void;
/** How many returns of the open room are Queued or sending now — the sign-out confirm's count, no names. */
export function returns_waiting_count(): number;
/** The pre-rooms safety copy of the open room's claim, for Settings › Import / Export: kept-until
 *  date (unix), or the reason it is kept, or none when there is no copy. */
export function safety_copy_status(): { kind: "none" } | { kind: "until"; stamp: number } | { kind: "kept"; reason: string };
"#;

/// Register the migration verbs (the moving and failure pages, the offer, the sign-out count, the
/// Settings safety-copy row).
pub fn register(module: HostModule) -> HostModule {
    module
        .function("migration_state", |_| migration_state_value())
        .function("migration_try_again", |_| migration_try_again())
        .function("migration_offer_answer", migration_offer_answer)
        .function("returns_waiting_count", |_| returns_waiting_count())
        .function("safety_copy_status", |_| safety_copy_status())
}

fn migration_state_value() -> HostResult {
    // §1: the state is computed from the phase — never a state of its own. The offer's binding
    // needs no comparison here: the note IS in the phase, so a close or a replace ends it (X2
    // by construction). DeviceStore is §7's, wired in commit 5; the page's union has no kind
    // for it yet, and no path produces it today.
    let (kind, copy_saved, detail) = match crate::session::migration_state() {
        None => ("none", false, String::new()),
        Some(crate::session::MigrationState::Moving) => ("moving", false, String::new()),
        Some(crate::session::MigrationState::Opening) => ("opening", false, String::new()),
        Some(crate::session::MigrationState::Transient { copy_saved, detail }) => {
            ("transient", copy_saved, detail)
        }
        Some(crate::session::MigrationState::Permanent { detail }) => ("permanent", false, detail),
        Some(crate::session::MigrationState::MovedNotOpened { detail }) => {
            ("moved_not_opened", false, detail)
        }
        Some(crate::session::MigrationState::DeviceStore { detail }) => {
            ("transient", false, detail)
        }
        Some(crate::session::MigrationState::Offered) => ("offered", false, String::new()),
    };
    Ok(HostObject::new()
        .field("kind", kind)
        .field("copy_saved", copy_saved)
        .field("detail", detail)
        .into())
}

/// The transient page's Try again: the open begins again for the saved sign-in (§3). The page
/// re-reads migration_state after it.
fn migration_try_again() -> HostResult {
    let stored = bir_core::cloud_account::load_stored_account()
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new("no one is signed in"))?;
    let owner = crate::session::Owner::of_stored(&stored)
        .ok_or_else(|| HostError::new("the sign-in is not a room owner's"))?;
    crate::session::begin_open(
        owner,
        crate::bir_module::this_engine(),
        crate::session::OpenWhy::TryAgain,
    );
    Ok(HostValue::Null)
}

/// §A.3.6: the offer's answer. Add runs the existing whole-profile export on the found file and
/// imports the archive into the open room, then renames the file to `bir_data.offered-<stamp>.db`;
/// Not now leaves it, and the offer shows again at the next launch.
fn migration_offer_answer(arguments: &HostArguments) -> HostResult {
    let add = arguments.boolean(0)?;
    if !add {
        crate::session::clear_offer_note();
        return Ok(HostValue::Null);
    }
    // X2: the answer runs only for the account the offer was made to, in the room it was made in.
    // The note is in the phase, so the snapshot's one read binds them by construction: an offer
    // note present means this open room IS the room it was made in.
    let snap = crate::session::snapshot();
    let bound = matches!(snap.note, crate::session::Note::Offered { .. }) && snap.room.is_some();
    if !bound {
        return Err(HostError::new(
            "the offer was not made to the account and room open now; the offer is the account's it was made to",
        ));
    }
    let layout = Layout::here();
    let legacy = layout.legacy_database();
    if !legacy.is_file() {
        crate::session::clear_offer_note();
        return Err(HostError::new("the file that was offered is gone"));
    }
    let stored = bir_core::cloud_account::load_stored_account()
        .map_err(|error| HostError::new(error.to_string()))?
        .ok_or_else(|| HostError::new("no one is signed in"))?;
    let owner = crate::session::Owner::of_stored(&stored)
        .ok_or_else(|| HostError::new("the sign-in is not a room owner's"))?;
    if snap.owner.as_ref() != Some(&owner) {
        return Err(HostError::new(
            "the signed-in account changed; the offer is the account's it was made to",
        ));
    }
    let claim = LegacyClaim::at(layout).map_err(|error| HostError::new(error.to_string()))?;
    let claimant = bir_core::legacy_claim::Claimant {
        server_url: stored.server_url,
        user_id: stored.user_id,
        email: stored.email,
        practice: stored.practice,
    };
    let failure_detail = |failure: bir_core::legacy_claim::Failure| match failure {
        bir_core::legacy_claim::Failure::Transient(transient) => transient.detail,
        bir_core::legacy_claim::Failure::Permanent(permanent) => permanent.detail,
    };
    let offer = match claim.start(&claimant) {
        bir_core::legacy_claim::Start::Offer(bir_core::legacy_claim::OfferStart::Open(offer)) => {
            offer
        }
        bir_core::legacy_claim::Start::Offer(bir_core::legacy_claim::OfferStart::Locked(_)) => {
            return Err(HostError::new(
                "This file is locked with a TIN and PIN. Moving locked files comes in a later version; choose Not now.",
            ));
        }
        bir_core::legacy_claim::Start::Failed(failure) => {
            return Err(HostError::new(failure_detail(failure)));
        }
        _ => {
            crate::session::clear_offer_note();
            return Err(HostError::new("the file that was offered is gone"));
        }
    };
    let (handle, ..) = snap
        .room
        .ok_or_else(|| HostError::new("no room is open to add them to"))?;
    let room = bir_core::lock_timing::lock(&handle)
        .map_err(|_| HostError::new("database lock poisoned"))?;
    match offer.add(&claim, &room) {
        Ok(_) => {
            crate::session::clear_offer_note();
            Ok(HostValue::Null)
        }
        Err(failure) => Err(HostError::new(failure_detail(failure))),
    }
}

/// The sign-out confirm's count (§A.5): the open room's Queued returns, plus any send in flight —
/// a number, never a name. Zero while no room is open, so the confirm does not show.
fn returns_waiting_count() -> HostResult {
    let count = match crate::session::snapshot().room {
        None => 0,
        Some((handle, _, _, _)) => {
            let guard = match bir_core::lock_timing::lock(&handle) {
                Ok(guard) => guard,
                Err(_) => return Ok(HostValue::Number(0.0)),
            };
            let queued = guard.count_queued_drafts().unwrap_or(0);
            let open_prefix = crate::open_room::job_key_prefix();
            let sending = bir_core::background_cron::get_active_jobs()
                .lock()
                .map(|jobs| {
                    jobs.iter()
                        .filter(|key| {
                            bir_core::background_cron::room_of_job_key(key) == open_prefix
                        })
                        .count() as i64
                })
                .unwrap_or(0);
            queued + sending
        }
    };
    // X6: a plain number, as the declaration says and the page compares — the object it used to
    // return made `waiting > 0` false forever, so the sign-out count and the notice never showed.
    Ok(HostValue::Number(count as f64))
}

/// The Settings row's copy state (§A.3.8): the pre-rooms zip of this room's claim — kept until the
/// date the rule lets it go, or "kept" with the reason when the rule already keeps it for good.
fn safety_copy_status() -> HostResult {
    let layout = Layout::here();
    let claim = match LegacyClaim::at(layout) {
        Ok(claim) => claim,
        Err(_) => return Ok(HostObject::new().field("kind", "none").into()),
    };
    let record = match claim.record() {
        Ok(Some(record)) => record,
        _ => return Ok(HostObject::new().field("kind", "none").into()),
    };
    // X5: the record is the device's one claim, but the row belongs in this account's row only
    // while the move it names IS this room — otherwise the person reads "safety copy kept" for
    // another account's move.
    let open_room = crate::session::snapshot()
        .room
        .and_then(|(_, _, room, _)| room.map(|room| room.as_str().to_owned()));
    if record.room.as_deref() != open_room.as_deref() {
        return Ok(HostObject::new().field("kind", "none").into());
    }
    let Some(zip) = &record.zip else {
        return Ok(HostObject::new().field("kind", "none").into());
    };
    if !std::path::Path::new(zip).is_file() {
        return Ok(HostObject::new().field("kind", "none").into());
    }
    // The rule's keep-until: the claim's done stamp plus ZIP_KEEP_DAYS. Reasons it is kept for good
    // read off the rule itself.
    match claim.zip_due(chrono::Utc::now()) {
        Ok(_) => Ok(HostObject::new().field("kind", "none").into()),
        Err(bir_core::legacy_claim::ZipKept::TooSoon { until }) => Ok(HostObject::new()
            .field("kind", "until")
            .field("stamp", until.timestamp())
            .into()),
        Err(reason) => Ok(HostObject::new()
            .field("kind", "kept")
            .field("reason", format!("{reason:?}"))
            .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bir_core::db::Database;
    use bir_core::legacy_claim::{Record, Step};
    use bir_core::profile::TaxpayerType;
    use bir_core::rooms::Layout;
    use bir_core::tax_profile::{BranchFacts, NewTaxpayer};
    use gpui_shell::{HostArguments, HostModule, HostResult, HostValue};

    use super::register;

    fn add_taxpayer(db: &Database, root: &str, name: &str) {
        db.create_taxpayer(NewTaxpayer {
            tin_root: root.into(),
            name: name.into(),
            taxpayer_type: TaxpayerType::Corporation,
            birth_date: None,
            head_office: BranchFacts {
                rdo_code: "040".into(),
                registered_address: "1 Example Street, Pasig".into(),
                zip_code: "1600".into(),
                phone: "0280000000".into(),
                email: "office@example.com".into(),
            },
        })
        .unwrap();
    }

    struct OfferScratch {
        _turn: std::sync::MutexGuard<'static, ()>,
        layout: Layout,
        room: Arc<Mutex<Database>>,
        _folder: tempfile::TempDir,
    }

    fn offer_scratch() -> OfferScratch {
        let _turn = crate::session::test_turn();
        crate::session::CLAIM_SPAWN_SUPPRESSED.store(true, std::sync::atomic::Ordering::SeqCst);
        crate::open_room::reset_for_tests();
        crate::session::reset_for_tests();
        bir_core::db::set_open_room_database(None);
        crate::account_gate::set_unlocked(false);
        bir_core::account_session::clear_stored();
        bir_core::account_session::reset_owner_cache_for_tests();
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        let layout = Layout::at(folder.path().to_path_buf());
        std::fs::create_dir_all(layout.device_dir()).unwrap();
        drop(bir_core::rooms::DeviceStore::open(&layout).unwrap());

        let server_url = "https://buwiz.example.test";
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: server_url.into(),
            access_token: "access-a".into(),
            refresh_token: "refresh-a".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-a".into(),
            email: Some("user-a@example.test".into()),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        bir_core::account_session::save_stored(&stored).unwrap();
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(&stored));
        let owner = crate::session::Owner::of_stored(&stored).unwrap();
        let room_id = bir_core::rooms::room_id_for(&owner.origin, &owner.user_id);
        std::fs::create_dir_all(layout.room_dir(&room_id)).unwrap();
        let room_path = layout.room_database(&room_id);
        let room_db = Database::open(&room_path).unwrap();
        add_taxpayer(&room_db, "123456788", "Room Owner");
        let room = Arc::new(Mutex::new(room_db));

        let legacy_db = Database::open(layout.legacy_database()).unwrap();
        add_taxpayer(&legacy_db, "723456788", "Offered Profile");
        legacy_db.close().unwrap();

        let record = Record {
            stamp: "2026-11-28T00:00:00Z".into(),
            room: Some(room_id.as_str().into()),
            origin: Some(owner.origin.clone()),
            user_id: Some(owner.user_id.clone()),
            email: stored.email.clone(),
            step: Step::Done,
            done_at: Some("2026-11-28T00:00:00Z".into()),
            ..Record::default()
        };
        let conn = rusqlite::Connection::open(layout.device_db()).unwrap();
        conn.execute(
            "INSERT INTO migration (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                bir_core::legacy_claim::RECORD_KEY,
                serde_json::to_string(&record).unwrap()
            ],
        )
        .unwrap();
        drop(conn);

        crate::session::begin_open(owner, 1, crate::session::OpenWhy::SignIn);
        let epoch = crate::session::snapshot().epoch;
        crate::session::commit(
            epoch,
            crate::session::Target::Room {
                room_id: room_id.clone(),
                note: crate::session::Note::Offered {
                    room: room_id.clone(),
                },
                count_opened: false,
            },
            Some((room.clone(), room_path)),
        );
        assert_eq!(
            crate::session::snapshot().phase_kind,
            crate::session::PhaseKind::Open
        );
        OfferScratch {
            _turn,
            layout,
            room,
            _folder: folder,
        }
    }

    fn offer_answer(add: bool) -> HostResult {
        let module = register(HostModule::new("bir"));
        module.call(
            "migration_offer_answer",
            &HostArguments::new([HostValue::Bool(add)]),
        )
    }

    /// B2 through the real migration verb: the rename fails after the import committed; the file is
    /// offered again, but the second Add consumes the same offer without importing it again.
    #[test]
    fn b2_a_failed_offer_rename_does_not_import_the_file_twice() {
        let scratch = offer_scratch();
        bir_core::legacy_claim::fail_next_offer_rename_for_test();
        let first = offer_answer(true).expect_err("the injected rename failure");
        assert!(
            first
                .message()
                .contains("injected offered-file rename failure"),
            "{}",
            first.message()
        );
        assert!(scratch.layout.legacy_database().is_file());
        {
            let room = scratch.room.lock().unwrap();
            assert!(
                room.get_profile_by_tin("72345678800000").unwrap().is_some(),
                "positive control: the first import committed"
            );
            room.delete_profile("72345678800000").unwrap();
        }

        offer_answer(true).expect("the repeated offer is consumed idempotently");
        assert!(!scratch.layout.legacy_database().exists());
        let room = scratch.room.lock().unwrap();
        assert!(
            room.get_profile_by_tin("72345678800000").unwrap().is_none(),
            "the second Add imported the consumed offer again"
        );
        assert!(
            room.get_profile_by_tin("12345678800000").unwrap().is_some(),
            "the room's own profile stays"
        );
        let offered = std::fs::read_dir(scratch.layout.device_dir())
            .unwrap()
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("bir_data.offered-")
            });
        assert!(offered, "the rename eventually moved the legacy file aside");
        assert_eq!(
            crate::session::migration_state(),
            None,
            "the offer note cleared: {:?}",
            crate::session::migration_state()
        );
        drop(room);
        drop(scratch);
    }
}
