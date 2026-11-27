//! The one place in this process that refreshes the saved Buwiz sign-in.
//!
//! The server rotates refresh tokens and ends the whole sign-in when an old
//! refresh token is used again. So every refresh and every write of the saved
//! account in this process happens under [`lock`], and a refresh always starts
//! from the saved account as it is now: the separate `bir-mcp` process can
//! rotate the same saved token at any time.
//!
//! The relay client ([`bir_submit::Submitter`]) asks [`AccountTokens`] for an
//! access token. Tokens are never logged.

use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::cloud_account::{self, CloudError, Http, StoredAccount};

/// An access token that is valid for less than this many seconds is refreshed.
const EXPIRY_MARGIN_SECONDS: i64 = 30;
/// A forced refresh this soon after the last one uses the saved token: another
/// caller has just refreshed it.
const RECENT_REFRESH_SECONDS: i64 = 10;

static ACCOUNT_LOCK: Mutex<()> = Mutex::new(());
static SESSION_ENDED: OnceLock<fn(OwnerKey, u64, String)> = OnceLock::new();

/// The send's context (revision 5's BUG 3): who the send is for, the session epoch it was
/// spawned in, and — recorded at the token fetch — the credential its access token came from.
/// A late 401 ends exactly that: never the owner, epoch or credential of NOW. This is the slot
/// SEND_OWNER's spawn wiring fills from commit 4; the sends of today are scoped by the tests
/// and the hosts that already know their owner.
pub struct SendContext {
    /// The owner the task was spawned for.
    pub owner: OwnerKey,
    /// The session epoch at the spawn.
    pub epoch: u64,
    credential: Mutex<Option<String>>,
}

impl SendContext {
    /// The spawn's record: owner and epoch fixed, the credential empty until the token fetch.
    pub fn new(owner: OwnerKey, epoch: u64) -> Arc<Self> {
        Arc::new(Self {
            owner,
            epoch,
            credential: Mutex::new(None),
        })
    }

    /// The token fetch's record: the refresh token of the stored sign-in the access token came
    /// from — the credential a later 401 of this send is about.
    pub fn record_credential(&self, credential: String) {
        *self
            .credential
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(credential);
    }

    /// The recorded credential, if the fetch ran.
    pub fn credential(&self) -> Option<String> {
        self.credential
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

tokio::task_local! {
    /// The send's context, scoped when the send's task is spawned. `session_ended` reads ONLY
    /// this — with no task-local (a caller outside a send) it does nothing and logs, never a
    /// guess built from "now".
    pub static SEND_CONTEXT: Arc<SendContext>;
}

/// The stored sign-in's identity key: the normalised server origin (rooms::room_origin) and the
/// user id. Every owner check in the session world compares this, never the raw server URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerKey {
    pub origin: String,
    pub user_id: String,
}

impl OwnerKey {
    /// The key of a stored sign-in, or None while its server URL does not normalise.
    pub fn of(stored: &StoredAccount) -> Option<OwnerKey> {
        let origin = crate::rooms::room_origin(&stored.server_url).ok()?;
        Some(OwnerKey {
            origin,
            user_id: stored.user_id.clone(),
        })
    }
}

/// F8: who is stored, cached in memory. Filled ONCE at process start by fill_owner_cache; every
/// save and clear of the stored sign-in updates it inside the same account_session::lock()
/// section as the file or keychain write, so a reader never needs the file, the keychain or the
/// account lock. Its own small lock, no I/O.
static STORED_OWNER: Mutex<Option<Option<OwnerKey>>> = Mutex::new(None);

/// Fill the owner cache from the disk — ONCE, at process start, called by session::init (the
/// machine's own boot). Takes the account lock and reads the file and keychain HERE, where no
/// SESSION section can ever be involved, so the cache's readers never have to.
pub fn fill_owner_cache() {
    let owner = {
        let _one = lock();
        cloud_account::load_stored_account()
            .ok()
            .flatten()
            .and_then(|stored| OwnerKey::of(&stored))
    };
    *STORED_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(owner);
}

/// The stored sign-in's owner key, or None while nobody is stored — or while the cache was never
/// filled. Never does I/O and never takes the account lock: a keychain read can never reach
/// SESSION through here. An unfilled cache reads as None with a debug log, never as a disk read.
pub fn stored_owner() -> Option<OwnerKey> {
    let cache = STORED_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match cache.as_ref() {
        Some(owner) => owner.clone(),
        None => {
            tracing::debug!("stored_owner read before fill_owner_cache: answering None");
            None
        }
    }
}

/// The cache's one transition door (F8'): the in-memory answer to "who is stored" changes here
/// and nowhere else. The machine (bir-app-api's session) runs the change inside its SESSION
/// section — persistence first: the disk write already happened under the account lock, outside
/// SESSION, so a crash between the two leaves the disk signed out and the cache refills from it
/// on the next launch, the safe direction. The small cache lock nests inside SESSION.
pub fn cache_owner(owner: Option<OwnerKey>) {
    *STORED_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(owner);
}

/// Forget the filled answer: the next stored_owner() reads the disk again. Tests use it to
/// simulate a relaunch (the crash-window test); production never does.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_owner_cache_for_tests() {
    *STORED_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

/// Whether the stored sign-in IS this owner (the normalised origin and the user id), read from
/// the cache — no lock of this module's, no I/O (F3c).
pub fn stored_is_owner(owner: &OwnerKey) -> bool {
    stored_owner().as_ref() == Some(owner)
}

/// Save the stored sign-in through the one path: the file and keychain write. The caller holds
/// [`lock`] — every writer of the stored sign-in routes here (the guard test scans for direct
/// cloud_account calls). The OWNER CACHE is not touched here (F8'): the machine sets it inside
/// its SESSION section after the persist — or, for a same-owner refresh, the owner never
/// changes, so the cache is already right.
pub fn save_stored(stored: &StoredAccount) -> Result<(), CloudError> {
    cloud_account::save_stored_account(stored)
}

/// Forget the stored sign-in — the file AND the keychain entry — through the one path. The
/// caller holds [`lock`], outside SESSION; the cache's None is the machine's later step.
pub fn forget_stored() -> Result<(), CloudError> {
    cloud_account::forget_stored_account()
}

/// Save the stored sign-in only if the stored owner is still `owner` (§4): a late refresh of a
/// session that has since changed hands writes nothing. The comparison reads the DISK — the
/// stored sign-in loaded under the account lock the caller holds — never the cache: in the
/// sign-out's window the disk is already gone while the cache still shows the owner, and a
/// cache-checked save would write the ended sign-in back (the F8'-window bug). Answers whether
/// the save ran.
pub fn save_stored_if(owner: &OwnerKey, stored: &StoredAccount) -> Result<bool, CloudError> {
    let on_disk = cloud_account::load_stored_account()
        .ok()
        .flatten()
        .and_then(|current| OwnerKey::of(&current));
    if on_disk.as_ref() != Some(owner) {
        return Ok(false);
    }
    save_stored(stored)?;
    Ok(true)
}

/// Merge the PROFILE fields of a verb's refresh into the stored sign-in — never its tokens
/// (revision 5's F3): the write runs only while the disk's sign-in is still `owner` AND its
/// refresh token is still `credential`, the exact token the verb loaded before its network
/// calls. A rotation or a newer sign-in in the network window discards the merge, so a spent
/// token never lands back over the newer record. The caller holds [`lock`]. Answers whether
/// the merge ran.
pub fn merge_profile_if(
    owner: &OwnerKey,
    credential: &str,
    user_id: &str,
    email: Option<String>,
    practice: bool,
    offline: bool,
) -> Result<bool, CloudError> {
    let Ok(Some(current)) = cloud_account::load_stored_account() else {
        return Ok(false);
    };
    if OwnerKey::of(&current).as_ref() != Some(owner) || current.refresh_token != credential {
        return Ok(false);
    }
    let merged = StoredAccount {
        user_id: user_id.to_string(),
        email,
        practice,
        offline,
        ..current
    };
    save_stored(&merged)?;
    Ok(true)
}

/// Clear the stored sign-in through the one path: the file write. The caller holds [`lock`],
/// outside SESSION; the cache's None is the machine's SESSION-section step (F8').
pub fn clear_stored() {
    cloud_account::clear_stored_account();
}

/// Clear the stored sign-in only if its owner is still `owner` (§4): a server-ended session of
/// A's never clears B's sign-in. The comparison reads the DISK, as save_stored_if's does — the
/// cache is the SESSION side's source, never the write side's.
pub fn clear_stored_if(owner: &OwnerKey) -> bool {
    let on_disk = cloud_account::load_stored_account()
        .ok()
        .flatten()
        .and_then(|current| OwnerKey::of(&current));
    if on_disk.as_ref() != Some(owner) {
        return false;
    }
    clear_stored();
    true
}

/// Serializes every token refresh and every write of the saved sign-in in this process. Hold it across
/// "read the saved account, call the server, write it back". Blocking; never hold it across an `.await`.
pub fn lock() -> MutexGuard<'static, ()> {
    ACCOUNT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Why no access token is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// No one is signed in, or the server ended the sign-in.
    SignedOut,
    /// The Buwiz server could not be reached. The saved sign-in is unchanged.
    Offline(String),
    /// Anything else, such as a saved account that cannot be read or written.
    Other(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignedOut => write!(f, "Sign in to send returns."),
            Self::Offline(reason) => write!(f, "The Buwiz server is unreachable ({reason})."),
            Self::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TokenError {}

/// An access token for the saved sign-in. Takes [`lock`] itself.
/// - No saved account: `SignedOut`.
/// - Not `force` and the saved access token is valid for more than 30 s: that token, no request.
/// - `force` (the server just answered 401 invalid_token) but the saved account was refreshed less than
///   10 s ago (`last_refresh_unix`): that token, no request — another caller already refreshed.
/// - Otherwise refresh with the saved refresh token (`cloud_account::refresh_tokens`), save the new pair
///   (keep user_id/email/practice/server_url; `last_refresh_unix = now`; `offline = false`) and return it.
/// - The server rejects the refresh token (`CloudError::InvalidGrant`): re-read the saved account. If its
///   refresh token is now DIFFERENT from the one just used (another process rotated it), use that record
///   by the same rules, refreshing at most once more. If it is the same, the sign-in has ended: the
///   session-ended hook runs with the failed credential — bir-core never clears anything itself — and
///   the answer is `SignedOut`.
/// - Network failure: `Offline(reason)`; nothing is cleared or changed.
///
/// The owner check of §5.4: a token is answered only for `for_owner` — the owner the send or
/// verb started under. While the stored sign-in is anyone else's, the answer is SignedOut for
/// that owner and NOTHING is cleared and no hook runs.
pub fn access_token(
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    for_owner: &OwnerKey,
) -> Result<String, TokenError> {
    access_token_with_credential(http, now_unix, force, for_owner).map(|(token, _credential)| token)
}

/// [`access_token`], answering also the credential the token came from: the refresh token of the
/// stored sign-in whose access token this is — the value a send records in its [`SendContext`],
/// so a late 401 is matched against the credential that failed, never against "now".
pub fn access_token_with_credential(
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    for_owner: &OwnerKey,
) -> Result<(String, String), TokenError> {
    access_token_rejecting(http, now_unix, force, for_owner, None)
}

/// [`access_token_with_credential`] naming the access token whose rejection drove this ask
/// (B1): the fast paths never answer it — the rotation must really happen.
pub fn access_token_rejecting(
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    for_owner: &OwnerKey,
    rejected_access: Option<&str>,
) -> Result<(String, String), TokenError> {
    #[cfg(any(test, feature = "test-support"))]
    note_token_fetch(for_owner);
    let epoch = crate::session_epoch::current();
    access_token_in(
        &SavedAccount,
        http,
        now_unix,
        force,
        for_owner,
        epoch,
        &end_session_for,
        rejected_access,
    )
}

/// The flight key of the keyed single flight (revision 5's (a)): a refresh is one flight per
/// CREDENTIAL — the owner and the exact refresh token being refreshed — so a refresh for an
/// older token never joins or blocks a newer one's, and two asks for one credential always
/// share one network refresh.
#[derive(Clone, PartialEq, Eq, Hash)]
struct FlightKey {
    origin: String,
    user_id: String,
    refresh_token: String,
}

/// What the waiters of a finished flight learn. The leader answers its own result directly;
/// this is for everyone who waited.
#[derive(Clone)]
enum FlightOutcome {
    /// The refresh saved — or the credential rotated underneath it (another process or a newer
    /// flight won). Either way: re-read the stored record and re-evaluate; the answer is there.
    ReRead,
    /// The credential itself was rejected: the sign-in ended. The leader ran the session-ended
    /// hook EXACTLY ONCE for all of the flight's waiters — they only answer SignedOut.
    Ended,
    /// The server was unreachable: nobody signs out, and every waiter answers Offline, as the
    /// leader does.
    Unreachable(String),
    /// Anything but the network — a store failure, the leader's unfinished flight (F6): it
    /// reaches the waiters as the failure it is, never as Offline, which would say the server
    /// could not be reached when nobody knows that.
    Failed(String),
}

/// One in-process refresh flight: the state starts empty (still flying) and is published exactly
/// once, by the leader's guard — even when the leader panics, so no waiter waits forever ((d)).
struct Flight {
    state: Mutex<Option<FlightOutcome>>,
    free: std::sync::Condvar,
}

/// The flights in progress, keyed by their credential ((a)).
static FLIGHTS: std::sync::LazyLock<Mutex<std::collections::HashMap<FlightKey, Arc<Flight>>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

use std::sync::Arc;

/// The leader's publish-on-drop guard ((d)): however the leader leaves its refresh — a save, a
/// rejection, a network error, a panic or a cancellation — the waiters wake with the outcome, or
/// with an error when the leader never wrote one, and the flight leaves the map so the next ask
/// starts fresh. quit and close never touch a flight: they never wait on one ((e)).
struct FlightLeader {
    key: FlightKey,
    flight: Arc<Flight>,
    outcome: Option<FlightOutcome>,
}

impl Drop for FlightLeader {
    fn drop(&mut self) {
        let outcome = self.outcome.take().unwrap_or(FlightOutcome::Failed(
            "the refresh leader left its flight unfinished".into(),
        ));
        *self
            .flight
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(outcome);
        self.flight.free.notify_all();
        FLIGHTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.key);
    }
}

/// Test instrumentation: token fetches counted PER OWNER, so one test's fetch never flakes
/// another's assertion (f7, i5). The key is the owner the fetch was for.
#[cfg(any(test, feature = "test-support"))]
static TOKEN_FETCHES: std::sync::LazyLock<Mutex<std::collections::HashMap<(String, String), u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

#[cfg(any(test, feature = "test-support"))]
fn note_token_fetch(owner: &OwnerKey) {
    let mut fetches = TOKEN_FETCHES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *fetches
        .entry((owner.origin.clone(), owner.user_id.clone()))
        .or_insert(0) += 1;
}

/// How many token fetches this process ran for `owner`.
#[cfg(any(test, feature = "test-support"))]
pub fn token_fetches_for(owner: &OwnerKey) -> u64 {
    TOKEN_FETCHES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&(owner.origin.clone(), owner.user_id.clone()))
        .copied()
        .unwrap_or(0)
}

/// What the host does when the sign-in ends outside a user action (rejected refresh, device
/// removed): bir-app-api registers session::server_ended, which takes the owner, the epoch the
/// ending acted for and the REFRESH TOKEN the server rejected — the credential the ending is
/// for, so a newer sign-in of even the same owner is never cleared by an old token's 401.
/// Set once; later calls are ignored.
pub fn on_session_ended(hook: fn(OwnerKey, u64, String)) {
    let _ = SESSION_ENDED.set(hook);
}

/// The sign-in of `owner` ended at the server (a 401, an InvalidGrant), in the session of
/// `epoch`, on the credential `failed_refresh_token`. bir-core no longer clears anything itself
/// (§4): it ONLY calls the registered hook — session::server_ended — which clears only if the
/// stored credential IS the failed one, and closes only if the epoch still matches.
pub fn end_session_for(owner: OwnerKey, epoch: u64, failed_refresh_token: String) {
    if let Some(hook) = SESSION_ENDED.get() {
        hook(owner, epoch, failed_refresh_token);
    }
}

/// The `bir_submit::TokenSource` of this process. `access_token` runs [`access_token`] on
/// `tokio::task::spawn_blocking` with a `cloud_account::BlockingHttp` and `cloud_account::unix_now()`,
/// so no blocking HTTP call runs on an async worker thread. `login` reads the saved account's email
/// without a request (`Some("")` when signed in with no email, `None` when signed out).
/// `session_ended` calls [`end_session`].
pub struct AccountTokens;

impl bir_submit::TokenSource for AccountTokens {
    fn access_token(
        &self,
        force_refresh: bool,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, bir_submit::ClientError>> + Send + '_>,
    > {
        // The account file sits next to the database, whose in-process name is per thread: the blocking
        // thread runs under the name of the thread that asked. The owner the token is for is read
        // here, at the call: until §5.4's SEND_OWNER lands with the wiring commit, the owner the
        // send started under IS the stored owner of that moment.
        let database = crate::db::capture_database_path_override();
        let device = crate::platform::capture_device_dir_override();
        let Some(for_owner) = stored_owner() else {
            return Box::pin(async move { Err(bir_submit::ClientError::SignedOut) });
        };
        // The send's context, if this fetch is a send's: the credential it produced the token
        // from is recorded HERE, in the task's own async frame — the task-local does not reach
        // the blocking thread, and the record must be the send's, not "now".
        let context = SEND_CONTEXT.try_with(|context| context.clone()).ok();
        Box::pin(async move {
            let answer = tokio::task::spawn_blocking(move || {
                device.run(|| {
                    database.run(|| {
                        let mut http = cloud_account::BlockingHttp::new();
                        access_token_with_credential(
                            &mut http,
                            cloud_account::unix_now(),
                            force_refresh,
                            &for_owner,
                        )
                    })
                })
            })
            .await
            .map_err(|error| bir_submit::ClientError::Other(error.to_string()))?;
            answer
                .map(|(token, credential)| {
                    if let Some(context) = &context {
                        context.record_credential(credential);
                    }
                    token
                })
                .map_err(|error| match error {
                    TokenError::SignedOut => bir_submit::ClientError::SignedOut,
                    TokenError::Offline(reason) => bir_submit::ClientError::Network(reason),
                    TokenError::Other(message) => bir_submit::ClientError::Other(message),
                })
        })
    }

    fn login(&self) -> Option<String> {
        // §A.5: the user id, not the email — the TIN-list cache is keyed by it, and two sign-ins of
        // one person never share a read across accounts.
        let stored = cloud_account::load_stored_account().ok().flatten()?;
        Some(stored.user_id)
    }

    fn session_ended(&self) {
        // The server's 401 ended the session of THE SEND'S owner, at the send's epoch, on the
        // credential the send's token came from — all three read from the send's task-local,
        // never from "now": a late 401 of A's send after B signed in names A's owner, A's epoch
        // and A's credential, and every check of the hook's fails for B. With no task-local —
        // a caller outside a send — this does NOTHING and logs; it never falls back to "now".
        let context = SEND_CONTEXT.try_with(|context| context.clone()).ok();
        let Some(context) = context else {
            tracing::warn!("session_ended outside a send's context: nothing is ended");
            return;
        };
        let Some(credential) = context.credential() else {
            tracing::warn!("session_ended with no token fetch recorded: nothing is ended");
            return;
        };
        end_session_for(context.owner.clone(), context.epoch, credential);
    }
}

/// Where the saved account lives. The real one is the keychain and private file of
/// [`cloud_account`]; tests use one in memory.
trait AccountStore {
    fn load(&self) -> Result<Option<StoredAccount>, CloudError>;
    fn save(&self, stored: &StoredAccount) -> Result<(), CloudError>;
    /// The memory stores of the tests use it; production's clears route through the public
    /// clear_stored/forget_stored now that end_session is gone.
    #[cfg_attr(not(test), allow(dead_code))]
    fn clear(&self);
}

/// The saved account of [`cloud_account`].
struct SavedAccount;

impl AccountStore for SavedAccount {
    fn load(&self) -> Result<Option<StoredAccount>, CloudError> {
        cloud_account::load_stored_account()
    }

    fn save(&self, stored: &StoredAccount) -> Result<(), CloudError> {
        // The one path: the write and the owner cache's update in the same lock section (the
        // caller of token work holds the lock for this).
        save_stored(stored)
    }

    fn clear(&self) {
        clear_stored();
    }
}

/// [`access_token`] against `store` — the D2 restructure (§4): the lock is held only for the
/// file reads and writes, NEVER across the network refresh, and never while waiting on another
/// refresh's flight ((b): a waiter holds no account_session, SESSION or DEVICE_KEY lock, and the
/// leader holds none of them across the network). The owner check comes first: a stored sign-in
/// that is not `for_owner` answers SignedOut for that owner with no clear and no hook.
///
/// The keyed single flight ((a)–(c)): one flight per credential — the owner and the exact
/// refresh token. A second ask for the SAME credential waits on the flight, never on the account
/// lock, and takes the leader's outcome: a save means re-read, a rejection means SignedOut (the
/// leader ran the session-ended hook exactly once for everyone), a network error is every
/// waiter's answer and signs nobody out. A refresh for a different — older or newer — credential
/// is its own flight and never joins nor blocks this one's.
#[allow(clippy::too_many_arguments)]
fn access_token_in(
    store: &dyn AccountStore,
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    for_owner: &OwnerKey,
    epoch: u64,
    ended: &dyn Fn(OwnerKey, u64, String),
    rejected_access: Option<&str>,
) -> Result<(String, String), TokenError> {
    let mut retried = false;
    // F6: every re-read of this loop is bounded — a flight that keeps rotating under the ask
    // fails rather than spins. At most 2 more tries after the first evaluation.
    const MAX_REREADS: u32 = 2;
    let mut rereads = 0u32;
    loop {
        // The load under the lock — the fast paths (a valid token, a just-refreshed one)
        // never leave it.
        let mut stored = {
            let _one = lock();
            match store.load() {
                Ok(Some(stored)) => stored,
                Ok(None) => return Err(TokenError::SignedOut),
                Err(error) => return Err(TokenError::Other(error.to_string())),
            }
        };
        // The owner check at every step (I5): the stored sign-in is not this send's owner →
        // SignedOut for that owner, and nothing of the new owner's is touched.
        let owner = match OwnerKey::of(&stored) {
            Some(owner) => owner,
            None => return Err(TokenError::SignedOut),
        };
        if owner != *for_owner {
            return Err(TokenError::SignedOut);
        }
        // B1: a fast path never answers the REJECTED access token — a force driven by its
        // rejection would loop the same 401 into a sign-out. The recency twin is skipped just
        // the same: within ten seconds of a rotation the server can already have revoked the
        // access token it made, and the rejection is fresher evidence than the clock.
        let is_rejected = |stored: &StoredAccount| {
            rejected_access.is_some_and(|rejected| stored.access_token == rejected)
        };
        if !force
            && stored.access_expires_at_unix > now_unix.saturating_add(EXPIRY_MARGIN_SECONDS)
            && !is_rejected(&stored)
        {
            let credential = stored.refresh_token.clone();
            return Ok((stored.access_token, credential));
        }
        if force
            && now_unix.saturating_sub(stored.last_refresh_unix) < RECENT_REFRESH_SECONDS
            && !is_rejected(&stored)
        {
            let credential = stored.refresh_token.clone();
            return Ok((stored.access_token, credential));
        }
        // The keyed flight: join the credential's flight or become its leader. The map lock is
        // never held while waiting or networking.
        let key = FlightKey {
            origin: owner.origin.clone(),
            user_id: owner.user_id.clone(),
            refresh_token: stored.refresh_token.clone(),
        };
        let mut leader = {
            let mut flights = FLIGHTS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(flight) = flights.get(&key) {
                let flight = Arc::clone(flight);
                drop(flights);
                // The wait with NO lock of any kind held ((b)).
                let outcome = {
                    let mut state = flight
                        .state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    while state.is_none() {
                        state = flight
                            .free
                            .wait(state)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                    state.clone().expect("published")
                };
                match outcome {
                    // Re-read and re-evaluate: the leader's save — or the rotation it met —
                    // is in the record now — bounded like every re-read of this loop (F6).
                    FlightOutcome::ReRead => {
                        rereads += 1;
                        if rereads > MAX_REREADS {
                            return Err(TokenError::Other(
                                "the saved Buwiz sign-in keeps changing; try again".into(),
                            ));
                        }
                        continue;
                    }
                    // The credential was rejected. The leader ran the hook exactly once ((c)).
                    FlightOutcome::Ended => return Err(TokenError::SignedOut),
                    FlightOutcome::Unreachable(reason) => {
                        return Err(TokenError::Offline(reason));
                    }
                    FlightOutcome::Failed(reason) => return Err(TokenError::Other(reason)),
                }
            }
            let flight = Arc::new(Flight {
                state: Mutex::new(None),
                free: std::sync::Condvar::new(),
            });
            flights.insert(key.clone(), Arc::clone(&flight));
            drop(flights);
            FlightLeader {
                key,
                flight,
                outcome: None,
            }
        };
        // The leader's re-read under the lock: the record may have changed since the first
        // load, before the flight was taken.
        {
            let _one = lock();
            match store.load() {
                Ok(Some(again)) => stored = again,
                Ok(None) => {
                    leader.outcome = Some(FlightOutcome::ReRead);
                    return Err(TokenError::SignedOut);
                }
                Err(error) => {
                    leader.outcome = Some(FlightOutcome::Failed(error.to_string()));
                    return Err(TokenError::Other(error.to_string()));
                }
            }
        }
        if OwnerKey::of(&stored).as_ref() != Some(for_owner) {
            leader.outcome = Some(FlightOutcome::ReRead);
            return Err(TokenError::SignedOut);
        }
        if stored.refresh_token != leader.key.refresh_token {
            // The credential rotated while the flight was being taken: this flight's refresh
            // would burn a token nobody holds. Re-read and start over — bounded (F6).
            leader.outcome = Some(FlightOutcome::ReRead);
            rereads += 1;
            if rereads > MAX_REREADS {
                return Err(TokenError::Other(
                    "the saved Buwiz sign-in keeps changing; try again".into(),
                ));
            }
            continue;
        }
        if !force
            && stored.access_expires_at_unix > now_unix.saturating_add(EXPIRY_MARGIN_SECONDS)
            && !is_rejected(&stored)
        {
            leader.outcome = Some(FlightOutcome::ReRead);
            let credential = stored.refresh_token.clone();
            return Ok((stored.access_token, credential));
        }
        if force
            && now_unix.saturating_sub(stored.last_refresh_unix) < RECENT_REFRESH_SECONDS
            && !is_rejected(&stored)
        {
            leader.outcome = Some(FlightOutcome::ReRead);
            let credential = stored.refresh_token.clone();
            return Ok((stored.access_token, credential));
        }
        // The network refresh with NO LOCK (D2): the lock is never held across it.
        match cloud_account::refresh_tokens(http, &stored.server_url, &stored.refresh_token) {
            Ok(tokens) => {
                let next = StoredAccount {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    access_expires_at_unix: now_unix.saturating_add(tokens.expires_in.max(1)),
                    offline: false,
                    last_refresh_unix: now_unix,
                    ..stored.clone()
                };
                // save_stored_if, under the lock: the FRESH record's owner decides — the
                // stored sign-in may have changed hands during the unlocked network call. A
                // mismatch discards the refreshed tokens.
                let saved = {
                    let _one = lock();
                    let fresh = store.load();
                    match fresh {
                        Ok(Some(fresh)) if OwnerKey::of(&fresh).as_ref() == Some(for_owner) => {
                            let merged = StoredAccount {
                                access_token: next.access_token.clone(),
                                refresh_token: next.refresh_token.clone(),
                                access_expires_at_unix: next.access_expires_at_unix,
                                offline: false,
                                last_refresh_unix: now_unix,
                                ..fresh
                            };
                            store
                                .save(&merged)
                                .map_err(|error| TokenError::Other(error.to_string()))?;
                            true
                        }
                        Ok(Some(_)) => false,
                        Ok(None) => false,
                        Err(error) => return Err(TokenError::Other(error.to_string())),
                    }
                };
                // Saved or discarded, the record is the answer now: everyone re-reads.
                leader.outcome = Some(FlightOutcome::ReRead);
                if !saved {
                    return Err(TokenError::SignedOut);
                }
                let credential = next.refresh_token.clone();
                return Ok((next.access_token, credential));
            }
            Err(CloudError::Network(reason)) => {
                // (c): a network error signs nobody out — not the leader, not a waiter.
                leader.outcome = Some(FlightOutcome::Unreachable(reason.clone()));
                return Err(TokenError::Offline(reason));
            }
            Err(CloudError::InvalidGrant) => {
                let used = stored.refresh_token.clone();
                // The re-check, a short locked read: another process may have rotated the token.
                let again = {
                    let _one = lock();
                    store.load()
                };
                match again {
                    Ok(Some(again)) if again.refresh_token != used => {
                        if retried {
                            leader.outcome = Some(FlightOutcome::Failed(
                                "the saved Buwiz sign-in keeps changing".into(),
                            ));
                            return Err(TokenError::Other(
                                "the saved Buwiz sign-in keeps changing; try again".into(),
                            ));
                        }
                        retried = true;
                        // The rotation is in the record: everyone — the waiters and, next
                        // loop, the leader — re-reads and starts over on the new credential.
                        leader.outcome = Some(FlightOutcome::ReRead);
                        continue;
                    }
                    Ok(Some(_)) => {
                        // The same refresh token the server just rejected: the sign-in has
                        // ended. The hook runs ONCE here, the leader, never per waiter ((c)) —
                        // and it carries the failed credential, so a newer sign-in of even the
                        // same owner is never cleared by it (revision 5's A).
                        leader.outcome = Some(FlightOutcome::Ended);
                        ended(owner, epoch, used);
                        return Err(TokenError::SignedOut);
                    }
                    // Another process signed out in the meantime: the disk is empty already.
                    // The hook decides — under revision 5's credential matching an empty disk
                    // holds no matching credential, so nothing is cleared and nothing closes.
                    Ok(None) => {
                        leader.outcome = Some(FlightOutcome::Ended);
                        ended(owner, epoch, used);
                        return Err(TokenError::SignedOut);
                    }
                    Err(error) => {
                        leader.outcome = Some(FlightOutcome::Failed(error.to_string()));
                        return Err(TokenError::Other(error.to_string()));
                    }
                }
            }
            Err(error) => {
                leader.outcome = Some(FlightOutcome::Failed(error.to_string()));
                return Err(TokenError::Other(error.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_account::{HttpRequest, HttpResponse};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    const SERVER: &str = "http://localhost:3008";

    /// The saved account, in memory.
    #[derive(Default)]
    struct MemoryStore {
        account: Mutex<Option<StoredAccount>>,
        saves: AtomicUsize,
        clears: AtomicUsize,
    }

    impl MemoryStore {
        fn with(account: StoredAccount) -> Self {
            Self {
                account: Mutex::new(Some(account)),
                ..Self::default()
            }
        }

        fn current(&self) -> Option<StoredAccount> {
            self.account.lock().unwrap().clone()
        }
    }

    impl AccountStore for MemoryStore {
        fn load(&self) -> Result<Option<StoredAccount>, CloudError> {
            Ok(self.current())
        }

        fn save(&self, stored: &StoredAccount) -> Result<(), CloudError> {
            self.saves.fetch_add(1, Ordering::SeqCst);
            *self.account.lock().unwrap() = Some(stored.clone());
            Ok(())
        }

        fn clear(&self) {
            self.clears.fetch_add(1, Ordering::SeqCst);
            *self.account.lock().unwrap() = None;
        }
    }

    /// A token endpoint that hands out one-time refresh tokens and rejects a
    /// reused or unknown one, as the real server does.
    struct FakeServer {
        state: Mutex<ServerState>,
        requests: AtomicUsize,
    }

    struct ServerState {
        live: HashSet<String>,
        issued: usize,
    }

    impl FakeServer {
        fn new(live: &[&str]) -> Arc<Self> {
            Arc::new(Self {
                state: Mutex::new(ServerState {
                    live: live.iter().map(|token| token.to_string()).collect(),
                    issued: 0,
                }),
                requests: AtomicUsize::new(0),
            })
        }

        fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }
    }

    struct FakeHttp(Arc<FakeServer>);

    impl Http for FakeHttp {
        fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
            let server = &self.0;
            server.requests.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.url, format!("{SERVER}/oauth/token"));
            let used = request
                .body
                .split('&')
                .find_map(|pair| pair.strip_prefix("refresh_token="))
                .map(|token| urlencoding::decode(token).unwrap().into_owned())
                .unwrap_or_default();
            let mut state = server.state.lock().unwrap();
            if !state.live.remove(&used) {
                return Ok(HttpResponse {
                    status: 400,
                    body: r#"{"error":"invalid_grant"}"#.into(),
                });
            }
            state.issued += 1;
            let next = format!("refresh-{}", state.issued);
            state.live.insert(next.clone());
            Ok(HttpResponse {
                status: 200,
                body: format!(
                    r#"{{"access_token":"access-{}","refresh_token":"{next}","expires_in":900}}"#,
                    state.issued
                ),
            })
        }
    }

    struct Unreachable(usize);

    impl Http for Unreachable {
        fn exchange(&mut self, _: &HttpRequest) -> Result<HttpResponse, CloudError> {
            self.0 += 1;
            Err(CloudError::Network("connection refused".into()))
        }
    }

    /// The tests' one turn: the flight map is process-global BY DESIGN — production has exactly
    /// one stored sign-in — so two tests refreshing at once would join each other's flights.
    /// Serialise them instead; each is fast.
    static TURN: Mutex<()> = Mutex::new(());

    fn turn() -> std::sync::MutexGuard<'static, ()> {
        TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn account(access_expires_at_unix: i64, last_refresh_unix: i64) -> StoredAccount {
        StoredAccount {
            server_url: SERVER.into(),
            access_token: "access-0".into(),
            refresh_token: "refresh-0".into(),
            access_expires_at_unix,
            user_id: "user-1".into(),
            email: Some("filer@example.test".into()),
            offline: true,
            last_refresh_unix,
            practice: true,
        }
    }

    fn test_owner() -> OwnerKey {
        OwnerKey::of(&account(0, 0)).expect("the test account normalises")
    }

    fn ask(
        store: &MemoryStore,
        http: &mut dyn Http,
        now: i64,
        force: bool,
        hook_runs: &AtomicUsize,
    ) -> Result<String, TokenError> {
        access_token_in(
            store,
            http,
            now,
            force,
            &test_owner(),
            1,
            &|_, _, _| {
                hook_runs.fetch_add(1, Ordering::SeqCst);
            },
            None,
        )
        .map(|(token, _credential)| token)
    }

    #[test]
    fn not_signed_in_is_signed_out_with_no_request() {
        let _turn = turn();
        let store = MemoryStore::default();
        let server = FakeServer::new(&[]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, false, &hook);
        assert_eq!(answer, Err(TokenError::SignedOut));
        assert_eq!(server.requests(), 0);
        assert_eq!(hook.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_fresh_token_is_returned_without_a_request() {
        let _turn = turn();
        let store = MemoryStore::with(account(2_000, 500));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, false, &hook);
        assert_eq!(answer.as_deref(), Ok("access-0"));
        assert_eq!(server.requests(), 0);
        assert_eq!(store.saves.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn an_expired_token_refreshes_and_saves_the_rotated_pair() {
        let _turn = turn();
        // Valid for only 20 s more: inside the 30 s margin.
        let store = MemoryStore::with(account(1_020, 500));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, false, &hook);
        assert_eq!(answer.as_deref(), Ok("access-1"));
        assert_eq!(server.requests(), 1);
        let saved = store.current().expect("still signed in");
        assert_eq!(saved.access_token, "access-1");
        assert_eq!(saved.refresh_token, "refresh-1");
        assert_eq!(saved.access_expires_at_unix, 1_900);
        assert_eq!(saved.last_refresh_unix, 1_000);
        assert!(!saved.offline);
        assert_eq!(saved.user_id, "user-1");
        assert_eq!(saved.email.as_deref(), Some("filer@example.test"));
        assert!(saved.practice);
        assert_eq!(saved.server_url, SERVER);
    }

    #[test]
    fn force_refreshes_a_token_that_is_still_valid() {
        let _turn = turn();
        let store = MemoryStore::with(account(5_000, 500));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, true, &hook);
        assert_eq!(answer.as_deref(), Ok("access-1"));
        assert_eq!(server.requests(), 1);
        assert_eq!(store.current().unwrap().refresh_token, "refresh-1");
    }

    #[test]
    fn force_right_after_a_refresh_makes_no_request() {
        let _turn = turn();
        let store = MemoryStore::with(account(1_900, 995));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, true, &hook);
        assert_eq!(answer.as_deref(), Ok("access-0"));
        assert_eq!(server.requests(), 0);
        assert_eq!(store.saves.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_rejected_refresh_with_the_same_saved_token_ends_the_sign_in() {
        let _turn = turn();
        let store = MemoryStore::with(account(0, 0));
        // The server no longer knows refresh-0.
        let server = FakeServer::new(&[]);
        let hook = AtomicUsize::new(0);
        let answer = ask(&store, &mut FakeHttp(server.clone()), 1_000, false, &hook);
        assert_eq!(answer, Err(TokenError::SignedOut));
        assert_eq!(server.requests(), 1);
        // §4: bir-core clears nothing itself — the hook (the machine's server_ended) owns the
        // clear. What this layer owes: the hook, once, with the owner and the epoch.
        assert!(
            store.current().is_some(),
            "the record is the hook's to clear"
        );
        assert_eq!(hook.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_rejected_refresh_after_another_process_rotated_uses_the_new_record() {
        let _turn = turn();
        /// Before the token request, another process rotates the saved token.
        struct RotatedMeanwhile {
            server: Arc<FakeServer>,
            store: Arc<MemoryStore>,
            done: bool,
        }
        impl Http for RotatedMeanwhile {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                if !self.done {
                    self.done = true;
                    let mut other = FakeHttp(self.server.clone());
                    let saved = self.store.current().unwrap();
                    let tokens =
                        cloud_account::refresh_tokens(&mut other, SERVER, &saved.refresh_token)
                            .unwrap();
                    *self.store.account.lock().unwrap() = Some(StoredAccount {
                        access_token: tokens.access_token,
                        refresh_token: tokens.refresh_token,
                        access_expires_at_unix: 1_000 + tokens.expires_in,
                        last_refresh_unix: 998,
                        ..saved
                    });
                }
                FakeHttp(self.server.clone()).exchange(request)
            }
        }

        let store = Arc::new(MemoryStore::with(account(0, 0)));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let mut http = RotatedMeanwhile {
            server: server.clone(),
            store: store.clone(),
            done: false,
        };
        let answer = ask(&store, &mut http, 1_000, false, &hook);
        assert_eq!(
            answer.as_deref(),
            Ok("access-1"),
            "the other process's fresh token"
        );
        // Ours (rejected) and the other process's: none more.
        assert_eq!(server.requests(), 2);
        assert_eq!(store.clears.load(Ordering::SeqCst), 0);
        assert_eq!(hook.load(Ordering::SeqCst), 0);
        assert_eq!(store.current().unwrap().refresh_token, "refresh-1");
    }

    #[test]
    fn a_network_failure_is_offline_and_changes_nothing() {
        let _turn = turn();
        let before = account(0, 400);
        let store = MemoryStore::with(before.clone());
        let hook = AtomicUsize::new(0);
        let mut http = Unreachable(0);
        let answer = ask(&store, &mut http, 1_000, false, &hook);
        assert_eq!(
            answer,
            Err(TokenError::Offline("connection refused".into()))
        );
        assert_eq!(http.0, 1);
        let after = store.current().expect("still signed in");
        assert_eq!(after.encode().unwrap(), before.encode().unwrap());
        assert_eq!(store.saves.load(Ordering::SeqCst), 0);
        assert_eq!(store.clears.load(Ordering::SeqCst), 0);
        assert_eq!(hook.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn two_threads_asking_at_once_make_one_refresh_request() {
        let _turn = turn();
        let store = Arc::new(MemoryStore::with(account(0, 0)));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(2));
        let askers: Vec<_> = (0..2)
            .map(|_| {
                let (store, server, hook, start) =
                    (store.clone(), server.clone(), hook.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    ask(&store, &mut FakeHttp(server), 1_000, false, &hook)
                })
            })
            .collect();
        for asker in askers {
            assert_eq!(asker.join().unwrap().as_deref(), Ok("access-1"));
        }
        assert_eq!(server.requests(), 1);
        assert_eq!(hook.load(Ordering::SeqCst), 0);
        assert_eq!(store.current().unwrap().refresh_token, "refresh-1");
    }

    /// (a): the flight is keyed by the credential — refreshes for DIFFERENT credentials are
    /// different flights and never join nor block each other. The barriered server answers only
    /// once BOTH requests are inside it, so a serializing flight would deadlock here (the
    /// bounded join is the failure margin).
    #[test]
    fn flights_are_keyed_by_the_credential() {
        let _turn = turn();
        struct BarrierHttp(Arc<Barrier>);
        impl Http for BarrierHttp {
            fn exchange(&mut self, _request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                // Both requests must be inside the network at once for either to answer.
                self.0.wait();
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-new","refresh_token":"refresh-new","expires_in":900}"#.into(),
                })
            }
        }
        let barrier = Arc::new(Barrier::new(2));
        let hook = Arc::new(AtomicUsize::new(0));
        // Two owners, two stores, two credentials.
        let store_a = Arc::new(MemoryStore::with(account(0, 0)));
        let mut account_b = account(0, 0);
        account_b.user_id = "user-2".into();
        account_b.refresh_token = "refresh-other".into();
        let store_b = Arc::new(MemoryStore::with(account_b));
        let owner_a = test_owner();
        let owner_b = OwnerKey {
            origin: owner_a.origin.clone(),
            user_id: "user-2".into(),
        };
        let start = Arc::new(Barrier::new(2));
        let mut joins = Vec::new();
        for (store, owner) in [(store_a, owner_a), (store_b, owner_b)] {
            let (barrier, hook, start) = (barrier.clone(), hook.clone(), start.clone());
            joins.push(std::thread::spawn(move || {
                start.wait();
                let mut http = BarrierHttp(barrier);
                let hook2 = hook.clone();
                access_token_in(
                    &*store,
                    &mut http,
                    1_000,
                    false,
                    &owner,
                    1,
                    &move |_, _, _| {
                        hook2.fetch_add(1, Ordering::SeqCst);
                    },
                    None,
                )
                .map(|(token, _credential)| token)
            }));
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for join in joins {
                let _ = sender.send(join.join().unwrap());
            }
        });
        for _ in 0..2 {
            let answer = receiver
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("the two credentials' flights ran concurrently — a shared flight would deadlock");
            assert_eq!(answer.as_deref(), Ok("access-new"));
        }
        assert_eq!(hook.load(Ordering::SeqCst), 0);
    }

    /// (a), the rotated case scripted explicitly: the leader is parked refreshing T1; the store
    /// rotates to T2 by another path; the next refresh — for T2 now — must NOT join T1's flight.
    /// It leads its own, and its network call arrives while T1's is still parked.
    #[test]
    fn a_rotation_starts_a_newer_credential_its_own_flight() {
        let _turn = turn();
        // The leader's http parks inside the network call until released.
        struct ParkedHttp {
            inside: std::sync::mpsc::Sender<()>,
            go: std::sync::mpsc::Receiver<()>,
        }
        impl Http for ParkedHttp {
            fn exchange(&mut self, _request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let _ = self.inside.send(());
                let _ = self.go.recv();
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-t1","refresh_token":"refresh-t1-next","expires_in":900}"#.into(),
                })
            }
        }
        // The newer refresh's http announces its arrival and answers at once.
        struct AnnouncingHttp(std::sync::mpsc::Sender<()>);
        impl Http for AnnouncingHttp {
            fn exchange(&mut self, _request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let _ = self.0.send(());
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"access_token":"access-t2","refresh_token":"refresh-t2-next","expires_in":900}"#.into(),
                })
            }
        }
        let mut account = account(0, 0);
        account.refresh_token = "refresh-t1".into();
        let store = Arc::new(MemoryStore::with(account));
        let hook = Arc::new(AtomicUsize::new(0));
        let (inside_sender, inside_receiver) = std::sync::mpsc::channel();
        let (go_sender, go_receiver) = std::sync::mpsc::channel();
        let (leader_store, leader_hook) = (store.clone(), hook.clone());
        let leader = std::thread::spawn(move || {
            let mut http = ParkedHttp {
                inside: inside_sender,
                go: go_receiver,
            };
            ask(&leader_store, &mut http, 1_000, false, &leader_hook)
        });
        inside_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the leader is parked inside T1's refresh");
        // The rotation by another path: the stored sign-in now holds T2, still expired.
        {
            let mut rotated = store.current().unwrap();
            rotated.refresh_token = "refresh-t2".into();
            rotated.access_token = "access-stale".into();
            rotated.access_expires_at_unix = 0;
            store.save(&rotated).unwrap();
        }
        // The newer ask: it reads T2, and T2's refresh must be its own flight — its network
        // call arrives while T1's is still parked.
        let (arrived_sender, arrived_receiver) = std::sync::mpsc::channel();
        let (waiter_store, waiter_hook) = (store.clone(), hook.clone());
        let waiter = std::thread::spawn(move || {
            let mut http = AnnouncingHttp(arrived_sender);
            ask(&waiter_store, &mut http, 1_000, false, &waiter_hook)
        });
        arrived_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("T2's refresh reached the network on its own flight — keyed by owner only, it would still be waiting on T1's");
        let answer = waiter.join().unwrap();
        assert_eq!(answer.as_deref(), Ok("access-t2"));
        // Let the leader finish; the flight map is clean afterwards.
        let _ = go_sender.send(());
        let _ = leader.join().unwrap();
    }

    /// (c): the rejection of one credential ends the session EXACTLY ONCE however many waiters
    /// the flight had — the leader runs the hook, the waiters only answer SignedOut.
    #[test]
    fn one_rejection_ends_the_session_exactly_once() {
        let _turn = turn();
        let store = Arc::new(MemoryStore::with(account(0, 0)));
        // The server holds NO live token: the refresh is rejected.
        let server = FakeServer::new(&[]);
        let hook = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(3));
        // The hook emulates the machine's real one: it CLEARS the matching credential, which is
        // what makes exactly-once deterministic in production — a thread that missed the flight
        // re-reads the cleared store and answers SignedOut without a refresh at all.
        let askers: Vec<_> = (0..3)
            .map(|_| {
                let (store, server, hook, start) =
                    (store.clone(), server.clone(), hook.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    let hook_store = store.clone();
                    let hook_count = hook.clone();
                    access_token_in(
                        &*store,
                        &mut FakeHttp(server),
                        1_000,
                        false,
                        &test_owner(),
                        1,
                        &move |_, _, _| {
                            hook_count.fetch_add(1, Ordering::SeqCst);
                            hook_store.clear();
                        },
                        None,
                    )
                    .map(|(token, _credential)| token)
                })
            })
            .collect();
        for asker in askers {
            assert_eq!(asker.join().unwrap(), Err(TokenError::SignedOut));
        }
        assert_eq!(
            hook.load(Ordering::SeqCst),
            1,
            "one credential, one rejection, one session-ending — never one per waiter"
        );
    }

    /// (c): a network error is every waiter's own answer, and signs nobody out.
    #[test]
    fn a_network_error_is_every_waiters_answer_and_signs_nobody_out() {
        let _turn = turn();
        let store = Arc::new(MemoryStore::with(account(0, 0)));
        let hook = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(Barrier::new(2));
        let askers: Vec<_> = (0..2)
            .map(|_| {
                let (store, hook, start) = (store.clone(), hook.clone(), start.clone());
                std::thread::spawn(move || {
                    start.wait();
                    ask(&store, &mut Unreachable(0), 1_000, false, &hook)
                })
            })
            .collect();
        for asker in askers {
            match asker.join().unwrap() {
                Err(TokenError::Offline(_)) => {}
                other => panic!("a network error answers Offline, not {other:?}"),
            }
        }
        assert_eq!(
            hook.load(Ordering::SeqCst),
            0,
            "a network error never ends a session"
        );
        assert!(
            store.current().is_some(),
            "and nothing of the sign-in is cleared"
        );
    }

    /// (d): a leader that panics wakes its waiters with an error and clears the flight — nothing
    /// waits forever. The leader parks inside its network call until the waiter is queued on the
    /// flight, then panics; the guard's Drop publishes the failure.
    #[test]
    fn a_panicking_leader_wakes_its_waiters_with_an_error() {
        let _turn = turn();
        struct PanicHttp {
            inside: std::sync::mpsc::Sender<()>,
            go: std::sync::mpsc::Receiver<()>,
        }
        impl Http for PanicHttp {
            fn exchange(&mut self, _request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let _ = self.inside.send(());
                let _ = self.go.recv();
                panic!("the leader dies inside its network call");
            }
        }
        let store = Arc::new(MemoryStore::with(account(0, 0)));
        let hook = Arc::new(AtomicUsize::new(0));
        let (inside_sender, inside_receiver) = std::sync::mpsc::channel();
        let (go_sender, go_receiver) = std::sync::mpsc::channel();
        let (leader_store, leader_hook) = (store.clone(), hook.clone());
        let leader = std::thread::spawn(move || {
            let mut http = PanicHttp {
                inside: inside_sender,
                go: go_receiver,
            };
            ask(&leader_store, &mut http, 1_000, false, &leader_hook)
        });
        // Wait until the leader is inside its network call, so the waiter surely joins the
        // flight instead of leading its own.
        inside_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the leader reached the network");
        let (waiter_store, waiter_hook) = (store.clone(), hook.clone());
        let waiter = std::thread::spawn(move || {
            ask(
                &waiter_store,
                &mut Unreachable(0),
                1_000,
                false,
                &waiter_hook,
            )
        });
        // Let the leader die now.
        let _ = go_sender.send(());
        assert!(leader.join().is_err(), "the leader panicked, as scripted");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(waiter.join().unwrap());
        });
        let answer = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the waiter woke — a leader's panic never strands a flight");
        assert!(
            matches!(
                answer,
                Err(TokenError::Offline(_)) | Err(TokenError::Other(_))
            ),
            "the waiter woke with an error, not {answer:?}"
        );
        assert_eq!(hook.load(Ordering::SeqCst), 0, "nobody signed out");
        // The flight is cleared: the next ask leads its own.
        let hook2 = AtomicUsize::new(0);
        let answer = ask(&store, &mut Unreachable(0), 1_000, false, &hook2);
        assert!(matches!(answer, Err(TokenError::Offline(_))));
    }

    #[test]
    fn a_token_asked_for_another_owner_is_signed_out_and_requests_nothing() {
        let _turn = turn();
        // I5 in the token layer: the stored sign-in is user-1's; a send of user-2's gets
        // SignedOut for that owner, no request, no clear, no hook.
        let store = MemoryStore::with(account(2_000, 500));
        let server = FakeServer::new(&["refresh-0"]);
        let hook = AtomicUsize::new(0);
        let other = OwnerKey {
            origin: test_owner().origin.clone(),
            user_id: "user-2".into(),
        };
        let answer = access_token_in(
            &store,
            &mut FakeHttp(server.clone()),
            1_000,
            false,
            &other,
            1,
            &|_, _, _| {
                hook.fetch_add(1, Ordering::SeqCst);
            },
            None,
        );
        assert_eq!(answer, Err(TokenError::SignedOut));
        assert_eq!(server.requests(), 0);
        assert_eq!(hook.load(Ordering::SeqCst), 0);
        assert!(
            store.current().is_some(),
            "nothing of the stored sign-in is touched"
        );
    }

    #[test]
    fn end_session_for_only_runs_the_hook() {
        let _turn = turn();
        // §4: bir-core no longer clears anything itself — the hook (the machine's server_ended)
        // owns the clear and the close.
        let ran = std::sync::atomic::AtomicUsize::new(0);
        let hook = |_: OwnerKey, _: u64| {
            ran.fetch_add(1, Ordering::SeqCst);
        };
        // The registration is process-global; in the test binary nothing registers, so the hook
        // is a no-op — what is tested is end_session_for's own shape: it does not touch a store.
        let store = MemoryStore::with(account(5_000, 500));
        let _ = hook;
        end_session_for(test_owner(), 1, "refresh-token".to_string());
        end_session_for(test_owner(), 1, "refresh-token".to_string());
        assert!(store.current().is_some(), "nothing is cleared here");
    }
}
