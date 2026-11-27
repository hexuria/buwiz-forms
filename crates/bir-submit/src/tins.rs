//! The TINs on the signed-in Buwiz account, and the state of each:
//! `GET /api/tax-profiles?include=all`; and adding one from the app,
//! `POST /api/tax-profiles` ([`Submitter::add_account_tin`]).
//!
//! - One read is shared: concurrent callers wait for one request, and a
//!   list younger than the caller's `max_age` is answered from memory. A
//!   failure is kept for 30 seconds.
//! - Offline, the last list is answered for 10 minutes, marked `stale`,
//!   unless a send was refused for a TIN since that list's read started.
//! - A banned read forgets the earlier list.
//! - Signed out, there is no request. Signing out or in clears the list.
//! - The list is complete (a TIN missing from it is not on the account)
//!   only when the server says so with `"include":"all"`.
//! - A 400 on the `include` parameter (a server that rejects it) is asked
//!   again without it; a 403 never is.
//! - A `429` keeps the last list: until its `Retry-After` the server is not
//!   asked again, whatever the caller's `max_age`, and the last list of
//!   this generation is answered, marked `stale`.
//!
//! No row, TIN or name is logged, and the types' `Debug` shows none.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::client::{ClientError, Submitter};
use crate::device;
use crate::wire::TinKey;

/// How long a list read before going offline is still answered.
pub const STALE_LIMIT_SECS: i64 = 600;
/// How long a failed read is answered before the next request.
const FAILURE_SECS: i64 = 30;
/// The shortest time between two forced reads ("Check again").
const FORCED_SECS: i64 = 15;
/// How long a `429` without a usable `Retry-After` holds requests off.
const LIMIT_DEFAULT_SECS: i64 = 60;
/// The unverified TINs an account may hold (the server's cap, in the app's
/// words for `too_many_unverified`).
pub const MAX_UNVERIFIED: u32 = 20;
pub(crate) const PATH: &str = "/api/tax-profiles?include=all";

/// The Buwiz account site: the only host a row's `action.url` may open,
/// and the site of every link the apps build for a TIN's step.
pub const ACCOUNT_SITE: &str = "https://account.buwiz.com";

/// The account site in use: [`ACCOUNT_SITE`], or in a debug build
/// `BUWIZ_ACCOUNT_URL` when it is https or http to this machine.
pub fn account_site() -> String {
    account_site_from(
        cfg!(debug_assertions)
            .then(|| std::env::var("BUWIZ_ACCOUNT_URL").ok())
            .flatten()
            .as_deref(),
    )
}

/// [`account_site`] with `overridden` as the debug build's variable.
fn account_site_from(overridden: Option<&str>) -> String {
    overridden
        .map(str::trim)
        .filter(|url| crate::client::is_private_url(url) && url::Url::parse(url).is_ok())
        .unwrap_or(ACCOUNT_SITE)
        .trim_end_matches('/')
        .to_owned()
}

/// `url` when it is a page of [`account_site`], as the URL parser writes
/// it: the site's scheme (https), that host exactly, no user name or
/// password, the site's port (the default). `None` for anything else.
pub fn pinned_account_url(url: &str) -> Option<String> {
    let site = url::Url::parse(&account_site()).expect("the account site is a URL");
    let page = url::Url::parse(url.trim()).ok()?;
    (page.scheme() == site.scheme()
        && page.host_str().is_some()
        && page.host_str() == site.host_str()
        && page.username().is_empty()
        && page.password().is_none()
        && page.port() == site.port())
    .then(|| page.to_string())
}
pub(crate) const PLAIN_PATH: &str = "/api/tax-profiles";

/// A row's state on the account. An unknown value is kept as `Other` and
/// never blocks anything.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TinStatus {
    Approved,
    Unverified,
    Pending,
    NeedsFix,
    Paused,
    /// A test TIN (root `000-000-000`): Buwiz never sends its returns.
    TestOnly,
    Other(String),
}

impl TinStatus {
    /// A missing `status` is `approved` (servers before the state list).
    fn parse(status: Option<&str>) -> Self {
        match status {
            None | Some("approved") => TinStatus::Approved,
            Some("unverified") => TinStatus::Unverified,
            Some("pending") => TinStatus::Pending,
            Some("needs_fix") => TinStatus::NeedsFix,
            Some("paused") => TinStatus::Paused,
            Some("test_only") => TinStatus::TestOnly,
            Some(other) => TinStatus::Other(other.to_owned()),
        }
    }
}

/// What the app does next with a row (`next`): send its returns, wait for
/// Buwiz (in review, or paused), or take the row's step on the account
/// site (verify it, or fix it: the row's `action`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Send,
    Wait,
    Fix,
}

impl Next {
    /// The row's `next`. A row without one (a server from before it), or
    /// with a word this app does not know, falls back: `can_send` is Send,
    /// an `action` is Fix, anything else Wait.
    fn parse(next: Option<&str>, can_send: bool, has_action: bool) -> Self {
        match next {
            Some("send") => Next::Send,
            Some("wait") => Next::Wait,
            Some("fix") => Next::Fix,
            _ if can_send => Next::Send,
            _ if has_action => Next::Fix,
            _ => Next::Wait,
        }
    }
}

/// One TIN on the account.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountTin {
    /// The one key a return's TIN is compared by.
    pub key: TinKey,
    pub registered_name: String,
    pub status: TinStatus,
    pub can_send: bool,
    /// What the app does next with it ([`Next`]).
    pub next: Next,
    /// The server's line for the row: where it stands and what is next.
    pub message: Option<String>,
    /// A test TIN (root `000-000-000` or `111-111-114`): its sends are dry
    /// runs, never sent to the BIR.
    pub test_only: bool,
    /// The fixed reason sentences of a `needs_fix` row.
    pub reasons: Vec<String>,
    /// Staff's free-text note of a `needs_fix` row. For app-only surfaces:
    /// never copied into a stored reason, a line an assistant reads, or an
    /// alert.
    pub note: Option<String>,
    /// The row's own page for its Verify or Fix step (`action.url`), kept
    /// only when it is a page of [`ACCOUNT_SITE`] ([`pinned_account_url`]).
    /// For app-only surfaces, like `note`: it names the row's claim.
    pub action_url: Option<String>,
}

impl AccountTin {
    fn parse(row: &Value) -> Option<Self> {
        let key = TinKey::parse(row["tin"].as_str()?)?;
        let status = TinStatus::parse(row["status"].as_str());
        let text = |value: &Value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        };
        let can_send = row["can_send"]
            .as_bool()
            .unwrap_or(status == TinStatus::Approved);
        Some(Self {
            key,
            registered_name: text(&row["registered_name"]).unwrap_or_default(),
            can_send,
            next: Next::parse(row["next"].as_str(), can_send, !row["action"].is_null()),
            message: text(&row["message"]),
            test_only: row["test_only"]
                .as_bool()
                .unwrap_or(status == TinStatus::TestOnly),
            status,
            reasons: row["reasons"]
                .as_array()
                .map(|reasons| reasons.iter().filter_map(text).collect())
                .unwrap_or_default(),
            note: text(&row["note"]),
            action_url: text(&row["action"]["url"]).and_then(|url| pinned_account_url(&url)),
        })
    }
}

impl std::fmt::Debug for AccountTin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountTin")
            .field("key", &self.key)
            .field("status", &self.status)
            .field("can_send", &self.can_send)
            .field("next", &self.next)
            .field("test_only", &self.test_only)
            .field("reasons", &self.reasons.len())
            .field("note", &self.note.is_some())
            .field("action_url", &self.action_url.is_some())
            .finish_non_exhaustive()
    }
}

/// The account's list as one read saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountTins {
    pub rows: Vec<AccountTin>,
    /// The server listed every state (`"include":"all"`): a TIN missing
    /// from `rows` is not on the account.
    pub complete: bool,
    /// When the list was read, in seconds since the epoch.
    pub fetched_at: i64,
    /// Answered offline from an earlier read.
    pub stale: bool,
}

impl AccountTins {
    /// A `/api/tax-profiles` answer read at `fetched_at`. A row whose `tin`
    /// is not 9, 12 or 14 digits is left out, and then the list is not
    /// complete: a TIN missing from it may be that row. It is complete only
    /// when `include` is exactly `"all"` and `profiles` is a list whose every
    /// row was read.
    pub fn parse(body: &Value, fetched_at: i64) -> Self {
        let profiles = body["profiles"].as_array();
        let rows: Vec<AccountTin> = profiles
            .map(|profiles| profiles.iter().filter_map(AccountTin::parse).collect())
            .unwrap_or_default();
        let complete = body["include"].as_str() == Some("all")
            && profiles.is_some_and(|profiles| profiles.len() == rows.len());
        Self {
            rows,
            complete,
            fetched_at,
            stale: false,
        }
    }

    /// The row for `key`: of two rows for one TIN, the one that can send,
    /// else the first.
    pub fn find(&self, key: &TinKey) -> Option<&AccountTin> {
        let mut rows = self.rows.iter().filter(|row| &row.key == key);
        rows.clone()
            .find(|row| row.can_send)
            .or_else(|| rows.next())
    }
}

/// Why the list could not be read.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum TinsError {
    #[error("Sign in to Buwiz to see your TINs.")]
    SignedOut,
    #[error("This Buwiz account is banned.")]
    Banned,
    #[error("Buwiz can't be reached.")]
    Offline,
    /// The server's refusal or an unreadable answer; `message` is the
    /// server's and is not shown by `Debug`.
    #[error("The Buwiz server could not list your TINs ({code}).")]
    Server { code: String, message: String },
    /// `429`: Buwiz asked not to be asked again before `until` (seconds
    /// since the epoch), and there is no earlier list to answer.
    #[error("Buwiz asked to wait before your TINs are read again.")]
    RateLimited { until: i64 },
}

impl std::fmt::Debug for TinsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TinsError::SignedOut => f.write_str("SignedOut"),
            TinsError::Banned => f.write_str("Banned"),
            TinsError::Offline => f.write_str("Offline"),
            TinsError::Server { code, .. } => write!(f, "Server({code})"),
            TinsError::RateLimited { until } => write!(f, "RateLimited({until})"),
        }
    }
}

pub type TinsRead = Result<AccountTins, TinsError>;

/// The last reads, kept on the [`Submitter`].
#[derive(Default)]
pub(crate) struct TinsCache {
    state: Mutex<CacheState>,
    /// Held while a request is out, so concurrent callers share it.
    flight: tokio::sync::Mutex<()>,
}

/// Ages in the cache are timed on the monotonic clock, so a wall clock set
/// back or ahead cannot make a read look young. `fetched_at` stays the wall
/// clock's, for the gate.
///
/// Each read is kept with the generation it started in. An invalidation (a
/// send refused for a TIN) starts a new generation: a read from an older
/// one, even one still in flight then, is neither answered from memory nor
/// offline.
#[derive(Default)]
struct CacheState {
    /// The sign-in the reads below belong to.
    login: Option<String>,
    /// Bumped by every invalidation and every reset; never goes back.
    generation: u64,
    /// The last read, when it ended and the generation it started in.
    last: Option<(Instant, u64, TinsRead)>,
    /// The last list read, the same way, for answering offline.
    good: Option<(Instant, u64, AccountTins)>,
    /// A `429`'s `Retry-After`: no request before this instant. With it,
    /// the same moment on the wall clock, for [`TinsError::RateLimited`].
    blocked_until: Option<(Instant, i64)>,
}

/// Whether a read that ended at `at` is younger than `secs` at `now`. A
/// read that seems to have ended after `now` has no known age: it is not.
fn younger(at: Instant, now: Instant, secs: i64) -> bool {
    let limit = Duration::from_secs(u64::try_from(secs).unwrap_or(0));
    now.checked_duration_since(at)
        .is_some_and(|age| age < limit)
}

impl CacheState {
    /// Forget every read, for `login` (none: signed out), in a generation
    /// no earlier read started in.
    fn reset(&mut self, login: Option<String>) {
        *self = CacheState {
            login,
            generation: self.generation + 1,
            ..CacheState::default()
        };
    }

    /// What a failed read answers: offline, the last list (marked stale)
    /// while it is under [`STALE_LIMIT_SECS`] old and of this generation.
    ///
    /// Rate limited, the last list of this generation whatever its age: the
    /// server asked to keep it until `Retry-After`.
    fn fallback(&self, now: Instant, error: TinsError) -> TinsRead {
        match (&error, &self.good) {
            (TinsError::Offline, Some((at, generation, good)))
                if *generation == self.generation && younger(*at, now, STALE_LIMIT_SECS) =>
            {
                Ok(AccountTins {
                    stale: true,
                    ..good.clone()
                })
            }
            (TinsError::RateLimited { .. }, Some((_, generation, good)))
                if *generation == self.generation =>
            {
                Ok(AccountTins {
                    stale: true,
                    ..good.clone()
                })
            }
            _ => Err(error),
        }
    }

    /// The answer while a `429` holds requests off at `now`, if one does.
    fn blocked(&mut self, now: Instant) -> Option<TinsRead> {
        let (until, wall) = self.blocked_until?;
        if now >= until {
            self.blocked_until = None;
            return None;
        }
        Some(self.fallback(now, TinsError::RateLimited { until: wall }))
    }
}

impl TinsCache {
    /// Forget every read (a sign-out, or a new sign-in).
    pub(crate) fn clear(&self) {
        self.state.lock().expect("tins cache").reset(None);
    }

    /// The next read asks the server, and no read from before answers
    /// offline (a send was just refused for a TIN).
    pub(crate) fn invalidate(&self) {
        self.state.lock().expect("tins cache").generation += 1;
        INVALIDATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// The generation a read starting now belongs to.
    fn generation(&self) -> u64 {
        self.state.lock().expect("tins cache").generation
    }

    /// The answer from memory for `login` at `now`, when the last read is
    /// of this generation and young enough: a list younger than `max_age`,
    /// a failure younger than 30 seconds (and than `max_age`). `None`: ask
    /// the server.
    fn answer(&self, login: &str, now: Instant, max_age: i64) -> Option<TinsRead> {
        let mut state = self.state.lock().expect("tins cache");
        if state.login.as_deref() != Some(login) {
            state.reset(Some(login.to_owned()));
            return None;
        }
        if let Some(read) = state.blocked(now) {
            return Some(read);
        }
        match &state.last {
            Some((_, generation, _)) if *generation != state.generation => None,
            Some((at, _, Ok(list))) if younger(*at, now, max_age) => Some(Ok(list.clone())),
            Some((at, _, Err(error))) if younger(*at, now, max_age.min(FAILURE_SECS)) => {
                Some(state.fallback(now, error.clone()))
            }
            _ => None,
        }
    }

    /// Keep a read that started in generation `started` and ended at `now`,
    /// and answer it (offline, the last list while it is young enough). A
    /// banned read forgets the list from before the ban.
    fn store(&self, login: &str, started: u64, now: Instant, read: TinsRead) -> TinsRead {
        let mut state = self.state.lock().expect("tins cache");
        if read == Err(TinsError::SignedOut) {
            state.reset(None);
            return read;
        }
        if state.login.as_deref() != Some(login) {
            state.reset(Some(login.to_owned()));
        }
        state.last = Some((now, started, read.clone()));
        if let Err(TinsError::RateLimited { until }) = read {
            let wait = u64::try_from(until - device::now()).unwrap_or(0);
            state.blocked_until = Some((now + Duration::from_secs(wait), until));
        }
        match read {
            Ok(list) => {
                state.good = Some((now, started, list.clone()));
                Ok(list)
            }
            Err(TinsError::Banned) => {
                state.good = None;
                Err(TinsError::Banned)
            }
            Err(error) => state.fallback(now, error),
        }
    }

    /// Make every read `secs` older, as if that much time had passed (a
    /// negative `secs`: as if it ended that far ahead of now).
    #[cfg(test)]
    fn age_for_tests(&self, secs: i64) {
        let shift = |at: &mut Instant| {
            let by = Duration::from_secs(secs.unsigned_abs());
            *at = if secs >= 0 { *at - by } else { *at + by };
        };
        let mut state = self.state.lock().expect("tins cache");
        if let Some((at, _, _)) = &mut state.last {
            shift(at);
        }
        if let Some((at, _, good)) = &mut state.good {
            shift(at);
            good.fetched_at -= secs;
        }
        if let Some((at, wall)) = &mut state.blocked_until {
            shift(at);
            *wall -= secs;
        }
    }
}

impl Submitter {
    /// The account's TINs, from memory when the last read is younger than
    /// `max_age_secs` (screens pass 60, an assistant's card 300).
    pub async fn account_tins(&self, max_age_secs: i64) -> TinsRead {
        let Some(login) = self.login() else {
            self.tins.clear();
            return Err(TinsError::SignedOut);
        };
        if let Some(read) = self.tins.answer(&login, Instant::now(), max_age_secs) {
            return read;
        }
        let _flight = self.tins.flight.lock().await;
        // Another caller may have read it while this one waited.
        if let Some(read) = self.tins.answer(&login, Instant::now(), max_age_secs) {
            return read;
        }
        let started = self.tins.generation();
        let fetched_at = device::now();
        let read = match self.bearer_get(PATH).await {
            Ok(body) => Ok(AccountTins::parse(&body, fetched_at)),
            // A server that rejects `include`: the plain list, which says
            // nothing about a TIN missing from it.
            Err(ClientError::Server { status: 400, .. }) => match self.bearer_get(PLAIN_PATH).await
            {
                Ok(body) => Ok(AccountTins {
                    complete: false,
                    ..AccountTins::parse(&body, fetched_at)
                }),
                Err(error) => Err(tins_error(error)),
            },
            Err(error) => Err(tins_error(error)),
        };
        self.tins.store(&login, started, Instant::now(), read)
    }

    /// "Check again": a new read, unless one ended in the last 15 seconds.
    pub async fn refresh_account_tins(&self) -> TinsRead {
        self.account_tins(FORCED_SECS).await
    }

    /// The last list read for this sign-in, without a request (for
    /// rendering); judge its age by `fetched_at`. `None` after a banned
    /// read.
    pub fn cached_account_tins(&self) -> Option<AccountTins> {
        let login = self.login()?;
        let state = self.tins.state.lock().expect("tins cache");
        (state.login.as_deref() == Some(login.as_str()))
            .then(|| state.good.as_ref().map(|(_, _, good)| good.clone()))
            .flatten()
    }

    /// The next [`Self::account_tins`] asks the server whatever its age.
    pub fn invalidate_account_tins(&self) {
        self.tins.invalidate();
    }

    /// "Activate this TIN to send": `POST /api/tax-profiles` adds `tin` to
    /// the account as not verified. Only an explicit press in the app calls
    /// it, never the creation of a tax profile.
    ///
    /// - `kind` is sent as `personal` or `business`, the TIN as its 14
    ///   digits with the branch (`123-456-789-00000`), the names trimmed;
    ///   `trade_name` only when it is not empty, whatever the kind.
    /// - A name over [`MAX_NAME_BYTES`] bytes after trimming (counted as the
    ///   server counts it) is [`AddTinError::NameTooLong`] at once, and an
    ///   empty registered name [`AddTinError::NameMissing`], without a
    ///   request; the server's own answer still decides everything else.
    /// - The added row, as `include=all` lists it (`None` when the answer
    ///   could not be read: the TIN was added all the same).
    /// - After an add, after `tin_already_active`, `tin_already_added` or
    ///   `claim_already_pending`, and after [`AddTinError::Offline`] (a POST
    ///   that timed out may still have added it), the next list read asks
    ///   the server.
    pub async fn add_account_tin(
        &self,
        kind: TinKind,
        tin: &TinKey,
        registered_name: &str,
        trade_name: Option<&str>,
    ) -> Result<Option<AccountTin>, AddTinError> {
        if self.login().is_none() {
            return Err(AddTinError::SignedOut);
        }
        let registered_name = registered_name.trim();
        let trade_name = trade_name.map(str::trim).filter(|name| !name.is_empty());
        if registered_name.len() > MAX_NAME_BYTES
            || trade_name.is_some_and(|name| name.len() > MAX_NAME_BYTES)
        {
            return Err(AddTinError::NameTooLong);
        }
        if registered_name.is_empty() {
            return Err(AddTinError::NameMissing);
        }
        let mut body = serde_json::json!({
            "kind": kind.wire(),
            "tin": dashed(tin),
            "registered_name": registered_name,
        });
        if let Some(trade_name) = trade_name {
            body["trade_name"] = Value::from(trade_name);
        }
        let added = self
            .bearer_send(reqwest::Method::POST, PLAIN_PATH, Some(&body))
            .await
            .map_err(add_error);
        // A POST that timed out (`Offline`) may still have added the TIN on
        // the server: the next read asks rather than trust the list from
        // before.
        if added.as_ref().map_or_else(
            |error| error.refreshes_list() || *error == AddTinError::Offline,
            |_| true,
        ) {
            self.tins.invalidate();
        }
        Ok(AccountTin::parse(&added?["profile"]))
    }
}

/// The longest registered or trade name the server takes, in bytes after
/// trimming (as the account site's form counts it: `ñ` is two).
pub const MAX_NAME_BYTES: usize = 200;

/// What a TIN is on the account site: a person's or a business's. The
/// app's taxpayer types map onto it: an individual is personal, every
/// other type (corporation, partnership, cooperative, estate, trust) is a
/// business.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TinKind {
    Individual,
    NonIndividual,
}

impl TinKind {
    /// The request's `kind`.
    pub fn wire(self) -> &'static str {
        match self {
            TinKind::Individual => "personal",
            TinKind::NonIndividual => "business",
        }
    }
}

/// `123-456-789-00000`: the root and the 5-digit branch, as the account
/// site prints a TIN.
fn dashed(tin: &TinKey) -> String {
    let root = tin.root();
    format!(
        "{}-{}-{}-{}",
        &root[..3],
        &root[3..6],
        &root[6..],
        tin.branch()
    )
}

/// Why "Activate this TIN to send" did not add the TIN: each refusal of
/// `POST /api/tax-profiles` is its own case, in the app's own words (the
/// server's sentence is never shown).
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddTinError {
    /// `bad_tin`
    #[error("This TIN is not valid.")]
    BadTin,
    /// `name_missing`: the registered name is empty.
    #[error("Add the registered name first.")]
    NameMissing,
    /// `name_too_long`: a name over [`MAX_NAME_BYTES`] bytes.
    #[error("This name is too long. Shorten it and try again.")]
    NameTooLong,
    /// `test_tin`: roots `000-000-000` and `111-111-114` are added by staff.
    #[error("Test TINs are added by Buwiz.")]
    TestTin,
    /// `tin_already_active`. The list is read again.
    #[error("This TIN is already activated on your account.")]
    TinAlreadyActive,
    /// `tin_already_added`: not verified yet. The list is read again, and
    /// its row has the Verify step.
    #[error("This TIN is already on your account, waiting to be verified.")]
    TinAlreadyAdded,
    /// `claim_already_pending`: in review or sent back. The list is read
    /// again.
    #[error("Buwiz is checking this TIN.")]
    ClaimAlreadyPending,
    /// `too_many_unverified`
    #[error(
        "Your account has {MAX_UNVERIFIED} TINs waiting to be verified. Verify or remove one on your account first."
    )]
    TooManyUnverified,
    /// `429 rate_limited`: tried again after `retry_after` seconds.
    #[error("Too many tries. Try again in {}.", minutes(.retry_after))]
    RateLimited { retry_after: i64 },
    /// `bad_request`: the server could not read what the app sent.
    #[error("Buwiz could not read this request. Update eBIRForms and try again.")]
    BadRequest,
    /// `403` (`insufficient_scope`): a sign-in that is not the BIR app's.
    #[error("This sign-in cannot add TINs. Sign out of Buwiz and sign in again.")]
    Forbidden,
    /// `403 account_banned`
    #[error("This Buwiz account is banned.")]
    Banned,
    #[error("Sign in to Buwiz to activate this TIN.")]
    SignedOut,
    #[error("Buwiz can't be reached.")]
    Offline,
    /// Any other answer; `message` is the server's and is not shown.
    #[error("The Buwiz server could not add this TIN ({code}).")]
    Other { code: String, message: String },
}

impl std::fmt::Debug for AddTinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddTinError::RateLimited { retry_after } => write!(f, "RateLimited({retry_after})"),
            AddTinError::Other { code, .. } => write!(f, "Other({code})"),
            other => f.write_str(other.code()),
        }
    }
}

impl AddTinError {
    /// The server's code for it (`signed_out`, `offline` for the two the
    /// server never sends).
    pub fn code(&self) -> &str {
        match self {
            AddTinError::BadTin => "bad_tin",
            AddTinError::NameMissing => "name_missing",
            AddTinError::NameTooLong => "name_too_long",
            AddTinError::TestTin => "test_tin",
            AddTinError::TinAlreadyActive => "tin_already_active",
            AddTinError::TinAlreadyAdded => "tin_already_added",
            AddTinError::ClaimAlreadyPending => "claim_already_pending",
            AddTinError::TooManyUnverified => "too_many_unverified",
            AddTinError::RateLimited { .. } => "rate_limited",
            AddTinError::BadRequest => "bad_request",
            AddTinError::Forbidden => "forbidden",
            AddTinError::Banned => "account_banned",
            AddTinError::SignedOut => "signed_out",
            AddTinError::Offline => "offline",
            AddTinError::Other { code, .. } => code,
        }
    }

    /// The account already holds the TIN: its list is read again.
    pub fn refreshes_list(&self) -> bool {
        matches!(
            self,
            AddTinError::TinAlreadyActive
                | AddTinError::TinAlreadyAdded
                | AddTinError::ClaimAlreadyPending
        )
    }
}

/// "N minutes" (at least one, rounded up; "1 minute").
fn minutes(seconds: &i64) -> String {
    let minutes = ((*seconds).max(1) + 59) / 60;
    if minutes == 1 {
        "1 minute".to_owned()
    } else {
        format!("{minutes} minutes")
    }
}

fn add_error(error: ClientError) -> AddTinError {
    match error {
        ClientError::SignedOut | ClientError::DeviceRevoked => AddTinError::SignedOut,
        // A second `invalid_token` after the forced refresh: the grant was
        // revoked on the server.
        ClientError::Server {
            status: 401, code, ..
        } if code == "invalid_token" => AddTinError::SignedOut,
        ClientError::Network(_) => AddTinError::Offline,
        ClientError::NotATry {
            status: 429,
            wait_until,
            ..
        } => AddTinError::RateLimited {
            retry_after: wait_until.map_or(LIMIT_DEFAULT_SECS, |until| {
                (until - chrono::Utc::now()).num_seconds().max(1)
            }),
        },
        ClientError::Server {
            status,
            code,
            message,
        } => match code.as_str() {
            "bad_tin" => AddTinError::BadTin,
            "name_missing" => AddTinError::NameMissing,
            "name_too_long" => AddTinError::NameTooLong,
            "test_tin" => AddTinError::TestTin,
            "tin_already_active" => AddTinError::TinAlreadyActive,
            "tin_already_added" => AddTinError::TinAlreadyAdded,
            "claim_already_pending" => AddTinError::ClaimAlreadyPending,
            "too_many_unverified" => AddTinError::TooManyUnverified,
            "bad_request" => AddTinError::BadRequest,
            "account_banned" => AddTinError::Banned,
            _ if status == 403 => AddTinError::Forbidden,
            _ => AddTinError::Other { code, message },
        },
        ClientError::NotATry { code, message, .. } => AddTinError::Other { code, message },
        error @ (ClientError::DeviceKeyMissing | ClientError::InstallInUse) => AddTinError::Other {
            code: "device_not_registered".to_owned(),
            message: error.to_string(),
        },
        ClientError::Other(message) => AddTinError::Other {
            code: "unreadable".to_owned(),
            message,
        },
    }
}

/// Test instrumentation: how many invalidations this process ran. The real-path sign-out test
/// counts them — the cache itself is the submitter's private state.
pub static INVALIDATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn tins_error(error: ClientError) -> TinsError {
    match error {
        ClientError::NotATry {
            status: 429,
            wait_until,
            ..
        } => TinsError::RateLimited {
            until: wait_until
                .map(|until| until.timestamp())
                .unwrap_or_else(|| device::now() + LIMIT_DEFAULT_SECS),
        },
        ClientError::SignedOut | ClientError::DeviceRevoked => TinsError::SignedOut,
        // A second `invalid_token` after the forced refresh: the grant was
        // revoked on the server.
        ClientError::Server {
            status: 401, code, ..
        } if code == "invalid_token" => TinsError::SignedOut,
        error @ (ClientError::DeviceKeyMissing | ClientError::InstallInUse) => TinsError::Server {
            code: "device_not_registered".to_owned(),
            message: error.to_string(),
        },
        ClientError::Network(_) => TinsError::Offline,
        ClientError::Server { code, .. } if code == "account_banned" => TinsError::Banned,
        ClientError::Server { code, message, .. } | ClientError::NotATry { code, message, .. } => {
            TinsError::Server { code, message }
        }
        ClientError::Other(message) => TinsError::Server {
            code: "unreadable".to_owned(),
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::client::SubmitConfig;
    use crate::secrets::{MemoryStore, SecretStore};
    use crate::testkit::FakeTokens;
    use crate::wire::TinRefusal;

    /// What the fake server answers: a status and a body, or `None` to
    /// drop the connection unanswered (an outage).
    type Answer = Option<(u16, String)>;

    /// A loopback server that answers each request with `answer(n, path)`
    /// (`n` counts from 0) after `delay_ms`; the request lines it saw.
    fn fake_server(
        delay_ms: u64,
        answer: impl Fn(usize, &str) -> Answer + Send + 'static,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        let (url, seen, _) = fake_server_with_headers(delay_ms, move |n, path| {
            answer(n, path).map(|(status, body)| (status, body, Vec::new()))
        });
        (url, seen)
    }

    /// What [`fake_server_with_headers`] answers: a status, a body and extra
    /// headers, or `None` to drop the connection.
    type Reply = Option<(u16, String, Vec<(&'static str, String)>)>;

    /// [`fake_server`] whose answers carry headers too; also the bodies of
    /// the requests it saw.
    #[allow(clippy::type_complexity)]
    fn fake_server_with_headers(
        delay_ms: u64,
        answer: impl Fn(usize, &str) -> Reply + Send + 'static,
    ) -> (String, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let body_log = bodies.clone();
        std::thread::spawn(move || {
            for (n, stream) in listener.incoming().enumerate() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap_or_default();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                        break;
                    }
                    if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let request = line.trim().trim_end_matches(" HTTP/1.1").to_owned();
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                log.lock().unwrap().push(request);
                body_log
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&body).into_owned());
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                if let Some((status, json, headers)) = answer(n, &path) {
                    let extra: String = headers
                        .iter()
                        .map(|(name, value)| format!("{name}: {value}\r\n"))
                        .collect();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{json}",
                        json.len()
                    );
                }
            }
        });
        (format!("http://{addr}"), seen, bodies)
    }

    const LOGIN: &str = "juana@example.com";

    fn submitter(url: &str, signed_in: bool) -> Submitter {
        submitter_tokens(url, signed_in).0
    }

    /// A submitter with a registered device (`d`) when `signed_in`, and the
    /// host's sign-in to change.
    fn submitter_tokens(url: &str, signed_in: bool) -> (Submitter, Arc<FakeTokens>) {
        let store = SharedStore::default();
        if signed_in {
            store.set("device_id", "d").unwrap();
        }
        let tokens = FakeTokens::new(signed_in.then_some(LOGIN));
        (
            submitter_with(url, store, tokens.clone(), Vec::new()),
            tokens,
        )
    }

    /// A submitter on `store` and `tokens` that pins `trust_keys`.
    fn submitter_with(
        url: &str,
        store: SharedStore,
        tokens: Arc<FakeTokens>,
        trust_keys: Vec<String>,
    ) -> Submitter {
        let config = SubmitConfig {
            server_url: url.to_owned(),
            device_name: "test".into(),
            trust_keys,
            install_id: None,
            platform: None,
        };
        Submitter::new(config, Box::new(store), tokens)
    }

    /// A secret store the test keeps a handle on, to write a device id
    /// the way a registration elsewhere would.
    #[derive(Clone, Default)]
    struct SharedStore(Arc<MemoryStore>);

    impl SecretStore for SharedStore {
        fn get(&self, name: &str) -> Result<Option<String>, crate::secrets::StoreError> {
            self.0.get(name)
        }

        fn set(&self, name: &str, value: &str) -> Result<(), crate::secrets::StoreError> {
            self.0.set(name, value)
        }

        fn delete(&self, name: &str) -> Result<(), crate::secrets::StoreError> {
            self.0.delete(name)
        }
    }

    /// How many list reads the fake server saw.
    fn list_reads(seen: &Mutex<Vec<String>>) -> usize {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("GET /api/tax-profiles"))
            .count()
    }

    /// The pinned key, a trust list it signed and the sealing key document
    /// the list names: what a send reads before its ticket.
    fn trust_documents() -> (String, String, String) {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use ed25519_dalek::{Signer, SigningKey};
        use sha2::{Digest, Sha256};

        let key = SigningKey::from_bytes(&[7; 32]);
        let public = URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes());
        let kid = crate::device::hex(&Sha256::digest(key.verifying_key().as_bytes())[..8]);
        let sealing = json!({ "kid": "s1", "kem": crate::seal::KEM, "public_key": URL_SAFE_NO_PAD.encode([1; 32]) });
        let claims = json!({
            "hosts": ["127.0.0.1:9"],
            "sealing_keys": [sealing],
            "receipt_keys": [{ "kty": "OKP", "crv": "Ed25519", "kid": kid, "x": public }],
            "iat": 1,
            "exp": 4_102_444_800_i64,
        });
        let header = json!({ "alg": "EdDSA", "kid": kid, "typ": crate::trust::TYP });
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap()),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let jws = format!(
            "{input}.{}",
            URL_SAFE_NO_PAD.encode(key.sign(input.as_bytes()).to_bytes())
        );
        (public, jws, json!({ "keys": [sealing] }).to_string())
    }

    fn ok(body: Value) -> Answer {
        Some((200, body.to_string()))
    }

    fn key(tin: &str) -> TinKey {
        TinKey::parse(tin).unwrap()
    }

    #[tokio::test]
    async fn account_tins_reads_today_servers_list() {
        let (url, seen) = fake_server(0, |_, _| {
            ok(
                json!({"profiles":[{"id":"p1","tin":"000-123-453-00000","registered_name":"X","permissions":["filing.submit"]}]}),
            )
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert_eq!(list.rows.len(), 1);
        let row = &list.rows[0];
        assert_eq!(row.key, key("000123453"));
        assert_eq!(row.status, TinStatus::Approved);
        assert!(row.can_send);
        assert!(!list.complete);
        assert!(!list.stale);
        assert_eq!(*seen.lock().unwrap(), ["GET /api/tax-profiles?include=all"]);
    }

    #[tokio::test]
    async fn account_tins_reads_pending_and_needs_fix_rows_of_today_server() {
        let (url, _) = fake_server(0, |_, _| {
            ok(json!({"profiles":[
                {"tin":"000-123-456-00000","registered_name":"X","status":"pending"},
                {"tin":"000-123-457-00000","registered_name":"Y","status":"needs_fix"}
            ]}))
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert_eq!(list.rows[0].status, TinStatus::Pending);
        assert_eq!(list.rows[1].status, TinStatus::NeedsFix);
        assert!(!list.rows[0].can_send && !list.rows[1].can_send);
        assert!(!list.complete);
    }

    #[tokio::test]
    async fn account_tins_new_server_marker_makes_it_complete() {
        let (url, _) = fake_server(0, |_, _| ok(json!({"include":"all","profiles":[]})));
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert!(list.complete);
        assert!(list.rows.is_empty());
    }

    /// A row that cannot be read leaves a hole: a TIN missing from the list
    /// may be that row, so the list is not complete.
    #[test]
    fn a_list_with_an_unreadable_row_is_not_complete() {
        let good = json!({"tin":"000-123-453-00000","registered_name":"X","status":"approved","can_send":true});
        for bad in [
            json!({"tin_masked":"***-***-454-00000","registered_name":"Y","status":"paused"}),
            json!({"tin":"000-123-454-0000","registered_name":"Y"}),
            json!({"tin":123454,"registered_name":"Y"}),
            json!("not a row"),
        ] {
            let list = AccountTins::parse(&json!({"include":"all","profiles":[good, bad]}), 1);
            assert_eq!(list.rows.len(), 1, "{bad}");
            assert!(!list.complete, "{bad}");
        }
        for body in [
            json!({"include":"all"}),
            json!({"include":"all","profiles":null}),
            json!({"include":"all","profiles":{"tin":"000123453"}}),
            json!({"include":"all","tins":[good]}),
        ] {
            assert!(!AccountTins::parse(&body, 1).complete, "{body}");
        }
        assert!(AccountTins::parse(&json!({"include":"all","profiles":[good]}), 1).complete);
    }

    #[test]
    fn find_prefers_the_row_that_can_send() {
        let list = AccountTins::parse(
            &json!({"include":"all","profiles":[
                {"tin":"000-123-453-00000","registered_name":"X","status":"pending","can_send":false},
                {"tin":"000123453","registered_name":"X","status":"approved","can_send":true}
            ]}),
            1,
        );
        let found = list.find(&key("000123453")).unwrap();
        assert!(found.can_send);
        assert_eq!(found.status, TinStatus::Approved);
    }

    /// Only `"include":"all"` itself marks the list complete; a server that
    /// echoes the request's include string does not.
    #[test]
    fn only_include_all_exactly_is_complete() {
        for include in [
            json!("all,pending"),
            json!("pending,all"),
            json!(" all"),
            json!("ALL"),
            json!(["all"]),
            json!(true),
        ] {
            let list = AccountTins::parse(&json!({"include":include,"profiles":[]}), 1);
            assert!(!list.complete, "{include}");
        }
    }

    #[tokio::test]
    async fn account_tins_reads_unverified_and_paused_with_can_send_false() {
        let (url, _) = fake_server(0, |_, _| {
            ok(json!({"include":"all","profiles":[
                {"tin":"000-123-455-00000","registered_name":"X","status":"unverified","can_send":false},
                {"tin":"000-123-454-00002","registered_name":"Y","status":"paused","can_send":false},
                {"tin":"000-123-458-00000","registered_name":"Z","status":"approved","can_send":false}
            ]}))
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert_eq!(list.rows[0].status, TinStatus::Unverified);
        assert_eq!(list.rows[1].status, TinStatus::Paused);
        assert_eq!(list.rows[1].key, key("000-123-454-002"));
        assert_eq!(list.rows[2].status, TinStatus::Approved);
        assert!(list.rows.iter().all(|row| !row.can_send));
    }

    #[tokio::test]
    async fn account_tins_needs_fix_keeps_reasons_and_note() {
        let (url, _) = fake_server(0, |_, _| {
            ok(json!({"include":"all","profiles":[{
                "tin":"000-123-457-00000","registered_name":"X","status":"needs_fix","can_send":false,
                "reasons":["A photo is too blurry to read."],"note":"Retake it in daylight.",
                "action":{"label":"Fix and send again","url":"https://account.buwiz.com/tins/claims/c1/fix"}
            }]}))
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        let row = &list.rows[0];
        assert_eq!(row.reasons, ["A photo is too blurry to read."]);
        assert_eq!(row.note.as_deref(), Some("Retake it in daylight."));
        assert_eq!(
            row.action_url.as_deref(),
            Some("https://account.buwiz.com/tins/claims/c1/fix")
        );
    }

    #[tokio::test]
    async fn an_unknown_status_is_kept_as_other() {
        let (url, _) = fake_server(0, |_, _| {
            ok(
                json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X","status":"frozen"}]}),
            )
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert_eq!(list.rows[0].status, TinStatus::Other("frozen".into()));
        assert!(!list.rows[0].can_send);
    }

    /// A test TIN's row (`status: test_only`, `"test_only": true`) is its
    /// own state, and never sends.
    #[test]
    fn a_test_tin_reads_as_test_only() {
        let body = json!({"include":"all","profiles":[
            {"tin":"000-000-000-00000","registered_name":"TEST","status":"test_only","can_send":false,"test_only":true},
            {"tin":"000-000-000-00001","registered_name":"TEST","status":"test_only"}
        ]});
        let list = AccountTins::parse(&body, 1);
        assert!(list.complete);
        assert_eq!(list.rows.len(), 2);
        for row in &list.rows {
            assert_eq!(row.status, TinStatus::TestOnly);
            assert!(!row.can_send);
            assert!(row.test_only);
        }
    }

    #[tokio::test]
    async fn account_tins_falls_back_when_include_is_rejected() {
        let (url, seen) = fake_server(0, |n, _| match n {
            0 => Some((
                400,
                r#"{"error":{"code":"bad_request","message":"unknown include"}}"#.into(),
            )),
            _ => ok(
                json!({"include":"all","profiles":[{"tin":"000-123-453-00000","registered_name":"X"}]}),
            ),
        });
        let list = submitter(&url, true).account_tins(60).await.unwrap();
        assert_eq!(list.rows.len(), 1);
        assert!(
            !list.complete,
            "a list read without include is never complete"
        );
        assert_eq!(
            *seen.lock().unwrap(),
            ["GET /api/tax-profiles?include=all", "GET /api/tax-profiles"]
        );
    }

    #[tokio::test]
    async fn account_tins_banned_is_its_own_error_and_is_not_asked_again_without_include() {
        let (url, seen) = fake_server(0, |_, _| {
            Some((
                403,
                r#"{"error":{"code":"account_banned","message":"This Buwiz account is banned."}}"#
                    .into(),
            ))
        });
        let (submitter, tokens) = submitter_tokens(&url, true);
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::Banned));
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "a 403 is never retried without include"
        );
        // Still signed in: a ban is not a sign-out.
        assert_eq!(submitter.login().as_deref(), Some(LOGIN));
        assert_eq!(tokens.ended(), 0);
    }

    #[tokio::test]
    async fn another_403_is_not_asked_again_either() {
        let (url, seen) = fake_server(0, |_, _| {
            Some((
                403,
                r#"{"error":{"code":"forbidden","message":"no"}}"#.into(),
            ))
        });
        let read = submitter(&url, true).account_tins(60).await;
        assert!(
            matches!(read, Err(TinsError::Server { ref code, .. }) if code == "forbidden"),
            "{read:?}"
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn account_tins_is_cached_and_single_flight() {
        let (url, seen) = fake_server(200, |_, _| ok(json!({"include":"all","profiles":[]})));
        let submitter = Arc::new(submitter(&url, true));
        let reads = (0..8).map(|_| {
            let submitter = submitter.clone();
            tokio::spawn(async move { submitter.account_tins(60).await })
        });
        for read in futures_util::future::join_all(reads).await {
            assert!(read.unwrap().is_ok());
        }
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            seen.lock().unwrap().len(),
            1,
            "a read inside max_age makes no request"
        );
        assert!(submitter.cached_account_tins().is_some());
        // A refusal seen on a send asks again.
        submitter.invalidate_account_tins();
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    /// A send refused while a list read is out: the list that read brings
    /// back is from before the refusal, so the next read still asks.
    #[tokio::test]
    async fn a_read_in_flight_when_the_list_is_invalidated_does_not_clear_it() {
        let (url, seen) = fake_server(300, |_, _| ok(json!({"include":"all","profiles":[]})));
        let submitter = Arc::new(submitter(&url, true));
        let reading = {
            let submitter = submitter.clone();
            tokio::spawn(async move { submitter.account_tins(60).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        submitter.invalidate_account_tins();
        assert!(reading.await.unwrap().is_ok());
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            seen.lock().unwrap().len(),
            2,
            "the read that started before the refusal does not count"
        );
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    /// After a refusal the list from before it is not answered offline: the
    /// read is offline, not the old list marked stale.
    #[tokio::test]
    async fn offline_after_an_invalidation_does_not_answer_the_older_list() {
        let (url, _) = fake_server(0, |n, _| match n {
            0 => {
                ok(json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X"}]}))
            }
            _ => None,
        });
        let submitter = submitter(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        submitter.invalidate_account_tins();
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::Offline));
        assert_eq!(submitter.account_tins(0).await, Err(TinsError::Offline));
    }

    /// After a banned read neither the cache nor the offline answer hands
    /// back the list from before the ban.
    #[tokio::test]
    async fn a_banned_read_hides_the_earlier_list() {
        let (url, _) = fake_server(0, |n, _| match n {
            0 => {
                ok(json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X"}]}))
            }
            1 => Some((
                403,
                r#"{"error":{"code":"account_banned","message":"banned"}}"#.into(),
            )),
            _ => None,
        });
        let submitter = submitter(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        assert!(submitter.cached_account_tins().is_some());
        assert_eq!(submitter.account_tins(0).await, Err(TinsError::Banned));
        assert_eq!(submitter.cached_account_tins(), None);
        assert_eq!(submitter.account_tins(0).await, Err(TinsError::Offline));
    }

    #[tokio::test]
    async fn a_failure_is_kept_for_thirty_seconds() {
        let (url, seen) = fake_server(0, |_, _| Some((500, "{}".into())));
        let submitter = submitter(&url, true);
        assert!(matches!(
            submitter.account_tins(60).await,
            Err(TinsError::Server { .. })
        ));
        assert!(matches!(
            submitter.account_tins(60).await,
            Err(TinsError::Server { .. })
        ));
        assert_eq!(seen.lock().unwrap().len(), 1);
        // A forced read waits out only its own 15 seconds; `max_age` 0 asks.
        assert!(submitter.account_tins(0).await.is_err());
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn account_tins_offline_serves_stale_then_errors() {
        let (url, _) = fake_server(0, |n, _| match n {
            0 => {
                ok(json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X"}]}))
            }
            _ => None,
        });
        let submitter = submitter(&url, true);
        let first = submitter.account_tins(60).await.unwrap();
        assert!(!first.stale);
        let offline = submitter.account_tins(0).await.unwrap();
        assert!(offline.stale);
        assert_eq!(offline.rows, first.rows);
        assert_eq!(offline.fetched_at, first.fetched_at);
        // Past the limit the list is too old to answer.
        submitter.tins.age_for_tests(STALE_LIMIT_SECS + 1);
        assert_eq!(submitter.account_tins(0).await, Err(TinsError::Offline));
    }

    /// A read whose time is ahead of now (the clock was set back) is of no
    /// known age: it is asked again, a failure does not hold requests off,
    /// and the list is not answered offline.
    #[tokio::test]
    async fn a_read_from_ahead_of_now_is_expired() {
        let (url, seen) = fake_server(0, |n, _| match n {
            1 => None,
            _ => {
                ok(json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X"}]}))
            }
        });
        let submitter = submitter(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        submitter.tins.age_for_tests(-86_400);
        assert_eq!(
            submitter.account_tins(60).await,
            Err(TinsError::Offline),
            "a list from ahead of now is neither answered from memory nor offline"
        );
        assert_eq!(seen.lock().unwrap().len(), 2);
        submitter.tins.age_for_tests(-86_400);
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            seen.lock().unwrap().len(),
            3,
            "a failure from ahead of now does not hold requests off"
        );
    }

    #[tokio::test]
    async fn an_unreachable_server_reads_offline() {
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        let submitter = submitter(&format!("http://{addr}"), true);
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::Offline));
    }

    #[tokio::test]
    async fn account_tins_signed_out_makes_no_request() {
        let (url, seen) = fake_server(0, |_, _| ok(json!({"profiles":[]})));
        let submitter = submitter(&url, false);
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::SignedOut));
        assert!(submitter.cached_account_tins().is_none());
        assert!(seen.lock().unwrap().is_empty());
    }

    /// Signing out forgets the list (the host calls `forget_device` on
    /// sign-out and sign-in): signed in again as the same login, with no
    /// read in between, the next read asks the server.
    #[tokio::test]
    async fn sign_out_clears_the_cached_list() {
        let (url, seen) = fake_server(0, |_, _| ok(json!({"include":"all","profiles":[]})));
        let (submitter, tokens) = submitter_tokens(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        assert!(submitter.cached_account_tins().is_some());
        tokens.sign_in(None);
        submitter.forget_device().unwrap();
        assert!(submitter.cached_account_tins().is_none());
        // The same login signs in again.
        tokens.sign_in(Some(LOGIN));
        submitter.forget_device().unwrap();
        assert!(submitter.cached_account_tins().is_none());
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(list_reads(&seen), 2, "a fresh read after signing in again");
        tokens.sign_in(None);
        submitter.forget_device().unwrap();
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::SignedOut));
    }

    /// A new sign-in in the host, another login or the same one, forgets
    /// the list.
    #[tokio::test]
    async fn a_new_sign_in_clears_the_cached_list() {
        let (url, seen) = fake_server(0, |_, _| ok(json!({"include":"all","profiles":[]})));
        let (submitter, tokens) = submitter_tokens(&url, true);
        let sign_in = |login: &str| {
            tokens.sign_in(Some(login));
            submitter.forget_device().unwrap();
        };
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(list_reads(&seen), 1);

        sign_in("pedro@example.com");
        assert!(submitter.cached_account_tins().is_none());
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(list_reads(&seen), 2, "another login reads its own list");

        sign_in("pedro@example.com");
        assert!(submitter.cached_account_tins().is_none());
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            list_reads(&seen),
            3,
            "the same login signing in again reads anew"
        );
    }

    /// A 401 `invalid_token` is asked once more with a token the host
    /// refreshed (`force_refresh`), and only once.
    #[tokio::test]
    async fn an_expired_token_is_refreshed_and_the_list_asked_once_more() {
        let (url, seen) = fake_server(0, |n, _| match n {
            0 => Some((
                401,
                r#"{"error":{"code":"invalid_token","message":"expired"}}"#.into(),
            )),
            _ => ok(json!({"include":"all","profiles":[]})),
        });
        let (submitter, tokens) = submitter_tokens(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "GET /api/tax-profiles?include=all",
                "GET /api/tax-profiles?include=all"
            ]
        );
        assert_eq!(tokens.asked(), [false, true]);

        let (url, seen) = fake_server(0, |_, _| {
            Some((
                401,
                r#"{"error":{"code":"invalid_token","message":"expired"}}"#.into(),
            ))
        });
        let (submitter, tokens) = submitter_tokens(&url, true);
        // A second `invalid_token` after the refresh: the grant was revoked
        // on the server, which is signed out, not a server failure.
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::SignedOut));
        assert_eq!(seen.lock().unwrap().len(), 2, "refreshed once, not again");
        assert_eq!(tokens.asked(), [false, true]);
    }

    /// The host's `SignedOut` is the list's: no request.
    #[tokio::test]
    async fn the_hosts_signed_out_is_no_list_and_no_request() {
        let (url, seen) = fake_server(0, |_, _| ok(json!({"include":"all","profiles":[]})));
        let (submitter, tokens) = submitter_tokens(&url, true);
        tokens.refuse_tokens();
        assert_eq!(submitter.account_tins(60).await, Err(TinsError::SignedOut));
        assert!(seen.lock().unwrap().is_empty());
    }

    /// A send the server refuses for its TIN makes the next read ask; one
    /// that fails otherwise does not.
    #[tokio::test]
    async fn a_send_refused_for_its_tin_makes_the_next_read_ask() {
        const FILE: &str = "00012345300000-1701Qv2018-2026Q1#juana@example.com#.xml";
        for (ticket, invalidates) in [
            (
                (
                    403,
                    r#"{"error":{"code":"tin_paused","message":"Sending is paused for this TIN."}}"#,
                ),
                true,
            ),
            (
                (
                    500,
                    r#"{"error":{"code":"internal","message":"try again later"}}"#,
                ),
                false,
            ),
        ] {
            let (public, trust, hpke) = trust_documents();
            let (url, seen) = fake_server(0, move |_, path| match path {
                "/.well-known/buwiz-trust.json" => Some((200, trust.clone())),
                "/.well-known/buwiz-hpke.json" => Some((200, hpke.clone())),
                "/api/tickets" => Some((ticket.0, ticket.1.to_owned())),
                _ => ok(json!({"include":"all","profiles":[]})),
            });
            let store = SharedStore::default();
            store.set("device_id", "d").unwrap();
            let submitter = submitter_with(&url, store, FakeTokens::new(Some(LOGIN)), vec![public]);
            assert!(submitter.account_tins(60).await.is_ok());
            let sent = submitter.send(FILE, b"<xml/>").await;
            let Err(error) = sent else {
                panic!("the ticket was refused");
            };
            assert!(matches!(error, crate::SendError::NotSent(_)), "{error:?}");
            assert_eq!(error.refusal().is_some(), invalidates, "{error:?}");
            assert!(submitter.account_tins(60).await.is_ok());
            assert_eq!(
                list_reads(&seen),
                if invalidates { 2 } else { 1 },
                "{error:?}"
            );
        }
    }

    /// A refusal at session open (here a banned account registering its
    /// device) makes the next read ask too.
    #[tokio::test]
    async fn a_refusal_at_session_open_makes_the_next_read_ask() {
        let (public, trust, _) = trust_documents();
        let (url, seen) = fake_server(0, move |_, path| match path {
            "/.well-known/buwiz-trust.json" => Some((200, trust.clone())),
            "/api/devices" => Some((
                403,
                r#"{"error":{"code":"account_banned","message":"This Buwiz account is banned."}}"#
                    .into(),
            )),
            _ => ok(json!({"include":"all","profiles":[]})),
        });
        // Signed in, no device registered yet.
        let store = SharedStore::default();
        let submitter = submitter_with(&url, store, FakeTokens::new(Some(LOGIN)), vec![public]);
        assert!(submitter.account_tins(60).await.is_ok());
        let ready = submitter.ready().await;
        assert_eq!(
            ready.as_ref().err().and_then(ClientError::tin_refusal),
            Some(TinRefusal::AccountBanned),
            "{ready:?}"
        );
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(list_reads(&seen), 2);
    }

    /// Pages that look like the account site but are not, and the account
    /// site with something odd about it.
    const LOOKALIKES: [&str; 14] = [
        "https://account.buwiz.com.evil.example/tins/claims/c1/fix",
        "https://account.buwiz.com@evil.example/tins/claims/c1/fix",
        "https://evil.example/tins/claims/c1/fix",
        "https://evil.example\\@account.buwiz.com/tins",
        "http://account.buwiz.com/tins/claims/c1/fix",
        "https://user@account.buwiz.com/tins/claims/c1/fix",
        "https://user:secret@account.buwiz.com/tins/claims/c1/fix",
        "https://account.buwiz.com:8443/tins/claims/c1/fix",
        "https://account.buwiz.com./tins/claims/c1/fix",
        "https://xaccount.buwiz.com/tins",
        "https://buwiz.com/tins",
        "//account.buwiz.com/tins",
        "javascript:alert(1)",
        "account.buwiz.com/tins",
    ];

    #[test]
    fn only_a_page_of_the_account_site_is_kept_as_a_rows_link() {
        for url in LOOKALIKES {
            assert_eq!(pinned_account_url(url), None, "{url}");
            let body = json!({"include":"all","profiles":[{
                "tin":"000-123-457-00000","registered_name":"X","status":"needs_fix","can_send":false,
                "action":{"label":"Fix","url":url}
            }]});
            assert_eq!(
                AccountTins::parse(&body, 1).rows[0].action_url,
                None,
                "{url}"
            );
        }
        for (url, kept) in [
            (
                "https://account.buwiz.com/tins/claims/c1/fix",
                "https://account.buwiz.com/tins/claims/c1/fix",
            ),
            (
                "https://account.buwiz.com:443/tins/claims/c1/verify",
                "https://account.buwiz.com/tins/claims/c1/verify",
            ),
            (
                "https://ACCOUNT.buwiz.com/tins",
                "https://account.buwiz.com/tins",
            ),
        ] {
            assert_eq!(pinned_account_url(url).as_deref(), Some(kept), "{url}");
        }
        assert!(ACCOUNT_SITE.starts_with("https://"));
    }

    /// A debug build's `BUWIZ_ACCOUNT_URL` is taken only when it is https
    /// or http to this machine.
    #[test]
    fn the_account_site_override_must_be_https_or_this_machine() {
        assert_eq!(account_site_from(None), ACCOUNT_SITE);
        for kept in [
            "http://localhost:3010",
            "http://127.0.0.1:3010/",
            "https://account.lab.example",
        ] {
            assert_eq!(
                account_site_from(Some(kept)),
                kept.trim_end_matches('/'),
                "{kept}"
            );
        }
        for refused in [
            "http://account.example",
            "ftp://localhost",
            "",
            "localhost:3010",
            "http://localhost.example",
        ] {
            assert_eq!(
                account_site_from(Some(refused)),
                ACCOUNT_SITE,
                "{refused:?}"
            );
        }
    }

    #[test]
    fn debug_output_never_shows_a_tin() {
        let body: Value =
            serde_json::from_str(include_str!("../tests/fixtures/tax_profiles_all.json")).unwrap();
        let list = AccountTins::parse(&body, 1);
        assert_eq!(list.rows.len(), 5);
        let shown = format!(
            "{list:?} {:?} {:?}",
            TinsError::Server {
                code: "x".into(),
                message: "TIN 000-123-453 SYNTHETIC TAXPAYER ONE".into()
            },
            Err::<AccountTins, _>(TinsError::Banned)
        );
        for secret in ["123", "453", "SYNTHETIC", "daylight", "claims"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
    }

    /// A row without `next` (a server from before it), or with a word this
    /// app does not know, reads it from `can_send` and `action`; a row's own
    /// `next` is taken as it is.
    #[test]
    fn row_without_next_falls_back() {
        let action = json!({"label":"Verify to send","url":"https://account.buwiz.com/tins/claims/c1/verify"});
        let rows = json!({"include":"all","profiles":[
            {"tin":"000-123-453-00000","registered_name":"A","status":"approved","can_send":true},
            {"tin":"000-123-455-00000","registered_name":"B","status":"unverified","can_send":false,"action":action},
            {"tin":"000-123-456-00000","registered_name":"C","status":"pending","can_send":false},
            {"tin":"000-123-454-00002","registered_name":"D","status":"paused","can_send":false,"action":null},
            {"tin":"000-123-457-00000","registered_name":"E","status":"needs_fix","can_send":false,"next":"later","action":action},
            {"tin":"000-123-458-00000","registered_name":"F","status":"approved","can_send":true,"next":"later"},
            {"tin":"000-123-459-00000","registered_name":"G","status":"pending","can_send":false,"next":"fix"},
            {"tin":"000-123-460-00000","registered_name":"H","status":"approved","can_send":true,"next":"wait"}
        ]});
        let list = AccountTins::parse(&rows, 1);
        let next: Vec<Next> = list.rows.iter().map(|row| row.next).collect();
        assert_eq!(
            next,
            [
                Next::Send,
                Next::Fix,
                Next::Wait,
                Next::Wait,
                Next::Fix,
                Next::Send,
                Next::Fix,
                Next::Wait
            ]
        );
        // An action link off the account site is dropped, but the row still
        // has an action: it is Fix.
        let off_site = json!({"include":"all","profiles":[
            {"tin":"000-123-455-00000","registered_name":"B","status":"unverified","can_send":false,
             "action":{"label":"Verify","url":"https://evil.example/verify"}}
        ]});
        let row = &AccountTins::parse(&off_site, 1).rows[0];
        assert_eq!(row.next, Next::Fix);
        assert_eq!(row.action_url, None);
    }

    /// The golden answer of the server's pull request carries `next`, the
    /// row's `message` and `test_only`.
    #[test]
    fn the_golden_rows_read_next_message_and_test_only() {
        let body: Value =
            serde_json::from_str(include_str!("../tests/fixtures/tax_profiles_all.json")).unwrap();
        let list = AccountTins::parse(&body, 1);
        let next: Vec<Next> = list.rows.iter().map(|row| row.next).collect();
        assert_eq!(
            next,
            [Next::Send, Next::Wait, Next::Fix, Next::Wait, Next::Fix]
        );
        for row in &list.rows {
            assert_eq!(row.next == Next::Fix, row.action_url.is_some(), "{row:?}");
            assert_eq!(row.next == Next::Send, row.can_send, "{row:?}");
            assert!(row.message.is_some(), "{row:?}");
            assert!(!row.test_only, "{row:?}");
        }
        let test = json!({"include":"all","profiles":[
            {"tin":"111-111-114-00000","registered_name":"T","status":"approved","can_send":true,"next":"send","test_only":true}
        ]});
        let row = &AccountTins::parse(&test, 1).rows[0];
        assert!(row.test_only && row.can_send);
        assert_eq!(row.status, TinStatus::Approved);
    }

    fn limited(seconds: u64) -> Reply {
        Some((
            429,
            json!({"error":{"code":"rate_limited","message":"Too many requests. Try again in 5 minutes."}})
                .to_string(),
            vec![("retry-after", seconds.to_string())],
        ))
    }

    /// A `429` keeps the last list: until `Retry-After` nothing asks the
    /// server, not even `max_age` 0 (Queue), and the list is answered
    /// marked stale; after it, the next read asks.
    #[tokio::test]
    async fn rate_limited_keeps_the_list_and_waits_retry_after() {
        let (url, seen, _) = fake_server_with_headers(0, |n, _| {
            match n {
            0 => Some((
                200,
                json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X","can_send":true,"next":"send"}]}).to_string(),
                Vec::new(),
            )),
            1 => limited(300),
            _ => Some((
                200,
                json!({"include":"all","profiles":[
                    {"tin":"000123453","registered_name":"X","can_send":true,"next":"send"},
                    {"tin":"000123455","registered_name":"Y","status":"unverified","can_send":false,"next":"fix"}
                ]})
                .to_string(),
                Vec::new(),
            )),
        }
        });
        let submitter = submitter(&url, true);
        let first = submitter.account_tins(60).await.unwrap();
        assert_eq!(first.rows.len(), 1);

        let kept = submitter.account_tins(0).await.unwrap();
        assert!(kept.stale, "the kept list is marked stale");
        assert_eq!(kept.rows, first.rows);
        assert_eq!(kept.fetched_at, first.fetched_at);
        assert_eq!(list_reads(&seen), 2);

        for max_age in [0, 15, 60, 300] {
            let kept = submitter.account_tins(max_age).await.unwrap();
            assert!(kept.stale);
            assert_eq!(kept.rows, first.rows);
        }
        assert!(submitter.refresh_account_tins().await.unwrap().stale);
        assert_eq!(
            list_reads(&seen),
            2,
            "nothing asks before Retry-After, whatever max_age"
        );
        // Still held well past the 30 seconds a failure is kept, and past
        // the 10 minutes a list is answered offline.
        submitter.tins.age_for_tests(200);
        assert!(submitter.account_tins(0).await.unwrap().stale);
        assert_eq!(list_reads(&seen), 2);

        // A refusal seen on a send: the kept list is no longer answered, and
        // the server is still not asked.
        submitter.invalidate_account_tins();
        let read = submitter.account_tins(0).await;
        let Err(TinsError::RateLimited { until }) = read else {
            panic!("{read:?}");
        };
        assert!((until - device::now() - 100).abs() <= 5, "{until}");
        assert_eq!(list_reads(&seen), 2);

        submitter.tins.age_for_tests(101);
        let fresh = submitter.account_tins(0).await.unwrap();
        assert!(!fresh.stale);
        assert_eq!(fresh.rows.len(), 2);
        assert_eq!(
            list_reads(&seen),
            3,
            "after Retry-After the server is asked"
        );
    }

    /// A `429` with no list read before: the error, and still no request
    /// before `Retry-After`. A new sign-in is a new token: it asks.
    #[tokio::test]
    async fn rate_limited_with_no_list_waits_too() {
        let (url, seen, _) = fake_server_with_headers(0, |_, _| limited(120));
        let (submitter, tokens) = submitter_tokens(&url, true);
        for _ in 0..3 {
            assert!(matches!(
                submitter.account_tins(0).await,
                Err(TinsError::RateLimited { .. })
            ));
        }
        assert_eq!(list_reads(&seen), 1);
        tokens.sign_in(Some("pedro@example.com"));
        submitter.forget_device().unwrap();
        assert!(submitter.account_tins(0).await.is_err());
        assert_eq!(list_reads(&seen), 2);
    }

    /// The row `POST /api/tax-profiles` answers for an added TIN.
    fn added_row(tin: &str) -> String {
        json!({"profile":{
            "id":"c9","tin":tin,"tin_masked":"***-***-453-00000","registered_name":"Dela Cruz Juan",
            "activated_at":null,"permissions":["profile.view","filing.prepare"],"status":"unverified",
            "can_send":false,"next":"fix","message":"Not verified. Verify it with its papers to send returns.",
            "action":{"label":"Verify to send","url":"https://account.buwiz.com/tins/claims/c9/verify"}
        }})
        .to_string()
    }

    /// The kind is `personal` or `business`, the TIN its 14 digits with the
    /// branch, the names trimmed and a trade name only when there is one.
    #[tokio::test]
    async fn post_maps_kind_and_sends_14_digits() {
        let (url, seen, bodies) = fake_server_with_headers(0, |_, path| match path {
            "/api/tax-profiles" => Some((201, added_row("000-123-453-00000"), Vec::new())),
            _ => Some((
                200,
                json!({"include":"all","profiles":[]}).to_string(),
                Vec::new(),
            )),
        });
        let submitter = submitter(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        let added = submitter
            .add_account_tin(
                TinKind::Individual,
                &key("000123453"),
                "  Dela Cruz Juan ",
                None,
            )
            .await
            .unwrap()
            .expect("the row");
        assert_eq!(added.status, TinStatus::Unverified);
        assert_eq!(added.next, Next::Fix);
        assert_eq!(
            added.action_url.as_deref(),
            Some("https://account.buwiz.com/tins/claims/c9/verify")
        );
        submitter
            .add_account_tin(
                TinKind::NonIndividual,
                &key("000-123-453-002"),
                "Dela Cruz Trading Corp.",
                Some(" Juan's Store "),
            )
            .await
            .unwrap();
        submitter
            .add_account_tin(
                TinKind::NonIndividual,
                &key("00012345300003"),
                "X Corp",
                Some("  "),
            )
            .await
            .unwrap();
        submitter
            .add_account_tin(
                TinKind::Individual,
                &key("000123453"),
                "Dela Cruz Juan",
                Some("Juan's"),
            )
            .await
            .unwrap();
        let requests = seen.lock().unwrap().clone();
        let bodies = bodies.lock().unwrap().clone();
        let posts: Vec<Value> = requests
            .iter()
            .zip(&bodies)
            .filter(|(request, _)| request.as_str() == "POST /api/tax-profiles")
            .map(|(_, body)| serde_json::from_str(body).unwrap())
            .collect();
        assert_eq!(
            posts,
            [
                json!({"kind":"personal","tin":"000-123-453-00000","registered_name":"Dela Cruz Juan"}),
                json!({"kind":"business","tin":"000-123-453-00002","registered_name":"Dela Cruz Trading Corp.","trade_name":"Juan's Store"}),
                json!({"kind":"business","tin":"000-123-453-00003","registered_name":"X Corp"}),
                json!({"kind":"personal","tin":"000-123-453-00000","registered_name":"Dela Cruz Juan","trade_name":"Juan's"}),
            ]
        );
        // An add changes the list: the next read asks.
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(list_reads(&seen), 2);
    }

    /// Every refusal of `POST /api/tax-profiles` is its own case in the
    /// app's own words; the three that say the account holds the TIN make
    /// the next list read ask.
    /// A refusal's status, code and `Retry-After`; the error, its words and
    /// whether the list is read again.
    type RefusalCase = (
        u16,
        &'static str,
        Option<u64>,
        AddTinError,
        &'static str,
        bool,
    );

    #[tokio::test]
    async fn each_refusal_has_its_own_words() {
        let cases: [RefusalCase; 14] = [
            (
                400,
                "bad_tin",
                None,
                AddTinError::BadTin,
                "This TIN is not valid.",
                false,
            ),
            (
                400,
                "name_missing",
                None,
                AddTinError::NameMissing,
                "Add the registered name first.",
                false,
            ),
            (
                400,
                "name_too_long",
                None,
                AddTinError::NameTooLong,
                "This name is too long. Shorten it and try again.",
                false,
            ),
            (
                400,
                "test_tin",
                None,
                AddTinError::TestTin,
                "Test TINs are added by Buwiz.",
                false,
            ),
            (
                409,
                "tin_already_active",
                None,
                AddTinError::TinAlreadyActive,
                "This TIN is already activated on your account.",
                true,
            ),
            (
                409,
                "tin_already_added",
                None,
                AddTinError::TinAlreadyAdded,
                "This TIN is already on your account, waiting to be verified.",
                true,
            ),
            (
                409,
                "claim_already_pending",
                None,
                AddTinError::ClaimAlreadyPending,
                "Buwiz is checking this TIN.",
                true,
            ),
            (
                409,
                "too_many_unverified",
                None,
                AddTinError::TooManyUnverified,
                "Your account has 20 TINs waiting to be verified. Verify or remove one on your account first.",
                false,
            ),
            (
                429,
                "rate_limited",
                Some(1500),
                AddTinError::RateLimited { retry_after: 1500 },
                "Too many tries. Try again in 25 minutes.",
                false,
            ),
            (
                429,
                "rate_limited",
                Some(30),
                AddTinError::RateLimited { retry_after: 30 },
                "Too many tries. Try again in 1 minute.",
                false,
            ),
            (
                400,
                "bad_request",
                None,
                AddTinError::BadRequest,
                "Buwiz could not read this request. Update eBIRForms and try again.",
                false,
            ),
            (
                403,
                "insufficient_scope",
                None,
                AddTinError::Forbidden,
                "This sign-in cannot add TINs. Sign out of Buwiz and sign in again.",
                false,
            ),
            (
                403,
                "account_banned",
                None,
                AddTinError::Banned,
                "This Buwiz account is banned.",
                false,
            ),
            (
                500,
                "internal",
                None,
                AddTinError::Other {
                    code: "internal".into(),
                    message: "something went wrong".into(),
                },
                "The Buwiz server could not add this TIN (internal).",
                false,
            ),
        ];
        let mut words = std::collections::HashSet::new();
        for (status, code, retry_after, expected, sentence, refreshes) in cases {
            let (url, seen, _) = fake_server_with_headers(0, move |_, path| match path {
                "/api/tax-profiles" => Some((
                    status,
                    json!({"error":{"code":code,"message":"something went wrong"}}).to_string(),
                    retry_after
                        .map(|seconds| vec![("retry-after", seconds.to_string())])
                        .unwrap_or_default(),
                )),
                _ => Some((
                    200,
                    json!({"include":"all","profiles":[]}).to_string(),
                    Vec::new(),
                )),
            });
            let submitter = submitter(&url, true);
            assert!(submitter.account_tins(60).await.is_ok());
            let refused = submitter
                .add_account_tin(
                    TinKind::Individual,
                    &key("000123453"),
                    "Dela Cruz Juan",
                    None,
                )
                .await
                .unwrap_err();
            match (&refused, &expected) {
                // The wait is counted from the answer: a second may pass.
                (
                    AddTinError::RateLimited { retry_after: got },
                    AddTinError::RateLimited { retry_after: want },
                ) => assert!((want - got).abs() <= 2, "{code}: {got}"),
                _ => assert_eq!(refused, expected, "{code}"),
            }
            assert_eq!(refused.to_string(), sentence, "{code}");
            assert_eq!(refused.refreshes_list(), refreshes, "{code}");
            assert!(submitter.account_tins(60).await.is_ok());
            assert_eq!(list_reads(&seen), if refreshes { 2 } else { 1 }, "{code}");
            if !matches!(refused, AddTinError::RateLimited { .. }) {
                assert!(words.insert(sentence), "{code}: its own words");
            }
        }
    }

    /// A name over 200 bytes after trimming is refused at once, as too long,
    /// counting bytes as the server does: 150 letters `Ñ` are 300 bytes. It
    /// is never "add the name". An empty name is "add the name", at once
    /// too. Neither asks the server; 200 bytes exactly is sent.
    #[tokio::test]
    async fn a_long_multibyte_name_is_too_long_and_never_missing() {
        let (url, seen, bodies) = fake_server_with_headers(0, |_, _| {
            Some((201, added_row("000-123-453-00000"), Vec::new()))
        });
        let submitter = submitter(&url, true);
        let long = "Ñ".repeat(150);
        assert!(long.chars().count() < MAX_NAME_BYTES && long.len() > MAX_NAME_BYTES);
        for (registered, trade) in [
            (long.as_str(), None),
            ("Dela Cruz Trading", Some(long.as_str())),
        ] {
            let refused = submitter
                .add_account_tin(TinKind::NonIndividual, &key("000123453"), registered, trade)
                .await
                .unwrap_err();
            assert_eq!(refused, AddTinError::NameTooLong);
            assert_ne!(refused, AddTinError::NameMissing);
            assert_eq!(
                refused.to_string(),
                "This name is too long. Shorten it and try again."
            );
            assert_ne!(refused.to_string(), "Add the registered name first.");
        }
        for empty in ["", "   "] {
            assert_eq!(
                submitter
                    .add_account_tin(TinKind::Individual, &key("000123453"), empty, None)
                    .await,
                Err(AddTinError::NameMissing)
            );
        }
        assert!(seen.lock().unwrap().is_empty(), "no request");
        let exactly = format!("  {}  ", "ñ".repeat(100));
        assert!(
            submitter
                .add_account_tin(TinKind::Individual, &key("000123453"), &exactly, None)
                .await
                .is_ok()
        );
        let body: Value = serde_json::from_str(&bodies.lock().unwrap()[0]).unwrap();
        assert_eq!(body["registered_name"].as_str().unwrap().len(), 200);
    }

    /// A grant revoked on the server answers `invalid_token` again after
    /// the forced refresh: the add is signed out, not "could not add".
    #[tokio::test]
    async fn a_revoked_grant_adds_as_signed_out() {
        let (url, seen) = fake_server(0, |_, _| {
            Some((
                401,
                r#"{"error":{"code":"invalid_token","message":"revoked"}}"#.into(),
            ))
        });
        let (submitter, tokens) = submitter_tokens(&url, true);
        assert_eq!(
            submitter
                .add_account_tin(TinKind::Individual, &key("000123453"), "X", None)
                .await,
            Err(AddTinError::SignedOut)
        );
        assert_eq!(seen.lock().unwrap().len(), 2, "refreshed once, not again");
        assert_eq!(tokens.asked(), [false, true]);
    }

    /// A POST that gets no answer may still have added the TIN: the next
    /// read asks the server instead of answering the list from before.
    #[tokio::test]
    async fn an_add_with_no_answer_makes_the_next_read_ask() {
        let (url, seen) = fake_server(0, |n, _| match n {
            0 | 2 => {
                ok(json!({"include":"all","profiles":[{"tin":"000123453","registered_name":"X"}]}))
            }
            _ => None,
        });
        let submitter = submitter(&url, true);
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            submitter
                .add_account_tin(TinKind::Individual, &key("000123462"), "Y", None)
                .await,
            Err(AddTinError::Offline)
        );
        assert!(submitter.account_tins(60).await.is_ok());
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "GET /api/tax-profiles?include=all",
                "POST /api/tax-profiles",
                "GET /api/tax-profiles?include=all"
            ],
            "the read after the unanswered add asked the server"
        );
    }

    #[tokio::test]
    async fn adding_signed_out_makes_no_request() {
        let (url, seen) = fake_server(0, |_, _| ok(json!({})));
        let submitter = submitter(&url, false);
        assert_eq!(
            submitter
                .add_account_tin(TinKind::Individual, &key("000123453"), "X", None)
                .await,
            Err(AddTinError::SignedOut)
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    /// End to end against a local buwiz-com (TRACK-D-CONTRACT.md), never by
    /// default: `BUWIZ_TRACKD_E2E_URL=http://127.0.0.1:3028
    /// BUWIZ_TRACKD_E2E_LOG=<its server.log> cargo test -p bir-submit --lib
    /// trackd_e2e_against_a_local_server -- --ignored`. Only a server on this
    /// computer is taken. It signs a new `example.test` address in with the
    /// email code (read from the server's log, `MAIL_TRANSPORT=log`), the
    /// way bir-core's `email_sign_in` does: bir-submit never signs in and
    /// cannot use that crate. `BUWIZ_TRACKD_E2E_TOKEN` skips the sign-in;
    /// `BUWIZ_TRACKD_E2E_ACCOUNT_URL` is the server's account site
    /// (`http://account.localhost:3028` by default).
    #[tokio::test]
    #[ignore = "needs a local buwiz-com: BUWIZ_TRACKD_E2E_URL"]
    async fn trackd_e2e_against_a_local_server() {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use sha2::{Digest, Sha256};

        let url = std::env::var("BUWIZ_TRACKD_E2E_URL")
            .expect("BUWIZ_TRACKD_E2E_URL")
            .trim_end_matches('/')
            .to_owned();
        let host = url::Url::parse(&url).unwrap();
        assert!(
            matches!(host.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
            "only a server on this computer: {url}"
        );
        let account_site = std::env::var("BUWIZ_TRACKD_E2E_ACCOUNT_URL")
            .unwrap_or_else(|_| "http://account.localhost:3028".into());
        let http = reqwest::Client::new();
        let tag = crate::device::hex(&rand::random::<[u8; 4]>());
        let email = format!("trackd-{tag}@example.test");

        let token = match std::env::var("BUWIZ_TRACKD_E2E_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                let log = std::env::var("BUWIZ_TRACKD_E2E_LOG").expect("BUWIZ_TRACKD_E2E_LOG");
                let secret = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
                let started: Value = http
                    .post(format!("{url}/auth/email/start"))
                    .json(&json!({
                        "client_id": "buwiz-desktop",
                        "email": email,
                        "intent": "create",
                        "secret_hash": URL_SAFE_NO_PAD.encode(Sha256::digest(secret.as_bytes())),
                        "platform": "macos",
                        "install_id": format!("install-id-of-trackd-e2e-{tag}"),
                    }))
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                let request_id = started["request_id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{started}"));
                let mut code = None;
                for _ in 0..50 {
                    let text = std::fs::read_to_string(&log).unwrap_or_default();
                    code = text
                        .lines()
                        .rev()
                        .find(|line| line.contains(&email) && line.contains("sign-in code"))
                        .and_then(|line| {
                            let line: String = line
                                .split('\u{1b}')
                                .map(|part| part.split_once('m').map_or(part, |(_, rest)| rest))
                                .collect();
                            let after = &line[line.find("code=")? + 5..];
                            let digits: String = after
                                .trim_start_matches('"')
                                .chars()
                                .take_while(char::is_ascii_digit)
                                .collect();
                            (!digits.is_empty()).then_some(digits)
                        });
                    if code.is_some() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                let code = code.expect("the sign-in code in the server's log");
                let tokens: Value = http
                    .post(format!("{url}/auth/email/verify"))
                    .json(&json!({"request_id": request_id, "secret": secret, "code": code}))
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                assert_eq!(tokens["scope"], "app", "{tokens}");
                tokens["access_token"].as_str().unwrap().to_owned()
            }
        };
        /// The token the sign-in gave, as the host would hand it.
        struct Signed(String, String);
        impl crate::TokenSource for Signed {
            fn access_token(
                &self,
                _force_refresh: bool,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, ClientError>> + Send + '_>,
            > {
                let token = self.0.clone();
                Box::pin(async move { Ok(token) })
            }
            fn login(&self) -> Option<String> {
                Some(self.1.clone())
            }
            fn session_ended(&self) {}
        }
        let client = Submitter::new(
            SubmitConfig {
                server_url: url.clone(),
                device_name: "trackd e2e".into(),
                trust_keys: Vec::new(),
                install_id: None,
                platform: None,
            },
            Box::new(SharedStore::default()),
            Arc::new(Signed(token.clone(), email.clone())),
        );
        let raw_get = || {
            http.get(format!("{url}/api/tax-profiles?include=all"))
                .bearer_auth(&token)
                .send()
        };
        let raw_post = |body: Value| {
            http.post(format!("{url}/api/tax-profiles"))
                .bearer_auth(&token)
                .json(&body)
                .send()
        };
        let invariants = |body: &Value| {
            let rows = body["profiles"].as_array().expect("profiles");
            for row in rows {
                let next = row["next"]
                    .as_str()
                    .unwrap_or_else(|| panic!("next on {row}"));
                assert_eq!(next == "fix", !row["action"].is_null(), "{row}");
                assert_eq!(next == "send", row["can_send"] == true, "{row}");
            }
            rows.len()
        };

        // GET ?include=all: a new account lists nothing, completely.
        let list = client.account_tins(0).await.unwrap();
        assert!(list.complete && list.rows.is_empty(), "{list:?}");

        // POST a new TIN: 201, unverified, Fix, linking to the account site.
        let root = format!(
            "2{:08}",
            u32::from_le_bytes(rand::random::<[u8; 4]>()) % 100_000_000
        );
        let tin = key(&format!("{root}00000"));
        let added = client
            .add_account_tin(TinKind::Individual, &tin, "Dela Cruz Juan", None)
            .await
            .unwrap()
            .expect("the added row");
        assert_eq!(added.status, TinStatus::Unverified);
        assert_eq!(added.next, Next::Fix);
        assert!(!added.can_send && !added.test_only);
        let answer: Value = raw_get().await.unwrap().json().await.unwrap();
        assert_eq!(answer["include"], "all");
        assert_eq!(invariants(&answer), 1);
        let row = &answer["profiles"][0];
        let link = row["action"]["url"].as_str().unwrap();
        assert!(
            link.starts_with(&format!("{account_site}/tins/claims/")) && link.ends_with("/verify"),
            "{link}"
        );
        let list = client.account_tins(0).await.unwrap();
        assert!(list.complete);
        let listed = list.find(&tin).unwrap();
        assert_eq!(
            (listed.status.clone(), listed.next),
            (TinStatus::Unverified, Next::Fix)
        );

        // The refusals this account can reach.
        assert_eq!(
            client
                .add_account_tin(TinKind::Individual, &tin, "Dela Cruz Juan", None)
                .await,
            Err(AddTinError::TinAlreadyAdded)
        );
        assert_eq!(
            client
                .add_account_tin(
                    TinKind::Individual,
                    &key("111111114"),
                    "Dela Cruz Juan",
                    None
                )
                .await,
            Err(AddTinError::TestTin)
        );
        let refusal = |answer: Value| {
            answer["error"]["code"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        let bad: Value = raw_post(json!({"kind":"personal","tin":"12345","registered_name":"X"}))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(refusal(bad), "bad_tin");
        let empty: Value = raw_post(
            json!({"kind":"personal","tin":format!("{root}00001"),"registered_name":"   "}),
        )
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(refusal(empty), "name_missing");
        let long: Value = raw_post(
            json!({"kind":"personal","tin":format!("{root}00001"),"registered_name":"Ñ".repeat(150)}),
        )
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        match refusal(long).as_str() {
            "name_too_long" => eprintln!("name_too_long: answered"),
            "name_missing" => {
                eprintln!("name_too_long: EXPECTED-MISSING (this server answers name_missing)")
            }
            other => panic!("a long name answered {other}"),
        }

        // The add limit: 10 tries per sign-in an hour, then 429 with Retry-After.
        let mut limited = None;
        for _ in 0..12 {
            let response =
                raw_post(json!({"kind":"personal","tin":tin_text(&tin),"registered_name":"X"}))
                    .await
                    .unwrap();
            if response.status() == 429 {
                limited = Some(response);
                break;
            }
        }
        let limited = limited.expect("a 429 within 10 adds");
        let retry_after: i64 = limited.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=3600).contains(&retry_after), "{retry_after}");
        let body: Value = limited.json().await.unwrap();
        assert_eq!(refusal(body), "rate_limited");
        let refused = client
            .add_account_tin(TinKind::Individual, &tin, "X", None)
            .await
            .unwrap_err();
        assert!(
            matches!(refused, AddTinError::RateLimited { .. }),
            "{refused:?}"
        );
        assert!(
            refused
                .to_string()
                .starts_with("Too many tries. Try again in ")
        );

        // The read limit: 120 per sign-in in 10 minutes, then 429; the client keeps its list.
        let kept = client.account_tins(0).await.unwrap();
        let mut read_limited = None;
        for _ in 0..125 {
            let response = raw_get().await.unwrap();
            if response.status() == 429 {
                read_limited = Some(response);
                break;
            }
        }
        let read_limited = read_limited.expect("a 429 within 120 reads");
        assert!(read_limited.headers().contains_key("retry-after"));
        let body: Value = read_limited.json().await.unwrap();
        assert_eq!(body["error"]["code"], "rate_limited");
        let stale = client.account_tins(0).await.unwrap();
        assert!(stale.stale, "the kept list, marked stale");
        assert_eq!(stale.rows, kept.rows);
    }

    fn tin_text(tin: &TinKey) -> String {
        dashed(tin)
    }
}
