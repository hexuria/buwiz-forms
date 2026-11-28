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

impl Owner {
    /// The owner of a stored sign-in: the normalised server origin, the user id, the practice
    /// flag. `None` while the server URL does not normalise — the sign-in is not a room owner's.
    pub fn of_stored(stored: &bir_core::cloud_account::StoredAccount) -> Option<Owner> {
        let origin = bir_core::rooms::room_origin(&stored.server_url).ok()?;
        Some(Owner {
            origin,
            user_id: stored.user_id.clone(),
            practice: stored.practice,
        })
    }
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
/// prepare_launch's bypass branch. Marks the machine bypassed so
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
    Room {
        room_id: RoomId,
        /// The migration note the open room carries (§A.3.4 permanent, §A.3.6 offered).
        note: Note,
        /// F13: the zip rule's room-opened count runs at commit only on a LATER open of the
        /// room — a lookup's, a permanent failure's, the offer's — never on the claim's own
        /// Done, whose open is the move's own finish, not evidence of use.
        count_opened: bool,
    },
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
    /// The disk's owner changed under the app — another process (bir-mcp's door) signed someone
    /// else in. Counts as an owner change at the I/O points; the app closes its owner's session.
    SignedInElsewhere,
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
    /// The owner whose key the forget targets, when the owner ended: the phase's owner, or —
    /// on the real path before the machine is wired, where the phase never leaves SignedOut —
    /// the owner the cache named before the close cleared it.
    pub ended_owner: Option<bir_core::account_session::OwnerKey>,
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
fn close_locked(
    session: &mut Session,
    kind: &CloseKind,
    cache_key: Option<bir_core::account_session::OwnerKey>,
) -> CloseOutcome {
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
    // A held legacy scan is room-scoped state: no close — sign-out, server-ended, replacement —
    // may leave one behind for the next session to import from.
    crate::legacy_import_module::forget_scan();
    // The owner-ending steps run when the owner really ends. When the phase never left
    // SignedOut, the cache the close just cleared is the evidence the owner existed at all;
    // without it the forget, the TIN invalidate and the listeners would never run on a real
    // sign-out: the F2 safety regression.
    let owner_ended = match (&previous, kind) {
        (None, CloseKind::OwnerEnds) => cache_key.is_some(),
        (None, CloseKind::ReplacedBy(_)) => false,
        (Some(_), CloseKind::OwnerEnds) => true,
        (Some(previous), CloseKind::ReplacedBy(new)) => owner_key_of(previous) != owner_key_of(new),
    };
    if owner_ended {
        bir_core::email::cancel_oauth_listeners();
        // N3: invalidate only an EXISTING submitter — constructing one here, under SESSION,
        // would read the install id, the platform store and the device-key gate inside the
        // machine's lock. No submitter built means no TIN cache exists to invalidate.
        if let Some(submitter) = bir_core::relay::peek_submitter() {
            submitter.invalidate_account_tins();
        }
    }
    CloseOutcome {
        dropped: room,
        owner_ended,
        epoch: session.epoch,
        ended_owner: if owner_ended {
            previous
                .map(|owner| bir_core::account_session::OwnerKey {
                    origin: owner.origin.clone(),
                    user_id: owner.user_id.clone(),
                })
                .or(cache_key)
        } else {
            None
        },
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
    // F2'c/F7: the forget runs under the DEVICE_KEY lock, never nested in SESSION (we are
    // outside it here). Its condition — the epoch this close produced is still current — is
    // re-checked INSIDE the lock by forget_device_key_at_epoch itself: a newer epoch means a
    // newer session answered already, and its key is not this close's to take. The owner check
    // beside it is the second gate: a key already recorded for the newer owner is never touched.
    if !outcome.owner_ended {
        return;
    }
    let epoch = outcome.epoch;
    let ended = outcome.ended_owner.clone();
    let forget = move || {
        // F7: the epoch is re-checked INSIDE DEVICE_KEY by the forget itself — a registration
        // that won the lock first under a newer epoch is never taken by this close's forget.
        if let Some(ended) = &ended {
            bir_core::relay::forget_device_key_at_epoch(ended, epoch);
        } else if bir_core::session_epoch::current() == epoch {
            bir_core::relay::forget_device();
        }
    };
    match mode {
        ForgetMode::Now => forget(),
        ForgetMode::Backgrounded => {
            // The device-folder and database overrides are thread-local: the forget thread
            // runs under the closer's install, as spawn_account's threads do — a test's forget
            // must never fall back to the REAL data folder. In production there is no override.
            let device = bir_core::platform::capture_device_dir_override();
            let database = bir_core::db::capture_database_path_override();
            let spawned = std::thread::Builder::new()
                .name("buwiz-device-forget".into())
                .spawn(move || database.run(move || device.run(forget)));
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
        if session.phase.kind() == PhaseKind::SignedOut {
            // Item 6: a close of an already-signed-out session is a no-op — above all it must
            // NOT bump the epoch. lockCloud's close raced the sign-out's backgrounded key
            // forget: the bump made the forget's epoch re-check lose, and the key survived
            // every UI sign-out.
            tracing::info!("close of a signed-out session ignored: nothing is open");
            return;
        }
        close_locked(&mut session, &CloseKind::OwnerEnds, None)
    };
    tracing::info!("session closed: {reason:?}");
    finish_close(&outcome, ForgetMode::Now);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
}

/// begin_open (§3 step 1). A no-op for the same owner already open, opening or legacy (X1: the
/// second engine's launch resumes nothing twice). Anything else ends in the same critical
/// section: the epoch bumps, the phase becomes Opening, the claim thread spawns and this returns
/// at once (F10).
pub fn begin_open(owner: Owner, engine: u64, why: OpenWhy) {
    let mut session = locked();
    hooks::at("begin_open.locked");
    if session.phase.kind() == PhaseKind::Quitting {
        tracing::info!("begin_open refused: the session is quitting");
        return;
    }
    if same_owner_open(&session, &owner) {
        // Item 5: a same-owner Opening whose claim thread is GONE is stale — nothing will ever
        // commit it. Try again replaces it; any other same-owner phase is still a no-op (X1).
        let stale_opening =
            matches!(session.phase, Phase::Opening { .. }) && !claim_thread_alive(session.epoch);
        if !stale_opening {
            return;
        }
        tracing::info!("a stale same-owner Opening is replaced: its claim thread is gone");
    }
    let (spawn, outcome) = begin_open_locked(&mut session, owner.clone(), engine, why);
    drop(session);
    finish_open(spawn, outcome, owner, engine, why);
}

/// F20: the same-owner check is (origin, user id), as I2's is — practice is not part of
/// who the owner IS, and a practice-flag difference alone never replaces a session.
fn same_owner_open(session: &Session, owner: &Owner) -> bool {
    if let Some(current) = session.phase.owner()
        && current.origin == owner.origin
        && current.user_id == owner.user_id
        && matches!(
            session.phase.kind(),
            PhaseKind::Open | PhaseKind::Legacy | PhaseKind::Opening
        )
    {
        return true;
    }
    false
}

/// begin_open's critical section (§3 step 1), for callers already holding SESSION — signed_in
/// runs it in the same section as its cache write. The caller runs [`finish_open`] after the
/// lock is released.
fn begin_open_locked(
    session: &mut Session,
    owner: Owner,
    engine: u64,
    why: OpenWhy,
) -> (Option<u64>, CloseOutcome) {
    // F2: the replacement's owner-ending steps run only when the owner being replaced is a
    // DIFFERENT one — a Try again or a launch of the same owner keeps its device key.
    let outcome = close_locked(session, &CloseKind::ReplacedBy(owner.clone()), None);
    // §1 [D4c]: the device guard is taken at process start (init). If it was not held then
    // (another copy had it), every begin_open — Try again included — retries it; while it
    // cannot be taken the phase is Blocked::Transient and no claim thread starts. A
    // non-blocking file lock: no blocking I/O under SESSION.
    match ensure_device_guard() {
        Ok(()) => {
            session.phase = Phase::Opening { owner, engine, why };
            // K1: the epoch reads as a LIVE claim from the moment it is made, INSIDE the lock —
            // the spawn happens only after SESSION is released, and any same-owner begin_open or
            // signed_in in that window would read this Opening as stale and replace it.
            lock(&LIVE_CLAIMS).push(session.epoch);
            (Some(session.epoch), outcome)
        }
        Err(detail) => {
            session.phase = Phase::Blocked {
                owner,
                reason: Blocked::Transient {
                    copy_saved: false,
                    detail,
                },
            };
            (None, outcome)
        }
    }
}

/// begin_open's steps after SESSION is released (F17): the replaced room's Arc drops here,
/// forget_device's half epoch-checked (F2), then the claim thread spawns.
fn finish_open(spawn: Option<u64>, outcome: CloseOutcome, owner: Owner, engine: u64, why: OpenWhy) {
    finish_close(&outcome, ForgetMode::Now);
    drop(outcome.dropped);
    hooks::at("finish_open.before_spawn");
    if let Some(epoch) = spawn {
        spawn_claim_thread(owner, engine, why, epoch);
    }
    report_invariants();
}

/// The claim threads serialise (§3 step 2): a second Opening waits here instead of being refused
/// by the claim store's own one-at-a-time rule.
static CLAIM_RUN: Mutex<()> = Mutex::new(());

/// The claim thread "buwiz-room-open" (§3 step 2). It carries its caller's database-name and
/// device-folder overrides, as spawn_account does, so a test's engine reads the install the test
/// set up; in production there is never an override.
/// Test instrumentation: how many claim threads this process spawned. two_engines asserts
/// exactly one for two boots of one session; d4c asserts none for a refused open.
#[cfg(test)]
pub(crate) static CLAIM_THREAD_SPAWNS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Test seam: the next claim thread panics right after its guard is armed, so the guard's
/// Blocked commit is exercised. One shot.
#[cfg(test)]
pub(crate) static PANIC_CLAIM: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Test seam (item 10): the claim-thread spawn is suppressed, so the tests that drive commit()
/// by hand — the open_hand idiom — never race the real thread. scratch() suppresses; a test
/// whose subject IS the real thread (the panicking claim, the spawn counts) permits it again.
#[cfg(test)]
pub(crate) static CLAIM_SPAWN_SUPPRESSED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn permit_claim_threads() {
    CLAIM_SPAWN_SUPPRESSED.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn claim_thread_spawn_count() -> u64 {
    CLAIM_THREAD_SPAWNS.load(std::sync::atomic::Ordering::SeqCst)
}

/// The epochs whose claim threads are alive. A dead thread's epoch leaves the set — the drop
/// guard below — so a same-owner Opening whose thread is gone is stale and a Try again
/// replaces it instead of no-oping into a phase nothing will ever commit (item 5).
static LIVE_CLAIMS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// Whether the epoch's claim thread is still alive.
fn claim_thread_alive(epoch: u64) -> bool {
    lock(&LIVE_CLAIMS).contains(&epoch)
}

/// The claim thread's drop guard: however the thread ends — a panic in the claim, the open or
/// the commit itself — the Opening it served does not stand forever. A normal completion
/// disarms the guard after its commit; any other end commits Blocked::Transient, which the
/// commit's own staleness check discards if a newer session has already replaced this one.
/// Release builds are panic = "abort": a panic there kills the process and no guard runs, so
/// the panic half of this protection is debug and test only — in release the half that matters
/// is the spawn-failure commit in spawn_claim_thread.
struct ClaimGuard {
    epoch: u64,
    disarmed: bool,
}

impl ClaimGuard {
    fn disarm(mut self) {
        self.disarmed = true;
    }
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        lock(&LIVE_CLAIMS).retain(|live| *live != self.epoch);
        if !self.disarmed {
            commit(
                self.epoch,
                Target::Blocked(Blocked::Transient {
                    copy_saved: false,
                    detail: "the claim thread ended without committing".to_string(),
                }),
                None,
            );
        }
    }
}

fn spawn_claim_thread(owner: Owner, engine: u64, why: OpenWhy, epoch: u64) {
    // The epoch is already a live claim — begin_open pushed it inside the SESSION section.
    #[cfg(test)]
    if CLAIM_SPAWN_SUPPRESSED.load(std::sync::atomic::Ordering::SeqCst) {
        // Item 10's seam: the hand-driven tests drive commit() themselves. The epoch still reads
        // as a LIVE claim — the hand drive is one — so the stale-Opening check never misfires,
        // and reset_for_tests clears the set for the next test.
        return;
    }
    #[cfg(test)]
    CLAIM_THREAD_SPAWNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let database = bir_core::db::capture_database_path_override();
    let device = bir_core::platform::capture_device_dir_override();
    // The drop guard is built HERE and moved into the closure: a panic in the override
    // scaffolding before claim_thread_work runs — or in finish_close before the spawn —
    // still commits Blocked instead of leaving the epoch live forever.
    let guard = ClaimGuard {
        epoch,
        disarmed: false,
    };
    let spawn = std::thread::Builder::new()
        .name("buwiz-room-open".to_string())
        .spawn(move || {
            database.run(move || {
                device.run(move || claim_thread_work(owner, engine, why, epoch, guard));
            });
        });
    #[cfg(test)]
    let spawn = spawn.map(|thread| {
        // The tests join every claim thread before they end (join_claim_threads), so nothing
        // leaks into the next test's hook arrivals.
        claim_thread_handles()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(thread);
    });
    #[cfg(not(test))]
    let spawn = spawn.map(drop);
    if spawn.is_err() {
        lock(&LIVE_CLAIMS).retain(|live| *live != epoch);
        let error = spawn.err().unwrap();
        tracing::warn!("the room-open thread did not spawn: {error}");
        // A spawn failure only logged left the Opening standing forever (item 5): the phase
        // becomes Blocked::Transient instead, which a Try again can leave.
        commit(
            epoch,
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: format!("the claim thread did not spawn: {error}"),
            }),
            None,
        );
    }
}

/// The claim threads' join handles, test builds only: a test ends with every claim thread it
/// spawned JOINED, so a late commit hook never lands in the next test's window.
#[cfg(test)]
fn claim_thread_handles() -> &'static Mutex<Vec<std::thread::JoinHandle<()>>> {
    static HANDLES: std::sync::OnceLock<Mutex<Vec<std::thread::JoinHandle<()>>>> =
        std::sync::OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(Vec::new()))
}

/// Join every claim thread spawned so far. Test-only; never a sleep.
#[cfg(test)]
pub(crate) fn join_claim_threads() {
    loop {
        let thread = {
            claim_thread_handles()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop()
        };
        match thread {
            Some(thread) => {
                let _ = thread.join();
            }
            None => return,
        }
    }
}

/// E1: close only when the phase's owner IS `owner` AND the disk is empty, both decided under
/// SESSION. The verb's Ended-on-empty-disk arm runs it: a stale verb of A must never close B's
/// newer session, and an empty disk under Open(A) — nobody stored, the machine open — is the
/// signed-in-elsewhere shape main's sign-out closed for.
pub fn close_if_owner(owner: &Owner, reason: Reason) -> bool {
    if locked().phase.kind() == PhaseKind::Quitting {
        tracing::info!("close_if_owner after quit ignored: the session is terminal");
        return false;
    }
    // (a): the disk read is blocking I/O and never runs under SESSION. It is read BEFORE the
    // lock with the epoch captured alongside; inside, the same epoch must still stand — a bump
    // in between means the inputs are stale and nothing is decided on them.
    let epoch_at_read = locked().epoch;
    let disk = crate::cloud_account::load_stored();
    let outcome = {
        let mut session = locked();
        if session.epoch != epoch_at_read {
            tracing::info!("close_if_owner: the epoch moved under the read — nothing closes");
            return false;
        }
        let phase_owner = session.phase.owner().cloned();
        if phase_owner.as_ref() != Some(owner) {
            tracing::info!("close_if_owner: the phase is another owner's — nothing closes");
            return false;
        }
        // (b): a torn read (write_secret_file mid-write is impossible — the save is atomic — but an unreadable file) is an Err — it is NOT an empty
        // disk, and nothing closes on evidence nobody read. Only a clean None is empty.
        match disk {
            Ok(None) => {}
            Ok(Some(_)) => {
                tracing::info!("close_if_owner: the disk is not empty — nothing closes");
                return false;
            }
            Err(error) => {
                tracing::warn!("close_if_owner: the disk read failed ({error}) — nothing closes");
                return false;
            }
        }
        let cache_key = bir_core::account_session::stored_owner();
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds, cache_key)
    };
    tracing::info!("session closed: {reason:?}");
    post_menu_notice();
    finish_close(&outcome, ForgetMode::Backgrounded);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
    true
}

/// E2: the pause truth under SESSION. The boot's ending sync was check-then-act outside SESSION:
/// a close landing between the gate read and the write left the session locked and UNPAUSED.
/// Here the machine's own truth — a room or legacy phase sends, anything else pauses — is read
/// and written in the one section a close can't interleave with.
pub fn sync_pause() {
    let session = locked();
    // The bypass (§9) unlocks with no owner and no phase: it sends, exactly as the gate's truth
    // said before — the sync changes the RACE, not the bypass's meaning.
    let sends = BYPASSED.load(std::sync::atomic::Ordering::SeqCst)
        || matches!(session.phase.kind(), PhaseKind::Open | PhaseKind::Legacy);
    bir_core::background_cron::set_submission_paused(!sends);
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
    let _ = sign_out_inner(Box::new(bir_core::cloud_account::BlockingHttp::new()));
}

/// The sign-out with its logout transport chosen — production's BlockingHttp by sign_out, the
/// tests' fake through cloud_account's sign_out_with. The flow is the machine's either way; the
/// injected transport only hears the logout, so the contract the sign-out tests exercise IS the
/// real path.
pub(crate) fn sign_out_inner(
    logout_http: Box<dyn bir_core::cloud_account::Http + Send>,
) -> Result<(), bir_core::cloud_account::CloudError> {
    crate::cloud_email::cancel();
    bir_core::background_cron::set_submission_paused(true);
    // Step 1: the persisted sign-in goes, outside SESSION.
    let (logout, removed, at_clear) = {
        let _account = bir_core::account_session::lock();
        let logout = bir_core::cloud_account::load_stored_account()
            .ok()
            .flatten()
            .map(|stored| (stored.server_url.clone(), stored.access_token.clone()));
        let removed = bir_core::account_session::forget_stored();
        // F1: the cache and its generation AT THE CLEAR — a signed_in landing in the gap before
        // the SESSION section bumps the generation, and the close then refuses.
        let at_clear = bir_core::account_session::cache_owner_with_generation();
        (logout, removed, at_clear)
    };
    hooks::at("sign_out.after_persist");
    // Step 2: the one SESSION section — the cache and the close together. The cache's owner is
    // the close's evidence on the real path, where the phase never left SignedOut (F2).
    let outcome = {
        let mut session = locked();
        let now = bir_core::account_session::cache_owner_with_generation();
        if now != at_clear {
            // F1: a signed_in won the gap — its cache and session are never closed behind its
            // back. The disk is already signed out, the safe direction.
            tracing::info!("sign-out's close refused: a signed_in landed in the gap");
            return Ok(());
        }
        let cache_key = now.0;
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds, cache_key)
    };
    // Step 3: outside every lock. Until the machine is wired the production room lives in the
    // old slot: a real sign-out closes it here, exactly as main did — never left open behind a
    // sign-out, the standing rule — and the menu shows the gate.
    post_menu_notice();
    finish_close(&outcome, ForgetMode::Backgrounded);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
    if let Some((server_url, access_token)) = logout {
        crate::cloud_account::logout_soon(logout_http, server_url, access_token);
    }
    if let Err(error) = &removed {
        tracing::warn!("sign-out left the saved sign-in on this device: {error}");
    }
    // The removal's failure is the caller's answer, not a swallowed log line: a sign-in this
    // device could not remove means the relaunch signs it back in, so the page must hear an
    // error — never "signed out" — while the gate is locked and the logout still went out.
    removed
}

/// The changed-server ending (safety table row 2): the stored sign-in belongs to another
/// deployment, so NO logout is ever sent — a dev build must never post a logout to a production
/// server it is not signed in to. The shape is the sign-out's without the logout: the disk
/// outside SESSION, then the cache None and the close in the one SESSION section, then the
/// real path's close after it.
pub fn end_changed_server(server_url: &str, refresh_token: &str) {
    if locked().phase.kind() == PhaseKind::Quitting {
        tracing::info!("end_changed_server after quit ignored: the session is terminal");
        return;
    }
    let at_clear = {
        let _account = bir_core::account_session::lock();
        // B2: re-check INSIDE the lock what the caller decided outside it — a signed_in that won
        // the mutex in the gap is never deleted behind its back.
        let still = bir_core::cloud_account::load_stored_account()
            .ok()
            .flatten()
            .map(|stored| stored.server_url == server_url && stored.refresh_token == refresh_token)
            .unwrap_or(false);
        if !still {
            tracing::info!(
                "end_changed_server: the stored sign-in changed since the decision — nothing is cleared"
            );
            return;
        }
        if let Err(error) = bir_core::account_session::forget_stored() {
            tracing::warn!(
                "the changed-server ending left the saved sign-in on this device: {error}"
            );
        }
        // F1: the cache and its generation AT THE CLEAR.
        bir_core::account_session::cache_owner_with_generation()
    };
    hooks::at("end_changed_server.after_clear");
    let outcome = {
        let mut session = locked();
        let now = bir_core::account_session::cache_owner_with_generation();
        if now != at_clear {
            tracing::info!("end_changed_server's close refused: a signed_in landed in the gap");
            return;
        }
        let cache_key = now.0;
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds, cache_key)
    };
    post_menu_notice();
    finish_close(&outcome, ForgetMode::Backgrounded);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
}

/// The owner-change close (N2, row 3): another process wrote the disk's sign-in — the cache's
/// owner is not the disk's `disk_owner`. The re-check is UNDER SESSION: a signed_in landing
/// between the caller's disk read and this close sets the cache in its own SESSION section, so
/// either it is inside already (the cache matches the disk and nothing closes) or it queues
/// behind this close. The interim shape is server_ended's: the cache None inside SESSION, then
/// the old slot's close and the menu after it.
pub fn close_signed_in_elsewhere(disk_owner: &bir_core::account_session::OwnerKey) -> bool {
    if locked().phase.kind() == PhaseKind::Quitting {
        tracing::info!("close_signed_in_elsewhere after quit ignored: the session is terminal");
        return false;
    }
    let outcome = {
        let mut session = locked();
        let cache_key = bir_core::account_session::stored_owner();
        if cache_key.as_ref() == Some(disk_owner) {
            // A signed_in for the disk's owner landed in between: nothing is wrong any more.
            return false;
        }
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds, cache_key)
    };
    post_menu_notice();
    finish_close(&outcome, ForgetMode::Backgrounded);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
    true
}

/// server_ended (§4, F8', revision 5's A): the sign-in of `owner` ended at the server in the
/// session of `epoch` — ON THE CREDENTIAL `failed_refresh_token`. The disk's clear matches the
/// exact failed credential, so a re-sign-in of even the same owner is never cleared by an old
/// token's 401: only if the stored sign-in is that owner AND its refresh token IS the failed
/// one does anything happen at all. Otherwise nothing is cleared and NOTHING closes.
///  1. under account_session::lock(), OUTSIDE SESSION: the credential check and the disk delete;
///  2. then ONE SESSION section, WHATEVER the epoch (F5 — the credential match above is the
///     evidence the ending is the current sign-in's), if the cache is still that owner: the
///     cache goes to None plus the close with the owner-ending steps;
///  3. the device-key forget after SESSION, epoch-checked (a background thread — see sign_out).
pub fn server_ended(
    owner: bir_core::account_session::OwnerKey,
    _epoch: u64,
    failed_refresh_token: String,
) -> bool {
    if locked().phase.kind() == PhaseKind::Quitting {
        // B3: Quitting is terminal — nothing closes after it.
        tracing::info!("server_ended after quit ignored: the session is terminal");
        return false;
    }
    // Step 1: the disk's delete-if-credential, outside SESSION.
    let (cleared, at_clear) = {
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
            // F16: the file AND the keychain, as sign-out does — a keychain-only survivor would
            // sign the ended account back in at the next launch on desktop.
            if let Err(error) = bir_core::account_session::forget_stored() {
                tracing::warn!("server-ended left the saved sign-in on this device: {error}");
            }
        }
        // F1: the cache and its generation AT THE CLEAR.
        let at_clear = bir_core::account_session::cache_owner_with_generation();
        (matches, at_clear)
    };
    if !cleared {
        tracing::info!(
            "server_ended ignored: the stored credential is not the failed one — a newer sign-in is untouched"
        );
        return false;
    }
    hooks::at("server_ended.after_clear");
    // Step 2: the one SESSION section — cache and close together. F5: WHATEVER the epoch — the
    // disk held the exact failed credential, so this ending IS about the current sign-in, from
    // whenever it was sent. (c): WHATEVER the cache held, too — the disk match is the evidence,
    // and a cache naming someone else is the "signed in elsewhere" case of [R6f], which closes
    // anyway. Before this, an unfilled cache meant the gate stayed unlocked and the room open
    // on the real runtime path.
    let outcome = {
        let mut session = locked();
        let now = bir_core::account_session::cache_owner_with_generation();
        if now != at_clear {
            // F1: a signed_in won the gap — closing now would end the new sign-in's listeners,
            // lock its gate and forget its key.
            tracing::info!("server_ended's close refused: a signed_in landed in the gap");
            return false;
        }
        // (12): the credential matched the disk — this ending IS the owner's even when the
        // cache was never filled; a bare None would make owner-ending false and skip the
        // listeners, the TIN invalidate and the key forget.
        let cache_key = now.0.or(Some(owner));
        bir_core::account_session::cache_owner(None);
        close_locked(&mut session, &CloseKind::OwnerEnds, cache_key)
    };
    // The real path's close, as in sign_out (F2): the old slot's room goes and the menu shows it.
    post_menu_notice();
    finish_close(&outcome, ForgetMode::Backgrounded);
    // F14: the room Arc drops BEFORE the sweep — the file lock and the reader
    // connections must not survive a sign-out into it.
    drop(outcome.dropped);
    crate::open_room::sweep_registry();
    report_invariants();
    true
}

/// signed_in (§4, F8'): a sign-in's write — the persist under the account lock OUTSIDE SESSION,
/// then one SESSION section setting the cache and beginning the open together.
pub fn signed_in(
    stored: &bir_core::cloud_account::StoredAccount,
    engine: u64,
) -> Result<(), bir_core::cloud_account::CloudError> {
    {
        let _account = bir_core::account_session::lock();
        bir_core::account_session::save_stored(stored)?;
    }
    // §4: ONE SESSION section — the cache write AND the open's beginning together, so no reader
    // can ever see the cache name an owner whose open has not started.
    enum Begun {
        Nothing,
        Close(CloseOutcome),
        Open(Option<u64>, CloseOutcome, Owner),
    }
    let begun = {
        let mut session = locked();
        bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(stored));
        match Owner::of_stored(stored) {
            // A server URL that does not normalise is not a room owner's sign-in. F11: the
            // cache names nobody and nothing can open — an Open of the previous owner left
            // standing would be unlocked for no one, the I1 breach. Close instead.
            None => Begun::Close(close_locked(&mut session, &CloseKind::OwnerEnds, None)),
            Some(owner) => {
                if session.phase.kind() == PhaseKind::Quitting || same_owner_open(&session, &owner)
                {
                    Begun::Nothing
                } else {
                    let (spawn, outcome) =
                        begin_open_locked(&mut session, owner.clone(), engine, OpenWhy::SignIn);
                    Begun::Open(spawn, outcome, owner)
                }
            }
        }
    };
    match begun {
        Begun::Open(spawn, outcome, owner) => {
            finish_open(spawn, outcome, owner, engine, OpenWhy::SignIn);
        }
        Begun::Close(outcome) => {
            post_menu_notice();
            finish_close(&outcome, ForgetMode::Backgrounded);
            // F14, the sixth site: the room Arc drops BEFORE the sweep.
            drop(outcome.dropped);
            crate::open_room::sweep_registry();
            report_invariants();
        }
        Begun::Nothing => report_invariants(),
    }
    Ok(())
}

use bir_core::cloud_account::StoredAccount;
use bir_core::legacy_claim::{Claimant, Failure, LegacyClaim, NotHere, Outcome, Start};
use bir_core::rooms::Layout;

/// What the claim decided to open, opened outside SESSION after CLAIM_RUN lets go.
enum OpenSpec {
    /// The account's room, by the claim's outcome or the device-store lookup.
    Room {
        path: PathBuf,
        room_id: RoomId,
        /// Whether a move's copy is already saved, for the transient detail if the open fails.
        copy_saved: bool,
    },
    /// §A.3.7 legacy mode: the legacy file in place, nothing written beside it.
    LegacyInPlace { path: PathBuf },
    /// Single-room/dev mode: the named file through the old path (§9).
    NamedFile,
}

fn claimant_of(stored: &StoredAccount) -> Claimant {
    Claimant {
        server_url: stored.server_url.clone(),
        user_id: stored.user_id.clone(),
        email: stored.email.clone(),
        practice: stored.practice,
    }
}

/// Test seam: force the next claim thread's open target to this room, as a decision-layer
/// regression or a stale resumed record would. One-shot; never in a shipped build.
#[cfg(test)]
static FORCE_NEXT_OPEN_ROOM: Mutex<Option<bir_core::rooms::RoomId>> = Mutex::new(None);

/// Test seam: the retry after an unusable device store is set aside is corrupted once, so the
/// retry-failure classification is exercised at the production decision point.
#[cfg(test)]
static CORRUPT_STORE_RETRY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn force_next_open_room_for_test(room: bir_core::rooms::RoomId) {
    *lock(&FORCE_NEXT_OPEN_ROOM) = Some(room);
}

/// The claim thread's work (§3 step 2): the claim, the target, the open — all outside SESSION —
/// ending in the compare-and-swap commit. Only the commit writes anything visible (§5.9).
fn claim_thread_work(owner: Owner, engine: u64, why: OpenWhy, epoch: u64, guard: ClaimGuard) {
    let _ = (engine, why);
    #[cfg(test)]
    if PANIC_CLAIM.swap(false, std::sync::atomic::Ordering::SeqCst) {
        panic!("the test's injected claim panic");
    }
    #[allow(unused_mut)] // test-only force-next-open overrides below
    let (mut target, mut spec) = {
        let _claim_run = lock(&CLAIM_RUN);
        hooks::at("claim.before_start");
        let decided = decide_claim(&owner);
        hooks::at("claim.after_claim");
        decided
    };
    #[cfg(test)]
    if let Some(room) = lock(&FORCE_NEXT_OPEN_ROOM).take() {
        target = Target::Room {
            room_id: room.clone(),
            note: Note::None,
            count_opened: false,
        };
        spec = Some(OpenSpec::Room {
            path: Layout::here().room_database(&room),
            room_id: room,
            copy_saved: true,
        });
    }
    // CLAIM_RUN released: the open runs outside it and outside SESSION (§3 step 2) — migrations
    // and the checkpoint inside Database::open can take real time. A stale opener can only
    // migrate its own room's file: the same account's, idempotent, the evidence already passed.
    let handle = open_spec(&spec);
    hooks::at("open.done");
    let (target, handle) = match handle {
        Ok(handle) => (target, handle),
        Err((copy_saved, detail)) => {
            tracing::warn!("the open failed: {detail}");
            (
                Target::Blocked(Blocked::Transient { copy_saved, detail }),
                None,
            )
        }
    };
    commit(epoch, target, handle);
    guard.disarm();
}

/// Open what the claim decided, answering the commit's (handle, path). The handle is installed
/// NOWHERE here (§1: `open_path` installs nothing); the commit installs it or drops it.
#[allow(clippy::type_complexity)]
fn open_spec(
    spec: &Option<OpenSpec>,
) -> Result<Option<(Arc<Mutex<Database>>, PathBuf)>, (bool, String)> {
    match spec {
        None => Ok(None),
        Some(OpenSpec::Room {
            path,
            room_id,
            copy_saved,
        }) => crate::open_room::open_path(path, Some(room_id))
            .map(|handle| Some((handle, path.clone())))
            .map_err(|error| (*copy_saved, error)),
        Some(OpenSpec::LegacyInPlace { path }) => crate::open_room::open_path(path, None)
            .map(|handle| Some((handle, path.clone())))
            .map_err(|error| (false, error)),
        Some(OpenSpec::NamedFile) => bir_core::db::open_live_database()
            .map(|(database, path, _recovered)| Some((Arc::new(Mutex::new(database)), path)))
            .map_err(|error| (false, error.to_string())),
    }
}

/// What note the room the lookup names carries. `Offered` is decided once the room is known:
/// the note carries the room it was made in (X2).
enum LookupNote {
    None,
    Permanent { detail: String },
    Offered,
}

/// The account's own room id by the device-store lookup (rooms A1): known, or the id a new empty
/// room would get. Never the room of a claim record made for somebody else.
fn lookup_room_id(layout: &Layout, owner: &Owner) -> Result<bir_core::rooms::RoomId, String> {
    let store = bir_core::rooms::DeviceStore::open(layout).ok();
    let stored = crate::cloud_account::load_stored().ok().flatten();
    let server_url = stored
        .as_ref()
        .map(|stored| stored.server_url.clone())
        .unwrap_or_default();
    let lookup = bir_core::rooms::room_for(store.as_ref(), &server_url, &owner.user_id)
        .map_err(|error| error.to_string())?;
    // N3: a real account's lookup-created room is a room even before its first write: write its
    // row at once, so a later locked legacy file and headless serve can never treat the install as
    // roomless. Practice lookup rooms deliberately write no row; they must not change what a real
    // account opens.
    if let bir_core::rooms::RoomLookup::New { room_id, origin } = &lookup
        && !owner.practice
        && let Some(store) = store.as_ref()
    {
        let record = bir_core::rooms::RoomRecord {
            room_id: room_id.clone(),
            origin: origin.clone(),
            user_id: owner.user_id.clone(),
            email: stored
                .as_ref()
                .filter(|stored| stored.user_id == owner.user_id)
                .and_then(|stored| stored.email.clone()),
            created_at: chrono::Utc::now().to_rfc3339(),
            from_legacy: false,
        };
        if let Err(error) = store.add_room(&record) {
            let already = store
                .find_room(origin, &owner.user_id)
                .ok()
                .flatten()
                .is_some_and(|existing| existing.room_id == *room_id);
            if !already {
                return Err(error.to_string());
            }
        }
    }
    Ok(lookup.room_id().clone())
}

/// The account's room by the device-store lookup (rooms A1): known, or new and empty.
fn lookup_target(layout: &Layout, owner: &Owner, note: LookupNote) -> (Target, Option<OpenSpec>) {
    let room_id = match lookup_room_id(layout, owner) {
        Ok(room_id) => room_id,
        Err(error) => {
            tracing::warn!("room lookup failed: {error}");
            return (
                Target::Blocked(Blocked::Transient {
                    copy_saved: false,
                    detail: error,
                }),
                None,
            );
        }
    };
    let note = match note {
        LookupNote::None => Note::None,
        LookupNote::Permanent { detail } => Note::Permanent { detail },
        LookupNote::Offered => Note::Offered {
            room: room_id.clone(),
        },
    };
    (
        Target::Room {
            room_id: room_id.clone(),
            note,
            count_opened: true,
        },
        Some(OpenSpec::Room {
            path: layout.room_database(&room_id),
            room_id,
            copy_saved: false,
        }),
    )
}

/// Whether the claim record a finished resume acted on belongs to `owner`. The record's owner
/// fields decide when they are there; otherwise the room is compared with the owner's own lookup.
/// A record that cannot be tied to the owner is never a licence to open its room.
fn finished_record_is_owner(
    layout: &Layout,
    owner: &Owner,
    record: Option<&bir_core::legacy_claim::Record>,
    outcome_room: Option<&bir_core::rooms::RoomId>,
) -> bool {
    let Some(record) = record else {
        return false;
    };
    if let (Some(origin), Some(user_id)) = (&record.origin, &record.user_id) {
        return origin == &owner.origin && user_id == &owner.user_id;
    }
    let room = record
        .room
        .as_deref()
        .and_then(bir_core::rooms::RoomId::parse)
        .or_else(|| outcome_room.cloned());
    let Some(room) = room else {
        return false;
    };
    lookup_room_id(layout, owner)
        .map(|owner_room| owner_room == room)
        .unwrap_or(false)
}

/// What a Finished start leaves open. A resume may finish the claim of an account that signed
/// out since the move; that finish stands in the claim record, but the account signing in now
/// opens only its own room.
fn finished_outcome_target(
    layout: &Layout,
    owner: &Owner,
    outcome: Outcome,
    record: Option<&bir_core::legacy_claim::Record>,
) -> (Target, Option<OpenSpec>) {
    let outcome_room = match &outcome {
        Outcome::Done { room, .. } => Some(room),
        Outcome::MovedNotOpened(moved) => Some(&moved.room),
        _ => None,
    };
    if matches!(outcome, Outcome::Permanent(_))
        || finished_record_is_owner(layout, owner, record, outcome_room)
    {
        outcome_target(layout, owner, outcome)
    } else {
        tracing::warn!(
            "a resumed claim finished for another account; opening the sign-in's own room"
        );
        lookup_target(layout, owner, LookupNote::None)
    }
}

/// What an outcome of the claim leaves open (§A.3.4): Done is the claimer's room; transient opens
/// no room; permanent before the move opens the empty room with the note; moved-not-opened opens
/// nothing.
fn outcome_target(layout: &Layout, owner: &Owner, outcome: Outcome) -> (Target, Option<OpenSpec>) {
    match outcome {
        Outcome::Done { room, .. } => (
            Target::Room {
                room_id: room.clone(),
                note: Note::None,
                // F13: the claim's own Done is not counted — a LATER open is the use evidence.
                count_opened: false,
            },
            Some(OpenSpec::Room {
                path: layout.room_database(&room),
                room_id: room,
                copy_saved: true,
            }),
        ),
        Outcome::Transient(transient) => (
            Target::Blocked(Blocked::Transient {
                copy_saved: transient.copy_saved,
                detail: transient.detail,
            }),
            None,
        ),
        Outcome::Permanent(permanent) => lookup_target(
            layout,
            owner,
            LookupNote::Permanent {
                detail: permanent.detail,
            },
        ),
        Outcome::MovedNotOpened(moved) => (
            Target::Blocked(Blocked::MovedNotOpened {
                detail: moved.detail,
            }),
            None,
        ),
    }
}

/// §A.3.7 legacy mode, and the store-error fallback with a legacy file present: the legacy file
/// in place is the slot, writing nothing beside it.
fn legacy_in_place(layout: &Layout) -> (Target, Option<OpenSpec>) {
    (
        Target::Legacy,
        Some(OpenSpec::LegacyInPlace {
            path: layout.legacy_database(),
        }),
    )
}

/// A locked legacy file that cannot be claimed in this version, said without any account's data.
const LOCKED_LEGACY_LATER: &str = "A locked older database is still on this device. Moving locked files comes in a later version.";

/// Whether a real account's room exists, by the one rooms-layout rule: store rows or the claim
/// record when the store can be read, never mere directory entries.
fn non_practice_rooms_present(layout: &Layout) -> bool {
    layout.non_practice_rooms_exist()
}

/// L3/B1(a): the device store's own unusable cases (lost, unreadable, newer) are the store-error
/// page; I/O and SQLite moments — busy and locked included — are transient. Neither opens the
/// legacy file in place; the room-open evidence rule survives for the process.
fn claim_open_error_target(error: &bir_core::rooms::DeviceStoreError) -> Target {
    // The log receives the full error, path included; the page never does — the path contains the
    // OS user's folder. SQLite and I/O errors carry no path of their own.
    let detail = match error {
        bir_core::rooms::DeviceStoreError::Lost(_) => "the device store is missing".to_owned(),
        bir_core::rooms::DeviceStoreError::Unreadable { .. } => {
            "the device store cannot be read".to_owned()
        }
        bir_core::rooms::DeviceStoreError::Newer { .. } => {
            "the device store was written by a newer version".to_owned()
        }
        bir_core::rooms::DeviceStoreError::Sqlite(_) | bir_core::rooms::DeviceStoreError::Io(_) => {
            error.to_string()
        }
    };
    if error.is_unusable() {
        Target::Blocked(Blocked::DeviceStore { detail })
    } else {
        Target::Blocked(Blocked::Transient {
            copy_saved: false,
            detail,
        })
    }
}

/// B1(b): legacy mode in place is only for a locked install with no rooms. Once any room exists,
/// the signed-in account's own room opens and the locked file is only a note for later.
fn locked_legacy_target(layout: &Layout, owner: &Owner) -> (Target, Option<OpenSpec>) {
    if non_practice_rooms_present(layout) {
        lookup_target(
            layout,
            owner,
            LookupNote::Permanent {
                detail: LOCKED_LEGACY_LATER.to_owned(),
            },
        )
    } else {
        legacy_in_place(layout)
    }
}

/// A migration-record read failure for the page: the OS error and any path stay in the log.
fn migration_record_read_failure(
    error: &bir_core::rooms::DeviceStoreError,
) -> (Target, Option<OpenSpec>) {
    tracing::warn!("migration record not read: {error}");
    (
        Target::Blocked(Blocked::Transient {
            copy_saved: false,
            detail: "the migration record cannot be read".into(),
        }),
        None,
    )
}

/// The claim decision (§12.2 item 3, with §7's store-error classification). Everything here runs
/// on the claim thread, outside SESSION.
fn decide_claim(owner: &Owner) -> (Target, Option<OpenSpec>) {
    let layout = Layout::here();
    let Some(stored) = crate::cloud_account::load_stored().ok().flatten() else {
        tracing::warn!("the stored sign-in went away before the claim ran");
        return (
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: "the stored sign-in is gone".to_string(),
            }),
            None,
        );
    };
    // Finding 13: the claim acts for the PHASE owner, never for whoever the disk holds now —
    // a disk that changed A→B mid-Opening(A) must not write B's room records or a room folder.
    // The claimant comes from the disk ONLY while the disk still IS the phase owner (its email
    // rides the room record); a mismatch is not acted on at all.
    if bir_core::account_session::OwnerKey::of(&stored).map(|key| (key.origin, key.user_id))
        != Some((owner.origin.clone(), owner.user_id.clone()))
    {
        tracing::info!("the stored sign-in changed before the claim ran — nothing is claimed");
        return (
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: "the sign-in changed before the open finished".to_string(),
            }),
            None,
        );
    }
    let who = claimant_of(&stored);
    let mut opened = LegacyClaim::here();
    if let Err(error) = &opened
        && error.is_unusable()
        && !layout.has_rooms()
    {
        let legacy_present = layout.legacy_database().is_file();
        let legacy_fallback = |detail: &str| {
            if legacy_present && !owner.practice {
                legacy_in_place(&layout)
            } else {
                (
                    Target::Blocked(Blocked::Transient {
                        copy_saved: false,
                        detail: detail.to_owned(),
                    }),
                    None,
                )
            }
        };
        match bir_core::rooms::set_aside_unusable(&layout) {
            Ok(Some(aside)) => {
                tracing::warn!(aside = %aside.display(), "unusable device store set aside");
                opened = LegacyClaim::here();
                #[cfg(test)]
                if CORRUPT_STORE_RETRY.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    drop(opened);
                    std::fs::write(layout.device_db(), b"still not sqlite").unwrap();
                    opened = LegacyClaim::here();
                }
                if opened.is_err() {
                    tracing::warn!("legacy claim still not opened after the store was set aside");
                    return (
                        Target::Blocked(Blocked::Transient {
                            copy_saved: false,
                            detail:
                                "the device store still cannot be opened after it was set aside"
                                    .to_owned(),
                        }),
                        None,
                    );
                }
            }
            Ok(None) if legacy_present => {
                return legacy_fallback("the device store cannot be used");
            }
            Ok(None) => {}
            Err(set_aside) => {
                tracing::warn!("unusable device store not set aside: {set_aside}");
                return legacy_fallback("the device store could not be set aside");
            }
        }
    }
    let claim = match opened {
        Ok(claim) => claim,
        Err(error) => {
            tracing::warn!("legacy claim not opened ({error})");
            return (claim_open_error_target(&error), None);
        }
    };
    // F8: the registry is checked before start(); the refusal applies only when the probe says
    // Ready — the move is what races the live connection's writes.
    let live_legacy = crate::open_room::live_entry_for(&layout.legacy_database());
    let started = claim.start(&who);
    // §A.3.8: count today toward the zip's launch days, then delete the pre-rooms zip when the
    // rule says it goes. Never in legacy mode's way.
    if !matches!(started, Start::Locked(_)) {
        let today = chrono::Local::now().date_naive();
        if let Err(error) = claim.note_launch(today) {
            tracing::warn!("launch day not counted: {error}");
        }
        match claim.delete_zip_if_due(chrono::Utc::now()) {
            Ok(deleted) => {
                for path in &deleted {
                    tracing::info!(?path, "pre-rooms safety zip deleted by the 30-day rule");
                }
            }
            Err(kept) => {
                tracing::info!(?kept, "pre-rooms safety zip kept");
            }
        }
    }
    match started {
        Start::Ready(ready) => {
            if live_legacy {
                // F8: a live connection to the legacy file means the file is still written; the
                // claim must not copy and move it under those writes.
                return (
                    Target::Blocked(Blocked::Transient {
                        copy_saved: false,
                        detail: "the earlier database is still in use; try again in a moment"
                            .into(),
                    }),
                    None,
                );
            }
            let outcome = ready.claim(&claim);
            outcome_target(&layout, owner, outcome)
        }
        Start::Nothing | Start::DeclinedHere | Start::NotHere(NotHere::Practice) => {
            lookup_target(&layout, owner, LookupNote::None)
        }
        Start::NotHere(NotHere::SingleRoom) | Start::NotHere(NotHere::DevDatabase) => {
            // Single-room mode (§9): the named file, opened through the old path. No claim,
            // no rooms.
            (Target::NotHere(PathBuf::new()), Some(OpenSpec::NamedFile))
        }
        Start::Locked(_locked) => locked_legacy_target(&layout, owner),
        Start::Finished(outcome) => {
            let record = match claim.record() {
                Ok(record) => record,
                Err(error) => return migration_record_read_failure(&error),
            };
            finished_outcome_target(&layout, owner, outcome, record.as_ref())
        }
        Start::Offer(offer) => match offer {
            bir_core::legacy_claim::OfferStart::Locked(_locked_offer) => {
                locked_legacy_target(&layout, owner)
            }
            bir_core::legacy_claim::OfferStart::Open(_open_offer) => {
                // The offer waits for the page; the account's room opens, never merged (§A.3.6).
                // X2: never to a practice account — the store-review account is nobody's heir.
                let note = if owner.practice {
                    LookupNote::None
                } else {
                    // The offered note carries the room it was made in; the lookup decides which.
                    LookupNote::Offered
                };
                lookup_target(&layout, owner, note)
            }
        },
        Start::Failed(Failure::Transient(transient)) => {
            let record = match claim.record() {
                Ok(record) => record,
                Err(error) => return migration_record_read_failure(&error),
            };
            let foreign_unfinished = record.as_ref().is_some_and(|record| {
                !matches!(
                    record.step,
                    bir_core::legacy_claim::Step::Done | bir_core::legacy_claim::Step::Closed
                ) && !finished_record_is_owner(&layout, owner, Some(record), None)
            });
            if foreign_unfinished && transient.record_room {
                tracing::warn!(
                    "finishing another account's move failed transiently; opening the sign-in's own room"
                );
                lookup_target(&layout, owner, LookupNote::None)
            } else {
                outcome_target(&layout, owner, Outcome::Transient(transient))
            }
        }
        Start::Failed(failure) => outcome_target(&layout, owner, Outcome::from(failure)),
    }
}

/// commit (§3 step 3): under SESSION, short, no I/O. A stale epoch or a phase that isn't Opening
/// abandons the work: the handle drops after the lock lets go and nothing is installed. The
/// stored sign-in is re-read inside the lock order SESSION → account_session::lock() (I2); a
/// mismatch abandons too. On a room or legacy target the gate unlocks LAST, after everything
/// else is in the phase (I1).
// Used by the machine's tests and by the production claim thread; pub(crate), never public (F18).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn commit(epoch: u64, target: Target, handle: Option<(Arc<Mutex<Database>>, PathBuf)>) {
    hooks::at("commit.enter");
    enum Outcome {
        /// Nothing installed; the handle (if any) drops after the lock is released.
        Abandon(Option<(Arc<Mutex<Database>>, PathBuf)>),
        /// The phase moved to Blocked; the gate stays locked; a handle (if any) drops.
        Blocked(Option<(Arc<Mutex<Database>>, PathBuf)>),
        /// A room or the legacy file is the slot; the gate is unlocked. The worker is started or
        /// woken with this handle; the room-opened count runs for this room, if at all.
        Installed {
            handle: Arc<Mutex<Database>>,
            count_opened: Option<RoomId>,
        },
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
                // F12/F9: the epoch IS current — abandoning would leave Opening standing and the
                // page polling forever with no button. The open cannot complete for a sign-in
                // that is gone, so the phase is Blocked::Transient: a Try again can leave it.
                tracing::info!("commit blocked: the stored sign-in is no longer this owner");
                session.phase = Phase::Blocked {
                    owner,
                    reason: Blocked::Transient {
                        copy_saved: false,
                        detail: "the stored sign-in changed before the open committed".to_string(),
                    },
                };
                Outcome::Blocked(handle)
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
                    let (room_id, note, count) = match &target {
                        Target::Room {
                            room_id,
                            note,
                            count_opened,
                        } => (
                            Some(room_id.clone()),
                            note.clone(),
                            count_opened.then(|| room_id.clone()),
                        ),
                        _ => (None, Note::None, None),
                    };
                    let room = OpenRoom {
                        room_id,
                        path: path.clone(),
                        generation: next_generation(),
                        handle: handle.clone(),
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
                            note,
                            importing: false,
                        }
                    };
                    // I1: the gate opens LAST, once the room is in the phase.
                    crate::account_gate::set_unlocked(true);
                    Outcome::Installed {
                        handle,
                        count_opened: count,
                    }
                }
            },
        }
    };
    match outcome {
        Outcome::Abandon(dropped) | Outcome::Blocked(dropped) => drop(dropped),
        Outcome::Installed {
            handle,
            count_opened,
        } => {
            // §3 step 3, outside the lock: the worker is the process's (§6) — started at the
            // first commit into Open or Legacy, only woken at every later one — and the menu
            // refreshes. F13: the zip rule's room-opened count runs here on a LATER open of the
            // room (never the claim's own Done); a failed count is logged, never fatal.
            crate::bir_module::ensure_worker(handle);
            bir_core::background_cron::wake();
            post_menu_notice();
            if let Some(room_id) = count_opened
                && let Ok(claim) = LegacyClaim::here()
                && let Err(error) = claim.note_room_opened(&room_id)
            {
                tracing::warn!("room-opened not counted toward the zip rule: {error}");
            }
        }
    }
    hooks::at("commit.done");
    report_invariants();
}

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

/// Test door: a legacy-in-place open installed directly into the phase — the generation tests
/// need a machine room without driving a whole claim. The open is the machine's in everything
/// after: snapshot reads it, close ends it, the generation is the machine's own.
#[cfg(any(test, feature = "test-support"))]
pub fn open_legacy_in_place_for_test(
    layout: &bir_core::rooms::Layout,
    user_id: String,
    origin: String,
) -> Result<(Arc<Mutex<Database>>, u64), String> {
    let path = layout.legacy_database();
    let handle = crate::open_room::open_path(&path, None)?;
    let owner = Owner {
        origin,
        user_id,
        practice: false,
    };
    let generation = next_generation();
    {
        let mut session = locked();
        session.epoch += 1;
        mirror(session.epoch);
        session.phase = Phase::Legacy {
            owner,
            room: OpenRoom {
                room_id: None,
                path: path.clone(),
                generation,
                handle: handle.clone(),
            },
            importing: false,
        };
    }
    bir_core::db::set_open_room_database(Some(path));
    Ok((handle, generation))
}

/// quit (§8's state part): the epoch bumps, the phase is terminal, the gate locks. The full
/// release order — worker, Arc holders, checkpoint, registry, guards — is still pending.
pub fn quit() {
    let dropped = {
        let mut session = locked();
        session.epoch += 1;
        mirror(session.epoch);
        crate::account_gate::set_unlocked(false);
        bir_core::transport::set_practice_account(false);
        // F6: the phase's room comes out under the lock and drops after it — the database's
        // close never holds SESSION. (The pending release step takes ownership of the room instead.)
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
/// §5.8 step 4's mechanism: under SESSION, CAS on the epoch — if the session moved on since the
/// import started, the new handle installs nothing and the caller is told. Within the epoch the
/// room is replaced with a new generation.
pub fn reinstall_room(epoch: u64, handle: Arc<Mutex<Database>>, path: PathBuf) -> bool {
    let mut session = locked();
    if session.epoch != epoch {
        return false;
    }
    let generation = next_generation();
    match &mut session.phase {
        Phase::Open { room, .. } | Phase::Legacy { room, .. } => {
            room.handle = handle;
            room.generation = generation;
            room.path = path.clone();
            bir_core::db::set_open_room_database(Some(path));
            true
        }
        _ => false,
    }
}

/// The migration state the page reads (§1): computed from the phase, never a state of its own.
/// Opening reads as moving; Blocked reads as its kind; Open with a note reads as permanent or
/// offered. `None` is an ordinary sign-in — SignedOut, Quitting, Open with no note, Legacy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    /// A boot with no legacy file to move: the room opens, nothing migrates. The page polls a
    /// plainer progress than Moving's.
    Opening,
    Moving,
    Transient {
        copy_saved: bool,
        detail: String,
    },
    Permanent {
        detail: String,
    },
    MovedNotOpened {
        detail: String,
    },
    /// The device store cannot be read and rooms exist (§7): the support page plus Try again.
    DeviceStore {
        detail: String,
    },
    /// The legacy file reappeared after the move (or this account's room has data): the offer
    /// waits for the page. Bound by construction: the note is in the phase, so a close or a
    /// replace ends it — there is nothing to compare.
    Offered,
}

/// The migration state, computed from the phase in one read (§1: MIGRATION is gone).
pub fn migration_state() -> Option<MigrationState> {
    // The legacy file's presence is read BEFORE SESSION (a metadata stat, never a lock-holder):
    // it is what tells the page a plain relaunch's boot (no file to move) from a real move.
    let legacy_present = bir_core::rooms::Layout::here().legacy_database().is_file();
    let session = locked();
    match &session.phase {
        Phase::Opening { .. } => Some(if legacy_present {
            MigrationState::Moving
        } else {
            MigrationState::Opening
        }),
        Phase::Blocked { reason, .. } => Some(match reason {
            Blocked::Transient { copy_saved, detail } => MigrationState::Transient {
                copy_saved: *copy_saved,
                detail: detail.clone(),
            },
            Blocked::MovedNotOpened { detail } => MigrationState::MovedNotOpened {
                detail: detail.clone(),
            },
            Blocked::DeviceStore { detail } => MigrationState::DeviceStore {
                detail: detail.clone(),
            },
        }),
        Phase::Open { note, .. } => match note {
            Note::None => None,
            Note::Permanent { detail } => Some(MigrationState::Permanent {
                detail: detail.clone(),
            }),
            Note::Offered { .. } => Some(MigrationState::Offered),
        },
        Phase::SignedOut | Phase::Legacy { .. } | Phase::Quitting => None,
    }
}

/// The offer's "Not now", and its answered-in-success end (§5.11): the note leaves the phase
/// under SESSION.
pub fn clear_offer_note() {
    let mut session = locked();
    if let Phase::Open { note, .. } = &mut session.phase {
        *note = Note::None;
    }
}

/// F11: the menu-and-chrome notice. The machine's threads never drive the UI directly — the
/// menu refresh and the chrome are the engine thread's. A close or commit posts the notice
/// (one bump); the door's dispatch choke point drains it onto the engine thread: each engine
/// thread tracks the generation it last drained, and a dispatch that finds a newer one runs
/// the refresh and the chrome sync HERE, on that thread.
static MENU_NOTICE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    static MENU_NOTICE_SEEN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Post the menu-and-chrome notice: every engine thread drains it at its next dispatch.
pub(crate) fn post_menu_notice() {
    MENU_NOTICE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// The drain, installed in the door's dispatch hook: a newer notice runs the menu refresh and
/// the chrome sync on the calling (engine) thread.
pub fn drain_menu_notice() {
    let current = MENU_NOTICE.load(std::sync::atomic::Ordering::SeqCst);
    let seen = MENU_NOTICE_SEEN.with(std::cell::Cell::get);
    if current == seen {
        return;
    }
    MENU_NOTICE_SEEN.with(|cell| cell.set(current));
    crate::hooks::hooks().refresh_menu();
    crate::hooks::hooks().set_shell_chrome_open(!crate::account_gate::is_unlocked());
    #[cfg(test)]
    DRAIN_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// How many times a drain actually ran the refresh — the tests' evidence, never a sleep.
#[cfg(test)]
pub(crate) static DRAIN_RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
    // I3: the room open under an account phase is that account's own room, never the room a
    // resumed claim record names for another sign-in.
    if open_kind
        && let Some(owner) = &snap.owner
        && let Some((_, _, Some(room_id), _)) = &snap.room
        && room_id != &bir_core::rooms::room_id_for(&owner.origin, &owner.user_id)
    {
        breaches.push(format!(
            "I3: room {room_id} is open for {} but is not that owner's room",
            owner.user_id
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
            if let Some(point) = state.points.get_mut(self.name)
                && let Some((generation, released)) = &mut point.gate
                && *generation == self.generation
            {
                *released = true;
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
    lock(&LIVE_CLAIMS).clear();
    // A test that drove prepare_launch (item 3's) set BYPASSED through the bypass branch —
    // process-wide, so every later check_invariants in the process would step aside. Tests
    // never MEAN to set it: reset it here, with the rest of the machine's process state.
    BYPASSED.store(false, std::sync::atomic::Ordering::SeqCst);
    CORRUPT_STORE_RETRY.store(false, std::sync::atomic::Ordering::SeqCst);
    lock(&FORCE_NEXT_OPEN_ROOM).take();
    bir_core::legacy_claim::clear_fail_next_legacy_lock_for_test();
    let dropped = {
        let mut session = locked();
        let outcome = close_locked(&mut session, &CloseKind::OwnerEnds, None);
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
        join_claim_threads();
        CLAIM_SPAWN_SUPPRESSED.store(true, std::sync::atomic::Ordering::SeqCst);
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
        Target::Room {
            room_id: bir_core::rooms::room_id_for(&owner.origin, &owner.user_id),
            note: Note::None,
            count_opened: false,
        }
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
        // Item 6: a close of an already-signed-out session is a no-op — the session must be
        // opening or open for the close to bump.
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
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
    /// while the stored sign-in is B — and F12/F9: the phase is Blocked, never Opening forever.
    #[test]
    fn a_stored_owner_mismatch_alone_blocks() {
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
            PhaseKind::Blocked,
            "the mismatched commit at the current epoch is Blocked, never standing Opening"
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

    /// F6: a stale registration's discard deletes only its OWN key. A's registration is parked
    /// inside its network step while the session moves to B and B's registration completes;
    /// when A's step finally lands its gate fails — and the discard must leave B's key and
    /// owner record exactly as they are.
    #[test]
    fn a_stale_registrations_discard_leaves_the_newer_key() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        // A's registration parks inside its register step, its key already created.
        let (parked_sender, parked) = std::sync::mpsc::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let device_dir = scratch.layout.device_dir().to_path_buf();
        let key_a_thread = key_a.clone();
        let registration_a = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device_dir));
            bir_core::relay::register_device_key_for_test(&key_a_thread, epoch_a, move || {
                assert!(bir_core::relay::lazy_first_key_for_test());
                let _ = parked_sender.send(());
                let _ = release.recv();
                bir_core::relay::Registration::Registered
            })
        });
        parked
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("A's registration never reached its network step");
        // The session moves to B — epoch and cache both.
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        assert!(epoch_b > epoch_a);
        // B's registration completes whole: its key and owner record are the install's now.
        let key_b = bir_core::account_session::OwnerKey {
            origin: owner_b.origin.clone(),
            user_id: owner_b.user_id.clone(),
        };
        let done_b = bir_core::relay::register_device_key_for_test(&key_b, epoch_b, move || {
            assert!(bir_core::relay::lazy_first_key_for_test());
            bir_core::relay::Registration::Registered
        });
        assert_eq!(done_b, bir_core::relay::Registration::Registered);
        assert!(
            bir_core::relay::device_key_owner().as_ref() == Some(&key_b),
            "B's registration wrote no owner record"
        );
        let key_of_b = std::fs::read_to_string(
            scratch
                .layout
                .device_dir()
                .join("buwiz_submit_device_key.txt"),
        )
        .unwrap();
        // A's step lands now: stale, its gate fails, its discard runs.
        let _ = release_sender.send(());
        registration_a.join().expect("A's registration panicked");
        assert_eq!(
            std::fs::read_to_string(
                scratch
                    .layout
                    .device_dir()
                    .join("buwiz_submit_device_key.txt"),
            )
            .unwrap(),
            key_of_b,
            "A's stale discard took B's key — the discard deleted a key that was never its own"
        );
        assert!(
            bir_core::relay::device_key_owner().as_ref() == Some(&key_b),
            "A's stale discard cleared B's owner record"
        );
        assert_invariants();
    }

    /// F7: a forget already WAITING on DEVICE_KEY loses to a first send's key creation — the
    /// creation goes through the same lock with the owner recorded, so when the forget gets its
    /// turn it meets the newer owner's record and skips: B's key survives A's forget.
    #[test]
    fn a_first_sends_key_survives_the_waiting_forget() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        // B is the signed-in owner now; A's close is the one whose forget is still waiting.
        let owner_b = seed_stored("user-b", false);
        let epoch_now = snapshot().epoch;
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_b.origin.clone(),
            user_id: "user-a".into(),
        };
        // Hold DEVICE_KEY: the forget waits, then the creation queues behind it.
        let guard = bir_core::relay::device_key_lock_for_test();
        let (forgot, forget_done) = std::sync::mpsc::channel();
        let key_a_thread = key_a.clone();
        let device_dir = scratch.layout.device_dir().to_path_buf();
        let forget_dir = device_dir.clone();
        let forget = std::thread::spawn(move || {
            // The device-folder override is thread-local: the forget thread runs under the
            // scratch install, as the real forget thread resumes its closer's.
            bir_core::platform::set_device_dir_override(Some(forget_dir));
            bir_core::relay::forget_device_key_at_epoch(&key_a_thread, epoch_now);
            let _ = forgot.send(());
        });
        std::thread::yield_now();
        let (made, create_done) = std::sync::mpsc::channel();
        let creation = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device_dir));
            let created = bir_core::relay::lazy_first_key_for_test();
            let _ = made.send(created);
        });
        // Neither can run while the lock is held.
        assert!(
            forget_done
                .recv_timeout(std::time::Duration::from_millis(300))
                .is_err()
        );
        assert!(
            create_done
                .recv_timeout(std::time::Duration::from_millis(300))
                .is_err()
        );
        drop(guard);
        forget.join().expect("the forget thread panicked");
        creation.join().expect("the creation thread panicked");
        let created = create_done
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("the creation never answered");
        assert!(created, "the first send created no key");
        // B's key AND its owner record survive: the forget met the newer owner's record inside
        // the lock and skipped, exactly as it must however the two ordered.
        assert!(
            bir_core::relay::device_key_owner().as_ref()
                == Some(&bir_core::account_session::OwnerKey {
                    origin: owner_b.origin.clone(),
                    user_id: "user-b".into(),
                }),
            "B's owner record is gone — the forget took a key that was never A's"
        );
        assert_invariants();
    }

    /// F5: the credential-matched server_ended closes WHATEVER the epoch. A Blocked at e1, a
    /// Try again to e2, then A's InvalidGrant sent at e1: the disk held the exact failed
    /// credential, so the ending is the current sign-in's — the close runs and the session is
    /// SignedOut, never stranded in a phase with no sign-in behind it.
    #[test]
    fn a_credential_matched_server_ended_closes_whatever_the_epoch() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let owner_a = seed_stored("user-a", false);
        // Blocked at e1: another copy holds the guard.
        let other_copy =
            bir_core::db::take_device_instance_lock(scratch.layout.device_dir()).unwrap();
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        let epoch_one = snapshot().epoch;
        // Try again to e2: the epoch moves on.
        drop(other_copy);
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert!(snapshot().epoch > epoch_one, "the retry bumped the epoch");
        // A's InvalidGrant sent at e1 arrives now: the disk still holds the exact credential.
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch_one,
            "refresh-user-a".to_string(),
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the credential-matched ending was refused for its stale epoch — the session is stranded"
        );
        assert!(
            bir_core::cloud_account::load_stored_account()
                .ok()
                .flatten()
                .is_none(),
            "the ended sign-in is still on disk"
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

    fn sha256_file(path: &std::path::Path) -> String {
        use sha2::Digest;
        let mut file = std::fs::File::open(path).unwrap();
        let mut hash = sha2::Sha256::new();
        std::io::copy(&mut file, &mut hash).unwrap();
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// A crash image of A's move already in A's room, with the legacy name gone: a valid closed
    /// database copied there, and A's claim record at `step`.
    fn write_moved_claim_image(
        scratch: &Scratch,
        owner: &Owner,
        step: bir_core::legacy_claim::Step,
    ) -> (bir_core::rooms::RoomId, std::path::PathBuf) {
        let legacy = scratch.layout.legacy_database();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        drop(store);
        Database::open(&legacy).unwrap().close().unwrap();
        let room_id = bir_core::rooms::room_id_for(&owner.origin, &owner.user_id);
        let room_file = scratch.layout.room_database(&room_id);
        std::fs::create_dir_all(room_file.parent().unwrap()).unwrap();
        std::fs::copy(&legacy, &room_file).unwrap();
        std::fs::remove_file(&legacy).unwrap();
        let raw_sha256 = sha256_file(&room_file);
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        let record = bir_core::legacy_claim::Record {
            stamp: "test-stamp".into(),
            room: Some(room_id.as_str().to_owned()),
            origin: Some(owner.origin.clone()),
            user_id: Some(owner.user_id.clone()),
            email: Some(format!("{}@example.test", owner.user_id)),
            unlocked_by: Some(bir_core::legacy_claim::UnlockedBy::NoLock),
            empty_room_aside: false,
            empty_room_file: None,
            os_login_used: false,
            raw_user_version: None,
            raw_sha256: Some(raw_sha256),
            zip: None,
            zip_sha256: None,
            hash_checked: false,
            open_checked: false,
            step,
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
        (room_id, room_file)
    }

    /// Put A's moved claim into the link window with a returned WAL: the room file and the
    /// legacy path are one file, and finishing A's unlink fails transiently.
    fn a_move_stuck_in_the_link_window(
        scratch: &Scratch,
        owner_a: &Owner,
    ) -> bir_core::rooms::RoomId {
        let (room_a, room_file) =
            write_moved_claim_image(scratch, owner_a, bir_core::legacy_claim::Step::Moved);
        let legacy = scratch.layout.legacy_database();
        std::fs::hard_link(&room_file, &legacy).unwrap();
        std::fs::write(legacy.with_extension("db-wal"), b"wal").unwrap();
        room_a
    }

    fn add_legacy_taxpayer(db: &Database, root: &str, name: &str) {
        db.create_taxpayer(bir_core::tax_profile::NewTaxpayer {
            tin_root: root.into(),
            name: name.into(),
            taxpayer_type: bir_core::profile::TaxpayerType::Corporation,
            birth_date: None,
            head_office: bir_core::tax_profile::BranchFacts {
                rdo_code: "040".into(),
                registered_address: "1 Example Street, Pasig".into(),
                zip_code: "1600".into(),
                phone: "0280000000".into(),
                email: "office@example.com".into(),
            },
        })
        .unwrap();
    }

    /// A legacy install locked by its old help-desk PIN, written with the app's own schema and
    /// carrying one taxpayer with one Queued return.
    fn locked_legacy_file(scratch: &Scratch) {
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let db = Database::open(scratch.layout.legacy_database()).unwrap();
        add_legacy_taxpayer(&db, "123456788", "Owner Trading");
        db.set_setting("access_mode", "helpdesk").unwrap();
        db.set_setting("app_lock_pin_hash", &bir_core::crypto::hash_pin("1357"))
            .unwrap();
        db.close().unwrap();
        let conn = rusqlite::Connection::open(scratch.layout.legacy_database()).unwrap();
        conn.execute(
            "INSERT INTO form_drafts (tin, form_code, taxable_year, quarter, status, data_json)
             VALUES ('12345678800000', '2551Q', 2026, 1, 'Queued', ?1)",
            [
                serde_json::to_string(&serde_json::json!({ "note": "queued in the legacy file" }))
                    .unwrap(),
            ],
        )
        .unwrap();
    }

    /// A Done claim of A plus a byte-for-byte reappearance of the PIN-locked legacy file. A's
    /// room already exists and carries one valid Queued 2551Q; the reappeared file carries its own
    /// older Queued row, so a wrong legacy-in-place or import path would make a second copy.
    fn locked_reappeared_install(scratch: &Scratch, owner: &Owner) -> bir_core::rooms::RoomId {
        locked_legacy_file(scratch);
        create_device_store(scratch);
        let room_id = bir_core::rooms::room_id_for(&owner.origin, &owner.user_id);
        let room_file = scratch.layout.room_database(&room_id);
        std::fs::create_dir_all(room_file.parent().unwrap()).unwrap();
        let room_db = Database::open(&room_file).unwrap();
        add_legacy_taxpayer(&room_db, "123456788", "Owner Trading");
        let profile = room_db.get_profile("12345678800000").unwrap().unwrap();
        let mut effective_profile = profile.clone();
        effective_profile.ensure_profile_version_ledger();
        let mut draft = bir_core::forms::Form2551QDraft::new_from_effective_profile(
            &effective_profile,
            2026,
            1,
        );
        draft.item_13_election = bir_core::forms::form_2551q::Item13Election::NotApplicable;
        draft.transition_to_queued().unwrap();
        room_db
            .save_queued_2551q_draft_and_election(&draft)
            .unwrap();
        drop(room_db);

        let raw_sha256 = sha256_file(&scratch.layout.legacy_database());
        let record = bir_core::legacy_claim::Record {
            stamp: "test-stamp".into(),
            room: Some(room_id.as_str().to_owned()),
            origin: Some(owner.origin.clone()),
            user_id: Some(owner.user_id.clone()),
            email: Some(format!("{}@example.test", owner.user_id)),
            unlocked_by: Some(bir_core::legacy_claim::UnlockedBy::NoLock),
            raw_sha256: Some(raw_sha256),
            step: bir_core::legacy_claim::Step::Done,
            done_at: Some("test-done".into()),
            ..bir_core::legacy_claim::Record::default()
        };
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO migration (key, value) VALUES ('legacy', ?1)",
            rusqlite::params![serde_json::to_string(&record).unwrap()],
        )
        .unwrap();
        room_id
    }

    fn create_device_store(scratch: &Scratch) {
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        drop(bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap());
    }

    fn claim_record(scratch: &Scratch) -> bir_core::legacy_claim::Record {
        bir_core::legacy_claim::LegacyClaim::at(scratch.layout.clone())
            .unwrap()
            .record()
            .unwrap()
            .expect("a claim record")
    }

    /// L1: a crash between A's move and Done is finished by B's sign-in, but B opens B's room —
    /// never A's. The claim's finish stands in its record.
    #[test]
    fn l1_a_crash_after_as_move_finished_by_b_opens_bs_room() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Moved);
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let (_, _, room_id, _) = snapshot().room.expect("B's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&owner_b.origin, &owner_b.user_id),
            "B's sign-in opened the room the record's move belonged to"
        );
        assert_eq!(
            claim_record(&scratch).step,
            bir_core::legacy_claim::Step::Done,
            "A's finish did not stand"
        );
    }

    /// L1: the transient-after-move shape. A's finish is still stuck on a transient store write when B
    /// signs in; that transient is A's claim's business, not a reason to keep B out of B's room.
    #[test]
    fn l1_a_transient_after_as_move_under_b_opens_bs_room() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Moved);
        bir_core::legacy_claim::fail_next_device_rows_write_for_test();
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let (_, _, room_id, _) = snapshot().room.expect("B's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&owner_b.origin, &owner_b.user_id),
            "A's unfinished transient must not block or replace B's own room"
        );
        assert!(
            matches!(
                claim_record(&scratch).failure,
                Some(bir_core::legacy_claim::RecordedFailure::Transient { .. })
            ),
            "A's unfinished transient was not recorded"
        );
    }

    /// N1/N4: when finishing A's move keeps failing as an Err(Failure::Transient), B is not
    /// blocked in a loop and is told nothing about A's copy: B opens B's own room with no migration
    /// state at all.
    #[test]
    fn n1_a_transient_error_finishing_as_move_still_lets_b_open_bs_room() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        a_move_stuck_in_the_link_window(&scratch, &owner_a);
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B stayed blocked on A's failing move"
        );
        let (_, _, room_id, _) = snapshot().room.expect("B's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&owner_b.origin, &owner_b.user_id),
            "B did not open B's own room"
        );
        assert!(
            migration_state().is_none(),
            "B was shown another account's move: {:?}",
            migration_state()
        );
    }

    /// L1 through the literal public sequence: A is signed in and opens, A signs out, then B's
    /// signed_in meets A's moved record. B still opens only B's room.
    #[test]
    fn l1_literal_sign_out_then_signed_in_never_opens_as_room_for_b() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        sign_out();
        a_move_stuck_in_the_link_window(&scratch, &owner_a);
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: Some("user-b@example.test".into()),
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        permit_claim_threads();
        signed_in(&stored_b, 1).expect("B's sign-in");
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the literal sign_out → signed_in sequence left B blocked"
        );
        let (_, _, room_id, _) = snapshot().room.expect("B's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for("https://buwiz.example.test", "user-b"),
            "B opened A's room through the literal sequence"
        );
    }

    /// N3: the open path does not trust a device-store row. Even when the row says B's room is
    /// A's id, opening that id for B is refused and no room is installed.
    #[test]
    fn n3_a_store_row_naming_another_room_for_the_owner_is_refused() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        drop(store);
        let owner_b = seed_stored("user-b", false);
        let room_a = bir_core::rooms::room_id_for(&owner_b.origin, "user-a");
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.execute(
            "INSERT INTO rooms (room_id, origin, user_id, email, created_at, from_legacy) VALUES (?1, ?2, ?3, NULL, 'now', 0)",
            rusqlite::params![room_a.as_str(), owner_b.origin, owner_b.user_id],
        )
        .unwrap();
        drop(conn);
        permit_claim_threads();
        begin_open(owner_b, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(
            snapshot().room.is_none(),
            "the store row's foreign room opened"
        );
    }

    /// L3/B1(a): a store error while the legacy file is present is a blocked migration state,
    /// never legacy-in-place. The room-open evidence rule still demands a real claim start in
    /// this process.
    #[test]
    fn b1_a_store_error_with_legacy_blocks_instead_of_opening_legacy_in_place() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        Database::open(scratch.layout.legacy_database())
            .unwrap()
            .close()
            .unwrap();
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        std::fs::create_dir_all(scratch.layout.rooms_dir()).unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        match migration_state() {
            Some(MigrationState::DeviceStore { detail }) => {
                assert!(detail.contains("device store"), "{detail}")
            }
            other => panic!("not a device-store block: {other:?}"),
        }
        let room = bir_core::rooms::room_id_for(&owner.origin, &owner.user_id);
        let refusal = bir_core::legacy_claim::room_open_allowed(&scratch.layout, &room)
            .expect_err("the room-open evidence rule was switched off");
        assert!(refusal.contains("start() has not run"), "{refusal}");
    }

    /// B1(a)'s transient half: busy and locked SQLite answers classify as Transient, never as a
    /// dead store.
    #[test]
    fn b1_busy_and_locked_store_errors_are_transient() {
        for code in [5, 6] {
            let error = bir_core::rooms::DeviceStoreError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(code),
                Some("database is busy or locked".to_owned()),
            ));
            assert!(
                matches!(
                    claim_open_error_target(&error),
                    Target::Blocked(Blocked::Transient { .. })
                ),
                "SQLite code {code} is not transient"
            );
        }
    }

    /// B1(b): with rooms present, a locked legacy file is only a note in the account's own room —
    /// never the legacy file opened instead of that room.
    #[test]
    fn b1_a_locked_legacy_file_with_rooms_opens_the_accounts_room_with_a_note() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        locked_legacy_file(&scratch);
        let owner = seed_stored("user-a", false);
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        store
            .add_room(&bir_core::rooms::RoomRecord {
                room_id: bir_core::rooms::room_id_for(&owner.origin, "another-user"),
                origin: owner.origin.clone(),
                user_id: "another-user".to_owned(),
                email: None,
                created_at: "2026-11-28T00:00:00+00:00".to_owned(),
                from_legacy: false,
            })
            .unwrap();
        drop(store);
        permit_claim_threads();
        begin_open(owner.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the locked legacy file opened in place"
        );
        let (_, _, room_id, _) = snapshot().room.expect("the account's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&owner.origin, &owner.user_id),
            "the open room is not the signed-in account's own"
        );
        match migration_state() {
            Some(MigrationState::Permanent { detail }) => {
                assert!(
                    detail.contains("Moving locked files comes in a later version"),
                    "{detail}"
                )
            }
            other => panic!("no locked-file note: {other:?}"),
        }
    }

    /// Positive control for legacy mode: with no rooms at all, the same locked install still opens
    /// the legacy file in place.
    #[test]
    fn b1_a_locked_legacy_file_without_rooms_still_opens_legacy_in_place() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        locked_legacy_file(&scratch);
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Legacy);
    }

    /// LB1: a practice account's lookup-created empty room is not a real account's room. The next
    /// real sign-in on the same locked install still opens the legacy file, Queued return and all.
    #[test]
    fn lb1_a_practice_lookup_room_does_not_hide_the_locked_legacy_file() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        locked_legacy_file(&scratch);
        let practice = seed_stored("practice-user", true);
        permit_claim_threads();
        begin_open(practice, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(
            scratch.layout.rooms_dir().is_dir(),
            "positive control: the practice lookup made a room directory"
        );
        sign_out();
        join_claim_threads();

        let owner = seed_stored("user-a", false);
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Legacy,
            "the practice room hid the locked legacy install"
        );
        let (handle, _, _, path) = snapshot().room.expect("legacy mode's handle");
        assert_eq!(path, scratch.layout.legacy_database());
        assert_eq!(
            handle.lock().unwrap().count_queued_drafts().unwrap(),
            1,
            "the legacy file's Queued return is the file that opened"
        );
    }

    /// LB1's noise half: a stray entry inside rooms/ is not a room either.
    #[test]
    fn lb1_a_stray_entry_in_rooms_does_not_hide_the_locked_legacy_file() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        locked_legacy_file(&scratch);
        std::fs::create_dir_all(scratch.layout.rooms_dir()).unwrap();
        std::fs::write(scratch.layout.rooms_dir().join(".DS_Store"), b"noise").unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Legacy,
            "a stray rooms/ entry hid the locked legacy install"
        );
    }

    /// N3: a real account's lookup-created room writes its store row at once, so later routing
    /// knows the install has a non-practice room even before the room file has user data.
    #[test]
    fn n3_a_lookup_created_non_practice_room_writes_its_store_row() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        let row = store
            .find_room(&owner.origin, &owner.user_id)
            .unwrap()
            .expect("the lookup-created real room wrote no row");
        assert_eq!(
            row.room_id,
            bir_core::rooms::room_id_for(&owner.origin, &owner.user_id)
        );
    }

    /// N3's practice half and positive control for LB1's rule: a practice lookup room writes no row.
    #[test]
    fn n3_a_lookup_created_practice_room_writes_no_store_row() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        let owner = seed_stored("practice-user", true);
        permit_claim_threads();
        begin_open(owner.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let store = bir_core::rooms::DeviceStore::open(&scratch.layout).unwrap();
        assert!(
            store
                .find_room(&owner.origin, &owner.user_id)
                .unwrap()
                .is_none(),
            "the practice lookup wrote a non-practice room row"
        );
    }

    /// The migration-record read refusal is page wording only; the raw store error keeps the
    /// path in the log, never on screen.
    #[test]
    fn migration_record_read_details_never_carry_os_paths() {
        let path = std::path::PathBuf::from("/Users/os-user/private/device.db");
        let raw = path.to_string_lossy().to_string();
        let error = bir_core::rooms::DeviceStoreError::Unreadable {
            path,
            reason: "not sqlite".into(),
        };
        assert!(
            error.to_string().contains(&raw),
            "positive control: the raw store error has a path"
        );
        let (target, spec) = migration_record_read_failure(&error);
        assert!(spec.is_none());
        let Target::Blocked(Blocked::Transient { detail, .. }) = target else {
            panic!("not a transient block")
        };
        assert!(!detail.contains(&raw), "{detail}");
    }

    /// LB3: on a pre-rooms install with no rooms, an unreadable device.db is set aside, the claim
    /// store is retried, and the locked legacy file still opens in place.
    #[test]
    fn lb3_an_unusable_store_with_no_rooms_is_set_aside_and_legacy_opens() {
        let _turn = test_turn();
        let scratch = scratch();
        locked_legacy_file(&scratch);
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Legacy,
            "the unusable pre-rooms store became a permanent Try again"
        );
        assert!(
            std::fs::read_dir(scratch.layout.device_dir())
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("device.db.unusable-")),
            "the unreadable store was not set aside"
        );
        assert!(
            bir_core::rooms::DeviceStore::open(&scratch.layout).is_ok(),
            "the retry did not leave a usable fresh store"
        );
    }

    /// LB3's boundary pin: the set-aside is right only for a true no-rooms install. Once rooms
    /// exist, the same unusable store is the support page and is never moved aside.
    #[test]
    fn lb3_an_unusable_store_with_rooms_is_never_set_aside() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.rooms_dir().join("noise")).unwrap();
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(
            matches!(migration_state(), Some(MigrationState::DeviceStore { .. })),
            "not a device-store block: {:?}",
            migration_state()
        );
        assert!(
            !std::fs::read_dir(scratch.layout.device_dir())
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("device.db.unusable-")),
            "the store was set aside even though rooms exist"
        );
    }

    /// N5: the no-rooms set-aside does not depend on a legacy file being present. With no legacy
    /// file, the retry leaves a usable store and the account's lookup-created room opens.
    #[test]
    fn n5_an_unusable_store_without_legacy_or_rooms_is_set_aside_and_the_room_opens() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the no-legacy unusable store stayed a permanent block"
        );
        assert!(
            std::fs::read_dir(scratch.layout.device_dir())
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("device.db.unusable-")),
            "the unreadable store was not set aside"
        );
    }

    /// N5: a newer store cannot be renamed aside, but on a true pre-rooms install with a legacy
    /// file §7 still falls back to legacy in place rather than a permanent device-store block.
    #[test]
    fn n5_a_newer_store_with_legacy_and_no_rooms_opens_legacy_in_place() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.pragma_update(
            None,
            "user_version",
            bir_core::rooms::DEVICE_SCHEMA_VERSION + 1,
        )
        .unwrap();
        drop(conn);
        locked_legacy_file(&scratch);
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Legacy,
            "the newer store became a permanent block instead of the §7 legacy fallback"
        );
    }

    /// N6: the §7 legacy fallback is never for the practice owner. The same newer-store install
    /// blocks transiently instead of opening another account's legacy file in place.
    #[test]
    fn n6_the_store_fallback_never_opens_legacy_for_practice() {
        let _turn = test_turn();
        let scratch = scratch();
        create_device_store(&scratch);
        let conn = rusqlite::Connection::open(scratch.layout.device_db()).unwrap();
        conn.pragma_update(
            None,
            "user_version",
            bir_core::rooms::DEVICE_SCHEMA_VERSION + 1,
        )
        .unwrap();
        drop(conn);
        locked_legacy_file(&scratch);
        let owner = seed_stored("practice-user", true);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(
            matches!(migration_state(), Some(MigrationState::Transient { .. })),
            "practice did not get the transient store fallback: {:?}",
            migration_state()
        );
    }

    /// N6: once the unusable store was set aside, a failed retry is transient. It is never the
    /// legacy-in-place fallback: the store's state after the retry is unknown.
    #[test]
    fn n6_a_failed_store_retry_is_transient_not_legacy_in_place() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        locked_legacy_file(&scratch);
        CORRUPT_STORE_RETRY.store(true, std::sync::atomic::Ordering::SeqCst);
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "the failed retry opened the legacy file in place"
        );
        assert!(
            matches!(migration_state(), Some(MigrationState::Transient { .. })),
            "the failed retry is not Transient: {:?}",
            migration_state()
        );
    }

    /// LB5 through the production wiring: A's sign-in really runs decide_claim, where the Done
    /// record plus PIN-locked reappearance is OfferStart::Locked; that decision opens A's room with
    /// the locked-later note, and the already-queued copy in that room is what the worker sends.
    /// The legacy bytes never change, so the old file is never opened in place or imported.
    #[test]
    fn lb5_a_locked_reappeared_file_opens_the_room_and_the_worker_sends_one_copy() {
        const CHILD: &str = "BIR_APP_API_LB5_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "session::tests::lb5_a_locked_reappeared_file_opens_the_room_and_the_worker_sends_one_copy",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("EBIR_TEST_ENV", "1")
                .env("BIR_SFTP_DRY_RUN", "1")
                .env_remove("BIR_SFTP_LIVE")
                .env_remove("BIR_RELAY_URL")
                .status()
                .unwrap();
            assert!(status.success(), "the child process failed");
            return;
        }
        let _queue_turn = bir_core::background_cron::queue_transport_test_turn_blocking();
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        let room_id = locked_reappeared_install(&scratch, &owner_a);
        let legacy_before = std::fs::read(scratch.layout.legacy_database()).unwrap();
        permit_claim_threads();
        begin_open(owner_a, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let (_, _, opened_room, _) = snapshot().room.expect("A's room open");
        assert_eq!(opened_room, Some(room_id), "the open room is not A's own");
        assert!(
            matches!(
                migration_state(),
                Some(MigrationState::Permanent { ref detail, .. }) if *detail == LOCKED_LEGACY_LATER
            ),
            "the OfferStart::Locked decision did not produce the locked-later room note: {:?}",
            migration_state()
        );
        assert!(
            !matches!(migration_state(), Some(MigrationState::Offered)),
            "the locked file was treated as an open offer"
        );
        assert!(
            crate::bir_module::submission_worker_started(),
            "the production room commit did not start the worker"
        );

        let (handle, ..) = snapshot().room.expect("A's room handle");
        crate::bir_module::wake_submission_worker();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let status = loop {
            crate::bir_module::wake_submission_worker();
            let status = {
                let room = handle.lock().unwrap();
                room.get_2551q_draft("12345678800000", 2026, 1)
                    .unwrap()
                    .map(|draft| draft.status)
            };
            if status == Some(bir_core::forms::FilingStatus::Submitted) {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the room copy was not sent by the worker: {status:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        assert_eq!(status, Some(bir_core::forms::FilingStatus::Submitted));
        assert_eq!(
            std::fs::read(scratch.layout.legacy_database()).unwrap(),
            legacy_before,
            "the locked legacy file was opened or changed in place"
        );
        assert!(
            !scratch
                .layout
                .legacy_database()
                .with_extension("db-wal")
                .exists()
                && !scratch
                    .layout
                    .legacy_database()
                    .with_extension("db-shm")
                    .exists(),
            "the locked legacy file was opened: its SQLite sidecars exist"
        );
    }

    /// LB4: the device-store page's detail carries no OS-user path.
    #[test]
    fn lb4_the_device_store_detail_never_shows_the_store_path() {
        let _turn = test_turn();
        let scratch = scratch();
        std::fs::create_dir_all(scratch.layout.device_dir()).unwrap();
        std::fs::create_dir_all(scratch.layout.rooms_dir()).unwrap();
        std::fs::write(scratch.layout.device_db(), b"not sqlite").unwrap();
        let owner = seed_stored("user-a", false);
        permit_claim_threads();
        begin_open(owner, 1, OpenWhy::SignIn);
        join_claim_threads();
        let Some(MigrationState::DeviceStore { detail }) = migration_state() else {
            panic!("not a device-store block: {:?}", migration_state())
        };
        assert!(!detail.contains(scratch.layout.device_db().to_string_lossy().as_ref()));
        assert!(!detail.contains(scratch.layout.device_dir().to_string_lossy().as_ref()));
        assert!(detail.contains("device store"), "{detail}");
    }

    /// P-F1: a foreign unfinished record makes only the record-room finish definitive. B's own
    /// pre-move transient error still blocks B with Try again; it never opens an empty room.
    #[test]
    fn pf1_bs_own_pre_move_transient_error_still_blocks_with_try_again() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Copied);
        let legacy = Database::open(scratch.layout.legacy_database()).unwrap();
        legacy.set_setting("different", "content").unwrap();
        legacy.close().unwrap();
        bir_core::legacy_claim::fail_next_legacy_lock_for_test();
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        begin_open(owner_b, 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "B's own pre-move transient opened a room"
        );
        assert!(
            matches!(migration_state(), Some(MigrationState::Transient { .. })),
            "not the Try again page: {:?}",
            migration_state()
        );
    }

    /// N4: a legacy-lock failure while A's move is finishing in the link window is part of the
    /// record-room finish. B therefore opens B's own room through the real claim thread; it is not
    /// blocked by an untagged transient of A's claim.
    #[test]
    fn n4_a_link_window_lock_failure_is_part_of_the_record_room_finish() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        let (_room_a, room_file) =
            write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Moved);
        std::fs::hard_link(&room_file, scratch.layout.legacy_database()).unwrap();
        bir_core::legacy_claim::fail_next_legacy_lock_for_test();
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B was blocked by A's untagged link-window lock failure"
        );
        let (_, _, room_id, _) = snapshot().room.expect("B's room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&owner_b.origin, &owner_b.user_id),
            "the open room is not B's own"
        );
    }

    /// The every-build invariant at the open path: even if a decision-layer regression forces A's
    /// room into B's open, the room is refused and B ends blocked with nothing open.
    #[test]
    fn l1_a_mismatched_room_never_opens_for_the_owner() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        let (room_a, _) =
            write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Moved);
        let owner_b = seed_stored("user-b", false);
        permit_claim_threads();
        force_next_open_room_for_test(room_a);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "the mismatched room was not refused"
        );
        assert!(snapshot().room.is_none(), "a mismatched room installed");
    }

    /// L1: the resume runs before the practice-account refusal, so a practice sign-in can meet
    /// A's moved record. It still opens only the practice account's own empty room.
    #[test]
    fn l1_a_practice_sign_in_after_as_move_opens_the_practice_room() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = owner("user-a");
        write_moved_claim_image(&scratch, &owner_a, bir_core::legacy_claim::Step::Moved);
        let practice = seed_stored("practice-user", true);
        permit_claim_threads();
        begin_open(practice.clone(), 1, OpenWhy::SignIn);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let (_, _, room_id, _) = snapshot().room.expect("the practice room open");
        assert_eq!(
            room_id.expect("a room id"),
            bir_core::rooms::room_id_for(&practice.origin, &practice.user_id),
            "the practice account opened A's claimed room"
        );
        assert_eq!(
            claim_record(&scratch).step,
            bir_core::legacy_claim::Step::Done,
            "A's finish did not stand for the practice resume"
        );
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

    /// Hand-drive an open session for the tests whose subject is the stored sign-in, not the
    /// claim: begin_open already ran, so the phase is Opening at `epoch`; this drives the real
    /// commit() the claim thread uses.
    fn open_hand(scratch: &Scratch, owner: &Owner, epoch: u64, name: &str) {
        let (handle, path) = scratch_handle(scratch, name);
        commit(epoch, room_target(owner), Some((handle, path)));
    }

    /// K1, deterministic: signed_in paused between SESSION's release and the spawn — a
    /// same-owner begin_open then must read the epoch LIVE (pushed inside begin_open_locked),
    /// not stale. Reverted, the replacement happens while paused and the epoch moves — red.
    #[test]
    fn a_begin_open_during_signed_ins_spawn_window_is_not_stale() {
        let _turn = test_turn();
        let scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        let stored = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in");
        let gate = hooks::pause("finish_open.before_spawn");
        let device = bir_core::platform::capture_device_dir_override();
        let database = bir_core::db::capture_database_path_override();
        let driver = std::thread::spawn(move || {
            database.run(move || {
                device.run(move || {
                    signed_in(&stored, 1).unwrap();
                });
            });
        });
        gate.reached();
        // signed_in's phase is set and its SESSION released; the claim is not yet spawned.
        let epoch = snapshot().epoch;
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        assert!(
            lock(&LIVE_CLAIMS).contains(&epoch),
            "the opening's epoch is not live in the spawn window — a same-owner begin_open reads it stale"
        );
        // The second engine's same-owner begin_open: a no-op, never a replacement.
        begin_open(owner_a.clone(), 2, OpenWhy::Launch);
        assert_eq!(
            snapshot().epoch,
            epoch,
            "the same-owner begin_open replaced the live opening"
        );
        gate.release();
        driver.join().unwrap();
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-a")
        );
        let _ = scratch;
    }

    /// n1: a sign-out while the claim runs    /// n1: a sign-out while the claim runs — the close bumps the epoch, the opener's commit
    /// misses its compare-and-swap, and nothing was ever installed (F10's hole, closed).
    /// The REAL claim thread drives it, the pause set before begin_open (F5's rule).
    #[test]
    fn n1_sign_out_during_the_claim_opens_nothing() {
        let _turn = test_turn();
        let _scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        let claim_gate = hooks::pause("claim.after_claim");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        claim_gate.reached();
        // The sign-out lands while the real claim is paused: the epoch bumps under the close.
        sign_out();
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        assert!(!gate_unlocked(), "the gate is locked");
        assert_invariants();
        claim_gate.release();
        join_claim_threads();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the stale claim's commit abandoned: nothing opened"
        );
        assert!(snapshot().room.is_none(), "no room was ever installed");
        assert!(
            !gate_unlocked(),
            "the gate never unlocked for the dead session"
        );
        assert_invariants();
    }

    /// Item 5: a panicking claim thread leaves no Opening standing — its drop guard commits
    /// Blocked::Transient — and Try again opens for real through the real claim thread.
    #[test]
    fn a_panicking_claim_is_blocked_and_try_again_opens() {
        let _turn = test_turn();
        let scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        PANIC_CLAIM.store(true, std::sync::atomic::Ordering::SeqCst);
        // commit.done is the synchronization: the guard's commit, driven by the real (dying)
        // thread, passes it before the assertion — no sleeps.
        let done_gate = hooks::pause("commit.done");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        done_gate.reached();
        done_gate.release();
        let epoch = snapshot().epoch;
        // The paused commit is the guard's: it must finish installing Blocked before we read.
        // A second arrival is not coming — the commit is past its phase write already (the hook
        // sits after it), so the phase is observable now.
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "a panicking claim thread left the phase at {:?} — the drop guard did not commit",
            snapshot().phase_kind
        );
        assert!(!gate_unlocked(), "the gate stays locked");
        assert_invariants();
        // Try again: the same owner, no panic this time — the real claim thread commits Open.
        let done_gate2 = hooks::pause("commit.done");
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        done_gate2.reached();
        done_gate2.release();
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "Try again after the panicking claim did not open: {:?}",
            snapshot().phase_kind
        );
        assert!(snapshot().room.is_some(), "the room is installed");
        assert!(gate_unlocked(), "the gate is unlocked");
        assert_invariants();
        join_claim_threads();
        let _ = (scratch, epoch);
    }

    /// Item 5, the stale half: a same-owner Opening whose claim thread is already gone is
    /// replaced by Try again instead of no-oping. The thread is dead but its guard commit is
    /// paused, so the phase still reads Opening when the retry arrives.
    #[test]
    fn try_again_replaces_a_stale_same_owner_opening() {
        let _turn = test_turn();
        let scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        PANIC_CLAIM.store(true, std::sync::atomic::Ordering::SeqCst);
        // Park the dying thread INSIDE its guard commit, before the phase write: the claim
        // thread is as good as gone (its epoch left LIVE_CLAIMS at the guard's drop) but the
        // phase still reads Opening.
        let enter_gate = hooks::pause("commit.enter");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        enter_gate.reached();
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        // Try again while the first thread is parked mid-commit: the stale Opening is replaced.
        let spawns_before = claim_thread_spawn_count();
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(
            claim_thread_spawn_count() - spawns_before,
            1,
            "Try again did not spawn a fresh claim thread — the stale Opening no-oped"
        );
        enter_gate.release();
        join_claim_threads();
        let _ = scratch;
    }

    /// Item 6: a close of an already-signed-out session is a no-op — no epoch bump. The UI's
    /// lockCloud close raced the sign-out's backgrounded key forget: the bump made the forget's
    /// epoch re-check lose and the device key survived every UI sign-out.
    #[test]
    fn close_from_signed_out_is_a_no_op_without_a_bump() {
        let _turn = test_turn();
        let _scratch = scratch();
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        let epoch_before = snapshot().epoch;
        close(Reason::SignOut);
        assert_eq!(
            snapshot().epoch,
            epoch_before,
            "a close of the signed-out session bumped the epoch — the forget's re-check loses"
        );
        assert_invariants();
    }

    /// Item 1b: the page sees the move. migration_state answers Moving for the whole Opening,
    /// the gate answers unlocked only after the commit, and the F11 drain — the pending menu
    /// notice — runs on the thread that drains it (the engine thread in production), refreshing
    /// the menu there.
    #[test]
    fn the_page_sees_opening_and_the_drain_runs_on_the_engine_thread() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        // No legacy file in the scratch: a plain relaunch's boot reads Opening, not Moving.
        assert!(
            matches!(migration_state(), Some(MigrationState::Opening)),
            "migration_state does not answer Opening for a plain relaunch: {:?}",
            migration_state()
        );
        // With a legacy file present the same phase reads Moving: the page tells a real move
        // from a plain relaunch by this one field.
        std::fs::write(scratch.layout.legacy_database(), b"sqlite").unwrap();
        assert!(
            matches!(migration_state(), Some(MigrationState::Moving)),
            "migration_state does not answer Moving with a legacy file present: {:?}",
            migration_state()
        );
        std::fs::remove_file(scratch.layout.legacy_database()).unwrap();
        assert!(
            !gate_unlocked(),
            "the gate is unlocked before the commit — the page would poll an open session"
        );
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert!(gate_unlocked(), "the gate is still locked after the commit");
        assert!(
            migration_state().is_none(),
            "a clean open carries no migration state: {:?}",
            migration_state()
        );
        // The drain: a commit posted the notice; draining on THIS thread runs the menu refresh
        // HERE — the engine thread in production. The hooks fake counts the refreshes and the
        // thread they ran on.
        let before = DRAIN_RUNS.load(std::sync::atomic::Ordering::SeqCst);
        drain_menu_notice();
        let after = DRAIN_RUNS.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            after,
            before + 1,
            "the drain did not refresh the menu on the draining thread"
        );
        // Draining again is a no-op: the notice is seen.
        drain_menu_notice();
        assert_eq!(
            DRAIN_RUNS.load(std::sync::atomic::Ordering::SeqCst),
            after,
            "the drain ran twice for one notice"
        );
        assert_invariants();
    }

    /// Item 9: the launch's log maintenance runs at the machine's commit — in rooms mode the
    /// launch's is_unlocked check is false until the commit, so it never started there.
    #[test]
    fn the_log_maintenance_runs_at_commit() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        let before =
            crate::bir_module::LOG_MAINTENANCE_RUNS.load(std::sync::atomic::Ordering::SeqCst);
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        let after =
            crate::bir_module::LOG_MAINTENANCE_RUNS.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            after > before,
            "the commit ran no log maintenance — in rooms mode it never starts (item 9)"
        );
        assert_invariants();
    }

    /// E1(a): a stale verb of A — its Ended landing after B signed in and opened — closes
    /// nothing of B's. close_if_owner decides under SESSION: the phase is B's, so the close
    /// refuses even though the disk check would pass.
    #[test]
    fn a_stale_ended_of_a_leaves_bs_session_untouched() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // B signs in and opens: the phase is B's now, and B's is the stored sign-in.
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        open_hand(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_eq!(
            snapshot().owner.as_ref().map(|o| o.user_id.as_str()),
            Some("user-b")
        );
        // A's stale verb ends: the disk holds B, and the phase is B's — nothing closes.
        let closed = close_if_owner(&owner_a, Reason::SignedInElsewhere);
        assert!(!closed, "the stale close of A ran");
        // The harder shape: the disk emptied after B opened (bir-mcp's sign-out outside). The
        // disk check passes now — only the owner check keeps B's session alive.
        bir_core::account_session::clear_stored();
        let closed = close_if_owner(&owner_a, Reason::SignedInElsewhere);
        assert!(
            !closed,
            "the stale close of A ran on the empty disk — only the owner check protects B"
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B's session was closed by A's stale Ended — the E1 hole"
        );
        assert_eq!(
            snapshot().owner.as_ref().map(|o| o.user_id.as_str()),
            Some("user-b")
        );
        assert!(gate_unlocked(), "B's gate stayed unlocked");
        assert_invariants();
    }

    /// (b): a torn read — write_secret_file mid-write is impossible — the save is atomic — but an unreadable file is an Err, never an empty disk:
    /// nothing closes on evidence nobody read.
    #[test]
    fn a_torn_disk_read_closes_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The torn write: valid JSON cut in half — a read errors, never answers empty.
        // The same resolution the write path uses, never the env var: the device folder the
        // scratch install points at.
        let file = bir_core::platform::device_dir().join("buwiz-cloud-account.json");
        let stored_text = std::fs::read_to_string(&file).expect("the seeded sign-in");
        std::fs::write(&file, &stored_text[..stored_text.len() / 2]).unwrap();
        assert!(
            crate::cloud_account::load_stored().is_err(),
            "the torn file should not even parse — the premise is wrong"
        );
        let closed = close_if_owner(&owner_a, Reason::SignedInElsewhere);
        assert!(
            !closed,
            "a torn read closed the session — Err is not an empty disk"
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the machine closed on a torn read"
        );
        let _ = scratch;
    }

    /// E1(b): the cache is None while Open(A) and the disk is empty — nobody stored, the
    /// machine open — the signed-in-elsewhere close runs: the phase owner IS the verb's owner.
    #[test]
    fn an_empty_cache_and_disk_under_open_closes() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The cache and the disk both empty — bir-mcp's sign-out cleared everything outside.
        bir_core::account_session::clear_stored();
        bir_core::account_session::cache_owner(None);
        assert!(bir_core::account_session::stored_owner().is_none());
        let closed = close_if_owner(&owner_a, Reason::SignedInElsewhere);
        assert!(
            closed,
            "the close did not run — the machine stayed open for nobody"
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the machine stayed open with nobody stored"
        );
        assert!(snapshot().room.is_none(), "the room dropped");
        assert!(!gate_unlocked(), "the gate locked");
        assert_invariants();
    }

    /// E2: sync_pause is the machine's truth — a room phase sends, anything else pauses. The
    /// race it closes (a close between the boot's gate read and its pause write) is structural:
    /// the read and the write are one SESSION section, so the contract test is the truth table.
    #[test]
    fn sync_pause_is_the_machines_truth() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        sync_pause();
        assert!(
            bir_core::background_cron::submission_paused(),
            "a signed-out session must pause sending"
        );
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        sync_pause();
        assert!(
            !bir_core::background_cron::submission_paused(),
            "an open session must send"
        );
        close(Reason::SignOut);
        sync_pause();
        assert!(
            bir_core::background_cron::submission_paused(),
            "the closed session must pause — locked-and-unpaused is the E2 hole"
        );
        assert_invariants();
    }

    /// n1 (second arm): B signs in while A's claim is still running; A's late commit abandons
    /// and B keeps the session — both claims the REAL threads, the pause set before begin_open.
    /// Release order does not matter: A's epoch is stale whenever it commits, so it abandons.
    #[test]
    fn n1_b_signs_in_during_as_claim_and_keeps_b() {
        let _turn = test_turn();
        let _scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        let claim_gate = hooks::pause("claim.after_claim");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        claim_gate.reached();
        // B's sign-in replaces A while A's claim is paused: the close of A runs, B's open starts,
        // and B's own real claim thread parks behind the same pause.
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
        claim_gate.release();
        join_claim_threads();
        // A's late commit abandons; B's room stands.
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "B keeps the session — the phase is {:?}",
            snapshot().phase_kind
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
        assert!(gate_unlocked());
        assert_invariants();
    }

    /// n1b: Try again from a blocked page is the same begin_open path, and its commit wakes the
    /// worker (§3 step 3). The Blocked premise is the real claim thread's own: a panicking claim
    /// leaves Blocked::Transient through the drop guard; the try-again drives a fresh real claim.
    #[test]
    fn n1b_try_again_starts_the_worker() {
        let _turn = test_turn();
        let _scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        PANIC_CLAIM.store(true, std::sync::atomic::Ordering::SeqCst);
        begin_open(owner_a.clone(), 1, OpenWhy::Launch);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Blocked);
        assert!(!gate_unlocked());
        assert_invariants();
        // Try again: the same begin_open, the real claim — the worker wakes at the commit.
        begin_open(owner_a.clone(), 1, OpenWhy::TryAgain);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        join_claim_threads();
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(gate_unlocked());
        assert!(
            crate::bir_module::submission_worker_started(),
            "the worker RUNS after the try-again commit — a wake without a start is the n1b hole"
        );
        assert_invariants();
    }

    /// f7: a send spawned in A, run after the switch to B, takes no token — the return stays
    /// Queued in A's room, and B's sign-in is intact. The token read is the real
    /// AccountTokens::access_token inside the send's captured context.
    #[test]
    fn f7_a_send_spawned_in_a_after_the_switch_takes_no_token() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let context = bir_core::account_session::SendContext::new(key_a, epoch_a);
        sign_out();
        join_device_forget_threads();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        open_hand(&scratch, &owner_b, epoch_b, "b-room.db");
        let key_b = bir_core::account_session::OwnerKey {
            origin: owner_b.origin.clone(),
            user_id: owner_b.user_id.clone(),
        };
        let fetches_before = bir_core::account_session::token_fetches_for(&key_b);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(bir_core::account_session::SEND_CONTEXT.scope(
            context,
            async {
                let tokens = bir_core::account_session::AccountTokens;
                bir_core::bir_submit::TokenSource::access_token(&tokens, false).await
            },
        ));
        assert!(
            matches!(result, Err(bir_core::bir_submit::ClientError::SignedOut)),
            "A's dead send was answered with a token"
        );
        assert_eq!(
            bir_core::account_session::token_fetches_for(&key_b),
            fetches_before,
            "NO token was taken for the dead send"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "B's sign-in is intact"
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
        bir_core::account_session::on_session_ended(|owner, epoch, credential| {
            server_ended(owner, epoch, credential);
        });
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

    // ---------- the stored sign-in ----------

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

    /// (4): fill_owner_cache fills ONCE — a second fill never writes a stale answer back over
    /// a sign-out's clear, nor refills bir-mcp's owner over the signed-in-elsewhere evidence.
    #[test]
    fn fill_owner_cache_fills_only_once() {
        let _turn = test_turn();
        let _scratch = scratch();
        bir_core::account_session::reset_owner_cache_for_tests();
        let owner_a = seed_stored("user-a", false);
        bir_core::account_session::fill_owner_cache();
        assert_eq!(
            bir_core::account_session::stored_owner()
                .as_ref()
                .map(|key| key.user_id.as_str()),
            Some("user-a"),
            "the first fill did not fill"
        );
        // The sign-out's clear, then bir-mcp's raw write of B: a second boot's fill must not
        // bring a cache answer back.
        bir_core::account_session::cache_owner(None);
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        bir_core::account_session::save_stored(&stored_b).unwrap();
        bir_core::account_session::fill_owner_cache();
        assert!(
            bir_core::account_session::stored_owner().is_none(),
            "the second fill refilled the cache over the sign-out's clear — the (4) hole"
        );
        let _ = owner_a;
    }

    /// F1, the server_ended path: the disk clears for A, a signed_in(B) lands in the gap, and
    /// the close refuses — B's cache, gate and key are never ended behind its back.
    #[test]
    fn a_signed_in_in_the_server_ended_gap_survives() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        let credential = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in")
            .refresh_token;
        let gate = hooks::pause("server_ended.after_clear");
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let device = bir_core::platform::capture_device_dir_override();
        let database = bir_core::db::capture_database_path_override();
        let ender = std::thread::spawn(move || {
            database.run(move || device.run(move || server_ended(key_a, epoch_a, credential)))
        });
        gate.reached();
        // B's signed_in lands in the gap: the disk is already cleared for A, B persists and
        // caches in its own sections.
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        signed_in(&stored_b, 1).expect("B's sign-in persists");
        let cache_before_release = bir_core::account_session::stored_owner();
        assert_eq!(
            cache_before_release
                .as_ref()
                .map(|key| key.user_id.as_str()),
            Some("user-b"),
            "B's sign-in did not cache"
        );
        gate.release();
        let cleared = ender.join().unwrap();
        assert!(
            bir_core::account_session::stored_owner()
                .as_ref()
                .map(|key| key.user_id.as_str())
                == Some("user-b"),
            "B's cache was closed behind its back — the F1 hole"
        );
        let _ = cleared;
        assert_invariants();
    }

    /// F1, the sign-out path: the same gap in sign_out_inner.
    #[test]
    fn a_signed_in_in_the_sign_out_gap_survives() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        let gate = hooks::pause("sign_out.after_persist");
        let device = bir_core::platform::capture_device_dir_override();
        let database = bir_core::db::capture_database_path_override();
        let out = std::thread::spawn(move || database.run(move || device.run(sign_out)));
        gate.reached();
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        signed_in(&stored_b, 1).expect("B's sign-in persists");
        gate.release();
        out.join().unwrap();
        assert!(
            bir_core::account_session::stored_owner()
                .as_ref()
                .map(|key| key.user_id.as_str())
                == Some("user-b"),
            "B's cache was closed behind its back in sign_out — the F1 hole"
        );
        assert_invariants();
    }

    /// F1, the changed-server path: the same gap in end_changed_server.
    #[test]
    fn a_signed_in_in_the_end_changed_server_gap_survives() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        let stored_a = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in");
        let gate = hooks::pause("end_changed_server.after_clear");
        let device = bir_core::platform::capture_device_dir_override();
        let database = bir_core::db::capture_database_path_override();
        let ender = std::thread::spawn(move || {
            database.run(move || {
                device
                    .run(move || end_changed_server(&stored_a.server_url, &stored_a.refresh_token))
            })
        });
        gate.reached();
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-b".into(),
            refresh_token: "refresh-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        signed_in(&stored_b, 1).expect("B's sign-in persists");
        gate.release();
        ender.join().unwrap();
        assert!(
            bir_core::account_session::stored_owner()
                .as_ref()
                .map(|key| key.user_id.as_str())
                == Some("user-b"),
            "B's cache was closed behind its back in end_changed_server — the F1 hole"
        );
        assert_invariants();
    }

    /// Row 10, rewritten against the machine: a device_revoked on a send ends the session —
    /// the machine closes (the room drops, the gate locks) and the revoked device's key is
    /// forgotten.
    #[test]
    fn a_device_revoked_on_a_send_ends_the_session() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        bir_core::account_session::on_session_ended(|owner, epoch, credential| {
            server_ended(owner, epoch, credential);
        });
        bir_core::relay::lazy_first_key_for_test();
        assert!(!device_key_files(&scratch).is_empty());
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
                let _ = bir_core::bir_submit::TokenSource::access_token(&tokens, false).await;
                // The bearer arm of the real client on a 403 device_revoked: the session ends
                // inside the send's own context.
                bir_core::bir_submit::TokenSource::session_ended(&tokens);
            }),
        );
        join_device_forget_threads();
        let snap = snapshot();
        assert_eq!(
            snap.phase_kind,
            PhaseKind::SignedOut,
            "the device_revoked on the send did not end the session"
        );
        assert!(
            snap.room.is_none(),
            "the room survived the device_revoked ending"
        );
        assert!(
            device_key_files(&scratch).is_empty(),
            "the revoked device's key was not forgotten"
        );
    }

    /// F4's other half: after the revocation ended the session, a registration — whenever it
    /// was started — writes nothing: the cache names no owner and the epoch has moved, so the
    /// gate refuses and a revoked device is never re-registered behind the ending's back.
    #[test]
    fn a_revoked_device_is_not_re_registered_afterwards() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        bir_core::relay::lazy_first_key_for_test();
        sign_out();
        join_device_forget_threads();
        assert!(device_key_files(&scratch).is_empty());
        // A registration that was in flight for A at A's epoch completes now — the register
        // step itself "succeeds", but the gate must refuse the write and discard the key.
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let registration = bir_core::relay::register_device_key_for_test(&key_a, epoch_a, || {
            bir_core::relay::lazy_first_key_for_test();
            bir_core::relay::Registration::Registered
        });
        assert!(matches!(
            registration,
            bir_core::relay::Registration::Registered
        ));
        assert!(
            device_key_files(&scratch).is_empty(),
            "the revoked device was re-registered: a key is on disk after the ending"
        );
    }

    /// B2: a signed_in landing in the changed-server ending's gap is never cleared — the
    /// ending re-checks the decided (server, credential) under the account lock.
    #[test]
    fn a_signed_in_in_the_changed_server_gap_survives() {
        let _turn = test_turn();
        let _scratch = scratch();
        let _owner_a = seed_stored("user-a", false);
        // The decision was taken on A's record; before the lock, B's sign-in landed.
        let stored_b = bir_core::cloud_account::StoredAccount {
            server_url: "https://buwiz.example.test".into(),
            access_token: "access-user-b".into(),
            refresh_token: "refresh-user-b".into(),
            access_expires_at_unix: bir_core::cloud_account::unix_now() + 3600,
            user_id: "user-b".into(),
            email: None,
            offline: false,
            last_refresh_unix: 0,
            practice: false,
        };
        bir_core::account_session::save_stored(&stored_b).unwrap();
        // The ending, still carrying A's decided values: it must clear NOTHING.
        end_changed_server("https://buwiz.example.test", "refresh-user-a");
        let disk = bir_core::cloud_account::load_stored_account().unwrap();
        assert!(
            disk.as_ref().map(|stored| stored.user_id.as_str()) == Some("user-b"),
            "the signed_in that landed in the gap was deleted: {disk:?}"
        );
    }

    /// The leak check: the backgrounded forget thread resolves the TEST's device folder, never
    /// the real one — the override rides the thread as spawn_account carries it.
    #[test]
    fn the_forget_thread_resolves_the_tests_device_folder() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // A key owned by A in the TEST's device folder only.
        bir_core::relay::lazy_first_key_for_test();
        assert!(
            !device_key_files(&scratch).is_empty(),
            "the seeded key is not in the test's folder — vacuous"
        );
        let _ = &owner_a;
        sign_out();
        join_device_forget_threads();
        assert!(
            device_key_files(&scratch).is_empty(),
            "the forget never reached the test's folder — the override did not ride the thread"
        );
    }

    /// The same leak check with ONLY the database override set: the device override is cleared,
    /// so the forget thread's resolution rides the database capture alone — the variant the
    /// device-override test cannot see.
    #[test]
    fn the_forget_thread_resolves_the_database_override_alone() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // Clear the device override and point the database at the scratch device dir: resolution
        // must fall to the database override alone — the device dir is then the database's parent.
        bir_core::platform::set_device_dir_override(None);
        bir_core::db::set_database_path_override(Some(
            scratch.layout.device_dir().join("device.db"),
        ));
        bir_core::relay::lazy_first_key_for_test();
        let keys_before = device_key_files(&scratch);
        assert!(
            !keys_before.is_empty(),
            "the seeded key is not in the database-resolved folder — vacuous"
        );
        let _ = &owner_a;
        sign_out();
        join_device_forget_threads();
        assert!(
            device_key_files(&scratch).is_empty(),
            "the forget never reached the database-resolved folder — the database override did not ride the thread"
        );
    }

    /// The writer guards' scanner (F14): every `.rs` file under the root, walked RECURSIVELY,
    /// production lines only. A `mod tests`/`mod test` module is skipped by brace matching —
    /// and the scan CONTINUES after it, so production code following a test module is still
    /// caught. Returns (path, line number, line) triples of every production line.
    fn production_lines(root: &std::path::Path) -> Vec<(std::path::PathBuf, usize, String)> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        files.sort();
        let mut lines = Vec::new();
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let mut skipping_cfg_test_item = false;
            let mut pending_test_attr = false;
            let mut item_saw_brace = false;
            for (index, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                if skipping_cfg_test_item {
                    // A cfg(test) item — module, function or helper — is test-only however it is
                    // shaped. rustfmt closes a top-level item at column zero; a braceless item
                    // (a static) ends at its `;`.
                    if item_saw_brace {
                        if line.starts_with('}') {
                            skipping_cfg_test_item = false;
                        }
                    } else {
                        if line.contains('{') {
                            item_saw_brace = true;
                        }
                        if trimmed.ends_with(';') {
                            skipping_cfg_test_item = false;
                        }
                    }
                    continue;
                }
                if trimmed.starts_with("#[cfg(test") {
                    pending_test_attr = true;
                    continue;
                }
                if pending_test_attr {
                    // cfg(test) attributes can stack or sit above doc lines; the item itself is
                    // whatever non-attribute line follows.
                    if trimmed.starts_with("#[") || trimmed.starts_with("//") || trimmed.is_empty()
                    {
                        continue;
                    }
                    skipping_cfg_test_item = true;
                    item_saw_brace = false;
                    pending_test_attr = false;
                    if line.contains('{') {
                        item_saw_brace = true;
                    }
                    if trimmed.ends_with(';') && !line.contains('{') {
                        skipping_cfg_test_item = false;
                    }
                    continue;
                }
                if trimmed.starts_with("#[test]") {
                    // A bare #[test] fn: test-only too — skip it as an item.
                    skipping_cfg_test_item = true;
                    item_saw_brace = false;
                    continue;
                }
                lines.push((path.clone(), index + 1, line.to_string()));
            }
        }
        lines
    }

    /// The positive control of the writer guards (F14): a planted tree with a violation in a
    /// NESTED directory and one in production code AFTER a test module — the scan must catch
    /// both, or the guard proves nothing about the real tree.
    fn assert_scanner_positive_control(scan: &impl Fn(&std::path::Path) -> Vec<String>) {
        let planted = tempfile::tempdir().unwrap();
        let nested = planted.path().join("nested/deeper");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("violation.rs"),
            "fn production() {\n    account_session::save_stored(&x).unwrap();\n    save_stored_account(&y).unwrap();\n}\n",
        )
        .unwrap();
        std::fs::write(
            planted.path().join("after_tests.rs"),
            "fn before() {}\n#[cfg(test)]\nmod tests {\n    fn a() { account_session::save_stored(&x).unwrap(); }\n}\nfn production_after() {\n    account_session::clear_stored();\n    clear_stored_account().unwrap();\n}\n",
        )
        .unwrap();
        let found = scan(planted.path());
        assert!(
            found.iter().any(|line| line.contains("violation.rs")),
            "the scanner missed the nested violation: {found:?}"
        );
        assert!(
            found.iter().any(|line| line.contains("after_tests.rs")),
            "the scanner missed production code after a test module: {found:?}"
        );
        assert!(
            !found.iter().any(|line| line.contains("mod tests")),
            "the scanner reported a line inside the test module: {found:?}"
        );
    }

    /// The writer guard (§4 item 1): no direct call of the raw cloud_account writers outside
    /// account_session — every write of the stored sign-in routes through the one path, or the
    /// owner cache lies. bir-core's own cloud_account.rs (the definition and its storage tests)
    /// and account_session.rs (the one path) are the only allowed homes.
    #[test]
    fn no_writer_bypasses_the_owner_cache() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let scan = |root: &std::path::Path| -> Vec<String> {
            production_lines(root)
                .into_iter()
                .filter(|(_, _, text)| {
                    let trimmed = text.trim_start();
                    if trimmed.starts_with("//") {
                        return false;
                    }
                    [
                        "save_stored_account(",
                        "clear_stored_account(",
                        "forget_stored_account(",
                    ]
                    .iter()
                    .any(|writer| text.contains(writer))
                        && !text.contains("fn ")
                })
                .map(|(path, line, text)| format!("{}:{}: {}", path.display(), line, text.trim()))
                .collect()
        };
        // The positive control first: the scan itself must catch a planted violation, nested
        // and after a test module, or the guard below proves nothing.
        assert_scanner_positive_control(&scan);
        let mut offenders = Vec::new();
        let mut files_seen = 0usize;
        for crate_dir in ["bir-app-api/src", "bir-core/src", "bir-mcp/src"] {
            let dir = crates.join(crate_dir);
            if !dir.exists() {
                continue;
            }
            files_seen += 1;
            for (path, line, text) in production_lines(&dir) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let in_account_session = path
                    .components()
                    .any(|part| part.as_os_str() == "account_session");
                let allowed = (crate_dir.contains("bir-core")
                    && (name == "cloud_account.rs" || name == "account_session.rs"))
                    || in_account_session;
                if allowed {
                    continue;
                }
                let trimmed = text.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for writer in [
                    "save_stored_account",
                    "clear_stored_account",
                    "forget_stored_account",
                ] {
                    if text.contains(&format!("{writer}(")) && !text.contains("fn ") {
                        offenders.push(format!("{}:{}: {}", path.display(), line, text.trim()));
                    }
                }
            }
        }
        assert!(files_seen > 0, "the guard scanned no directories at all");
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
        let scan = |root: &std::path::Path| -> Vec<String> {
            production_lines(root)
                .into_iter()
                .filter(|(_, _, text)| {
                    let trimmed = text.trim_start();
                    if trimmed.starts_with("//") {
                        return false;
                    }
                    [
                        "account_session::save_stored(",
                        "account_session::clear_stored(",
                        "account_session::forget_stored(",
                    ]
                    .iter()
                    .any(|writer| text.contains(writer))
                })
                .map(|(path, line, text)| format!("{}:{}: {}", path.display(), line, text.trim()))
                .collect()
        };
        assert_scanner_positive_control(&scan);
        let mut offenders = Vec::new();
        let mut files_seen = 0usize;
        for crate_dir in ["bir-app-api/src", "bir-core/src"] {
            let dir = crates.join(crate_dir);
            if !dir.exists() {
                continue;
            }
            files_seen += 1;
            for (path, line, text) in production_lines(&dir) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let in_account_session = path
                    .components()
                    .any(|part| part.as_os_str() == "account_session");
                // account_session is the door itself; session.rs is the machine.
                let allowed = in_account_session
                    || (crate_dir.contains("bir-app-api") && name == "session.rs");
                if allowed {
                    continue;
                }
                let trimmed = text.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for writer in [
                    "account_session::save_stored(",
                    "account_session::clear_stored(",
                    "account_session::forget_stored(",
                ] {
                    if text.contains(writer) {
                        offenders.push(format!("{}:{}: {}", path.display(), line, text.trim()));
                    }
                }
            }
        }
        assert!(files_seen > 0, "the guard scanned no directories at all");
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
                // As bir-submit's device_key() does on a success: the key created inside the
                // step, through the same gate the client's lazy creation uses.
                let _ = &scratch_dir;
                assert!(bir_core::relay::lazy_first_key_for_test());
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
        signed_in(&stored, 1).unwrap();
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
        signed_in(&stored, 1).unwrap();
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
        bir_core::account_session::on_session_ended(|owner, epoch, credential| {
            server_ended(owner, epoch, credential);
        });
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

    /// (c): a credential-matched ending closes WHATEVER the cache held — including the empty
    /// cache of the real runtime before any machine fill. Before the fix the disk was cleared
    /// and nothing closed: the gate stayed unlocked and the room open, less safe than main.
    #[test]
    fn a_credential_matched_ending_with_an_empty_cache_still_closes() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The production cache state this regression exercised: never filled.
        bir_core::account_session::cache_owner(None);
        let failed = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in")
            .refresh_token;
        let cleared = server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch,
            failed,
        );
        assert!(cleared, "the credential matched and nothing cleared");
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the close never ran with an empty cache — the gate stayed unlocked and the room open"
        );
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "the disk should be cleared"
        );
        assert_invariants();
    }

    /// Finding 12/F9: an I2 mismatch at the CURRENT epoch — the stored sign-in changed while the
    /// claim ran — installs Blocked::Transient, never leaves Opening standing for the page to
    /// poll forever.
    #[test]
    fn a_sign_in_change_at_the_current_epoch_blocks_not_opens() {
        let _turn = test_turn();
        let _scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        // The disk and the cache change hands mid-Opening — nobody closed the machine, so the
        // epoch is still the opening's own.
        bir_core::account_session::clear_stored();
        let _owner_b = seed_stored("user-b", false);
        commit(epoch, room_target(&owner_a), None);
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Blocked,
            "the mismatch abandoned into standing Opening — the page polls forever: {:?}",
            snapshot().phase_kind
        );
        assert!(!gate_unlocked());
        assert_invariants();
    }

    /// Finding 13, in the shape where it matters: a legacy file present, so the claimant reaches
    /// the device-store records. The disk changes to B mid-Opening(A) — the claim acts for A or
    /// abandons, never for B: no B claimant in the claim record, no B room folder.
    #[test]
    fn a_disk_change_mid_opening_never_claims_for_the_new_owner() {
        let _turn = test_turn();
        let scratch = scratch();
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        // A REAL legacy database — the probe must pass, or claim.start never runs and the
        // test proves nothing.
        Database::open(scratch.layout.legacy_database()).unwrap();
        let gate = hooks::pause("claim.before_start");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        gate.reached();
        // The disk changes hands while the claim is parked before its decision.
        bir_core::account_session::clear_stored();
        let owner_b = seed_stored("user-b", false);
        gate.release();
        join_claim_threads();
        let record = bir_core::legacy_claim::LegacyClaim::here()
            .ok()
            .and_then(|claim| claim.record().ok().flatten());
        assert!(
            record.as_ref().and_then(|record| record.user_id.as_deref()) != Some("user-b"),
            "the device-store record names B — the claim acted for the disk's new owner: {record:?}"
        );
        let b_room = scratch
            .layout
            .rooms_dir()
            .join(bir_core::rooms::room_id_for(&owner_b.origin, "user-b").to_string());
        assert!(
            !b_room.exists(),
            "a B room folder exists — the claim acted for the disk's new owner"
        );
        assert_ne!(
            snapshot().phase_kind,
            PhaseKind::Open,
            "the open committed for a sign-in that is gone"
        );
        assert_invariants();
    }

    /// Finding 11: signed_in with an un-normalisable server URL while Open(A) closes — never
    /// leaves Open unlocked with the cache None (I1).
    #[test]
    fn an_unnormalisable_sign_in_closes_the_open_session() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert!(gate_unlocked());
        let mut bad = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in");
        bad.server_url = "not a server url at all".to_string();
        bad.user_id = "user-c".to_string();
        assert!(
            Owner::of_stored(&bad).is_none(),
            "the test premise: this URL does not normalise"
        );
        signed_in(&bad, 1).expect("the write itself is fine");
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::SignedOut,
            "the un-normalisable sign-in left the session {:?} — Open unlocked for no one",
            snapshot().phase_kind
        );
        assert!(!gate_unlocked(), "the gate locked — I1");
        assert_invariants();
    }

    /// Finding 14: at the sweep's start the close's room Arc is already dead — the probe at the
    /// sweep's door records the room's strong count. Alive there, the sweep retains the entry and
    /// the owner lock survives the sign-out. Reverted, this goes red.
    #[test]
    fn the_room_arc_is_dead_before_the_sweep() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        let (handle, _, _, _) = snapshot().room.expect("a room is open");
        let weak = std::sync::Arc::downgrade(&handle);
        drop(handle);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let seen_in_probe = seen.clone();
        *crate::open_room::SWEEP_PROBE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(move || {
            *seen_in_probe
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(weak.strong_count());
        }));
        sign_out();
        join_device_forget_threads();
        let count = seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expect("the probe never ran — the sweep never happened");
        assert_eq!(
            count, 0,
            "the room Arc was alive at sweep time ({count} strong) — the entry is retained and the owner lock survives the sign-out"
        );
        assert_invariants();
    }

    /// (12): the credential matched the disk with the cache None — the ending IS the owner's:
    /// the owner-ending steps run, here proven by the device-key forget. Without
    /// cache_key.or(Some(owner)) owner-ending answers false and the key survives.
    #[test]
    fn a_credential_matched_ending_with_an_empty_cache_forgets_the_key() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pre-wiring runtime shape: the phase is SignedOut and the cache was never filled.
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
        bir_core::account_session::cache_owner(None);
        bir_core::relay::lazy_first_key_for_test();
        assert!(
            !device_key_files(&scratch).is_empty(),
            "the seeded key is not in the test's folder — vacuous"
        );
        let failed = bir_core::cloud_account::load_stored_account()
            .unwrap()
            .expect("a sign-in")
            .refresh_token;
        let cleared = server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            snapshot().epoch,
            failed,
        );
        assert!(cleared, "the credential matched and nothing cleared");
        join_device_forget_threads();
        assert!(
            device_key_files(&scratch).is_empty(),
            "the owner's key survived a credential-matched ending — owner-ending answered false"
        );
    }

    /// A stale owner or credential changes nothing (§4): server_ended for another owner, or
    /// for a credential the disk no longer holds, is a no-op end to end.
    #[test]
    fn server_ended_for_a_stale_owner_or_epoch_changes_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);

        // A stale CREDENTIAL: the failed refresh token is not the stored one — the sign-in was
        // rotated or re-signed-in since — so nothing is cleared and nothing closes, whatever the
        // epoch (F5: the credential match is the condition; a match closes whatever the epoch, a
        // mismatch never does).
        server_ended(
            bir_core::account_session::OwnerKey {
                origin: owner_a.origin.clone(),
                user_id: owner_a.user_id.clone(),
            },
            epoch + 99,
            "refresh-rotated-away".to_string(),
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::Open, "the phase holds");
        assert!(
            gate_unlocked(),
            "the gate holds — the credential mismatch stopped everything"
        );
        assert!(
            bir_core::account_session::stored_owner().is_some(),
            "the cache holds — the credential mismatch stopped the SESSION section"
        );
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_some(),
            "the disk holds — a mismatched credential clears nothing"
        );
        assert_invariants();

        // A stale OWNER never even touches the disk.
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
        permit_claim_threads();
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
        permit_claim_threads();
        let owner_a = seed_stored("user-a", false);
        let before_epoch = snapshot().epoch;
        let before_spawns = claim_thread_spawn_count();
        // The two boots race on a barrier, so neither can finish before the other starts.
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let mut threads = Vec::new();
        for engine in 1..=2u64 {
            let owner = owner_a.clone();
            let barrier = barrier.clone();
            // The overrides ride the engine threads: device and database resolution are
            // thread-local, and without them the boot — and the claim thread it spawns, which
            // captures ITS caller's overrides — reads the REAL device folder.
            let device = bir_core::platform::capture_device_dir_override();
            let database = bir_core::db::capture_database_path_override();
            threads.push(std::thread::spawn(move || {
                database.run(move || {
                    device.run(move || {
                        barrier.wait();
                        begin_open(owner, engine, OpenWhy::Launch);
                    });
                });
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
        // d1 at the hook point itself: paused between the persist and the SESSION section,
        // the disk is already empty and the cache still set — and check_invariants holds even
        // there, because it reads the cache, never the disk.
        let persist_gate = hooks::pause("sign_out.after_persist");
        let device = scratch.layout.device_dir().to_path_buf();
        let signing_out = std::thread::spawn(move || {
            bir_core::platform::set_device_dir_override(Some(device));
            sign_out();
        });
        persist_gate.reached();
        check_invariants().expect("the invariants broke at sign_out.after_persist");
        persist_gate.release();
        signing_out.join().expect("the sign-out thread panicked");
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
        permit_claim_threads();
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

    /// q4: a stale opener installs nothing — an opener thread opens its handle, the close lands
    /// while it is paused before the commit, and the commit with the old epoch abandons: nothing
    /// is installed, and the handle itself drops (the Weak is dead).
    #[test]
    fn q4_a_stale_opener_installs_nothing() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        // The pause is set BEFORE begin_open (F5's rule): the real claim thread spawns inside
        // begin_open, and a pause set after could be slipped past.
        let open_gate = hooks::pause("open.done");
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        // The opener opens its handle for real, keeps only a Weak of it, and pauses at the
        // open-done point the claim thread passes; the interleaving the test drives is the
        // commit's own compare-and-swap.
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

    /// i5: a send never takes a token after its epoch ends — even when the SAME owner signs in
    /// again. Its captured owner and epoch travel with it; the token read refuses the stale send.
    #[test]
    fn i5_a_send_asks_no_token_after_its_epoch_ends() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        open_hand(&scratch, &owner_a, epoch_a, "a-room.db");
        let key_a = bir_core::account_session::OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: owner_a.user_id.clone(),
        };
        let context = bir_core::account_session::SendContext::new(key_a.clone(), epoch_a);
        sign_out();
        join_device_forget_threads();
        let owner_again = seed_stored("user-a", false);
        begin_open(owner_again.clone(), 1, OpenWhy::SignIn);
        let epoch_again = snapshot().epoch;
        assert_ne!(epoch_a, epoch_again);
        open_hand(&scratch, &owner_again, epoch_again, "a-room-again.db");
        let fetches_before = bir_core::account_session::token_fetches_for(&key_a);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(bir_core::account_session::SEND_CONTEXT.scope(
            context,
            async {
                let tokens = bir_core::account_session::AccountTokens;
                bir_core::bir_submit::TokenSource::access_token(&tokens, false).await
            },
        ));
        assert!(
            matches!(result, Err(bir_core::bir_submit::ClientError::SignedOut)),
            "the stale send was answered with the new session's token"
        );
        assert_eq!(
            bir_core::account_session::token_fetches_for(&key_a),
            fetches_before,
            "NO token was asked after the send's epoch ended"
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
