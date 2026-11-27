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
pub static BYPASSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

/// close_locked: the one close, under SESSION, in the design's order (§4). The room leaves the
/// phase here; its Arc drops after the lock is released, so the database's close never holds
/// SESSION. Returns what close() must drop outside the lock.
fn close_locked(session: &mut Session) -> Option<OpenRoom> {
    session.epoch += 1;
    mirror(session.epoch);
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    let room = match std::mem::replace(&mut session.phase, Phase::SignedOut) {
        Phase::Open { room, .. } | Phase::Legacy { room, .. } => Some(room),
        _ => None,
    };
    // The room path cache reads None once nothing is open (X1).
    bir_core::db::set_open_room_database(None);
    bir_core::email::cancel_oauth_listeners();
    if let Ok(submitter) = bir_core::relay::submitter() {
        submitter.invalidate_account_tins();
    }
    bir_core::relay::forget_device();
    room
}

/// The one sign-out, and begin_open's replace (§4). Runs entirely under the lock; the room's Arc
/// drops after it lets go.
pub fn close(reason: Reason) {
    hooks::at("close.locked");
    let dropped = {
        let mut session = locked();
        if session.phase.kind() == PhaseKind::Quitting {
            // F7: Quitting is terminal — a close after it is a logged no-op, never a resurrection
            // of SignedOut behind the quit's back.
            tracing::info!("close after quit ignored: the session is terminal");
            return;
        }
        close_locked(&mut session)
    };
    tracing::info!("session closed: {reason:?}");
    drop(dropped);
    report_invariants();
}

/// begin_open (§3 step 1). A no-op for the same owner already open, opening or legacy (X1: the
/// second engine's launch resumes nothing twice). Anything else ends in the same critical
/// section: the epoch bumps, the phase becomes Opening, the claim thread spawns and this returns
/// at once (F10).
pub fn begin_open(owner: Owner, engine: u64, why: OpenWhy) {
    let (spawn, dropped_room) = {
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
        let dropped_room = close_locked(&mut session);
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
                (Some(session.epoch), dropped_room)
            }
            Err(detail) => {
                session.phase = Phase::Blocked {
                    owner: owner.clone(),
                    reason: Blocked::Transient {
                        copy_saved: false,
                        detail,
                    },
                };
                (None, dropped_room)
            }
        }
    };
    // F17: the replaced room's Arc drops after SESSION is released.
    drop(dropped_room);
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
fn stored_is(owner: &Owner) -> bool {
    let _stored = bir_core::account_session::lock();
    stored_owner_key()
        .is_some_and(|(origin, user_id)| origin == owner.origin && user_id == owner.user_id)
}

/// The stored sign-in's (normalised origin, user id), or None while nobody is stored. The
/// caller holds `account_session::lock()` when it needs the read to be atomic with a write.
fn stored_owner_key() -> Option<(String, String)> {
    let stored = crate::cloud_account::load_stored().ok().flatten()?;
    let origin = bir_core::rooms::room_origin(&stored.server_url).ok()?;
    Some((origin, stored.user_id))
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
    let stored = {
        let _stored = bir_core::account_session::lock();
        stored_owner_key()
    };
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
        let dropped = close_locked(&mut session);
        session.phase = Phase::SignedOut;
        dropped
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
        crate::open_room::reset_for_tests();
        reset_for_tests();
        bir_core::db::set_open_room_database(None);
        crate::account_gate::set_unlocked(false);
        bir_core::cloud_account::clear_stored_account();
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
        bir_core::cloud_account::save_stored_account(&stored).unwrap();
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

    /// commit 2 wires the claim thread's real work (the claim, the classify, the open, the
    /// commit); the tests then drive the real thread with pauses on its real hooks, never this.
    fn commit2_claim_thread_opens(scratch: &Scratch, owner: &Owner, epoch: u64, name: &str) {
        let _ = (scratch, owner, epoch, name);
        panic!("commit 2 wires this")
    }

    /// commit 3 wires the server-ended hook: a 401 or InvalidGrant ends the session for the
    /// owner it arrived under, and only that owner.
    fn commit3_server_ended(owner: &Owner, epoch: u64) {
        let _ = (owner, epoch);
        panic!("commit 3 wires this")
    }

    /// commit 2 (the stored-sign-in commit, renumbered) wires sign_out(): the stored sign-in's
    /// clear inside the same critical section as the close. The stub must not contain D1's
    /// window — a clear-then-close in two steps IS the hole — so it panics instead.
    fn commit3_sign_out() {
        panic!("commit 2 wires this: the real session::sign_out")
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
    #[ignore = "commit 2"]
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
        commit3_sign_out();
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
    #[ignore = "commit 2"]
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
        bir_core::cloud_account::clear_stored_account();
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
        commit2_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
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
    #[ignore = "commit 2"]
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
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
        commit3_sign_out();
        bir_core::cloud_account::clear_stored_account();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        let fetches_before = bir_core::account_session::token_fetch_count();
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
            bir_core::account_session::token_fetch_count(),
            fetches_before,
            "NO token was taken for the dead send"
        );
        assert_invariants();
    }

    /// stored_b_survives_as_late_401: A's send's late 401 ends nothing of B's session.
    #[test]
    #[ignore = "commit 3"]
    fn stored_b_survives_as_late_401() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // The switch: A signs out, B signs in.
        commit3_sign_out();
        bir_core::cloud_account::clear_stored_account();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        assert_invariants();
        // A's late 401 arrives: it acts for the session that existed when the send started.
        commit3_server_ended(&owner_a, epoch_a);
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
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_some(),
            "B's stored sign-in survives"
        );
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
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
    #[ignore = "commit 3"]
    fn d1_sign_out_changes_the_stored_sign_in_and_the_gate_in_one_step() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        assert_invariants();
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_invariants();
        assert!(gate_unlocked());
        // The sign-out: one critical section holds the stored clear and the close (commit 3).
        commit3_sign_out();
        assert!(!gate_unlocked(), "the gate locked in the same step");
        assert!(
            bir_core::cloud_account::load_stored_account()
                .unwrap()
                .is_none(),
            "the stored sign-in is gone in the same step"
        );
        assert_invariants();
    }

    /// An Http stub whose exchange blocks on a harness gate: the refresh's network wait, parked
    /// exactly where the test wants it, with no sleeps.
    struct GateBlockedHttp {
        gate: hooks::Gate,
    }

    impl bir_core::cloud_account::Http for GateBlockedHttp {
        fn exchange(
            &mut self,
            _request: &bir_core::cloud_account::HttpRequest,
        ) -> Result<bir_core::cloud_account::HttpResponse, bir_core::cloud_account::CloudError>
        {
            hooks::at("refresh.network");
            let _ = &self.gate;
            // Released only when the test drops the gate: until then this thread parks inside
            // the wait below. The gate itself is the harness's — reaching here means the pause
            // already blocks us, so nothing further is needed.
            Err(bir_core::cloud_account::CloudError::Network(
                "parked".to_string(),
            ))
        }
    }

    /// d2: a slow refresh never holds SESSION — snapshot() and a commit return while the
    /// refresh's network wait is still parked (the refresh restructure of §4).
    #[test]
    #[ignore = "commit 3"]
    fn d2_a_slow_refresh_never_holds_session() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
        assert_eq!(snapshot().phase_kind, PhaseKind::Open);
        // A real refresh, parked in its network wait on the harness gate.
        let network_gate = hooks::pause("refresh.network");
        let refresh = std::thread::spawn(move || {
            let mut http = GateBlockedHttp {
                gate: hooks::pause("refresh.never"),
            };
            let secrets = bir_core::cloud_account::AccountSecrets {
                server_url: "https://buwiz.example.test".into(),
                access_token: "access-user-a".into(),
                refresh_token: "refresh-user-a".into(),
                access_expires_at_unix: 0,
                user_id: "user-a".into(),
                email: Some("user-a@example.test".into()),
                offline: false,
                practice: false,
            };
            let _ = bir_core::cloud_account::refresh_session(
                &mut http,
                &secrets,
                bir_core::cloud_account::unix_now(),
            );
        });
        network_gate.reached();
        // While the refresh is parked: snapshot() returns at once, and a commit does not wait
        // on the refresh either. The commit's own epoch is stale here — what matters is that it
        // RETURNS, not what it installs.
        let snap = snapshot();
        assert_eq!(
            snap.phase_kind,
            PhaseKind::Open,
            "snapshot never waits on a refresh"
        );
        let (handle, path) = scratch_handle(&scratch, "late-room.db");
        commit(epoch, room_target(&owner_a), Some((handle, path)));
        commit2_refresh_is_restructured();
        network_gate.release();
        refresh.join().unwrap();
        assert_invariants();
    }

    /// commit 2 (the stored-sign-in commit) restructures the refresh: load under the lock,
    /// release, the network wait with NO lock, save_stored_if under the lock. Until then the
    /// production path holds account_session::lock() across the network — D2 itself — and this
    /// test cannot pass.
    fn commit2_refresh_is_restructured() {
        panic!("commit 2 wires this: the refresh restructure")
    }

    /// d4a: a device-key registration of A's, resolving after the switch, writes nothing for B.
    #[test]
    #[ignore = "commit 4"]
    fn d4a_a_late_registration_of_a_writes_no_device_key_for_b() {
        let _turn = test_turn();
        let scratch = scratch();
        let owner_a = seed_stored("user-a", false);
        begin_open(owner_a.clone(), 1, OpenWhy::SignIn);
        let epoch_a = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        // The switch.
        commit3_sign_out();
        bir_core::cloud_account::clear_stored_account();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch, "a-room.db");
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
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
        commit3_sign_out();
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
        commit2_claim_thread_opens(&scratch, &owner_a, epoch_a, "a-room.db");
        // The switch before the send's token read.
        commit3_sign_out();
        bir_core::cloud_account::clear_stored_account();
        let owner_b = seed_stored("user-b", false);
        begin_open(owner_b.clone(), 1, OpenWhy::SignIn);
        let epoch_b = snapshot().epoch;
        commit2_claim_thread_opens(&scratch, &owner_b, epoch_b, "b-room.db");
        // A's send reaches its token: it must not read B's.
        let fetches_before = bir_core::account_session::token_fetch_count();
        commit4_spawn_send(&owner_a, epoch_a);
        assert_eq!(
            bir_core::account_session::token_fetch_count(),
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
