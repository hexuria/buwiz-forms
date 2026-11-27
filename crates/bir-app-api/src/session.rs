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
        if let Some(current) = session.phase.owner()
            && *current == owner
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
fn spawn_claim_thread(owner: Owner, engine: u64, why: OpenWhy, epoch: u64) {
    let _ = (engine, why);
    if let Err(error) = std::thread::Builder::new()
        .name("buwiz-room-open".to_string())
        .spawn(move || {
            let _ = (owner, epoch);
            // commit 2: CLAIM_RUN, the hooks, LegacyClaim::here() and classify, start(),
            // room_open_allowed, open_path, then commit(epoch, target, handle).
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
pub fn commit(epoch: u64, target: Target, handle: Option<(Arc<Mutex<Database>>, PathBuf)>) {
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
                tracing::info!("commit abandoned: epoch {epoch} is not current or nothing is opening");
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
fn stored_is(owner: &Owner) -> bool {
    let _stored = bir_core::account_session::lock();
    stored_owner_key().is_some_and(|(origin, user_id)| origin == owner.origin && user_id == owner.user_id)
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
    {
        let mut session = locked();
        session.epoch += 1;
        mirror(session.epoch);
        session.phase = Phase::Quitting;
        crate::account_gate::set_unlocked(false);
        bir_core::transport::set_practice_account(false);
    }
    hooks::at("quit.after_bump");
    report_invariants();
}

static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
    if crate::account_gate::auth_bypass_enabled() {
        // Outside the machine (§9): the bypass unlocks with no owner.
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
    let unlocked = crate::account_gate::is_unlocked();
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
    }

    static NEXT_GATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    pub fn pause(name: &'static str) -> Gate {
        let generation = NEXT_GATE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mutex = harness();
        {
            let mut state = lock(mutex);
            state
                .points
                .entry(name)
                .or_insert(Point {
                    gate: None,
                    arrivals: 0,
                })
                .gate = Some((generation, false));
        }
        Gate { name, generation }
    }

    impl Gate {
        /// Wait until a thread arrives at the paused point. Bounded, so a broken machine fails
        /// the test instead of hanging the suite.
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
                let waiting = state
                    .points
                    .get(self.name)
                    .and_then(|point| point.gate)
                    .is_some_and(|(generation, released)| {
                        generation == self.generation && !released
                    });
                if arrivals > 0 && waiting {
                    return;
                }
                if arrivals > 0 && !waiting {
                    // Arrived before the pause was set: still counts as reached.
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

    fn owner(user_id: &str) -> Owner {
        Owner {
            origin: "https://buwiz.example.test".to_string(),
            user_id: user_id.to_string(),
            practice: false,
        }
    }

    /// The epoch bumps at every ownership-ending transition, and the mirror follows (§1).
    #[test]
    fn close_bumps_the_epoch_and_the_mirror() {
        let _turn = test_turn();
        reset_for_tests();
        let before = snapshot().epoch;
        close(Reason::SignOut);
        assert_eq!(snapshot().epoch, before + 1, "close bumps the epoch");
        assert_eq!(
            bir_core::session_epoch::current(),
            before + 1,
            "the mirror follows the epoch"
        );
        assert_eq!(snapshot().phase_kind, PhaseKind::SignedOut);
    }

    /// begin_open is a no-op for the same owner already opening or open (X1), and refuses while
    /// quitting; another owner replaces (the epoch bumps again).
    #[test]
    fn begin_open_noops_for_the_same_owner() {
        let _turn = test_turn();
        reset_for_tests();
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
        let epoch = snapshot().epoch;
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        begin_open(owner("user-a"), 2, OpenWhy::Launch);
        assert_eq!(
            snapshot().epoch,
            epoch,
            "a second engine's begin_open for the same owner is a no-op"
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
    }

    /// quit is terminal: begin_open after it is refused in code and logged, and the phase does
    /// not move.
    #[test]
    fn begin_open_after_quit_is_refused() {
        let _turn = test_turn();
        reset_for_tests();
        quit();
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        let epoch = snapshot().epoch;
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
        assert_eq!(snapshot().phase_kind, PhaseKind::Quitting);
        assert_eq!(snapshot().epoch, epoch, "nothing moved");
    }

    /// A commit whose epoch isn't current abandons — even while the phase is still Opening, for
    /// the NEXT owner: A's opener committing after B replaced A must not move B's open (q4's
    /// machine half; the full pause-driven test un-ignores in commit 2).
    #[test]
    fn a_stale_commit_installs_nothing() {
        let _turn = test_turn();
        reset_for_tests();
        begin_open(owner("user-a"), 1, OpenWhy::Launch);
        let epoch_a = snapshot().epoch;
        begin_open(owner("user-b"), 1, OpenWhy::SignIn);
        assert_eq!(snapshot().phase_kind, PhaseKind::Opening);
        commit(
            epoch_a,
            Target::Blocked(Blocked::Transient {
                copy_saved: false,
                detail: "x".to_string(),
            }),
            None,
        );
        assert_eq!(
            snapshot().phase_kind,
            PhaseKind::Opening,
            "A's stale commit changed nothing of B's open"
        );
        assert_eq!(
            snapshot()
                .owner
                .as_ref()
                .map(|owner| owner.user_id.as_str()),
            Some("user-b"),
            "B is still the owner opening"
        );
    }

    /// A commit while nothing is opening is abandoned too (§2: illegal transitions refused).
    #[test]
    fn a_commit_outside_opening_installs_nothing() {
        let _turn = test_turn();
        reset_for_tests();
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
    }

    // ---- §11's named tests, present now, un-ignored by the commit that wires their path ----

    #[test]
    #[ignore = "commit 2"]
    fn n1_sign_out_during_the_claim_opens_nothing() {}

    #[test]
    #[ignore = "commit 2"]
    fn n1_b_signs_in_during_as_claim_and_keeps_b() {}

    #[test]
    #[ignore = "commit 2"]
    fn n1b_try_again_starts_the_worker() {}

    #[test]
    #[ignore = "commit 5"]
    fn n2_a_store_read_error_with_a_moved_record_opens_nothing() {}

    #[test]
    #[ignore = "commit 4"]
    fn n3_a_preconnect_then_the_get_completes_the_consent() {}

    #[test]
    #[ignore = "commit 4"]
    fn n3_a_bind_during_close_is_refused() {}

    #[test]
    #[ignore = "commit 4"]
    fn f7_a_send_spawned_in_a_after_the_switch_takes_no_token() {}

    #[test]
    #[ignore = "commit 3"]
    fn stored_b_survives_as_late_401() {}

    #[test]
    #[ignore = "commit 4"]
    fn r1_a_send_inserted_after_the_check_is_seen_under_the_room_lock() {}

    #[test]
    #[ignore = "commit 6"]
    fn quit_releases_in_order_and_a_relaunch_meets_no_live_writer() {}

    #[test]
    #[ignore = "commit 2"]
    fn two_engines_boot_one_session() {}

    #[test]
    #[ignore = "commit 3"]
    fn d1_sign_out_changes_the_stored_sign_in_and_the_gate_in_one_step() {}

    #[test]
    #[ignore = "commit 3"]
    fn d2_a_slow_refresh_never_holds_session() {}

    #[test]
    #[ignore = "commit 4"]
    fn d4a_a_late_registration_of_a_writes_no_device_key_for_b() {}

    #[test]
    #[ignore = "commit 2"]
    fn d4c_a_second_copy_refused_before_any_room_opens() {}

    #[test]
    #[ignore = "commit 4"]
    fn d5_a_close_during_the_import_installs_nothing() {}

    #[test]
    #[ignore = "commit 2"]
    fn q4_a_stale_opener_installs_nothing() {}

    #[test]
    #[ignore = "commit 4"]
    fn i3_inbox_write_after_a_switch_is_refused() {}

    #[test]
    #[ignore = "commit 4"]
    fn i5_a_send_asks_no_token_for_another_owner() {}

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
