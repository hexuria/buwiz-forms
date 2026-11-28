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
    kind: "none" | "opening" | "moving" | "transient" | "permanent" | "moved_not_opened" | "offered" | "device_store";
    copy_saved: boolean;
    detail: string;
};
/** The transient page's Try again: the claim and the room open run again for the saved sign-in. */
export function migration_try_again(): void;
/** The offer page's answer: `no` marks the file not this account's; `not_now` leaves it for the
 *  next launch. `add` is refused in this version. */
export function migration_offer_answer(answer: string): void;
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
    // by construction). DeviceStore is §7's store-error page.
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
            ("device_store", false, detail)
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

/// §A.3.6: the offer's answer. Add is out of reach in this version; Not now leaves the file
/// where it is, and No marks it not this account's without opening or moving it.
fn claim_read_error(error: bir_core::rooms::DeviceStoreError) -> HostError {
    tracing::warn!("migration offer record not read: {error}");
    HostError::new("the migration offer cannot be read")
}

fn offer_answer_error(error: bir_core::rooms::DeviceStoreError) -> HostError {
    tracing::warn!("migration offer answer not saved: {error}");
    HostError::new("the offer answer cannot be saved")
}

fn migration_offer_answer(arguments: &HostArguments) -> HostResult {
    let answer = arguments.string(0)?;
    match answer {
        "not_now" => {
            crate::session::clear_offer_note();
            return Ok(HostValue::Null);
        }
        "add" => return Err(HostError::new("not available in this version")),
        "no" => {}
        _ => return Err(HostError::new("the offer answer is not_now, no, or add")),
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
    let claim = LegacyClaim::at(layout).map_err(claim_read_error)?;
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
    match claim.start(&claimant) {
        bir_core::legacy_claim::Start::Offer(bir_core::legacy_claim::OfferStart::Open(offer)) => {
            offer.not_mine(&claim).map_err(offer_answer_error)?;
        }
        bir_core::legacy_claim::Start::Offer(bir_core::legacy_claim::OfferStart::Locked(offer)) => {
            offer.not_mine(&claim).map_err(offer_answer_error)?;
        }
        bir_core::legacy_claim::Start::Failed(failure) => {
            return Err(HostError::new(failure_detail(failure)));
        }
        _ => {
            crate::session::clear_offer_note();
            return Err(HostError::new("the file that was offered is gone"));
        }
    }
    crate::session::clear_offer_note();
    Ok(HostValue::Null)
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

    use super::{claim_read_error, offer_answer_error, register};

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

    fn write_stored(user_id: &str) -> bir_core::cloud_account::StoredAccount {
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: format!("access-{user_id}"),
            refresh_token: format!("refresh-{user_id}"),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: user_id.into(),
            email: Some(format!("{user_id}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        bir_core::account_session::save_stored(&stored).unwrap();
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(&stored));
        stored
    }

    struct OfferScratch {
        _turn: std::sync::MutexGuard<'static, ()>,
        layout: Layout,
        room: Arc<Mutex<Database>>,
        room_id: bir_core::rooms::RoomId,
        room_path: std::path::PathBuf,
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
            Some((room.clone(), room_path.clone())),
        );
        assert_eq!(
            crate::session::snapshot().phase_kind,
            crate::session::PhaseKind::Open
        );
        OfferScratch {
            _turn,
            layout,
            room,
            room_id,
            room_path,
            _folder: folder,
        }
    }

    fn offer_answer(answer: &str) -> HostResult {
        let module = register(HostModule::new("bir"));
        module.call(
            "migration_offer_answer",
            &HostArguments::new([HostValue::Str(answer.into())]),
        )
    }

    fn sha256_path(path: &std::path::Path) -> String {
        use sha2::Digest;
        let mut file = std::fs::File::open(path).unwrap();
        let mut hash = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hash).unwrap();
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn rewrite_claim_record(scratch: &OfferScratch, rewrite: impl FnOnce(&mut Record)) {
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        let raw: String = conn
            .query_row(
                "SELECT value FROM migration WHERE key = ?1",
                [bir_core::legacy_claim::RECORD_KEY],
                |row| row.get(0),
            )
            .unwrap();
        let mut record: Record = serde_json::from_str(&raw).unwrap();
        rewrite(&mut record);
        conn.execute(
            "UPDATE migration SET value = ?1 WHERE key = ?2",
            rusqlite::params![
                serde_json::to_string(&record).unwrap(),
                bir_core::legacy_claim::RECORD_KEY
            ],
        )
        .unwrap();
    }

    fn copy_legacy_to_a_new_inode(scratch: &OfferScratch) {
        let before = bir_core::safe_fs::file_id(&scratch.layout.legacy_database()).unwrap();
        let bytes = std::fs::read(scratch.layout.legacy_database()).unwrap();
        std::fs::remove_file(scratch.layout.legacy_database()).unwrap();
        std::fs::write(scratch.layout.legacy_database(), bytes).unwrap();
        assert_ne!(
            bir_core::safe_fs::file_id(&scratch.layout.legacy_database()).unwrap(),
            before,
            "positive control: the reappeared copy has a new file identity"
        );
    }

    fn sign_b_in(_scratch: &OfferScratch) {
        crate::session::sign_out();
        crate::session::permit_claim_threads();
        let stored_b = write_stored("user-b");
        let owner_b = crate::session::Owner::of_stored(&stored_b).unwrap();
        crate::session::begin_open(owner_b, 1, crate::session::OpenWhy::SignIn);
        crate::session::join_claim_threads();
    }

    /// Add is out of reach through the real migration verb. The file stays in place and nothing
    /// enters the open room.
    #[test]
    fn b2removal_add_is_refused_through_the_real_offer_verb() {
        let scratch = offer_scratch();
        let error = offer_answer("add").expect_err("Add must be refused");
        assert_eq!(error.message(), "not available in this version");
        assert!(scratch.layout.legacy_database().is_file());
        let room = scratch.room.lock().unwrap();
        assert!(
            room.get_profile_by_tin("72345678800000").unwrap().is_none(),
            "the refused Add imported the offered profile"
        );
    }

    /// No is the real not-mine answer: no import, no move, no change to the file, and the offer note ends.
    #[test]
    fn b2removal_no_marks_the_offer_not_mine_without_touching_the_file() {
        let scratch = offer_scratch();
        let before = std::fs::read(scratch.layout.legacy_database()).unwrap();
        offer_answer("no").expect("No answers the offer");
        assert_eq!(
            std::fs::read(scratch.layout.legacy_database()).unwrap(),
            before
        );
        assert!(crate::session::migration_state().is_none());
        let room = scratch.room.lock().unwrap();
        assert!(room.get_profile_by_tin("72345678800000").unwrap().is_none());
    }

    /// Positive control for the absence proofs: the same reappeared file is still detected and
    /// offered when it carries neither the moved-file content mark nor a consumed mark.
    #[test]
    fn b2removal_an_unmarked_reappeared_file_is_offered_to_another_account() {
        let scratch = offer_scratch();
        copy_legacy_to_a_new_inode(&scratch);
        sign_b_in(&scratch);
        assert!(
            matches!(
                crate::session::migration_state(),
                Some(crate::session::MigrationState::Offered)
            ),
            "positive control: the unmarked reappeared file was not offered"
        );
    }

    /// A copy of a file the claim record says A already moved is nothing for B: never Ready, never offered.
    #[test]
    fn b2removal_a_moved_files_copy_is_never_offered_to_another_account() {
        let scratch = offer_scratch();
        let sha256 = sha256_path(&scratch.layout.legacy_database());
        rewrite_claim_record(&scratch, |record| {
            record.step = Step::Done;
            record.raw_sha256 = Some(sha256);
        });
        copy_legacy_to_a_new_inode(&scratch);
        sign_b_in(&scratch);
        assert_eq!(
            crate::session::snapshot().phase_kind,
            crate::session::PhaseKind::Open
        );
        assert!(
            crate::session::migration_state().is_none(),
            "B was offered A's moved file: {:?}",
            crate::session::migration_state()
        );
        let (b_handle, _, b_room, _) = crate::session::snapshot().room.expect("B's room");
        assert_ne!(b_room.expect("B's room id"), scratch.room_id);
        let b_db = b_handle.lock().unwrap();
        assert!(b_db.get_profile_by_tin("72345678800000").unwrap().is_none());
    }

    /// The offer page never receives the machine user's path from the claim store; the raw
    /// store error proves the path exists and stays only in the log.
    #[test]
    fn offer_errors_never_carry_os_paths() {
        let path = std::path::PathBuf::from("/Users/os-user/private/device.db");
        let raw = path.to_string_lossy().to_string();
        let errors = [
            bir_core::rooms::DeviceStoreError::Unreadable {
                path: path.clone(),
                reason: "not sqlite".into(),
            },
            bir_core::rooms::DeviceStoreError::Newer {
                path: path.clone(),
                version: 99,
            },
            bir_core::rooms::DeviceStoreError::Lost(path.clone()),
        ];
        for error in errors {
            assert!(
                error.to_string().contains(&raw),
                "positive control: the raw store error has a path"
            );
            let page = claim_read_error(error);
            assert!(!page.message().contains(&raw), "{}", page.message());
        }
        let error = bir_core::rooms::DeviceStoreError::Unreadable {
            path,
            reason: "not sqlite".into(),
        };
        let page = offer_answer_error(error);
        assert!(!page.message().contains(&raw), "{}", page.message());
    }

    /// The content-keyed consumed mark protects B even when the consumed file returns with a new inode.
    #[test]
    fn b2removal_a_consumed_same_bytes_copy_is_never_offered_to_another_account() {
        let scratch = offer_scratch();
        let sha256 = sha256_path(&scratch.layout.legacy_database());
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                bir_core::legacy_claim::OFFER_CONSUMED_KEY,
                serde_json::json!({
                    "room": scratch.room_id.as_str(),
                    "sha256": sha256,
                    "state": "imported"
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);
        copy_legacy_to_a_new_inode(&scratch);
        sign_b_in(&scratch);
        assert_eq!(
            crate::session::snapshot().phase_kind,
            crate::session::PhaseKind::Open
        );
        assert!(
            crate::session::migration_state().is_none(),
            "B was offered A's consumed file: {:?}",
            crate::session::migration_state()
        );
    }

    /// The 1699c37e mark was a serialized FileId. It is unknown now, never a start failure.
    #[test]
    fn b2removal_an_old_format_consumed_mark_never_breaks_start() {
        let scratch = offer_scratch();
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                bir_core::legacy_claim::OFFER_CONSUMED_KEY,
                serde_json::json!({ "volume": 1, "index": 2 }).to_string()
            ],
        )
        .unwrap();
        drop(conn);
        let claim = bir_core::legacy_claim::LegacyClaim::at(scratch.layout.clone()).unwrap();
        let claimant = bir_core::legacy_claim::Claimant {
            server_url: "https://buwiz.example.test".into(),
            user_id: "user-a".into(),
            email: Some("user-a@example.test".into()),
            practice: false,
        };
        assert!(
            matches!(
                claim.start(&claimant),
                bir_core::legacy_claim::Start::Offer(_)
            ),
            "the old-format mark broke start"
        );
    }
}
