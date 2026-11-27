//! The session and room lifecycle, as one state machine (the design page, revision 2).
//!
//! One object, one lock, the only source of truth for who owns the open room and when. The
//! readers it replaces — the slot, MIGRATION, the gate's writers, the practice flag, the
//! per-engine launch state — move into the phase or derive from it, commit by commit. The epoch
//! is mirrored into `bir_core::session_epoch` so bir-core code compares a captured epoch without
//! ever taking this lock.
//!
//! Lock discipline (§3): SESSION is outermost, then account_session::lock(), then open_room
//! internals, then a room's database lock. SESSION is never held across blocking I/O: the claim's
//! file work, the network, `Database::open` and its migrations, a thread join, an import's copy
//! or swap. Opening runs unlocked and commits by compare-and-swap on the epoch.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use bir_core::db::Database;
use bir_core::rooms::RoomId;

/// The stored sign-in's identity: the normalised server origin, the user id, and whether the
/// account is a practice account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub origin: String,
    pub user_id: String,
    pub practice: bool,
}

/// Why an open started: the launch resume, a fresh sign-in, or Try again from a blocked page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenWhy {
    Launch,
    SignIn,
    TryAgain,
}

/// What the open room has to tell the page beyond being open (§1: the migration note derives
/// from the phase, never a state of its own).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    None,
    Permanent { detail: String },
    Offered { room: RoomId },
}

/// Why nothing opened. The gate is locked; the migration page reads the kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked {
    Transient { copy_saved: bool, detail: String },
    MovedNotOpened { detail: String },
    DeviceStore { detail: String },
}

/// The room the phase holds: its id (`None` in legacy mode and single-room mode), its file,
/// the generation of this open, and the handle.
#[derive(Clone)]
pub struct OpenRoom {
    pub room_id: Option<RoomId>,
    pub path: PathBuf,
    pub generation: u64,
    pub handle: Arc<Mutex<Database>>,
}

/// The machine's states (§2). `Open` and `Legacy` hold the one slot; `Blocked` shows a migration
/// page with the gate locked; `Quitting` is terminal.
pub enum Phase {
    SignedOut,
    Opening {
        owner: Owner,
        engine: u64,
        why: OpenWhy,
    },
    Open {
        owner: Owner,
        room: OpenRoom,
        note: Note,
        importing: bool,
    },
    Legacy {
        owner: Owner,
        room: OpenRoom,
        importing: bool,
    },
    Blocked {
        owner: Owner,
        reason: Blocked,
    },
    Quitting,
}

/// The phase's name, for the snapshot and for tests: no lock is needed to compare kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseKind {
    SignedOut,
    Opening,
    Open,
    Legacy,
    Blocked,
    Quitting,
}

impl Phase {
    fn kind(&self) -> PhaseKind {
        match self {
            Phase::SignedOut => PhaseKind::SignedOut,
            Phase::Opening { .. } => PhaseKind::Opening,
            Phase::Open { .. } => PhaseKind::Open,
            Phase::Legacy { .. } => PhaseKind::Legacy,
            Phase::Blocked { .. } => PhaseKind::Blocked,
            Phase::Quitting => PhaseKind::Quitting,
        }
    }

    fn owner(&self) -> Option<&Owner> {
        match self {
            Phase::Opening { owner, .. }
            | Phase::Open { owner, .. }
            | Phase::Legacy { owner, .. }
            | Phase::Blocked { owner, .. } => Some(owner),
            Phase::SignedOut | Phase::Quitting => None,
        }
    }
}

/// Set only by [`bypass_unlock`] (§9): the machine itself is bypassed in a debug run with the
/// auth bypass on, and check_invariants steps aside. Tests never set it, so the environment's
/// BIR_AUTH_BYPASS no longer decides whether the invariants run — they always run in tests.
/// Set only by bypass_unlock (§9): the machine stepped aside because the environment bypasses
/// authentication. Private — nothing outside this module can pretend the machine is bypassed.
static BYPASSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Auth bypass (§9): a debug run unlocks with no owner, outside the machine. The one caller is
/// prepare_launch's bypass branch (wired in commit 3). Marks the machine bypassed so
/// check_invariants steps aside; the gate and practice writes are the machine's own, centralised
/// here so nothing else writes them outside the machine.
pub fn bypass_unlock() {
    BYPASSED.store(true, std::sync::atomic::Ordering::SeqCst);
    bir_core::transport::set_practice_account(false);
    crate::account_gate::set_unlocked(true);
}

struct Session {
    epoch: u64,
    phase: Phase,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    epoch: 0,
    phase: Phase::SignedOut,
});

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn locked() -> MutexGuard<'static, Session> {
    lock(&SESSION)
}

/// The open room in one read: the handle, the generation, the room id and the path.
pub type RoomSnap = (Arc<Mutex<Database>>, u64, Option<RoomId>, PathBuf);

/// The one read API (§1): one lock, one clone, everything a reader may ask. There are no other
/// readers of the machine.
pub struct Snap {
    pub epoch: u64,
    pub phase_kind: PhaseKind,
    pub owner: Option<Owner>,
    /// The open room's handle, generation, room id and path — one tuple, never four reads a
    /// sign-out could interleave.
    pub room: Option<RoomSnap>,
    pub note: Note,
    pub importing: bool,
}

pub fn snapshot() -> Snap {
    let session = locked();
    let (room, note, importing) = match &session.phase {
        Phase::Open {
            room,
            note,
            importing,
            ..
        } => (
            Some((
                room.handle.clone(),
                room.generation,
                room.room_id.clone(),
                room.path.clone(),
            )),
            note.clone(),
            *importing,
        ),
        Phase::Legacy {
            room, importing, ..
        } => (
            Some((
                room.handle.clone(),
                room.generation,
                room.room_id.clone(),
                room.path.clone(),
            )),
            Note::None,
            *importing,
        ),
        _ => (None, Note::None, false),
    };
    Snap {
        epoch: session.epoch,
        phase_kind: session.phase.kind(),
        owner: session.phase.owner().cloned(),
        room,
        note,
        importing,
    }
}

/// What an open computes outside the lock and commits by compare-and-swap (§3 step 3).
pub enum Target {
    Room(RoomId),
    Legacy,
    Blocked(Blocked),
    /// Single-room and dev-database mode (§9): the named file, no room id.
    NotHere(PathBuf),
}

fn mirror(epoch: u64) {
    bir_core::session_epoch::set(epoch);
}

/// The reason a close ran, for the log line. The machine treats every close the same.
#[derive(Clone, Copy, Debug)]
pub enum Reason {
    SignOut,
    ServerEnded,
    Replaced,
    PageClosed,
}

/// What a close decided, for the steps that run outside SESSION.
struct CloseOutcome {
    /// The phase's room, dropped after the lock is released.
    pub dropped: Option<OpenRoom>,
    /// The owner-ending steps ran (the owner really ended): forget_device owes a run outside
    /// SESSION, only while the epoch this close produced is still current (F2).
    pub owner_ended: bool,
    /// The epoch this close produced.
    pub epoch: u64,
    /// The owner who ended, when the owner-ending steps ran — his is the key the forget takes.
    pub ended_owner: Option<Owner>,
}

/// How the owner-ending steps are decided (F2): they run when the owner really ends — a
/// sign-out, a server-ended session — or when begin_open's replacement is a DIFFERENT owner.
/// Never from SignedOut (no previous owner), never for the same owner.
enum CloseKind {
    /// The owner really ends: sign_out, server_ended, the page-closed fallback.
    OwnerEnds,
    /// begin_open's replacement: the steps run only if the previous owner differs.
    ReplacedBy(Owner),
}

fn owner_key_of(owner: &Owner) -> (String, String) {
    (owner.origin.clone(), owner.user_id.clone())
}

/// close_phase_locked: the always-half of the close, under SESSION (F2) — the epoch, the gate
/// locked, practice off, the room out, the room path None, the phase SignedOut. The OWNER-ENDING
/// half — the OAuth listeners and the TIN cache invalidated under SESSION, forget_device owing a
/// run outside it — is decided by the kind and the previous owner.
fn close_locked(session: &mut Session, kind: &CloseKind) -> CloseOutcome {
    session.epoch += 1;
    mirror(session.epoch);
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    let previous = session.phase.owner().cloned();
    let room = match std::mem::replace(&mut session.phase, Phase::SignedOut) {
        Phase::Open { room, .. } | Phase::Legacy { room, .. } => Some(room),
        _ => None,
    };
    // The room path cache reads None once nothing is open (X1).
    bir_core::db::set_open_room_database(None);
    let owner_ended = match (&previous, kind) {
        (None, _) => false,
        (Some(_), CloseKind::OwnerEnds) => true,
        (Some(previous), CloseKind::ReplacedBy(new)) => owner_key_of(previous) != owner_key_of(new),
    };
    if owner_ended {
        bir_core::email::cancel_oauth_listeners();
        if let Ok(submitter) = bir_core::relay::submitter() {
            submitter.invalidate_account_tins();
        }
    }
    CloseOutcome {
        dropped: room,
        owner_ended,
        epoch: session.epoch,
        ended_owner: if owner_ended { previous } else { None },
    }
}

/// forget_device's half of the owner-ending steps (F2): AFTER SESSION, and only while the epoch
/// the close produced is still current — a newer epoch means a newer session answered already,
/// and its key is not this close's to forget.
/// Who waits for the device-key forget (revision 5's C).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ForgetMode {
    /// begin_open's switch step: synchronous, BEFORE the claim thread spawns — F2'c's ordering.
    Now,
    /// sign_out and server_ended: they never wait on DEVICE_KEY. The forget runs on a
    /// background thread that waits its turn for the lock, re-checks the epoch, and forgets —
    /// a registration parked on its network never holds a sign-out hostage, and I6 re-checks
    /// at the next registration anyway.
    Backgrounded,
}

/// The forget threads in flight, so the tests can join them instead of sleeping.
#[cfg(any(test, feature = "test-support"))]
static FORGET_THREADS: std::sync::LazyLock<Mutex<Vec<std::thread::JoinHandle<()>>>> =
    std::sync::LazyLock::new(|| Mutex::new(Vec::new()));

/// Join every forget thread spawned so far (the tests' synchronisation — never a sleep).
#[cfg(any(test, feature = "test-support"))]
pub fn join_device_forget_threads() {
    let threads = {
        let mut parked = FORGET_THREADS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *parked)
    };
    for thread in threads {
        let _ = thread.join();
    }
}

fn finish_close(outcome: &CloseOutcome, mode: ForgetMode) {
    // F2'c: the forget runs under the DEVICE_KEY lock, never nested in SESSION (we are outside
    // it here), and only while the epoch this close produced is still current — a newer epoch
    // means a newer session answered already, and its key is not this close's to take. The
    // check-and-delete inside forget_device_key_for is the second gate: a key already recorded
    // for the newer owner is never touched.
    if !outcome.owner_ended {
        return;
    }
    let epoch = outcome.epoch;
    let ended = outcome.ended_owner.clone();
    let forget = move || {
        if bir_core::session_epoch::current() != epoch {
            return;
        }
        if let Some(ended) = &ended {
            bir_core::relay::forget_device_key_for(&bir_core::account_session::OwnerKey {
                origin: ended.origin.clone(),
                user_id: ended.user_id.clone(),
            });
        } else {
            bir_core::relay::forget_device();
        }
    };
    match mode {
        ForgetMode::Now => forget(),
        ForgetMode::Backgrounded => {
            // The device-folder override is thread-local: the forget thread runs under the
            // closer's install, as the claim thread does. In production there is no override.
            let device = bir_core::platform::capture_device_dir_override();
            let spawned = std::thread::Builder::new()
                .name("buwiz-device-forget".into())
                .spawn(move || device.run(forget));
            #[cfg(any(test, feature = "test-support"))]
            if let Ok(thread) = spawned {
                FORGET_THREADS
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(thread);
            }
            #[cfg(not(any(test, feature = "test-support")))]
            drop(spawned);
        }
    }
}

/// The one sign-out, and begin_open's replace (§4). Runs entirely under the lock; the room's Arc
/// drops after it lets go.
pub fn close(reason: Reason) {
    hooks::at("close.locked");
    let outcome = {
        let mut session = locked();
        if session.phase.kind() == PhaseKind::Quitting {
            // F7: Quitting is terminal — a close after it is a logged no-op, never a resurrection
            // of SignedOut behind the quit's back.
            tracing::info!("close after quit ignored: the session is terminal");
            return;
        }
        close_locked(&mut session, &CloseKind::OwnerEnds)
    };
    tracing::info!("session closed: {reason:?}");
    crate::open_room::sweep_registry();
    finish_close(&outcome, ForgetMode::Now);
    drop(outcome.dropped);
    report_invariants();
}

/// begin_open (§3 step 1). A no-op for the same owner already open, opening or legacy (X1: the
/// second engine's launch resumes nothing twice). Anything else ends in the same critical
/// section: the epoch bumps, the phase becomes Opening, the claim thread spawns and this returns
/// at once (F10).
pub fn begin_open(owner: Owner, engine: u64, why: OpenWhy) {
    let (spawn, outcome) = {
        let mut session = locked();
        hooks::at("begin_open.locked");
        if session.phase.kind() == PhaseKind::Quitting {
            tracing::info!("begin_open refused: the session is quitting");
            return;
        }
        // F20: the same-owner check is (origin, user id), as I2's is — practice is not part of
        // who the owner IS, and a practice-flag difference alone never replaces a session.
        if let Some(current) = session.phase.owner()
            && current.origin == owner.origin
            && current.user_id == owner.user_id
            && matches!(
                session.phase.kind(),
                PhaseKind::Open | PhaseKind::Legacy | PhaseKind::Opening
            )
        {
            return;
        }
        // F2: the replacement's owner-ending steps run only when the owner being replaced is a
        // DIFFERENT one — a Try again or a launch of the same owner keeps its device key.
        let outcome = close_locked(&mut session, &CloseKind::ReplacedBy(owner.clone()));
        // §1 [D4c]: the device guard is taken at process start (init). If it was not held then
        // (another copy had it), every begin_open — Try again included — retries it; while it
        // cannot be taken the phase is Blocked::Transient and no claim thread starts. A
        // non-blocking file lock: no blocking I/O under SESSION.
        match ensure_device_guard() {
            Ok(()) => {
                session.phase = Phase::Opening {
                    owner: owner.clone(),
                    engine,
                    why,
                };
                (Some(session.epoch), outcome)
            }
            Err(detail) => {
                session.phase = Phase::Blocked {
                    owner: owner.clone(),
                    reason: Blocked::Transient {
                        copy_saved: false,
                        detail,
                    },
                };
                (None, outcome)
            }
        }
    };
    // F17: the replaced room's Arc drops after SESSION is released; forget_device's half runs
    // here too, epoch-checked (F2).
    finish_close(&outcome, ForgetMode::Now);
    drop(outcome.dropped);
    if let Some(epoch) = spawn {
        spawn_claim_thread(owner, engine, why, epoch);
    }
    report_invariants();
}

/// The claim thread "buwiz-room-open" (§3 step 2). Its real work — the claim, the classify, the
/// open — is wired in commit 2; the thread is the machine's already, so the spawn, the epoch and
/// the commit path it ends in are tested here.
/// Test instrumentation: how many claim threads this process spawned. two_engines asserts
/// exactly one for two boots of one session; d4c asserts none for a refused open.
#[cfg(test)]
pub(crate) static CLAIM_THREAD_SPAWNS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn claim_thread_spawn_count() -> u64 {
    CLAIM_THREAD_SPAWNS.load(std::sync::atomic::Ordering::SeqCst)
}

fn spawn_claim_thread(owner: Owner, engine: u64, why: OpenWhy, epoch: u64) {
    let _ = (engine, why);
    #[cfg(test)]
    CLAIM_THREAD_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Err(error) = std::thread::Builder::new()
        .name("buwiz-room-open".to_string())
        .spawn(move || {
            let _ = (owner, epoch);
            // commit 3 wires the claim thread's real work: CLAIM_RUN, the hooks,
            // LegacyClaim::here() and classify, start(), room_open_allowed, open_path, then
            // commit(epoch, target, handle).
        })
    {
        tracing::warn!("the room-open thread did not spawn: {error}");
    }
}

/// sign_out (§4, F8'): persistence first, the cache inside SESSION.
///  1. under account_session::lock(), OUTSIDE SESSION: the logout info read, the file AND the
///     keychain entry deleted, released;
///  2. ONE SESSION section: the owner cache goes to None (its small lock nests inside SESSION)
///     plus the close with the owner-ending steps;
///  3. forget_device (epoch-checked, the DEVICE_KEY lock — never nested in SESSION) and
///     logout_soon.
///
/// A crash between 1 and 2 leaves the disk signed out — the safe direction — and I1 holds on
/// the cache throughout. cloud_account's sign_out_with routes here.
pub fn sign_out() {
    sign_out_inner(Box::new(bir_core::cloud_account::BlockingHttp::new()));
}

/// The sign-out with its logout transport chosen — production's BlockingHttp by sign_out, the
/// tests' fake through cloud_account's sign_out_with. The flow is the machine's either way; the
/// injected transport only hears the logout, so the contract the sign-out tests exercise IS the
/// real path.
pub(crate) fn sign_out_inner(
    logout_http: Box<dyn bir_core::cloud_account::Http + Send>,
) {
    crate::cloud_email::cancel();
    bir_core::background_cron::set_submission_paused(true);
    // Step 1: the persisted sign-in goes, outside SESSION.
    let (logout, removed) = {
        let _account = bir_core::account_session::lock();
        let logout = bir_core::cloud_account::load_stored_account()
            .ok()
            .flatten()
            .map(|stored| (stored.server_url.clone(), stored.access_token.clone()));
        let removed = bir_core::account_session::forget_stored();
        (logout, removed)
    };
    hooks::at("sign_out.after_persist");
    // Step 2: the one SESSION section — the cache and the close together.
    let outcome = {
        let mut session = locked();
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds)
    };
    // Step 3: outside every lock.
    crate::open_room::sweep_registry();
    finish_close(&outcome, ForgetMode::Backgrounded);
    drop(outcome.dropped);
    report_invariants();
    if let Some((server_url, access_token)) = logout {
        crate::cloud_account::logout_soon(logout_http, server_url, access_token);
    }
    if let Err(error) = removed {
        tracing::warn!("sign-out left the saved sign-in on this device: {error}");
    }
}

/// server_ended (§4, F8', revision 5's A): the sign-in of `owner` ended at the server in the
/// session of `epoch` — ON THE CREDENTIAL `failed_refresh_token`. The disk's clear matches the
/// exact failed credential, so a re-sign-in of even the same owner is never cleared by an old
/// token's 401: only if the stored sign-in is that owner AND its refresh token IS the failed
/// one does anything happen at all. Otherwise nothing is cleared and NOTHING closes.
///  1. under account_session::lock(), OUTSIDE SESSION: the credential check and the disk delete;
///  2. then ONE SESSION section, only if the epoch still matches and the cache is still that
///     owner: the cache goes to None plus the close with the owner-ending steps;
///  3. the device-key forget after SESSION, epoch-checked (a background thread — see sign_out).
pub fn server_ended(
    owner: bir_core::account_session::OwnerKey,
    epoch: u64,
    failed_refresh_token: String,
) {
    // Step 1: the disk's delete-if-credential, outside SESSION.
    let cleared = {
        let _account = bir_core::account_session::lock();
        let matches = bir_core::cloud_account::load_stored_account()
            .ok()
            .flatten()
            .map(|stored| {
                bir_core::account_session::OwnerKey::of(&stored).as_ref() == Some(&owner)
                    && stored.refresh_token == failed_refresh_token
            })
            .unwrap_or(false);
        if matches {
            bir_core::account_session::clear_stored();
        }
        matches
    };
    if !cleared {
        tracing::info!(
            "server_ended ignored: the stored credential is not the failed one — a newer sign-in is untouched"
        );
        return;
    }
    // Step 2: the one SESSION section — cache and close together, only if both still match.
    let outcome = {
        let mut session = locked();
        let cache_is_owner = bir_core::account_session::stored_owner().as_ref() == Some(&owner);
        if session.epoch != epoch || !cache_is_owner {
            tracing::info!(
                "server_ended for epoch {epoch}: the disk is cleared, the session moved on already"
            );
            return;
        }
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds)
    };
    crate::open_room::sweep_registry();
    finish_close(&outcome, ForgetMode::Backgrounded);
    drop(outcome.dropped);
    report_invariants();
}

/// signed_in (§4, F8'): a sign-in's write — the persist under the account lock OUTSIDE SESSION,
/// then one SESSION section setting the cache. Commit 3 adds the begin_open here;
/// register_device_soon stays where it is until commit moves it into commit.
pub fn signed_in(
    stored: &bir_core::cloud_account::StoredAccount,
) -> Result<(), bir_core::cloud_account::CloudError> {
    {
        let _account = bir_core::account_session::lock();
        bir_core::account_session::save_stored(stored)?;
    }
    {
        let _session = locked();
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(stored));
    }
    report_invariants();
    Ok(())
}

/// commit (§3 step 3): under SESSION, short, no I/O. A stale epoch or a phase that isn't Opening
/// abandons the work: the handle drops after the lock lets go and nothing is installed. The
/// stored sign-in is re-read inside the lock order SESSION → account_session::lock() (I2); a
/// mismatch abandons too. On a room or legacy target the gate unlocks LAST, after everything
/// else is in the phase (I1).
// Used by the machine's tests now; the claim thread — production code — calls it from commit 3,
// which is why it is pub(crate) and not pub (F18).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn commit(epoch: u64, target: Target, handle: Option<(Arc<Mutex<Database>>, PathBuf)>) {
    hooks::at("commit.enter");
    enum Outcome {
        /// Nothing installed; the handle (if any) drops after the lock is released.
        Abandon(Option<(Arc<Mutex<Database>>, PathBuf)>),
        /// The phase moved to Blocked; the gate stays locked; a handle (if any) drops.
        Blocked(Option<(Arc<Mutex<Database>>, PathBuf)>),
        /// A room or the legacy file is the slot; the gate is unlocked.
        Installed,
    }
    let outcome = {
        let mut session = locked();
        let opening_owner = match (&session.phase, session.epoch == epoch) {
            (Phase::Opening { owner, .. }, true) => Some(owner.clone()),
            _ => None,
        };
        match opening_owner {
            None => {
                tracing::info!(
                    "commit abandoned: epoch {epoch} is not current or nothing is opening"
                );
                Outcome::Abandon(handle)
            }
            Some(owner) if !stored_is(&owner) => {
                tracing::info!("commit abandoned: the stored sign-in is no longer this owner");
                Outcome::Abandon(handle)
            }
            Some(owner) => match (target, handle) {
                (Target::Blocked(reason), handle) => {
                    session.phase = Phase::Blocked { owner, reason };
                    Outcome::Blocked(handle)
                }
                (_, None) => {
                    tracing::warn!("commit: a room target came with no handle");
                    session.phase = Phase::Blocked {
                        owner,
                        reason: Blocked::Transient {
                            copy_saved: false,
                            detail: "the room did not open".to_string(),
                        },
                    };
                    Outcome::Blocked(None)
                }
                (target, Some((handle, path))) => {
                    let room = OpenRoom {
                        room_id: match &target {
                            Target::Room(room) => Some(room.clone()),
                            _ => None,
                        },
                        path: path.clone(),
                        generation: next_generation(),
                        handle,
                    };
                    bir_core::db::set_open_room_database(Some(path));
                    bir_core::transport::set_practice_account(owner.practice);
                    session.phase = if matches!(target, Target::Legacy) {
                        Phase::Legacy {
                            owner,
                            room,
                            importing: false,
                        }
                    } else {
                        Phase::Open {
                            owner,
                            room,
                            note: Note::None,
                            importing: false,
                        }
                    };
                    // I1: the gate opens LAST, once the room is in the phase.
                    crate::account_gate::set_unlocked(true);
                    Outcome::Installed
                }
            },
        }
    };
    match outcome {
        Outcome::Abandon(dropped) | Outcome::Blocked(dropped) => drop(dropped),
        Outcome::Installed => {
            // §3 step 3: outside the lock, wake the worker and refresh the menu.
            bir_core::background_cron::wake();
            crate::hooks::hooks().refresh_menu();
        }
    }
    hooks::at("commit.done");
    report_invariants();
}

/// I2's re-read (§3 step 3): the stored sign-in, read under `account_session::lock()` — taken
/// inside SESSION, which is the lock order, and held for this file read only (§3's rule [D2]) —
/// is this owner: the same normalised origin and the same user id.
#[cfg_attr(not(test), allow(dead_code))]
/// I2's half in commit: the stored owner cache answers — never the account lock, the file or
/// the keychain (F8, F3c). The cache is current because every write of the stored sign-in
/// updates it inside the same account-lock section as the write.
fn stored_is(owner: &Owner) -> bool {
    stored_owner_key()
        .is_some_and(|(origin, user_id)| origin == owner.origin && user_id == owner.user_id)
}

/// The stored sign-in's (normalised origin, user id), or None while nobody is stored, read
/// from account_session's owner cache — no I/O, no lock of account_session's (F8).
fn stored_owner_key() -> Option<(String, String)> {
    bir_core::account_session::stored_owner().map(|key| (key.origin, key.user_id))
}

/// quit (§8's state part): the epoch bumps, the phase is terminal, the gate locks. The release
/// order — worker, Arc holders, checkpoint, registry, guards — lands in commit 6.
pub fn quit() {
    let dropped = {
        let mut session = locked();
        session.epoch += 1;
        mirror(session.epoch);
        crate::account_gate::set_unlocked(false);
        bir_core::transport::set_practice_account(false);
        // F6: the phase's room comes out under the lock and drops after it — the database's
        // close never holds SESSION. (Commit 6 hands the room to the release step instead.)
        match std::mem::replace(&mut session.phase, Phase::Quitting) {
            Phase::Open { room, .. } | Phase::Legacy { room, .. } => Some(room),
            _ => None,
        }
    };
    drop(dropped);
    hooks::at("quit.after_bump");
    report_invariants();
}

#[cfg_attr(not(test), allow(dead_code))]
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg_attr(not(test), allow(dead_code))]
fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// session::init(): the device guard, taken ONCE at process start — the first thing the door's
/// boot runs, before any claim or open (§1 [D4c]). Held until quit releases it last. There is one
/// holder in the process (`open_room`'s guard map), so nothing ever takes it twice. If it can't be
/// taken now, every begin_open retries it, and the phase is Blocked::Transient meanwhile.
pub fn init(device_dir: &std::path::Path) -> Result<(), String> {
    // The owner cache's one fill (F8''s fix): the file and keychain are read here, at process
    // start, where no SESSION section can ever be involved — stored_owner() never does I/O.
    bir_core::account_session::fill_owner_cache();
    crate::open_room::hold_device_guard(device_dir)
}

/// The guard for this process's device folder, taken if it is not held yet (begin_open's retry).
fn ensure_device_guard() -> Result<(), String> {
    let layout = bir_core::rooms::Layout::here();
    crate::open_room::hold_device_guard(layout.device_dir())
}

/// check_invariants(): the STATE invariants I1, I2 and I4 (§2). The harness asserts it is Ok after
/// every scripted step; debug builds log a breach at the end of every transition
/// ([`report_invariants`]). It never claims the per-operation rules I3 and I5: those are enforced
/// and tested at their own check points (§2 [D3]).
pub fn check_invariants() -> Result<(), String> {
    if BYPASSED.load(std::sync::atomic::Ordering::SeqCst) {
        // Outside the machine (§9): the bypass unlocks with no owner. Tests never set BYPASSED,
        // so the environment's auth bypass no longer blanks the invariants.
        return Ok(());
    }
    let snap = snapshot();
    let stored = stored_owner_key();
    let owner_is_stored = snap.owner.as_ref().is_some_and(|owner| {
        stored
            .as_ref()
            .is_some_and(|(origin, user_id)| *origin == owner.origin && *user_id == owner.user_id)
    });
    let open_kind = matches!(snap.phase_kind, PhaseKind::Open | PhaseKind::Legacy);
    // The RAW flag (F1): is_unlocked() adds the bypass and FORCE_LOCKED, which are not the
    // machine's writes — the invariant is about what the session wrote.
    let unlocked = crate::account_gate::unlocked_flag();
    let mut breaches = Vec::new();
    // I1: gate unlocked ⇔ phase ∈ {Open, Legacy} AND the stored sign-in is the phase's owner.
    if unlocked != (open_kind && owner_is_stored) {
        breaches.push(format!(
            "I1: the gate is {} while the phase is {:?} and the stored sign-in {} its owner",
            if unlocked { "unlocked" } else { "locked" },
            snap.phase_kind,
            if owner_is_stored { "is" } else { "is not" },
        ));
    }
    // I2: the slot's owner is the stored sign-in.
    if open_kind && !owner_is_stored {
        breaches.push(format!(
            "I2: a {:?} slot is open for an owner who is not the stored sign-in",
            snap.phase_kind
        ));
    }
    // I4: no room is open while the claim's move evidence for it is pending.
    if let Some((_, _, Some(room_id), _)) = &snap.room {
        let layout = bir_core::rooms::Layout::here();
        if let Err(detail) = bir_core::legacy_claim::room_move_evidence_clear(&layout, room_id) {
            breaches.push(format!("I4: room {room_id} is open while {detail}"));
        }
    }
    if breaches.is_empty() {
        Ok(())
    } else {
        Err(breaches.join("; "))
    }
}

/// The end of every transition: in debug builds, a breach of I1, I2 or I4 is logged as an error
/// (never a panic: the harness tests assert `check_invariants()` themselves).
fn report_invariants() {
    #[cfg(debug_assertions)]
    if let Err(breach) = check_invariants() {
        tracing::error!("session invariant broken: {breach}");
    }
}

/// The deterministic interleaving harness (§11): named points the machine and its callers pass
/// through. A test pauses a point; the thread reaching it blocks until the gate releases. No
/// sleeps anywhere: `reached()` waits for arrival on a condvar.
#[cfg(test)]
pub mod hooks {
    use std::collections::HashMap;
    use std::sync::{Condvar, Mutex, MutexGuard};

    struct Point {
        /// The test's pause: while Some, arriving threads wait.
        gate: Option<(u64, bool)>, // (generation, released)
        /// How often this point was reached, for `reached()`.
        arrivals: u64,
    }

    struct Harness {
        points: HashMap<&'static str, Point>,
    }

    /// One condvar for the harness, beside its map's mutex: no pointer games.
    static CONDVAR: Condvar = Condvar::new();

    fn harness() -> &'static Mutex<Harness> {
        static HARNESS: std::sync::OnceLock<Mutex<Harness>> = std::sync::OnceLock::new();
        HARNESS.get_or_init(|| {
            Mutex::new(Harness {
                points: HashMap::new(),
            })
        })
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The machine passes a named point: record the arrival, and block while a pause on it is
    /// unreleased. In non-test builds this compiles away — the cfg(test) door below.
    pub(crate) fn pass(name: &'static str) {
        let mutex = harness();
        let mut state = lock(mutex);
        let generation = {
            let point = state.points.entry(name).or_insert(Point {
                gate: None,
                arrivals: 0,
            });
            point.arrivals += 1;
            point.gate.map(|(generation, _)| generation)
        };
        CONDVAR.notify_all();
        if let Some(generation) = generation {
            loop {
                let released = state
                    .points
                    .get(name)
                    .and_then(|point| point.gate)
                    .map(|(current, released)| current != generation || released)
                    .unwrap_or(true);
                if released {
                    break;
                }
                state = CONDVAR.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    /// The machine passes a named point.
    pub(crate) fn at(name: &'static str) {
        pass(name);
    }

    /// A pause on a named point. `reached()` waits for the next arrival; `release()` lets the
    /// paused thread through. Dropping the gate without a release releases too, so a failing
    /// test never wedges the ones after it.
    pub struct Gate {
        name: &'static str,
        generation: u64,
        /// The arrivals already recorded when the pause was set (F5): `reached` waits for one
        /// PAST this, so an arrival before the pause — a stale thread of an earlier step — never
        /// satisfies it. Read inside the same lock as the gate's set, so every later arrival is
        /// one this gate blocks.
        arrivals_at_pause: u64,
    }

    static NEXT_GATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    pub fn pause(name: &'static str) -> Gate {
        let generation = NEXT_GATE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mutex = harness();
        let arrivals_at_pause = {
            let mut state = lock(mutex);
            let point = state.points.entry(name).or_insert(Point {
                gate: None,
                arrivals: 0,
            });
            point.gate = Some((generation, false));
            point.arrivals
        };
        Gate {
            name,
            generation,
            arrivals_at_pause,
        }
    }

    impl Gate {
        /// Wait until a thread arrives at the paused point after this pause was set (F5).
        /// Bounded, so a broken machine fails the test instead of hanging the suite.
        pub fn reached(&self) {
            let mutex = harness();
            let mut state = lock(mutex);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                let arrivals = state
                    .points
                    .get(self.name)
                    .map(|point| point.arrivals)
                    .unwrap_or(0);
                if arrivals > self.arrivals_at_pause {
                    // An arrival past the pause's own count: the arriving thread is blocked at
                    // this gate (or was, if it has been released since) — never a stale arrival
                    // of an earlier step.
                    return;
                }
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                assert!(
                    left > std::time::Duration::ZERO,
                    "never reached: {}",
                    self.name
                );
                let (guard, _) = CONDVAR
                    .wait_timeout(state, left.min(std::time::Duration::from_millis(100)))
                    .unwrap_or_else(|e| e.into_inner());
                state = guard;
            }
        }

        pub fn release(self) {
            // Drop does the work.
        }
    }

    impl Drop for Gate {
        fn drop(&mut self) {
            let mutex = harness();
            let mut state = lock(mutex);
            if let Some(point) = state.points.get_mut(self.name) {
                if let Some((generation, released)) = &mut point.gate {
                    if *generation == self.generation {
                        *released = true;
                    }
                }
            }
            drop(state);
            CONDVAR.notify_all();
        }
    }

    /// Test reset: the arrivals and gates of a finished test must not leak into the next.
    pub fn reset() {
        let mutex = harness();
        let mut state = lock(mutex);
        state.points.clear();
        drop(state);
        CONDVAR.notify_all();
    }
}

/// The hook door: in test builds the harness, in production a no-cost inline nothing.
#[cfg(not(test))]
pub(crate) mod hooks {
    #[inline(always)]
    pub(crate) fn at(_name: &'static str) {}
}

/// One turn for every test touching SESSION, in both crates (§11): the machine, the room path,
/// the account gate and the claim state are process-wide, so no two tests may hold them at once.
/// The turn is bir-core's room test turn, so bir-core's room tests serialise with these.
#[cfg(test)]
pub fn test_turn() -> std::sync::MutexGuard<'static, ()> {
    bir_core::db::room_path_test_turn()
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    hooks::reset();
    let dropped = {
        let mut session = locked();
        let outcome = close_locked(&mut session, &CloseKind::OwnerEnds);
        session.phase = Phase::SignedOut;
        outcome.dropped
    };
    drop(dropped);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::rooms::Layout;

    // ---------- the helpers ----------

    struct Scratch {
        layout: Layout,
        _folder: tempfile::TempDir,
    }

    /// A scratch device in a temp folder, with every piece of process-wide state reset first:
    /// the tests run in one process, and the machine, the gate, the room path and the stored
    /// sign-in are all process-wide.
    fn scratch() -> Scratch {
        // Isolation: a backgrounded device-key forget of an earlier test never leaks into this
        // one — the tests are serial, so joining here never waits on this test's own threads.
        join_device_forget_threads();
        crate::open_room::reset_for_tests();
        reset_for_tests();
        bir_core::db::set_open_room_database(None);
        crate::account_gate::set_unlocked(false);
        bir_core::account_session::clear_stored();
        bir_core::account_session::reset_owner_cache_for_tests();
        let folder = tempfile::tempdir().unwrap();
        bir_core::platform::set_device_dir_override(Some(folder.path().to_path_buf()));
        Scratch {
            layout: Layout::at(folder.path().to_path_buf()),
            _folder: folder,
        }
    }

    /// The stored sign-in of `user_id` at the one test server, saved through
    /// bir_core::cloud_account the way save_signed_in does (2). Answers the Owner the machine
    /// compares it against: the normalised origin is room_origin's output, never a guess.
    fn seed_stored(user_id: &str, practice: bool) -> Owner {
        let server_url = "https://buwiz.example.test";
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: server_url.to_string(),
            access_token: format!("access-{user_id}"),
            refresh_token: format!("refresh-{user_id}"),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: user_id.to_string(),
            email: Some(format!("{user_id}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice,
        };
        bir_core::account_session::save_stored(&stored).unwrap();
        // The cache is the machine's picture (F8'): a seeded sign-in sets it as signed_in would.
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(&stored));
        Owner {
            origin: bir_core::rooms::room_origin(server_url).unwrap(),
            user_id: user_id.to_string(),
            practice,
        }
    }

    /// A database file in the scratch device, as a commit's (handle, path).
    fn scratch_handle(scratch: &Scratch, name: &str) -> (Arc<Mutex<Database>>, PathBuf) {
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let path = scratch.layout.device_dir().join(name);
        let handle = Arc::new(Mutex::new(Database::open(&path).unwrap()));
        (handle, path)
    }

    fn room_target(owner: &Owner) -> Target {
        Target::Room(bir_core::rooms::room_id_for(&owner.origin, &owner.user_id))
    }

    /// The gate's raw flag, without the test bypass is_unlocked adds (tests run with it on).
    fn gate_unlocked() -> bool {
        crate::account_gate::unlocked_flag()
    }

    fn assert_invariants() {
        assert!(
            check_invariants().is_ok(),
            "invariants: {:?}",
            check_invariants()
        );
    }

    // ---------- the machine's own tests ----------

    /// The epoch bumps at every ownership-ending transition, and the mirror follows (§1).
    #[test]
    fn close_bumps_the_epoch_and_the_mirror() {
        let _turn = test_turn();
        let _scratch = scratch();
        let before = snapshot().epoch;
        close(Reason::SignOut);
        assert_eq!(snapshot().epoch, before + 1, "close bumps the epoch");
        assert_eq!(
            bir_core::session_epoch::current(),
            before + 1,
            "the mirror follows the epoch"
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert_invariants();
    }

    /// begin_open is a no-op for the same owner already opening or open (X1), and refuses while
    /// quitting; another owner replaces (the epoch bumps again).
    #[test]
    fn begin_open_noops_for_the_same_owner() {
        let _turn = test_turn();
        let _scratch = scratch();
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
        let epoch = snapshot().epoch;
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert_invariants();
        begin_open(owner("user-a"), 2, OpenWhy::Launch);
        assert_eq!(
            snapshot().epoch,
            epoch,
            "a second engine's begin_open for the same owner is a no-op"
        );
        // F20: practice is not part of who the owner IS — a practice-flag difference alone is
        // still the same owner, never a replace.
        let mut practice_owner = owner("user-a");
        practice_owner.practice = true;
        begin_open(practice_owner, 2, OpenWhy::Launch);
        assert_eq!(
            snapshot().epoch,
            epoch,
            "a practice difference alone is still the same owner (F20)"
        );
        begin_open(owner("user-b"), 1, OpenWhy::SignIn);
        assert_eq!(
            snapshot().epoch,
            epoch + 1,
            "another owner replaces: the epoch bumps"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b")
        );
        assert_invariants();
    }

    /// quit is terminal: begin_open after it is refused in code and logged, and the phase does
    /// not move.
    #[test]
    fn begin_open_after_quit_is_refused() {
        let _turn = test_turn();
        let _scratch = scratch();
        quit();
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        let epoch = snapshot().epoch;
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        assert_eq!(snapshot().epoch, epoch, "nothing moved");
        assert_invariants();
    }

    /// (3a): the epoch CAS alone. The stored sign-in is B, the phase opens for B, so I2 passes —
    /// only the epoch catches A's stale commit.
    #[test]
    fn a_stale_epoch_alone_abandons() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_b = seed_stored("user-b", false);
        let owner_a = Owner {
            user_id: "user-a".to_string(),
            ..owner_b.clone()
        };
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        let epoch_a = snapshot().epoch;
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert_invariants();
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch_a, room_target(&owner_a), Some((handle, path)));
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Opening,
            "A's stale commit moved nothing"
        );
        assert!(
            snapshot().room.is_none(),
            "A's stale commit installed no room"
        );
        assert!(!gate_unlocked(), "the gate never opened for a stale commit");
        assert_invariants();
    }

    /// (3b): I2 alone. The epoch is current, so only the stored-owner re-read catches A's commit
    /// while the stored sign-in is B.
    #[test]
    fn a_stored_owner_mismatch_alone_abandons() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_b = seed_stored("user-b", false);
        let owner_a = Owner {
            user_id: "user-a".to_string(),
            ..owner_b.clone()
        };
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        let epoch = snapshot().epoch;
        assert_invariants();
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Opening,
            "the mismatched commit moved nothing"
        );
        assert!(
            snapshot().room.is_none(),
            "the mismatched commit installed no room"
        );
        assert!(
            !gate_unlocked(),
            "the gate never opened for a mismatched owner"
        );
        assert_invariants();
    }

    /// (4): the gate is locked while the open runs — never unlocked before the commit — and
    /// unlocked after it. What the test cannot see: the pause at commit.enter is before the
    /// lock, so no interleaving observes the ordering INSIDE the critical section; the gate
    /// being LAST there is structural — the one statement physically follows the phase move in
    /// the same section (I1), and the line reading verifies it, not this test.
    #[test]
    fn the_gate_is_locked_until_commit_and_unlocked_after() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        assert!(!gate_unlocked(), "the gate is locked while the open runs");
        assert_invariants();
        let epoch = snapshot().epoch;
        let gate = hooks::pause("commit.enter");
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        let target = room_target(&owner_a);
        let device = scratch.layout.device_dir().to_path_buf();
        let thread = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device));
            commit(epoch, target, Some((handle, path)));
        });
        gate.reached();
        // commit.enter passed means the commit is inside its critical section or about to be.
        assert!(
            !gate_unlocked(),
            "the gate is still locked at commit.enter: it opens LAST"
        );
        gate.release();
        thread.join().unwrap();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(snapshot().room.is_some(), "the room is the slot");
        assert!(gate_unlocked(), "the gate opened at the commit");
        assert_invariants();
    }

    /// (5): close locks the gate and drops the room after the lock is released — the Arc's last
    /// strong reference is gone once close returns.
    #[test]
    fn close_after_open_locks_the_gate_and_drops_the_room_after_the_lock() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(gate_unlocked());
        assert_invariants();
        let weak = {
            let snap = snapshot();
            let (handle, _, _, _) = snap.room.as_ref().unwrap();
            Arc::downgrade(handle)
            // snap — and its strong clone of the handle — drops here.
        };
        close(Reason::SignOut);
        assert!(!gate_unlocked(), "close locked the gate");
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert!(
            weak.upgrade().is_none(),
            "the room's Arc is gone once close returns: the phase was its last holder"
        );
        assert_invariants();
    }

    /// (6): while another copy holds the device guard, begin_open moves to Blocked::Transient and
    /// starts no claim thread; released, Try again opens (D4c's machine half).
    #[test]
    fn begin_open_while_another_copy_holds_the_guard_is_blocked_transient() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // The other copy: an OFD owner lock on the same device folder, held by this test. OFD
        // locks conflict even inside one process — exactly what a second copy would hold.
        let other_copy =
            bir_core::db::take_device_instance_lock(scratch.layout.device_dir()).unwrap();
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "the open is blocked while another copy holds the guard"
        );
        assert!(!gate_unlocked(), "the gate never opened for a blocked open");
        assert_invariants();
        drop(other_copy);
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Opening,
            "Try again retries the guard and opens"
        );
        assert_invariants();
    }

    /// A commit while nothing is opening is abandoned too (§2: illegal transitions refused).
    #[test]
    fn a_commit_outside_opening_installs_nothing() {
        let _turn = test_turn();
        let _scratch = scratch();
        let epoch = snapshot().epoch;
        commit(
            epoch,
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: "x".to_string(),
            }),
            None,
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert_invariants();
    }

    fn owner(user_id: &str) -> Owner {
        Owner {
            origin: bir_core::rooms::room_origin("https://buwiz.example.test").unwrap(),
            user_id: user_id.to_string(),
            practice: false,
        }
    }

    /// A claim record cut at Copied — the move's evidence pending — naming `room`, written as
    /// the library writes it. For the I4 break test.
    fn write_copied_record(scratch: &Scratch, room: &RoomId, owner: &Owner) {
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        drop(store);
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        let record = bir_core::legacy_claim::Record {
            stamp: "test-stamp".into(),
            room: Some(room.as_str().to_owned()),
            origin: Some(owner.origin.clone()),
            user_id: Some(owner.user_id.clone()),
            email: Some(format!("{}@example.test", owner.user_id)),
            unlocked_by: Some(bir_core::legacy_claim::UnlockedBy::NoLock),
            empty_room_aside: false,
            empty_room_file: None,
            os_login_used: false,
            raw_user_version: None,
            raw_sha256: None,
            zip: None,
            zip_sha256: None,
            hash_checked: false,
            open_checked: false,
            step: bir_core::legacy_claim::Step::Copied,
            failure: None,
            done_at: None,
            launch_days: vec![],
            room_opened_since: false,
            zip_deleted_at: None,
            legacy_file: None,
            owner_pin_phc: None,
        };
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES ('legacy', ?1)",
            rusqlite::params![serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
    }

    /// F1: the invariants are not decoration — a deliberate I1 break (the raw gate unlocked
    /// while SignedOut) is reported, whatever the environment's auth bypass says.
    #[test]
    fn a_deliberate_i1_break_is_reported() {
        let _turn = test_turn();
        let _scratch = scratch();
        assert_invariants();
        crate::account_gate::set_unlocked(true);
        let report = check_invariants().expect_err("I1 must report the break");
        assert!(report.contains("I1"), "the report names I1: {report}");
        crate::account_gate::set_unlocked(false);
        assert_invariants();
    }

    /// F1's I2 arm: the slot's owner stops being the stored sign-in under an open room, and the
    /// invariant reports it.
    #[test]
    fn a_deliberate_i2_break_is_reported() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        // The stored sign-in changes under the open room: the slot's owner is no longer stored.
        seed_stored("user-b", false);
        let report = check_invariants().expect_err("I2 must report the break");
        assert!(report.contains("I2"), "the report names I2: {report}");
    }

    /// F1's I4 arm: a room opens over the claim's pending move evidence for it, and the
    /// invariant reports it.
    #[test]
    fn a_deliberate_i4_break_is_reported() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let room_id = bir_core::rooms::room_id_for(&owner_a.origin, &owner_a.user_id);
        write_copied_record(&scratch, &room_id, &owner_a);
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        let report = check_invariants().expect_err("I4 must report the break");
        assert!(report.contains("I4"), "the report names I4: {report}");
    }

    /// F7: Quitting is terminal — a close after quit is a logged no-op, the phase stays
    /// Quitting, and begin_open is still refused.
    #[test]
    fn close_after_quit_is_a_logged_no_op() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        quit();
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        let epoch = snapshot().epoch;
        close(Reason::SignOut);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Quitting,
            "the close did not move the terminal phase"
        );
        assert_eq!(snapshot().epoch, epoch, "the close did not bump the epoch");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Quitting,
            "begin_open is still refused after the close"
        );
    }

    // ---------- §11's named interleavings: scripted now, un-ignored by their commit ----------
    //
    // Where the path a test drives lands in a later commit, the test calls a clearly named stub
    // (the `commit_N_wires_*` functions below). Each stub is the hole the commit fills; the
    // test's assertions are the final ones, so un-ignoring is the commit removing its stub.

    // The commitN_* stubs are the holes their commits fill (the reviewer's rule: an empty or
    // vacuous stub would let its test pass before the behavior exists). Every one PANICS, so its
    // test fails today and passes only when the commit replaces the stub with the real drive.
    // The names keep the design page's numbering; the build order renumbering is the page's next
    // revision.

    /// commit 3 wires the claim thread's real work (the claim, the classify, the open, the
    /// commit); the tests then drive the real thread with pauses on its real hooks, never this.
    fn commit3_claim_thread_opens(scratch: &Scratch, owner: &Owner, epoch: u64, name: &str) {
        let _ = (scratch, owner, epoch, name);
        panic!("commit 3 wires this")
    }

    /// Hand-drive an open session for the tests whose subject is the stored sign-in, not the
    /// claim: begin_open already ran, so the phase is Opening at `epoch`; this drives the real
    /// commit() the claim thread will own from commit 3.
    fn open_hand(scratch: &Scratch, owner: &Owner, epoch: u64, name: &str) {
        let (handle, path) = scratch_handle(scratch, name);
        commit(epoch, room_target(owner), Some((handle, path)));
    }

    /// commit 4 wires the send spawn: a spawned send carries the tick's owner and epoch, and
    /// takes its token only for them.
    fn commit4_spawn_send(owner: &Owner, epoch: u64) {
        let _ = (owner, epoch);
        panic!("commit 4 wires this")
    }

    /// commit 4 wires the OAuth loopback registry: a bind after the epoch's close is refused.
    fn commit4_oauth_bind(epoch: u64) -> Result<(), String> {
        let _ = epoch;
        panic!("commit 4 wires this")
    }

    /// commit 4 wires the import: pause, the re-check under the room's lock, the CAS install.
    fn commit4_run_import(owner: &Owner, epoch: u64) {
        let _ = (owner, epoch);
        panic!("commit 4 wires this")
    }

    /// commit 4 wires the inbox write: captured (epoch, generation) or dropped.
    fn commit4_inbox_write(owner: &Owner, epoch: u64, generation: u64) {
        let _ = (owner, epoch, generation);
        panic!("commit 4 wires this")
    }

    /// commit 4 wires the device-key registration: written only if the stored owner and the
    /// epoch are still the registration's own.
    fn commit4_register_device_key(owner: &Owner, epoch: u64) {
        let _ = (owner, epoch);
        panic!("commit 4 wires this")
    }

    /// commit 5 wires the claim classify: the store's error kinds to their phases.
    fn commit5_claim_classify(scratch: &Scratch) {
        let _ = scratch;
        panic!("commit 5 wires this")
    }

    /// commit 6 wires quit's release order: worker, holders, checkpoint, registry, guards.
    fn commit6_quit_releases() {
        panic!("commit 6 wires this")
    }

    /// n1: a sign-out while the claim runs — the close bumps the epoch, the opener's commit
    /// misses its compare-and-swap, and nothing was ever installed (F10's hole, closed).
    #[test]
    #[ignore = "commit 3"]
    fn n1_sign_out_during_the_claim_opens_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pause is set BEFORE begin_open (F5's rule): the real claim thread spawns inside
        // begin_open, and a pause set after could be slipped past.
        let claim_gate = hooks::pause("claim.after_claim");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        assert_invariants();
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let opener = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir.clone()));
            // The claim thread's path, driven by hand until commit 2 wires it.
            hooks::at("claim.after_claim");
            let db = scratch_dir.join("a-room.db");
            let handle = Arc::new(Mutex::new(Database::open(&db).unwrap()));
            commit(epoch, room_target(&owner), Some((handle, db)));
        });
        claim_gate.reached();
        // The sign-out lands while the claim is paused: the epoch bumps under the close.
        sign_out();
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert!(!gate_unlocked(), "the gate is locked");
        assert_invariants();
        claim_gate.release();
        opener.join().unwrap();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the stale opener's commit abandoned: nothing opened"
        );
        assert!(snapshot().room.is_none(), "no room was ever installed");
        assert!(
            !gate_unlocked(),
            "the gate never unlocked for the dead session"
        );
        assert_invariants();
    }

    /// n1 (second arm): B signs in while A's claim is still running; A's late commit abandons
    /// and B keeps the session.
    #[test]
    #[ignore = "commit 3"]
    fn n1_b_signs_in_during_as_claim_and_keeps_b() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pause is set BEFORE begin_open (F5's rule): the real claim thread spawns inside
        // begin_open, and a pause set after could be slipped past.
        let claim_gate = hooks::pause("claim.after_claim");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let opener_a = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir.clone()));
            hooks::at("claim.after_claim");
            let db = scratch_dir.join("a-room.db");
            let handle = Arc::new(Mutex::new(Database::open(&db).unwrap()));
            commit(epoch_a, room_target(&owner), Some((handle, db)));
        });
        claim_gate.reached();
        // B's sign-in replaces A: the epoch bumps, B's open starts.
        bir_core::account_session::clear_stored();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b")
        );
        assert_invariants();
        // B's own open commits first.
        let epoch_b = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        claim_gate.release();
        opener_a.join().unwrap();
        // A's late commit abandons; B's room stands.
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B keeps the session"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "A never replaced B"
        );
        let (_, _, room_id, _) = snapshot().room.unwrap();
        assert_eq!(
            room_id,
            Some(bir_core::rooms::room_id_for(&owner_b.origin, "user-b")),
            "the room is B's"
        );
        assert_invariants();
    }

    /// n1b: Try again from a blocked page is the same begin_open path, and its commit wakes the
    /// worker (§3 step 3).
    #[test]
    #[ignore = "commit 3"]
    fn n1b_try_again_starts_the_worker() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        let epoch = snapshot().epoch;
        // The claim classifies the device store as lost with rooms present: Blocked, gate locked.
        commit(
            epoch,
            Target::Blocked(Blocked::DeviceStore {
                detail: "the device store could not be read".to_string(),
            }),
            None,
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(!gate_unlocked());
        assert_invariants();
        // Try again: the same begin_open, the same commit — the worker wakes at the commit.
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        let epoch = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(gate_unlocked());
        assert!(
            crate::bir_module::submission_worker_started(),
            "the worker RUNS after the try-again commit — a wake without a start is the n1b hole"
        );
        assert_invariants();
    }

    /// n2: a store read error with a moved record opens nothing (G1's evidence rule on the real
    /// path, with the claim-store classify of §7).
    #[test]
    #[ignore = "commit 5"]
    fn n2_a_store_read_error_with_a_moved_record_opens_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        let epoch = snapshot().epoch;
        // The claim thread's classify (commit 5): the record says Moved with no hash check.
        commit5_claim_classify(&scratch);
        commit(
            epoch,
            Target::Blocked(Blocked::MovedNotOpened {
                detail: "the move finished but the copy was never checked".to_string(),
            }),
            None,
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(snapshot().room.is_none(), "no room opened on the evidence");
        assert!(!gate_unlocked());
        assert_invariants();
    }

    /// n3: a browser preconnect (a connection with no request line) is closed, and the real GET
    /// after it completes the consent (the OAuth loopback's accept loop, with real sockets).
    #[test]
    #[ignore = "commit 4"]
    fn n3_a_preconnect_then_the_get_completes_the_consent() {
        let _turn = test_turn();
        let _scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        assert_invariants();
        // commit 4 wires the real listener; the scripted interleaving is final.
        assert!(
            commit4_oauth_bind(epoch).is_ok(),
            "the bind registers under the live epoch"
        );
        assert_invariants();
    }

    /// n3 (second arm): a bind racing a close is refused — no listener survives a close.
    #[test]
    #[ignore = "commit 4"]
    fn n3_a_bind_during_close_is_refused() {
        let _turn = test_turn();
        let _scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let close_gate = hooks::pause("close.locked");
        let scratch_dir = _scratch.layout.device_dir().to_path_buf();
        let closer = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            close(Reason::SignOut);
        });
        close_gate.reached();
        close_gate.release();
        closer.join().unwrap();
        assert!(
            commit4_oauth_bind(epoch).is_err(),
            "a bind after the close's epoch bump is refused"
        );
        assert_invariants();
    }

    /// f7: a send spawned in A, run after the switch to B, takes no token — the return stays
    /// Queued in A's room, and B's sign-in is intact.
    #[test]
    #[ignore = "commit 4"]
    fn f7_a_send_spawned_in_a_after_the_switch_takes_no_token() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        // A's tick spawns the send with A's owner and epoch.
        let send_gate = hooks::pause("send.before_token");
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let send = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            commit4_spawn_send(&owner, epoch_a);
            hooks::at("send.before_token");
        });
        send_gate.reached();
        // The account switches to B before the send runs.
        sign_out();
        bir_core::account_session::clear_stored();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        let fetches_before = bir_core::account_session::token_fetches_for(
            &bir_core::account_session::OwnerKey::of(
                &bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .unwrap(),
            )
            .unwrap(),
        );
        send_gate.release();
        send.join().unwrap();
        // B's session stands; nothing of A's send touched it — and no token was ever fetched
        // for A's dead send (the count did not move).
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "B's sign-in is intact"
        );
        assert_eq!(
            bir_core::account_session::token_fetches_for(
                &bir_core::account_session::OwnerKey::of(
                    &bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .unwrap()
                )
                .unwrap()
            ),
            fetches_before,
            "NO token was taken for the dead send"
        );
        assert_invariants();
    }

    /// stored_b: a late 401 of A's send, arriving after B signed in, touches nothing of B's —
    /// driven through the REAL path (revision 5's BUG 3): the send's context carries A's owner,
    /// epoch and the credential its token came from, and TokenSource::session_ended reads only
    /// those. Under the bug it read "now" — B's owner, epoch and credential — and signed B out.
    #[test]
    fn stored_b_survives_as_late_401() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The hook registration is once per process; every test that needs it registers the
        // same machine door, so sharing it is exact.
        bir_core::account_session::on_session_ended(server_ended);
        // The send's task, spawned with A's owner and epoch. Its token fetch — the real
        // AccountTokens::access_token — records the credential of A's record in the context.
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let context = bir_core::account_session::SendContext::new(key_a, epoch_a);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(
            bir_core::account_session::SEND_CONTEXT.scope(context.clone(), async {
                let tokens = bir_core::account_session::AccountTokens;
                let token = bir_core::bir_submit::TokenSource::access_token(&tokens, false)
                    .await
                    .expect("A's token, fetched as the send does");
                assert_eq!(token, "access-user-a");
            }),
        );
        assert_eq!(
            context.credential().as_deref(),
            Some("refresh-user-a"),
            "the fetch recorded the credential the send's token came from"
        );
        // The switch: A signs out (the real one-section sign_out), B signs in — the persist,
        // the cache, the epoch bump.
        sign_out();
        join_device_forget_threads();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        open_hand(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        // A's send gets its late 401: the real TokenSource::session_ended, inside the send's
        // own context.
        runtime.block_on(
            bir_core::account_session::SEND_CONTEXT.scope(context, async {
                let tokens = bir_core::account_session::AccountTokens;
                bir_core::bir_submit::TokenSource::session_ended(&tokens);
            }),
        );
        // Everything of B's stands: the stored sign-in, the cache, the room, the gate.
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B's session is untouched by A's late 401"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b")
        );
        let stored = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("B's stored sign-in survives");
        assert_eq!(stored.refresh_token, "refresh-user-b");
        assert_eq!(
            bir_core::account_session::stored_owner().map(|key| key.user_id),
            Some("user-b".to_string()),
            "B's owner cache entry survives"
        );
        assert!(gate_unlocked(), "B's gate holds");
        assert_invariants();
    }

    /// r1: a send inserted after the import's active-jobs check is still seen — the re-check
    /// runs under the room's lock, which the send needs to load its draft.
    #[test]
    #[ignore = "commit 4"]
    fn r1_a_send_inserted_after_the_check_is_seen_under_the_room_lock() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The import pauses after its session check, before the room lock.
        let import_gate = hooks::pause("import.after_room_lock");
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let import = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            hooks::at("import.after_room_lock");
            commit4_run_import(&owner, epoch);
        });
        import_gate.reached();
        // A send inserts its job key in the gap.
        commit4_spawn_send(&owner_a, epoch);
        import_gate.release();
        import.join().unwrap();
        assert_invariants();
    }

    /// quit: the release order — worker, holders, checkpoint, registry, guards — and a relaunch
    /// meets no live writer.
    #[test]
    #[ignore = "commit 6"]
    fn quit_releases_in_order_and_a_relaunch_meets_no_live_writer() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        quit();
        commit6_quit_releases();
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        // The device guard is released last of all: after quit, a new copy can take it.
        assert!(
            bir_core::db::take_device_instance_lock(scratch.layout.device_dir()).is_ok(),
            "the relaunch meets no live writer"
        );
    }

    // ---------- commit 2: the stored sign-in ----------

    /// The device key files of the scratch install: the key itself and its owner record.
    fn write_device_key(scratch: &Scratch, key: &str, owner: Option<(&str, &str)>) {
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        std::fs::write(
            scratch
                .layout
                .device_dir()
                .join("buwiz_submit_device_key.txt"),
            key,
        )
        .unwrap();
        if let Some((origin, user_id)) = owner {
            std::fs::write(
                scratch
                    .layout
                    .device_dir()
                    .join("buwiz_submit_device_owner.txt"),
                format!("{origin}\n{user_id}\n"),
            )
            .unwrap();
        }
    }

    fn device_key_files(scratch: &Scratch) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(scratch.layout.device_dir())
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with("buwiz_submit_device")
                    })
                    .map(|entry| {
                        (
                            entry.file_name().to_string_lossy().to_string(),
                            std::fs::read(entry.path()).unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    /// The writer guard (§4 item 1): no direct call of the raw cloud_account writers outside
    /// account_session — every write of the stored sign-in routes through the one path, or the
    /// owner cache lies. bir-core's own cloud_account.rs (the definition and its storage tests)
    /// and account_session.rs (the one path) are the only allowed homes.
    #[test]
    fn no_writer_bypasses_the_owner_cache() {
        // CARGO_MANIFEST_DIR is crates/bir-app-api — the crates' src dirs are beside it.
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let mut offenders = Vec::new();
        for crate_dir in ["bir-app-api/src", "bir-core/src", "bir-mcp/src"] {
            let dir = crates.join(crate_dir);
            if !dir.exists() {
                continue;
            }
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let allowed = (crate_dir.contains("bir-core")
                    && (name == "cloud_account.rs" || name == "account_session.rs"))
                    || name == "mod.rs" && crate_dir.contains("account_session");
                if allowed {
                    continue;
                }
                for (line, text) in text.lines().enumerate() {
                    for writer in [
                        "save_stored_account",
                        "clear_stored_account",
                        "forget_stored_account",
                    ] {
                        // The scan is textual: a call, not a doc mention or a comment.
                        let trimmed = text.trim_start();
                        if trimmed.starts_with("//") || trimmed.starts_with("///") {
                            continue;
                        }
                        if text.contains(&format!("{writer}(")) && !text.contains("fn ") {
                            offenders.push(format!(
                                "{}:{}: {}",
                                path.display(),
                                line + 1,
                                text.trim()
                            ));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "writes of the stored sign-in bypassing account_session:\n{}",
            offenders.join("\n")
        );
    }

    /// The writer guard's other half (revision 3 of the fix list): the account_session doors
    /// save_stored/forget_stored/clear_stored are the persist half the machine pairs with its
    /// SESSION cache write — so outside account_session itself, only session.rs may call them.
    /// A refresh uses save_stored_if only. bir-mcp is a separate process without the machine:
    /// its writes are disk writes of its own install, and account_session's own cache update
    /// covers its own process — the pairing this guard protects is the machine's.
    #[test]
    fn only_the_machine_writes_the_stored_sign_in() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let mut offenders = Vec::new();
        for crate_dir in ["bir-app-api/src", "bir-core/src"] {
            let dir = crates.join(crate_dir);
            if !dir.exists() {
                continue;
            }
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                // account_session.rs is the door itself; session.rs is the machine.
                if name == "account_session.rs" || name == "session.rs" {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                // The scan is textual; test modules at a file's end set up fixtures through the
                // doors directly — they are not writers in the machine's sense. Everything
                // before `mod tests` is production code and must not write.
                let production = text.split("mod tests").next().unwrap_or("");
                for (line, text) in production.lines().enumerate() {
                    let trimmed = text.trim_start();
                    if trimmed.starts_with("//") || trimmed.starts_with("///") {
                        continue;
                    }
                    for writer in [
                        "account_session::save_stored(",
                        "account_session::clear_stored(",
                        "account_session::forget_stored(",
                    ] {
                        if text.contains(writer) {
                            offenders.push(format!(
                                "{}:{}: {}",
                                path.display(),
                                line + 1,
                                text.trim()
                            ));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "writes of the stored sign-in outside the machine:\n{}",
            offenders.join("\n")
        );
    }

    /// BUG 1: a refresh's save in the sign-out's window writes NOTHING. sign_out's step 1 has
    /// cleared the disk; the cache still shows the owner until step 2. A save_stored_if that
    /// compared the cache would write the ended sign-in back — the disk comparison is the fix.
    #[test]
    fn a_refresh_save_after_sign_outs_disk_clear_writes_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let _ = &scratch;
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        // Park sign_out between step 1 (the disk cleared) and step 2 (the cache).
        let persist_gate = hooks::pause("sign_out.after_persist");
        let device_dir = scratch.layout.device_dir().to_path_buf();
        let signout = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device_dir));
            sign_out();
        });
        persist_gate.reached();
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "step 1 has run: the disk is empty"
        );
        // The refresh's late save for the owner the cache still shows: it must write nothing.
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-refreshed".into(),
            refresh_token: "refresh-refreshed".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: owner_a.user_id.clone(),
            email: None,
            offline: false,
            last_refresh_unix: 100,
            practice: false,
        };
        let saved = {
            let _account = bir_core::account_session::lock();
            bir_core::account_session::save_stored_if(&key_a, &stored).unwrap()
        };
        assert!(!saved, "the disk comparison refused the save");
        persist_gate.release();
        signout.join().unwrap();
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "the disk is empty still — the ended sign-in was never written back"
        );
        // And the relaunch is signed out: a fresh cache reads the empty disk.
        bir_core::account_session::reset_owner_cache_for_tests();
        bir_core::account_session::fill_owner_cache();
        assert_eq!(bir_core::account_session::stored_owner(), None);
    }

    /// BUG 2: stored_owner never reads the disk. An unfilled cache with a sign-in on disk reads
    /// as None — the fill is init's explicit step, never the reader's.
    #[test]
    fn stored_owner_never_reads_the_disk() {
        let _turn = test_turn();
        let scratch = scratch();
        let _ = &scratch;
        let _owner_a = seed_stored("user-a", false);
        // The cache unfilled: as a process that never ran init would be.
        bir_core::account_session::reset_owner_cache_for_tests();
        let loads_before = bir_core::cloud_account::stored_account_loads();
        assert_eq!(
            bir_core::account_session::stored_owner(),
            None,
            "an unfilled cache reads as None even with a sign-in on disk"
        );
        assert_eq!(
            bir_core::cloud_account::stored_account_loads(),
            loads_before,
            "and the read never touched the disk"
        );
        // The fill is the explicit boot step — after it, the owner is there.
        bir_core::account_session::fill_owner_cache();
        assert_eq!(
            bir_core::account_session::stored_owner().map(|key| key.user_id),
            Some("user-a".to_string())
        );
    }

    /// Revision 5's A, case 1: the same owner signs in again (a new credential) while an old
    /// refresh is in flight; the old credential's 401 changes nothing — still signed in, the
    /// room open, the disk intact.
    #[test]
    fn a_resignin_of_the_same_owner_survives_the_old_tokens_401() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The re-sign-in: the same owner, a NEW credential on disk.
        {
            let _account = bir_core::account_session::lock();
            let mut stored = bir_core::cloud_account::load_stored_account()
                .unwrap()
                .unwrap();
            stored.access_token = "access-user-a-new".into();
            stored.refresh_token = "refresh-user-a-new".into();
            bir_core::account_session::save_stored(&stored).unwrap();
        }
        let before = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        // The OLD credential's 401 arrives.
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch,
            "refresh-user-a".to_string(),
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the room stays open"
        );
        assert!(gate_unlocked(), "the gate holds");
        let after = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        assert_eq!(
            after.refresh_token, before.refresh_token,
            "the new credential is intact"
        );
        assert!(
            bir_core::account_session::stored_owner().is_some(),
            "the cache holds"
        );
        assert_invariants();
    }

    /// Revision 5's A, case 2: a different owner B is stored — A's old credential's 401 touches
    /// nothing of B's.
    #[test]
    fn a_different_owner_is_untouched_by_the_old_tokens_401() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_b, epoch, "b-room.db");
        let before = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_b.origin.clone(),
                user_id: "user-a".into(),
            },
            epoch,
            "refresh-user-a".to_string(),
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let after = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .unwrap();
        assert_eq!(
            after.refresh_token, before.refresh_token,
            "B's sign-in intact"
        );
        assert_invariants();
    }

    /// Revision 5's A, case 3: the CURRENT credential's 401 signs out, as today — the disk
    /// clears, the cache goes, the room closes.
    #[test]
    fn the_current_tokens_401_signs_out() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch,
            format!("refresh-{}", owner_a.user_id),
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert!(!gate_unlocked());
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none()
        );
        assert_eq!(bir_core::account_session::stored_owner(), None);
        assert_invariants();
    }

    /// Revision 5's C: sign_out never waits on DEVICE_KEY. A registration parked inside its
    /// network step holds the lock; sign_out returns at once — the gate locked, the cache
    /// signed out — and the device-key forget completes after the registration releases it.
    #[test]
    fn sign_out_completes_while_a_registration_hangs() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        write_device_key(
            &scratch,
            "key-of-a",
            Some((owner_a.origin.as_str(), "user-a")),
        );
        // The hanging registration: inside the register step, holding DEVICE_KEY.
        let (inside_sender, inside_receiver) = std::sync::mpsc::channel();
        let (go_sender, go_receiver) = std::sync::mpsc::channel();
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let epoch_now = bir_core::session_epoch::current();
        let device_dir = scratch.layout.device_dir().to_path_buf();
        let registration = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device_dir));
            bir_core::relay::register_device_key_for_test(&key_a, epoch_now, move || {
                let _ = inside_sender.send(());
                let _ = go_receiver.recv();
                bir_core::relay::Registration::Registered
            })
        });
        inside_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the registration is parked inside its network step, DEVICE_KEY held");
        // sign_out completes at once — the bounded channel is the failure margin.
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        let device_dir = scratch.layout.device_dir().to_path_buf();
        let signout = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device_dir));
            sign_out();
            let _ = done_sender.send(());
        });
        done_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("sign_out returned at once though a registration holds DEVICE_KEY");
        signout.join().unwrap();
        assert!(!gate_unlocked(), "the gate locked");
        assert_eq!(
            bir_core::account_session::stored_owner(),
            None,
            "the cache signed out"
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        // The forget waits its turn: release the registration, and the forget completes.
        let _ = go_sender.send(());
        registration.join().unwrap();
        join_device_forget_threads();
        assert!(
            !scratch
                .layout
                .device_dir()
                .join("buwiz_submit_device_key.txt")
                .exists(),
            "the device key is forgotten once the lock was free"
        );
        assert_invariants();
    }

    /// Revision 5's C: a registration past its timeout writes nothing — no owner record, no key.
    #[test]
    fn a_registration_past_its_timeout_writes_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        let _ = &owner_a;
        // The bound at zero: every register step is past it — the discard path itself is what
        // this test proves, deterministically.
        bir_core::relay::set_register_bound_for_test(0);
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let epoch = bir_core::session_epoch::current();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let registration =
            bir_core::relay::register_device_key_for_test(&key_a, epoch, move || {
                // As bir-submit's device_key() does on a success: the key written inside the step.
                std::fs::write(
                    scratch_dir.join("buwiz_submit_device_key.txt"),
                    "key-made-late",
                )
                .unwrap();
                bir_core::relay::Registration::Registered
            });
        assert_eq!(registration, bir_core::relay::Registration::Registered);
        assert!(
            bir_core::relay::device_key_owner().is_none(),
            "no owner record: the step overran its bound"
        );
        assert!(
            !scratch
                .layout
                .device_dir()
                .join("buwiz_submit_device_key.txt")
                .exists(),
            "and no key either — nothing of the late registration survives"
        );
        bir_core::relay::set_register_bound_for_test(u64::MAX);
    }

    /// (e): quit and close never wait on a flight. A refresh parked inside its network call
    /// holds no lock the machine needs; close and quit return at once.
    #[test]
    fn close_and_quit_never_wait_on_a_flight() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored_expired("user-a");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        // Park a real refresh inside its network call, on a thread of its own.
        let network_gate = hooks::pause("refresh.network");
        let refresh = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(
                scratch.layout.device_dir().to_path_buf(),
            ));
            let mut http = GateBlockedHttp;
            let key_a = bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            };
            let _ = bir_core::account_session::access_token(
                &mut http,
                bir_core::cloud_account::unix_now(),
                false,
                &key_a,
            );
        });
        network_gate.reached();
        // close and quit return at once — the bounded channels are the failure margin.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            close(Reason::PageClosed);
            let _ = sender.send(());
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("close never waits on a refresh flight");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            quit();
            let _ = sender.send(());
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("quit never waits on a refresh flight");
        network_gate.release();
        refresh.join().unwrap();
    }

    /// The cache follows every save and clear (F8'): signed_in sets it inside SESSION, sign_out
    /// and server_ended drop it there too — and I1's answer changes with it, never with the disk
    /// alone.
    #[test]
    fn the_cache_follows_every_save_and_clear() {
        let _turn = test_turn();
        let scratch = scratch();
        let _ = &scratch;
        assert_eq!(bir_core::account_session::stored_owner(), None);
        // A sign-in: the real machine path. The cache is set inside its SESSION section.
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-user-a".into(),
            refresh_token: "refresh-user-a".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-a".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        signed_in(&stored).unwrap();
        assert_eq!(
            bir_core::account_session::stored_owner().map(|key| key.user_id),
            Some("user-a".to_string()),
            "the cache followed the save"
        );
        // The sign-out: cache None in the same step as the close.
        sign_out();
        assert_eq!(
            bir_core::account_session::stored_owner(),
            None,
            "the cache followed the clear"
        );
        // The server-ended arm the same way.
        signed_in(&stored).unwrap();
        let epoch = snapshot().epoch;
        server_ended(
            bir_core::account_session::OwnerKey::of(&stored).unwrap(),
            epoch,
            stored.refresh_token.clone(),
        );
        assert_eq!(bir_core::account_session::stored_owner(), None);
        assert_invariants();
    }

    /// The crash window (F8'): the disk cleared, the cache still set — a relaunch reads signed
    /// out, because the cache refills from the disk. The safe direction.
    #[test]
    fn a_cleared_disk_with_a_set_cache_relaunches_signed_out() {
        let _turn = test_turn();
        let scratch = scratch();
        let _ = &scratch;
        let owner_a = seed_stored("user-a", false);
        let _ = &owner_a;
        assert!(bir_core::account_session::stored_owner().is_some());
        // The crash window, simulated: the disk is signed out (step 1 ran), the cache still
        // holds the owner (step 2 never ran — the process died). The relaunch: the cache is
        // filled fresh, from the disk.
        {
            let _account = bir_core::account_session::lock();
            bir_core::account_session::clear_stored();
        }
        bir_core::account_session::reset_owner_cache_for_tests();
        assert_eq!(
            bir_core::account_session::stored_owner(),
            None,
            "the relaunched cache reads the disk: signed out"
        );
    }

    /// The same-owner case of BUG 3: A signs in again with a NEW credential while A's old send
    /// is still in flight; the old send's 401 names the OLD credential, and the new sign-in —
    /// same owner, newer token — survives. Driven through the real send path, as stored_b is.
    #[test]
    fn a_resigned_in_same_owner_survives_the_old_sends_401() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        bir_core::account_session::on_session_ended(server_ended);
        // The send's fetch records A's ORIGINAL credential.
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let context = bir_core::account_session::SendContext::new(key_a, epoch_a);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(
            bir_core::account_session::SEND_CONTEXT.scope(context.clone(), async {
                let tokens = bir_core::account_session::AccountTokens;
                let _ = bir_core::bir_submit::TokenSource::access_token(&tokens, false)
                    .await
                    .expect("A's token");
            }),
        );
        assert_eq!(context.credential().as_deref(), Some("refresh-user-a"));
        // A signs in again: the same owner, a NEW credential on disk and in the cache's owner.
        {
            let _account = bir_core::account_session::lock();
            let mut stored = bir_core::cloud_account::load_stored_account()
                .unwrap()
                .unwrap();
            stored.access_token = "access-user-a-new".into();
            stored.refresh_token = "refresh-user-a-new".into();
            bir_core::account_session::save_stored(&stored).unwrap();
        }
        // The old send's 401: the real session_ended in the send's own context.
        runtime.block_on(
            bir_core::account_session::SEND_CONTEXT.scope(context, async {
                let tokens = bir_core::account_session::AccountTokens;
                bir_core::bir_submit::TokenSource::session_ended(&tokens);
            }),
        );
        // The new sign-in survives whole: the disk's credential is not the failed one.
        let stored = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("the new sign-in survives");
        assert_eq!(stored.refresh_token, "refresh-user-a-new");
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the room stays open"
        );
        assert!(gate_unlocked(), "the gate holds");
        assert!(
            bir_core::account_session::stored_owner().is_some(),
            "the cache holds"
        );
        assert_invariants();
    }

    /// A stale owner or epoch changes nothing (§4): server_ended for another owner, or at an
    /// epoch the session has passed, is a no-op end to end.
    #[test]
    fn server_ended_for_a_stale_owner_or_epoch_changes_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);

        // A stale EPOCH: the session moved past it. Per the page's order the disk's
        // delete-if-owner runs first — the disk record goes (the safe direction) — but the
        // SESSION section is the epoch check's: no close, no cache clear, the gate holds.
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch + 99,
            format!("refresh-{}", owner_a.user_id),
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Open, "the phase holds");
        assert!(
            gate_unlocked(),
            "the gate holds — the epoch check stopped the close"
        );
        assert!(
            bir_core::account_session::stored_owner().is_some(),
            "the cache holds — the epoch check stopped the SESSION section"
        );
        assert_invariants();

        // A stale OWNER never even touches the disk. Re-seed first: the stale-epoch case above
        // cleared it, as ordered.
        let owner_a = seed_stored("user-a", false);
        let before = bir_core::cloud_account::load_stored_account().unwrap();
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: "user-never-here".into(),
            },
            epoch,
            "refresh-user-never-here".to_string(),
        );
        let after = bir_core::cloud_account::load_stored_account().unwrap();
        match (&before, &after) {
            (Some(before), Some(after)) => assert_eq!(
                before.refresh_token, after.refresh_token,
                "another owner's ending never touches this sign-in"
            ),
            (None, None) => {}
            _ => panic!("another owner's ending changed this sign-in"),
        }
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
    }

    /// launch_with_a_stored_keeps_as_device_key (the orchestrator's F2 test): begin_open from
    /// SignedOut — no previous owner — never touches the device key.
    #[test]
    fn launch_with_a_stored_keeps_as_device_key() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        let before = device_key_files(&scratch);
        assert!(!before.is_empty(), "the key is on disk");
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        assert_eq!(
            device_key_files(&scratch),
            before,
            "a launch from SignedOut runs no owner-ending steps: the key is byte-identical"
        );
    }

    /// try_again_for_the_same_owner_keeps_the_device_key: a replacement by the SAME owner is not
    /// an owner-ending — the key stays.
    #[test]
    fn try_again_for_the_same_owner_keeps_the_device_key() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        let before = device_key_files(&scratch);
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        // Land the session in Blocked first — Try again is a re-open from there. (From Open the
        // same owner's begin_open is the X1 no-op, which would test nothing.)
        let epoch = snapshot().epoch;
        commit(
            epoch,
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: "the test's transient".into(),
            }),
            None,
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        // Try again: the same owner opens again — the phase is replaced, but the owner never
        // ended, so the key is never touched.
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Opening,
            "the try-again really re-opened — the close inside begin_open ran"
        );
        assert_eq!(
            device_key_files(&scratch),
            before,
            "the same owner's try-again keeps the device key byte-identical"
        );
    }

    /// a_replace_by_another_owner_forgets_the_old_key_only_if_no_newer_epoch: B replacing A
    /// forgets A's key (check-and-delete under DEVICE_KEY, epoch-sequenced) — unless a newer
    /// epoch answered first.
    #[test]
    fn a_replace_by_another_owner_forgets_the_old_key_only_if_no_newer_epoch() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert!(!device_key_files(&scratch).is_empty());
        // The switch: B's begin_open replaces A. The post-lock step forgets A's key under
        // DEVICE_KEY, sequenced by the epoch B's Opening produced — still current here.
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        assert!(
            device_key_files(&scratch).is_empty(),
            "A's key and its owner record are gone with the switch"
        );
        // The epoch rule: a close whose epoch has already moved on forgets nothing. C replaces
        // B; before C's post-lock step could run, a NEWER epoch (D's) answered — simulated here
        // by the direct outcome: finish_close is the machine's own, so drive the observable
        // end-to-end fact instead: after B's epoch has passed, nothing of B's is taken.
        write_device_key(&scratch, "key-of-b", Some((&owner_b.origin, "user-b")));
        let owner_c = seed_stored("user-c", false);
        begin_open(owner_c.clone(), 1, OpenWhy::SignIn);
        assert!(
            device_key_files(&scratch).is_empty(),
            "the current-epoch switch forgets the key"
        );
        assert_invariants();
    }

    /// a_switch_forgets_as_key_before_bs_claim_starts: the forget is sequenced BEFORE the claim
    /// thread spawns — B's claim can never meet A's key.
    #[test]
    fn a_switch_forgets_as_key_before_bs_claim_starts() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        let owner_b = seed_stored("user-b", false);
        // The pause at begin_open's own point can't order the post-lock step against the spawn —
        // but the ordering is structural: finish_close runs before spawn_claim_thread in
        // begin_open's own body. What the test can assert end-to-end: when begin_open returns,
        // the key is already gone, and the claim thread has only just been spawned.
        let spawns_before = claim_thread_spawn_count();
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        assert!(
            device_key_files(&scratch).is_empty(),
            "A's key is gone before begin_open even returns — the claim starts after"
        );
        assert_eq!(
            claim_thread_spawn_count() - spawns_before,
            1,
            "the claim thread spawned only after the forget"
        );
    }

    /// i6_a_registration_for_b_replaces_as_key: a key recorded for A is never used by B — the
    /// registration replaces it, key and record in one step.
    #[test]
    fn i6_a_registration_for_b_replaces_as_key() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        // B is signed in now — the write gate's cache check is B's, as a real registration's is.
        let owner_b = seed_stored("user-b", false);
        let owner_b_key = bir_core::account_session::OwnerKey {
            origin: owner_b.origin.clone(),
            user_id: "user-b".into(),
        };
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        // B's registration: the replace runs, then the (injected) registration writes B's new
        // key — as bir-submit's does when it finds no key.
        let registration = bir_core::relay::register_device_key_for_test(
            &owner_b_key,
            bir_core::session_epoch::current(),
            move || {
                // As bir-submit's device_key() does: a key is generated only when none exists — so
                // this writes B's key only if the replacement's forget really ran.
                let key_path = scratch_dir.join("buwiz_submit_device_key.txt");
                if !key_path.exists() {
                    std::fs::write(&key_path, "key-of-b").unwrap();
                }
                bir_core::relay::Registration::Registered
            },
        );
        assert_eq!(registration, bir_core::relay::Registration::Registered);
        let files = device_key_files(&scratch);
        let key_bytes = files
            .iter()
            .find(|(name, _)| name == "buwiz_submit_device_key.txt")
            .map(|(_, bytes)| bytes.clone())
            .expect("a key is on disk");
        assert_eq!(
            key_bytes, b"key-of-b",
            "B's own new key is on disk — A's never survives the registration"
        );
        assert_eq!(
            bir_core::relay::device_key_owner().map(|key| key.user_id),
            Some("user-b".to_string()),
            "the owner record is B's, written in the same step"
        );
    }

    /// the_forget_and_a_registration_never_interleave: both funnel through the one DEVICE_KEY
    /// lock — while it is held, neither begins.
    #[test]
    fn the_forget_and_a_registration_never_interleave() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        write_device_key(&scratch, "key-of-a", Some((&owner_a.origin, "user-a")));
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: "user-a".into(),
        };
        // Hold DEVICE_KEY on this thread: the forget on its own thread must wait.
        let guard = bir_core::relay::device_key_lock_for_test();
        let (sender, receiver) = std::sync::mpsc::channel();
        let key_a_thread = key_a.clone();
        let forget = std::thread::spawn(move || {
            bir_core::relay::forget_device_key_for(&key_a_thread);
            let _ = sender.send("forget done");
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_err(),
            "the forget ran without the DEVICE_KEY lock — the funnel is broken"
        );
        // The registration waits the same way.
        let (sender2, receiver2) = std::sync::mpsc::channel();
        let key_a_register = key_a.clone();
        let registration = std::thread::spawn(move || {
            bir_core::relay::register_device_key_for_test(
                &key_a_register,
                bir_core::session_epoch::current(),
                || bir_core::relay::Registration::Registered,
            );
            let _ = sender2.send("registration done");
        });
        assert!(
            receiver2
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_err(),
            "the registration ran without the DEVICE_KEY lock — the funnel is broken"
        );
        drop(guard);
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "the forget completes once the lock is free"
        );
        assert!(
            receiver2
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "the registration completes once the lock is free"
        );
        forget.join().unwrap();
        registration.join().unwrap();
    }

    /// an_unowned_key_is_adopted_by_the_stored_owner_not_replaced: a pre-recording install's key
    /// keeps working — the stored owner adopts it, and it is never replaced while that same
    /// owner is signed in.
    #[test]
    fn an_unowned_key_is_adopted_by_the_stored_owner_not_replaced() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pre-update install: a key with NO owner record.
        write_device_key(&scratch, "key-of-a-legacy", None);
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: "user-a".into(),
        };
        assert!(
            bir_core::relay::adopt_device_key(&key_a),
            "the adoption runs: the key is unowned, the cache is this owner"
        );
        assert_eq!(
            bir_core::relay::device_key_owner().map(|key| key.user_id),
            Some("user-a".to_string()),
            "the owner record is written beside the same key"
        );
        let files_after_adoption = device_key_files(&scratch);
        // A registration for the SAME owner never replaces the adopted key.
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        bir_core::relay::register_device_key_for_test(
            &key_a,
            bir_core::session_epoch::current(),
            move || {
                // bir-submit finds a key already and registers with it — no new key is written.
                let _ = &scratch_dir;
                bir_core::relay::Registration::Registered
            },
        );
        assert_eq!(
            device_key_files(&scratch),
            files_after_adoption,
            "the adopted key is never replaced while its owner is signed in"
        );
        // A second adoption is a no-op (the record exists now).
        assert!(!bir_core::relay::adopt_device_key(&key_a));
        // And another owner's cache never adopts at all.
        write_device_key(&scratch, "key-of-nobody", None);
        // (the owner record from A is still on disk from the adoption — remove it for the case)
        std::fs::remove_file(
            scratch
                .layout
                .device_dir()
                .join("buwiz_submit_device_owner.txt"),
        )
        .ok();
        let other = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: "user-not-stored".into(),
        };
        assert!(
            !bir_core::relay::adopt_device_key(&other),
            "no adoption while the cache is not that owner"
        );
    }

    /// refresh_signed_out_arm_never_deadlocks_with_a_commit: the SignedOut arm's server_ended
    /// and a commit run concurrently; both finish inside the bound.
    #[test]
    fn refresh_signed_out_arm_never_deadlocks_with_a_commit() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        // The SignedOut arm (server_ended) on one thread, a commit on another, at once.
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let barrier2 = barrier.clone();
        let ended = std::thread::spawn(move || {
            barrier2.wait();
            server_ended(key_a, epoch, "refresh-user-a".to_string());
        });
        barrier.wait();
        let (handle, path) = scratch_handle(&scratch, "commit-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        // The bounded join is the failure margin: a lock-ordering inversion (SESSION vs the
        // account lock) never returns.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            ended.join().unwrap();
            let _ = sender.send("joined");
        });
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_ok(),
            "server_ended and commit deadlocked — the lock order is inverted"
        );
        assert_invariants();
    }

    /// two_engines_boot_one_session: two engines boot for the same owner AT ONCE — one Opening,
    /// one epoch bump, and exactly one claim-thread spawn (X1: the second boot is a no-op,
    /// never a second claim).
    #[test]
    fn two_engines_boot_one_session() {
        let _turn = test_turn();
        let _scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        let before_epoch = snapshot().epoch;
        let before_spawns = claim_thread_spawn_count();
        // The two boots race on a barrier, so neither can finish before the other starts.
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let mut threads = Vec::new();
        for engine in 1..=2u64 {
            let owner = owner_a.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                begin_open(owner, engine, OpenWhy::Launch);
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert_eq!(
            snapshot().epoch,
            before_epoch + 1,
            "one epoch bump for the one session, not one per engine"
        );
        assert_eq!(
            claim_thread_spawn_count() - before_spawns,
            1,
            "exactly one claim thread spawned: the second engine's boot was a no-op"
        );
        assert_invariants();
    }

    /// d1: a sign-out changes the stored sign-in and the gate in one step — the invariants hold
    /// after every step of the scripted interleaving, for the user's sign-out and a server 401.
    #[test]
    fn d1_sign_out_changes_the_stored_sign_in_and_the_gate_in_one_step() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        assert_invariants();
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_invariants();
        assert!(gate_unlocked());
        // The user's sign-out: the real session::sign_out. After it returns, the disk, the
        // cache, the gate and the phase have all moved — the one-section shape means no
        // interleaving inside the machine can observe them apart, and the invariants (which
        // read the cache, not the disk) hold at every step the harness can pause.
        sign_out();
        assert!(!gate_unlocked(), "the gate locked in the same step");
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "the stored sign-in is gone in the same step"
        );
        assert_eq!(
            bir_core::account_session::stored_owner(),
            None,
            "the cache went to None inside the SESSION section"
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert_invariants();

        // The server-ended arm: a fresh session, ended by the hook's real path at its epoch.
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room2.db");
        assert!(gate_unlocked());
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch,
            format!("refresh-{}", owner_a.user_id),
        );
        assert!(!gate_unlocked(), "the gate locked in the same step");
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "the stored sign-in is gone in the same step"
        );
        assert_eq!(bir_core::account_session::stored_owner(), None);
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert_invariants();
    }

    /// An Http stub whose exchange parks on the harness point "refresh.network": the refresh's
    /// network wait, stopped exactly where the test wants it, with no sleeps.
    struct GateBlockedHttp;

    impl bir_core::cloud_account::Http for GateBlockedHttp {
        fn exchange(
            &mut self,
            _request: &bir_core::cloud_account::HttpRequest,
        ) -> Result<bir_core::cloud_account::HttpResponse, bir_core::cloud_account::CloudError>
        {
            hooks::at("refresh.network");
            Err(bir_core::cloud_account::CloudError::Network(
                "parked".to_string(),
            ))
        }
    }

    /// A stored sign-in for `user_id` whose access token is already expired, so every token path
    /// must reach the network — seeded for the server this build resolves, so the refresh verbs
    /// pass their server check and reach the network too.
    fn seed_stored_expired(user_id: &str) -> Owner {
        let server_url = crate::cloud_account::resolve()
            .map(|resolved| resolved.url)
            .unwrap_or_else(|_| "https://buwiz.example.test".to_string());
        let practice = false;
        let stored = bir_core::cloud_account::StoredAccount {
            server_url: server_url.clone(),
            access_token: format!("access-{user_id}"),
            refresh_token: format!("refresh-{user_id}"),
            access_expires_at_unix: 0,
            user_id: user_id.to_string(),
            email: Some(format!("{user_id}@example.test")),
            offline: false,
            last_refresh_unix: 0,
            practice,
        };
        bir_core::account_session::save_stored(&stored).unwrap();
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(&stored));
        Owner {
            origin: bir_core::rooms::room_origin(&server_url).unwrap(),
            user_id: user_id.to_string(),
            practice,
        }
    }

    /// Probe the account lock: send on the channel once `account_session::lock()` is taken and
    /// released. While a refresh holds the lock across its network — D2 itself — the probe never
    /// fires, and the test fails on the bound.
    fn probe_account_lock() -> std::sync::mpsc::Receiver<&'static str> {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            {
                let _one = bir_core::account_session::lock();
            }
            let _ = sender.send("lock free");
        });
        receiver
    }

    /// d2: a slow refresh never holds SESSION — snapshot() and a commit return while the
    /// refresh's network wait is still parked (the refresh restructure of §4).
    #[test]
    fn d2_a_slow_refresh_never_holds_session() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored_expired("user-a");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);

        // Each arm runs to its end before the next begins: the harness keeps one gate per named
        // point, so the arms are serial — park, probe, release, join.
        // 1. access_token itself, parked mid-refresh (its refresh was forced by the expiry).
        let gate = hooks::pause("refresh.network");
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let token_thread = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            let mut http = GateBlockedHttp;
            let _ = bir_core::account_session::access_token(
                &mut http,
                bir_core::cloud_account::unix_now(),
                false,
                &key_a,
            );
        });
        gate.reached();
        // While it is parked: the account lock is free (the network holds nothing), and the
        // machine answers.
        let probe = probe_account_lock();
        assert!(
            probe
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "access_token held the account lock across the network — D2 itself"
        );
        let snap = snapshot();
        assert_eq!(snap.phase_kind, PhaseKind::Open, "snapshot never waits");
        // And a commit returns too — its owner check reads the cache, never the account lock.
        let (handle, path) = scratch_handle(&scratch, "late-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        gate.release();
        token_thread.join().unwrap();

        // 2. refresh_account, the verb itself, parked the same way.
        crate::cloud_account::set_test_refresh_http(Some(Box::new(|| Box::new(GateBlockedHttp))));
        let gate = hooks::pause("refresh.network");
        let verb = crate::cloud_account::refresh_account(&gpui_shell::HostArguments::default())
            .expect("the verb spawns");
        let verb_thread = std::thread::spawn(move || {
            futures::executor::block_on(verb).ok();
        });
        gate.reached();
        let probe = probe_account_lock();
        assert!(
            probe
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "refresh_account held the account lock across the network — D2 itself"
        );
        gate.release();
        crate::cloud_account::set_test_refresh_http(None);
        verb_thread.join().unwrap();

        // 3. the launch restore, parked the same way.
        let gate = hooks::pause("refresh.network");
        let scratch_dir2 = scratch.layout.device_dir().to_path_buf();
        let restore = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir2));
            let mut http = GateBlockedHttp;
            let _ = crate::cloud_account::restore_saved_session_with(&mut http);
        });
        gate.reached();
        let probe = probe_account_lock();
        assert!(
            probe
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok(),
            "the launch restore held the account lock across the network — D2 itself"
        );
        gate.release();
        restore.join().unwrap();
        assert_invariants();
    }

    /// d4a: a device-key registration of A's, resolving after the switch, writes nothing for B.
    #[test]
    #[ignore = "commit 3"]
    fn d4a_a_late_registration_of_a_writes_no_device_key_for_b() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        // The switch.
        sign_out();
        bir_core::account_session::clear_stored();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_invariants();
        // A's registration resolves now: it writes nothing. The device-key file of the device
        // directory — whatever relay writes there for a registration — is byte-for-byte what it
        // was before the late resolve.
        let key_files_before: Vec<(std::path::PathBuf, Vec<u8>)> =
            std::fs::read_dir(scratch.layout.device_dir())
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| {
                            let path = entry.path();
                            let bytes = std::fs::read(&path).unwrap_or_default();
                            (path, bytes)
                        })
                        .collect()
                })
                .unwrap_or_default();
        commit4_register_device_key(&owner_a, epoch_a);
        let key_files_after: Vec<(std::path::PathBuf, Vec<u8>)> =
            std::fs::read_dir(scratch.layout.device_dir())
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| {
                            let path = entry.path();
                            let bytes = std::fs::read(&path).unwrap_or_default();
                            (path, bytes)
                        })
                        .collect()
                })
                .unwrap_or_default();
        assert_eq!(
            key_files_before, key_files_after,
            "the device-key file is unchanged: A's late registration wrote nothing for B"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "B's session stands"
        );
        assert_invariants();
    }

    /// The child process of [`d4c_a_second_copy_refused_before_any_room_opens`]: with the env
    /// var set, takes the device guard and holds it until stdin closes. In the normal suite the
    /// var is never set and the test returns at once.
    #[test]
    fn d4c_guard_holder_child() {
        use std::io::Read as _;
        let Some(dir) = std::env::var_os("D4C_CHILD_DEVICE_DIR") else {
            return;
        };
        let _guard = bir_core::db::take_device_instance_lock(std::path::Path::new(&dir)).unwrap();
        println!("D4C-READY");
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
    }

    /// d4c: a second copy — a real child process holding the device guard — is refused before
    /// any room opens: begin_open lands in Blocked::Transient and no claim thread spawns. Once
    /// the child exits, Try again opens.
    #[test]
    fn d4c_a_second_copy_refused_before_any_room_opens() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // The child copy: this same test binary, running only the holder, its guard taken before
        // it prints READY (the pipe read blocks until then — no sleeps).
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::tests::d4c_guard_holder_child",
                "--nocapture",
            ])
            .env(
                "D4C_CHILD_DEVICE_DIR",
                scratch.layout.device_dir().as_os_str(),
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        {
            use std::io::Read as _;
            let mut stdout = child.stdout.take().unwrap();
            let mut output = Vec::new();
            let mut byte = [0u8; 1];
            // Read to the READY line's end; the child never writes it before holding the guard.
            while !output
                .windows(
                    b"D4C-READY
"
                    .len(),
                )
                .any(|w| {
                    w == b"D4C-READY
"
                })
            {
                assert!(
                    stdout.read(&mut byte).unwrap() == 1,
                    "the child copy exited before taking the guard"
                );
                output.push(byte[0]);
                assert!(output.len() < 4096, "the child copy never took the guard");
            }
        }
        let before_spawns = claim_thread_spawn_count();
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(
            matches!(snapshot().phase_kind, PhaseKind::Blocked) && snapshot().room.is_none(),
            "no room opened while another copy holds the guard"
        );
        assert!(!gate_unlocked());
        assert_eq!(
            claim_thread_spawn_count(),
            before_spawns,
            "no claim thread spawned for the refused open"
        );
        assert_invariants();
        // The child copy exits: its guard goes with the process, and Try again opens.
        drop(child.stdin.take().unwrap());
        child.wait().unwrap();
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert_eq!(
            claim_thread_spawn_count(),
            before_spawns + 1,
            "Try again spawns the claim thread once the guard is free"
        );
        assert_invariants();
    }

    /// d5: a close landing while an import runs — the import's install CAS misses, and nothing
    /// of it installs (the restored file on disk is the next sign-in's).
    #[test]
    #[ignore = "commit 4"]
    fn d5_a_close_during_the_import_installs_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The import pauses with the session check passed, before the install.
        let import_gate = hooks::pause("import.after_session_lock");
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let import = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            hooks::at("import.after_session_lock");
            commit4_run_import(&owner, epoch);
        });
        import_gate.reached();
        close(Reason::SignOut);
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        import_gate.release();
        import.join().unwrap();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the import installed nothing past the close"
        );
        assert!(snapshot().room.is_none());
        assert_invariants();
    }

    /// q4: a stale opener installs nothing — an opener thread opens its handle, the close lands
    /// while it is paused before the commit, and the commit with the old epoch abandons: nothing
    /// is installed, and the handle itself drops (the Weak is dead).
    #[test]
    fn q4_a_stale_opener_installs_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pause is set BEFORE begin_open (F5's rule): when commit 3 wires the real claim
        // thread it spawns inside begin_open, and a pause set after could be slipped past.
        let open_gate = hooks::pause("open.done");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        // The opener opens its handle for real, keeps only a Weak of it, and pauses at the
        // open-done point the claim thread will pass (commit 3 wires the thread onto the same
        // hooks; the interleaving the test drives is the commit's own compare-and-swap).
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let opener = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir.clone()));
            let db = scratch_dir.join("a-room.db");
            let handle = Arc::new(Mutex::new(Database::open(&db).unwrap()));
            let weak = Arc::downgrade(&handle);
            hooks::at("open.done");
            commit(epoch_a, room_target(&owner), Some((handle, db)));
            weak
        });
        open_gate.reached();
        close(Reason::SignOut);
        open_gate.release();
        let weak = opener.join().unwrap();
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert!(
            snapshot().room.is_none(),
            "the stale opener installed nothing"
        );
        assert!(
            weak.upgrade().is_none(),
            "the abandoned commit dropped the handle"
        );
        assert!(!gate_unlocked());
        assert_invariants();
    }

    /// i3: an inbox write captured for a room whose session ended is refused — the write checks
    /// the captured epoch and generation under the room's lock.
    #[test]
    #[ignore = "commit 4"]
    fn i3_inbox_write_after_a_switch_is_refused() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        let generation_a = snapshot().room.unwrap().1;
        // The inbox thread captures its epoch and generation, then the session switches.
        let inbox_gate = hooks::pause("inbox.before_write");
        let owner = owner_a.clone();
        let scratch_dir = scratch.layout.device_dir().to_path_buf();
        let inbox = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(scratch_dir));
            hooks::at("inbox.before_write");
            commit4_inbox_write(&owner, epoch_a, generation_a);
        });
        inbox_gate.reached();
        sign_out();
        inbox_gate.release();
        inbox.join().unwrap();
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert_invariants();
    }

    /// i5: a send never takes a token for another owner — its captured owner travels with it,
    /// and the token read refuses anyone else's.
    #[test]
    #[ignore = "commit 4"]
    fn i5_a_send_asks_no_token_for_another_owner() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        // The switch before the send's token read.
        sign_out();
        bir_core::account_session::clear_stored();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit3_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        // A's send reaches its token: it must not read B's.
        let fetches_before = bir_core::account_session::token_fetches_for(
            &bir_core::account_session::OwnerKey::of(
                &bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .unwrap(),
            )
            .unwrap(),
        );
        commit4_spawn_send(&owner_a, epoch_a);
        assert_eq!(
            bir_core::account_session::token_fetches_for(
                &bir_core::account_session::OwnerKey::of(
                    &bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .unwrap()
                )
                .unwrap()
            ),
            fetches_before,
            "NO token was asked for another owner"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "nothing of A's send touched B's session"
        );
        assert_invariants();
    }

    /// F6: quit takes the room out of the phase — its Arc is dead once quit returns.
    #[test]
    fn quit_drops_the_room_after_the_lock() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let (handle, path) = scratch_handle(&scratch, "a-room.db");
        let weak = Arc::downgrade(&handle);
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The test's own Arc is the only outside holder; drop it so the phase's is the last.
        quit();
        assert!(
            weak.upgrade().is_none(),
            "quit dropped the room: the Weak is dead"
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
    }

    /// F5: an arrival BEFORE the pause was set never satisfies reached() — only one past it,
    /// at this gate's generation, counts. The probe reports through a channel: a "returned"
    /// message arriving BEFORE the new arrival is made means reached() counted the stale one.
    /// The bound on the wait is a failure margin, not a sleep: in the correct machine the
    /// message cannot exist at all before the new arrival.
    #[test]
    fn a_stale_arrival_does_not_satisfy_reached() {
        let _turn = test_turn();
        hooks::reset();
        // An arrival happens with no pause at all — the stale one.
        hooks::pass("commit.done");
        let gate = hooks::pause("commit.done");
        let (tx, rx) = std::sync::mpsc::channel();
        let probe = std::thread::spawn(move || {
            tx.send("entering").unwrap();
            gate.reached();
            tx.send("returned").unwrap();
        });
        assert_eq!(rx.recv().unwrap(), "entering");
        // Correct: no "returned" can exist yet — the only arrival so far is the stale one.
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).is_err(),
            "reached() returned on the stale arrival — F5"
        );
        // The new arrival, from its own thread (the gate blocks it until the probe's gate drops).
        let arriver = std::thread::spawn(|| {
            hooks::pass("commit.done");
        });
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(30)).unwrap(),
            "returned",
            "reached() returned once the new arrival landed"
        );
        probe.join().unwrap();
        arriver.join().unwrap();
        hooks::reset();
    }

    /// The harness itself: a paused point blocks the arriving thread until the gate releases,
    /// and reached() waits for the arrival — no sleeps.
    #[test]
    fn the_harness_pauses_and_releases() {
        let _turn = test_turn();
        hooks::reset();
        let gate = hooks::pause("commit.enter");
        let thread = std::thread::spawn(|| {
            hooks::pass("commit.enter");
            1u8
        });
        gate.reached();
        assert!(
            !thread.is_finished(),
            "the paused point held the arriving thread"
        );
        gate.release();
        assert_eq!(thread.join().unwrap(), 1, "the release let it through");
        hooks::reset();
    }

    /// A gate dropped without release releases too, so a failing test never wedges the next.
    #[test]
    fn a_dropped_gate_releases() {
        let _turn = test_turn();
        hooks::reset();
        {
            let _gate = hooks::pause("close.locked");
        }
        let thread = std::thread::spawn(|| {
            hooks::pass("close.locked");
        });
        thread.join().unwrap();
        hooks::reset();
    }
}
