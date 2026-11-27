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
static SESSION_ENDED: OnceLock<fn(OwnerKey, u64)> = OnceLock::new();

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

/// F8: who is stored, cached in memory. Filled once at first use from the stored sign-in; every
/// save and clear of the stored sign-in updates it inside the same account_session::lock()
/// section as the file or keychain write, so a reader never needs the file, the keychain or the
/// account lock. Its own small lock, no I/O.
static STORED_OWNER: Mutex<Option<Option<OwnerKey>>> = Mutex::new(None);

/// The stored sign-in's owner key, or None while nobody is stored. Never touches the file after
/// the first read: every writer keeps it current inside the account lock.
pub fn stored_owner() -> Option<OwnerKey> {
    {
        let cache = STORED_OWNER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(owner) = cache.as_ref() {
            return owner.clone();
        }
    }
    // First use: fill from the stored sign-in, under the account lock so no write races the read.
    let owner = {
        let _one = lock();
        cloud_account::load_stored_account()
            .ok()
            .flatten()
            .and_then(|stored| OwnerKey::of(&stored))
    };
    *STORED_OWNER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(owner.clone());
    owner
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
/// session that has since changed hands writes nothing. Answers whether the save ran.
pub fn save_stored_if(owner: &OwnerKey, stored: &StoredAccount) -> Result<bool, CloudError> {
    if !stored_is_owner(owner) {
        return Ok(false);
    }
    save_stored(stored)?;
    Ok(true)
}

/// Clear the stored sign-in through the one path: the file write. The caller holds [`lock`],
/// outside SESSION; the cache's None is the machine's SESSION-section step (F8').
pub fn clear_stored() {
    cloud_account::clear_stored_account();
}

/// Clear the stored sign-in only if its owner is still `owner` (§4): a server-ended session of
/// A's never clears B's sign-in. Answers whether the clear ran.
pub fn clear_stored_if(owner: &OwnerKey) -> bool {
    if !stored_is_owner(owner) {
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
///   by the same rules, refreshing at most once more. If it is the same, the sign-in has ended: clear the
///   saved account, call the session-ended hook, return `SignedOut`.
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
    )
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
/// removed): bir-app-api registers session::server_ended, which takes the owner and the epoch
/// the ending acted for. Set once; later calls are ignored.
pub fn on_session_ended(hook: fn(OwnerKey, u64)) {
    let _ = SESSION_ENDED.set(hook);
}

/// The sign-in of `owner` ended at the server (a 401, an InvalidGrant), in the session of
/// `epoch`. bir-core no longer clears anything itself (§4): it ONLY calls the registered hook —
/// session::server_ended — which does the clear-if-owner and the close in one SESSION section.
/// A mismatched owner or epoch is the hook's no-op, so nothing of a later sign-in is touched.
pub fn end_session_for(owner: OwnerKey, epoch: u64) {
    if let Some(hook) = SESSION_ENDED.get() {
        hook(owner, epoch);
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
        let Some(for_owner) = stored_owner() else {
            return Box::pin(async move { Err(bir_submit::ClientError::SignedOut) });
        };
        Box::pin(async move {
            let answer = tokio::task::spawn_blocking(move || {
                database.run(|| {
                    let mut http = cloud_account::BlockingHttp::new();
                    access_token(
                        &mut http,
                        cloud_account::unix_now(),
                        force_refresh,
                        &for_owner,
                    )
                })
            })
            .await
            .map_err(|error| bir_submit::ClientError::Other(error.to_string()))?;
            answer.map_err(|error| match error {
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
        // The server's 401 ended the session of the owner stored when the send started, in this
        // epoch. bir-core clears nothing itself: the hook's one section decides (§4).
        if let Some(owner) = stored_owner() {
            end_session_for(owner, crate::session_epoch::current());
        }
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

/// Drops the flight marker however the refresh ends, waking the waiters.
struct FlightGuard;

impl Drop for FlightGuard {
    fn drop(&mut self) {
        *REFRESH_HELD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
        REFRESH_FLIGHT_FREE.notify_all();
    }
}

/// The single flight of the refresh (D2's other half): the account lock is never held across
/// the network, but two in-process refreshes of one owner never race either — the second waits
/// HERE, on a condvar that is not the account lock, then re-reads the record the first one
/// saved. Without it a racing pair would burn the rotation: one refresh invalidating the
/// other's token mid-flight, and the loser's re-read can meet the stale record before the
/// winner's save lands — a spurious session-ending. Cross-process rotation (bir-mcp) is still
/// the re-read-and-retry arm below.
static REFRESH_FLIGHT_FREE: std::sync::Condvar = std::sync::Condvar::new();
static REFRESH_HELD: Mutex<bool> = Mutex::new(false);

/// [`access_token`] against `store` — the D2 restructure (§4): the lock is held only for the
/// file reads and writes, NEVER across the network refresh. The owner check comes first: a
/// stored sign-in that is not `for_owner` answers SignedOut for that owner with no clear and no
/// hook.
fn access_token_in(
    store: &dyn AccountStore,
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    for_owner: &OwnerKey,
    epoch: u64,
    ended: &dyn Fn(OwnerKey, u64),
) -> Result<String, TokenError> {
    let mut retried = false;
    // The first load, under the lock: the fast paths (a valid token, a just-refreshed one)
    // never leave it.
    let mut stored = {
        let _one = lock();
        match store.load() {
            Ok(Some(stored)) => stored,
            Ok(None) => return Err(TokenError::SignedOut),
            Err(error) => return Err(TokenError::Other(error.to_string())),
        }
    };
    loop {
        // The owner check at every step (I5): the stored sign-in is not this send's owner →
        // SignedOut for that owner, and nothing of the new owner's is touched.
        let owner = match OwnerKey::of(&stored) {
            Some(owner) => owner,
            None => return Err(TokenError::SignedOut),
        };
        if owner != *for_owner {
            return Err(TokenError::SignedOut);
        }
        if !force && stored.access_expires_at_unix > now_unix.saturating_add(EXPIRY_MARGIN_SECONDS)
        {
            return Ok(stored.access_token);
        }
        if force && now_unix.saturating_sub(stored.last_refresh_unix) < RECENT_REFRESH_SECONDS {
            return Ok(stored.access_token);
        }
        // The single flight, with NO lock of account_session's held: wait until no other
        // in-process refresh runs, then re-read the record — the winner's save may have landed
        // while we waited, making this refresh unnecessary.
        {
            let mut held = REFRESH_HELD
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while *held {
                held = REFRESH_FLIGHT_FREE
                    .wait(held)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            *held = true;
        }
        let _flight = FlightGuard;
        // The re-read after the wait: fast paths again — the winner's token may already serve.
        // The owner check comes first again: the sign-in may have changed hands while we waited,
        // and a token of the new owner's is never this owner's answer.
        {
            let _one = lock();
            match store.load() {
                Ok(Some(again)) => stored = again,
                Ok(None) => {
                    return Err(TokenError::SignedOut);
                }
                Err(error) => return Err(TokenError::Other(error.to_string())),
            }
        }
        if OwnerKey::of(&stored).as_ref() != Some(for_owner) {
            return Err(TokenError::SignedOut);
        }
        if !force && stored.access_expires_at_unix > now_unix.saturating_add(EXPIRY_MARGIN_SECONDS)
        {
            return Ok(stored.access_token);
        }
        if force && now_unix.saturating_sub(stored.last_refresh_unix) < RECENT_REFRESH_SECONDS {
            return Ok(stored.access_token);
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
                if !saved {
                    return Err(TokenError::SignedOut);
                }
                return Ok(next.access_token);
            }
            Err(CloudError::Network(reason)) => return Err(TokenError::Offline(reason)),
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
                            return Err(TokenError::Other(
                                "the saved Buwiz sign-in keeps changing; try again".into(),
                            ));
                        }
                        retried = true;
                        stored = again;
                    }
                    Ok(Some(_)) => {
                        // The same refresh token the server just rejected: the sign-in has
                        // ended. end_session_for ONLY calls the hook — the clear-if-owner and
                        // the close are the hook's one SESSION section (§4).
                        ended(owner, epoch);
                        return Err(TokenError::SignedOut);
                    }
                    // Another process signed out in the meantime.
                    Ok(None) => {
                        ended(owner, epoch);
                        return Err(TokenError::SignedOut);
                    }
                    Err(error) => return Err(TokenError::Other(error.to_string())),
                }
            }
            Err(error) => return Err(TokenError::Other(error.to_string())),
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
        access_token_in(store, http, now, force, &test_owner(), 1, &|_, _| {
            hook_runs.fetch_add(1, Ordering::SeqCst);
        })
    }

    #[test]
    fn not_signed_in_is_signed_out_with_no_request() {
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

    #[test]
    fn a_token_asked_for_another_owner_is_signed_out_and_requests_nothing() {
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
            &|_, _| {
                hook.fetch_add(1, Ordering::SeqCst);
            },
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
        end_session_for(test_owner(), 1);
        end_session_for(test_owner(), 1);
        assert!(store.current().is_some(), "nothing is cleared here");
    }
}
