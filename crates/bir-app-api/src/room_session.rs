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
        Some(crate::session::MigrationState::DeviceStore { detail }) => ("transient", false, detail),
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
    // The probe first (N1), on a scratch copy: a credential means the legacy unlock page, which is
    // A3a-2 — until then the offer cannot run on a locked file.
    if let Ok(bir_core::legacy_claim::LegacyProbe::Locked(_)) =
        bir_core::legacy_claim::probe_legacy(&layout)
    {
        return Err(HostError::new(
            "This file is locked with a TIN and PIN. Moving locked files comes in a later version; choose Not now.",
        ));
    }
    let (handle, ..) = snap
        .room
        .ok_or_else(|| HostError::new("no room is open to add them to"))?;
    let stamp = bir_core::bir_submit::device::now();
    let scratch_dir = layout.device_dir().join("scratch");
    std::fs::create_dir_all(&scratch_dir).map_err(|error| HostError::new(error.to_string()))?;
    let archive = scratch_dir.join(format!("offered-{stamp}.zip"));
    let result = (|| -> Result<(), String> {
        let offered = bir_core::db::Database::open(&legacy).map_err(|error| error.to_string())?;
        bir_core::export::export_all_profiles_data(&offered, &archive)
            .map_err(|error| error.to_string())?;
        drop(offered);
        {
            let room = bir_core::lock_timing::lock(&handle)
                .map_err(|_| "database lock poisoned".to_owned())?;
            bir_core::import::import_profile_data(&room, &archive)
                .map_err(|error| error.to_string())?;
        }
        std::fs::rename(
            &legacy,
            layout
                .device_dir()
                .join(format!("bir_data.offered-{stamp}.db")),
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&archive);
    match result {
        Ok(()) => {
            crate::session::clear_offer_note();
            Ok(HostValue::Null)
        }
        Err(error) => Err(HostError::new(error)),
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
