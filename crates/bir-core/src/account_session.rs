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
static SESSION_ENDED: OnceLock<fn()> = OnceLock::new();

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
pub fn access_token(http: &mut dyn Http, now_unix: i64, force: bool) -> Result<String, TokenError> {
    access_token_in(&SavedAccount, http, now_unix, force, &run_session_ended)
}

/// What the host does when the sign-in ends outside a user action (rejected refresh, device removed):
/// bir-app-api registers closing its account gate. Set once; later calls are ignored.
pub fn on_session_ended(hook: fn()) {
    let _ = SESSION_ENDED.set(hook);
}

/// End the saved sign-in now: clear the saved account and run the hook. Takes [`lock`]. Idempotent.
pub fn end_session() {
    end_session_in(&SavedAccount, &run_session_ended);
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
        // thread runs under the name of the thread that asked.
        let database = crate::db::capture_database_path_override();
        Box::pin(async move {
            let answer = tokio::task::spawn_blocking(move || {
                database.run(|| {
                    let mut http = cloud_account::BlockingHttp::new();
                    access_token(&mut http, cloud_account::unix_now(), force_refresh)
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
        let stored = cloud_account::load_stored_account().ok().flatten()?;
        Some(stored.email.unwrap_or_default())
    }

    fn session_ended(&self) {
        end_session();
    }
}

/// Where the saved account lives. The real one is the keychain and private file of
/// [`cloud_account`]; tests use one in memory.
trait AccountStore {
    fn load(&self) -> Result<Option<StoredAccount>, CloudError>;
    fn save(&self, stored: &StoredAccount) -> Result<(), CloudError>;
    fn clear(&self);
}

/// The saved account of [`cloud_account`].
struct SavedAccount;

impl AccountStore for SavedAccount {
    fn load(&self) -> Result<Option<StoredAccount>, CloudError> {
        cloud_account::load_stored_account()
    }

    fn save(&self, stored: &StoredAccount) -> Result<(), CloudError> {
        cloud_account::save_stored_account(stored)
    }

    fn clear(&self) {
        cloud_account::clear_stored_account();
    }
}

fn run_session_ended() {
    if let Some(hook) = SESSION_ENDED.get() {
        hook();
    }
}

/// [`access_token`] against `store`. The hook runs after [`lock`] is released.
fn access_token_in(
    store: &dyn AccountStore,
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    session_ended: &dyn Fn(),
) -> Result<String, TokenError> {
    let mut ended = false;
    let answer = {
        let _one = lock();
        token_under_lock(store, http, now_unix, force, &mut ended)
    };
    if ended {
        session_ended();
    }
    answer
}

fn end_session_in(store: &dyn AccountStore, session_ended: &dyn Fn()) {
    {
        let _one = lock();
        store.clear();
    }
    session_ended();
}

/// The rules of [`access_token`]. The caller holds [`lock`]. `ended` is set when the
/// sign-in ended here and the hook must run.
fn token_under_lock(
    store: &dyn AccountStore,
    http: &mut dyn Http,
    now_unix: i64,
    force: bool,
    ended: &mut bool,
) -> Result<String, TokenError> {
    let mut stored = match store.load() {
        Ok(Some(stored)) => stored,
        Ok(None) => return Err(TokenError::SignedOut),
        Err(error) => return Err(TokenError::Other(error.to_string())),
    };
    let mut retried = false;
    loop {
        if !force && stored.access_expires_at_unix > now_unix.saturating_add(EXPIRY_MARGIN_SECONDS)
        {
            return Ok(stored.access_token);
        }
        if force && now_unix.saturating_sub(stored.last_refresh_unix) < RECENT_REFRESH_SECONDS {
            return Ok(stored.access_token);
        }
        match cloud_account::refresh_tokens(http, &stored.server_url, &stored.refresh_token) {
            Ok(tokens) => {
                let next = StoredAccount {
                    access_token: tokens.access_token,
                    refresh_token: tokens.refresh_token,
                    access_expires_at_unix: now_unix.saturating_add(tokens.expires_in.max(1)),
                    offline: false,
                    last_refresh_unix: now_unix,
                    ..stored
                };
                store
                    .save(&next)
                    .map_err(|error| TokenError::Other(error.to_string()))?;
                return Ok(next.access_token);
            }
            Err(CloudError::Network(reason)) => return Err(TokenError::Offline(reason)),
            Err(CloudError::InvalidGrant) => {
                let used = stored.refresh_token;
                match store.load() {
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
                        store.clear();
                        *ended = true;
                        return Err(TokenError::SignedOut);
                    }
                    // Another process signed out in the meantime.
                    Ok(None) => {
                        *ended = true;
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

    fn ask(
        store: &MemoryStore,
        http: &mut dyn Http,
        now: i64,
        force: bool,
        hook_runs: &AtomicUsize,
    ) -> Result<String, TokenError> {
        access_token_in(store, http, now, force, &|| {
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
        assert!(store.current().is_none(), "the saved account is cleared");
        assert_eq!(store.clears.load(Ordering::SeqCst), 1);
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
    fn ending_the_session_clears_and_runs_the_hook_each_time() {
        let store = MemoryStore::with(account(5_000, 500));
        let hook = AtomicUsize::new(0);
        let count = || {
            hook.fetch_add(1, Ordering::SeqCst);
        };
        end_session_in(&store, &count);
        end_session_in(&store, &count);
        assert!(store.current().is_none());
        assert_eq!(hook.load(Ordering::SeqCst), 2);
    }
}
