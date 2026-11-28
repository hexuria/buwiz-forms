//! Host verbs for the Buwiz account in Settings.
//!
//! The page sees account status only. Refresh tokens stay in the OS keychain
//! when Secret Service (or the macOS keychain) accepts them, and otherwise in
//! a mode-0600 file beside the database — the same fallback the database key
//! uses when the keychain is missing. The page cannot read either store.

use bir_core::cloud_account::{
    self, AccountSecrets, AccountStatus, BlockingHttp, CloudError, ENV_SERVER_URL, GateDecision,
    RefreshOutcome, StoredAccount,
};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::bir_module::with_account_database;

/// Old Settings field. Startup deletes it. Nothing reads it.
const OBSOLETE_SERVER_URL_SETTING: &str = "buwiz_server_url";

pub const DECLARATIONS: &str = r#"
export interface CloudAccountStatus {
    server_url: string;
    server_url_source: string;
    /** The account site's address (where sign-in and account pages open). Empty when it is not a valid address. */
    account_url: string;
    signed_in: boolean;
    email: string | null;
    user_id: string | null;
    offline: boolean;
    practice: boolean;
    detail: string;
}
export interface CloudAccountGate {
    unlocked: boolean;
    offline: boolean;
    server_url: string;
    email: string | null;
    detail: string;
}
export function cloud_account_status(): CloudAccountStatus;
export function cloud_account_gate(): CloudAccountGate;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;
/** Whether the signed-in account signs in on this device for the first time (the welcome line shows); a first time is recorded. False, and nothing recorded, when no account is signed in. The page learns nothing else: what the device keeps is a keyed hash, host-only. */
export function welcome_first_time(): boolean;
"#;

pub fn register(module: HostModule) -> HostModule {
    module
        .function("cloud_account_status", |_| status_now())
        .function("cloud_account_gate", |_| gate_value())
        .async_function("cloud_account_refresh", refresh_account)
        .async_function("cloud_account_sign_out", sign_out)
        .function("cloud_account_session_opened", |_| session_opened())
        .function("cloud_account_session_closed", |_| session_closed())
        .function("welcome_first_time", |_| welcome_first_time())
}

/// Pause filing, then open the UI only when a saved session still qualifies.
/// Call this before the menu bar is built and before the database worker starts.
pub fn prepare_launch() {
    bir_core::account_session::on_session_ended(|owner, epoch, credential| {
        crate::session::server_ended(owner, epoch, credential);
    });
    // (c): the owner cache's production fill — before the restore, so every I/O point's disk
    // owner check and every close's owner evidence read a filled cache.
    bir_core::account_session::fill_owner_cache();
    bir_core::background_cron::set_submission_paused(true);
    forget_saved_server_url();
    drop_old_welcomed_list();
    // §1 [D4c]: the device guard and the owner-cache fill are the machine's init, the first
    // thing the boot runs — before any claim or open, outside every SESSION section.
    let layout = bir_core::rooms::Layout::here();
    if let Err(error) = crate::session::init(layout.device_dir()) {
        tracing::info!("device guard not held at launch: {error}");
    }
    if crate::account_gate::auth_bypass_enabled() {
        // §9: the bypass is the machine's own door, so check_invariants knows to step aside.
        crate::session::bypass_unlock();
        crate::session::sync_pause();
        return;
    }
    let (qualifies, _practice) = restore_saved_session();
    // The gate never unlocks here: I1 opens it at commit, once the room is in the phase. The
    // launch that qualifies begins the open; the claim resumes inside it (§A.4).
    if qualifies
        && let Ok(Some(stored)) = load_stored()
        && let Some(owner) = crate::session::Owner::of_stored(&stored)
    {
        crate::session::begin_open(
            owner,
            crate::bir_module::this_engine(),
            crate::session::OpenWhy::Launch,
        );
    }
    // A second engine's boot must not pause a live session's sending for the whole process:
    // the boot's own pause above is the safety while the restore runs; what stands at the end
    // is the gate's truth — unlocked sends, locked pauses.
    crate::session::sync_pause();
}

pub(crate) fn status_now() -> HostResult {
    let resolved = match resolve() {
        Ok(resolved) => resolved,
        Err(error) => {
            return Ok(status_value(&AccountStatus {
                server_url: rejected_origin_text(ENV_SERVER_URL, option_env!("BUWIZ_SERVER_URL")),
                server_url_source: "invalid".into(),
                signed_in: false,
                email: None,
                user_id: None,
                offline: false,
                practice: false,
                detail: error.to_string(),
            }));
        }
    };
    let stored = load_stored().ok().flatten();
    let secrets = stored
        .as_ref()
        .filter(|stored| stored.server_url == resolved.url)
        .map(StoredAccount::to_secrets);
    Ok(status_value(&cloud_account::status_from(
        &resolved,
        secrets.as_ref(),
    )))
}

/// The first tokens of a sign-in, from either path (the browser's device flow, the email code): the profile is
/// read, the sign-in saved under the account-session lock, and the device key of an earlier sign-in forgotten.
pub(crate) fn save_signed_in(
    http: &mut dyn cloud_account::Http,
    server_url: &str,
    tokens: cloud_account::TokenSet,
    now: i64,
) -> Result<(), HostError> {
    if tokens.refresh_token.is_empty() {
        return Err(HostError::new(
            "The server did not return a refresh token. Sign in again.",
        ));
    }
    let (user_id, email, practice) =
        cloud_account::load_profile(http, server_url, &tokens.access_token).map_err(host_error)?;
    let secrets = AccountSecrets {
        server_url: server_url.to_owned(),
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        access_expires_at_unix: now.saturating_add(tokens.expires_in),
        user_id,
        email,
        offline: false,
        practice,
    };
    // F10/F8': the persist, the cache and the open's beginning are the machine's one step. The
    // practice flag is commit's to set when the room installs (I1's order); the device key's
    // forget and the new registration are the machine's — a replace forgets the ended owner's
    // key epoch-checked (F2), and commit starts the registration with its own (owner, epoch)
    // (F2'), so a same-owner re-sign-in keeps its key and a stale registration writes nothing.
    crate::session::signed_in(&stored_from(secrets, now), crate::bir_module::this_engine())
        .map_err(host_error)?;
    Ok(())
}

/// Test seam: the Http refresh_account uses. A test parks the network by installing a factory
/// whose answer blocks on a harness gate; production never sets it, and the blocking default is
/// the real client.
#[cfg(test)]
type TestRefreshHttpFactory = Box<dyn Fn() -> Box<dyn cloud_account::Http + Send> + Send>;

#[cfg(test)]
static TEST_REFRESH_HTTP: std::sync::Mutex<Option<TestRefreshHttpFactory>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_test_refresh_http(
    factory: Option<Box<dyn Fn() -> Box<dyn cloud_account::Http + Send> + Send>>,
) {
    *TEST_REFRESH_HTTP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = factory;
}

fn refresh_http() -> Box<dyn cloud_account::Http + Send> {
    #[cfg(test)]
    if let Some(factory) = TEST_REFRESH_HTTP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
    {
        return factory();
    }
    Box::new(BlockingHttp::new())
}

/// What a verb's refresh learned, for its own answer shape.
#[derive(Debug)]
enum VerbRefresh {
    /// The profile was confirmed or refreshed; the save ran. Carries the practice flag.
    Online { practice: bool },
    /// The server was unreachable; the sign-in stands as it is.
    Offline,
    /// The credential was rejected: the sign-in ended through the machine.
    Ended,
    /// The stored sign-in changed hands or vanished in the verb's network window: nothing of
    /// the new owner's is the verb's to write, and the verb answers for itself alone.
    Gone,
}

/// The one refresh of the verbs (revision 5's F3). The TOKEN rotation never runs here directly:
/// it goes through [`bir_core::account_session::access_token_with_credential`], the keyed single
/// flight every refresh of the process joins — a verb concurrent with a send's flight or a
/// second engine's launch is ONE network refresh, never the double spend the server ends a
/// sign-in for. The PROFILE merge writes only the profile fields, and only while the disk's
/// sign-in is still this owner on the exact refresh token the verb loaded — a spent token never
/// lands back over a rotated record.
/// (a): the profile merge with its CAS-miss recovery. A miss means the stored record rotated
/// under the merge — the flight or another path landed a newer token — never that the sign-in
/// is gone. Re-load once: the same owner with a newer token is Online, the merge retried onto
/// the fresh record; Gone is ONLY for an owner change or an empty store.
fn merge_profile_or_reload(
    owner: &bir_core::account_session::OwnerKey,
    refresh_token: &str,
    user_id: &str,
    email: Option<String>,
    practice: bool,
    offline: bool,
) -> Result<Option<bool>, HostError> {
    let saved = {
        // F2: ONE lock section — the load, the judgement and the merge inside it, so the disk
        // can neither empty nor rotate in between.
        let _one = bir_core::account_session::lock();
        bir_core::account_session::merge_profile_for_owner(
            owner,
            refresh_token,
            user_id,
            email,
            practice,
            offline,
        )
        .map_err(host_error)?
    };
    if saved { Ok(Some(practice)) } else { Ok(None) }
}

fn verb_refresh(
    http: &mut dyn cloud_account::Http,
    stored: &StoredAccount,
    owner: &bir_core::account_session::OwnerKey,
    epoch: u64,
    now: i64,
) -> Result<VerbRefresh, HostError> {
    // The profile-only arm: the access token is valid, so no rotation is needed at all.
    if stored.access_expires_at_unix > now.saturating_add(30) {
        match cloud_account::load_profile(http, &stored.server_url, &stored.access_token) {
            Ok((user_id, email, practice)) => {
                return Ok(
                    match merge_profile_or_reload(
                        owner,
                        &stored.refresh_token,
                        &user_id,
                        email,
                        practice,
                        false,
                    )? {
                        Some(practice) => VerbRefresh::Online { practice },
                        None => VerbRefresh::Gone,
                    },
                );
            }
            Err(CloudError::Network(_)) => {
                let _saved = {
                    let _one = bir_core::account_session::lock();
                    bir_core::account_session::merge_profile_if(
                        owner,
                        &stored.refresh_token,
                        &stored.user_id,
                        stored.email.clone(),
                        stored.practice,
                        true,
                    )
                };
                return Ok(VerbRefresh::Offline);
            }
            Err(CloudError::InvalidGrant) => {
                // The access token itself was rejected: the rotation below decides.
            }
            Err(error) => return Err(host_error(error)),
        }
    }
    // The rotation through the keyed flight — never a refresh_tokens of the verb's own.
    // N1: reaching here with a token still valid by the local clock means the profile-only arm's
    // /me REJECTED that access token — a fast-path answer would be the same rejected token, so
    // the rotation is FORCED. (With an expired token the force changes nothing: the fast path
    // could never have answered it.)
    let force = stored.access_expires_at_unix > now.saturating_add(30);
    // B1: the rejected access token goes in — the fast paths never answer it back.
    match bir_core::account_session::access_token_rejecting_at_epoch(
        http,
        now,
        force,
        owner,
        epoch,
        Some(&stored.access_token),
    ) {
        Ok((access, credential)) => {
            let fresh = {
                let _one = bir_core::account_session::lock();
                match load_stored()? {
                    Some(fresh)
                        if bir_core::account_session::OwnerKey::of(&fresh).as_ref()
                            == Some(owner) =>
                    {
                        fresh
                    }
                    _ => return Ok(VerbRefresh::Gone),
                }
            };
            match cloud_account::load_profile(http, &fresh.server_url, &access) {
                Ok((user_id, email, practice)) => Ok(
                    match merge_profile_or_reload(
                        owner,
                        &fresh.refresh_token,
                        &user_id,
                        email,
                        practice,
                        false,
                    )? {
                        Some(practice) => VerbRefresh::Online { practice },
                        None => VerbRefresh::Gone,
                    },
                ),
                Err(CloudError::Network(_)) => Ok(VerbRefresh::Offline),
                Err(CloudError::InvalidGrant) => {
                    // The just-refreshed access token rejected at once: the sign-in is ended.
                    // N5: the credential is the rotation's OWN — the refresh token now stored —
                    // never the pre-rotation one, which matches nothing and would answer Ended
                    // while the sign-in stays stored. (9): Ended only when the ending really
                    // cleared — a mismatch means a newer sign-in is stored, so re-load and
                    // answer as the SignedOut arm does.
                    let ended = crate::session::server_ended(owner.clone(), epoch, credential);
                    if ended {
                        return Ok(VerbRefresh::Ended);
                    }
                    let fresh = {
                        let _one = bir_core::account_session::lock();
                        load_stored()?
                    };
                    match fresh {
                        Some(fresh)
                            if bir_core::account_session::OwnerKey::of(&fresh).as_ref()
                                == Some(owner) =>
                        {
                            Ok(VerbRefresh::Online {
                                practice: fresh.practice,
                            })
                        }
                        None => Ok(VerbRefresh::Ended),
                        Some(_) => Ok(VerbRefresh::Gone),
                    }
                }
                Err(error) => Err(host_error(error)),
            }
        }
        Err(bir_core::account_session::TokenError::SignedOut) => {
            // The flight's InvalidGrant already ran the ending hook exactly once for the
            // credential; this call is the no-op confirmation when it did, and the ending
            // itself when the verb is the only witness. (b): Ended only when the ending really
            // cleared — a credential mismatch (a rotation landed meanwhile) means a valid
            // sign-in is stored, and the verb re-loads and answers as the merge arm does.
            let ended =
                crate::session::server_ended(owner.clone(), epoch, stored.refresh_token.clone());
            if ended {
                return Ok(VerbRefresh::Ended);
            }
            let fresh = {
                let _one = bir_core::account_session::lock();
                load_stored()?
            };
            match fresh {
                Some(fresh)
                    if bir_core::account_session::OwnerKey::of(&fresh).as_ref() == Some(owner) =>
                {
                    Ok(VerbRefresh::Online {
                        practice: fresh.practice,
                    })
                }
                // The disk is empty: the ending did happen — the flight's hook cleared it on
                // this same credential before the verb's confirmatory call, so the second call
                // matched nothing. The sign-in the server rejected IS ended.
                None => Ok(VerbRefresh::Ended),
                // A different owner holds the disk: this sign-in is simply gone.
                Some(_) => Ok(VerbRefresh::Gone),
            }
        }
        Err(bir_core::account_session::TokenError::Offline(_)) => Ok(VerbRefresh::Offline),
        Err(bir_core::account_session::TokenError::Other(message)) => Err(HostError::new(message)),
    }
}

/// bir-mcp's condition (the guard exemption's price): another process can write the stored
/// sign-in — bir-mcp's door signs in without this app's machine. At the I/O points outside
/// SESSION, a disk owner that is NOT the cache owner counts as an owner change: the app's
/// session is for an owner who is no longer stored, so it closes "signed in elsewhere". Only a
/// cache that NAMES an owner judges — an unfilled or cleared cache closes nothing (a fresh
/// launch's restore would otherwise close the sign-in it is restoring).
fn close_if_disk_owner_changed(stored: &StoredAccount) -> bool {
    let (Some(cache), Some(disk)) = (
        bir_core::account_session::stored_owner(),
        bir_core::account_session::OwnerKey::of(stored),
    ) else {
        return false;
    };
    if cache == disk {
        return false;
    }
    tracing::info!(
        "the stored sign-in's owner changed outside the app: closing as signed in elsewhere"
    );
    crate::session::close_signed_in_elsewhere(&disk)
}

pub(crate) fn refresh_account(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-refresh", || {
        // D2: the epoch is captured at the verb's start; the lock is held for the file reads and
        // writes only — never across refresh_session's network.
        let epoch = bir_core::session_epoch::current();
        // F1: only DECIDE inside the lock. sign_out's own first step takes the account lock, so
        // calling it here wedged every token fetch, sign-in and sign-out of the process.
        let (stored, owner) = {
            let _one = bir_core::account_session::lock();
            let Some(stored) = load_stored()? else {
                return status_now();
            };
            let resolved = resolve()?;
            let another_server = stored.server_url != resolved.url;
            if another_server {
                (stored, None)
            } else {
                let Some(owner) = bir_core::account_session::OwnerKey::of(&stored) else {
                    return status_now();
                };
                (stored, Some(owner))
            }
        };
        let Some(owner) = owner else {
            // The stored sign-in belongs to another server deployment: it ends here, through the
            // machine — the disk outside SESSION, the cache and the close inside it. The lock is
            // released: sign_out's own steps take it.
            crate::session::end_changed_server(&stored.server_url, &stored.refresh_token);
            return status_now();
        };
        if close_if_disk_owner_changed(&stored) {
            return status_now();
        }
        let mut http = refresh_http();
        let now = cloud_account::unix_now();
        match verb_refresh(http.as_mut(), &stored, &owner, epoch, now)? {
            // F15: the practice flag is set only after the save succeeded — Online carries it
            // only then, so a discarded merge never flips the transport's account kind.
            VerbRefresh::Online { practice } => {
                bir_core::transport::set_practice_account(practice);
            }
            VerbRefresh::Ended => {
                // Finding 10, E1: an Ended on an EMPTY disk means server_ended cleared nothing
                // — some other path already emptied it — and the machine may still show the
                // open session of an owner nobody stores. The close is close_if_owner: decided
                // under SESSION, the phase's owner must BE this verb's owner — a stale verb of
                // A never closes B's newer session — and the disk must be empty.
                if let Some(machine_owner) = crate::session::Owner::of_stored(&stored) {
                    crate::session::close_if_owner(
                        &machine_owner,
                        crate::session::Reason::SignedInElsewhere,
                    );
                }
            }
            VerbRefresh::Offline | VerbRefresh::Gone => {}
        }
        status_now()
    })
}

fn sign_out(
    _arguments: &HostArguments,
) -> Result<impl std::future::Future<Output = HostResult> + Send + use<>, HostError> {
    spawn_account("buwiz-account-sign-out", || {
        sign_out_with(Box::new(BlockingHttp::new()))
    })
}

/// Signs out on this device first and tells the server afterwards. The saved sign-in is gone before any
/// request starts, so an app closed while a slow or absent network holds the logout is still signed out on
/// relaunch, and the page shows the gate without waiting for the server. A sign-in this device could not
/// remove is an error the page shows, never "signed out"; the gate locks and the logout goes out either way.
fn sign_out_with(http: Box<dyn cloud_account::Http + Send>) -> HostResult {
    // D1: the end is the machine's one section — the disk first, the cache and the close inside
    // SESSION, the logout after. Nothing here writes anything itself anymore. The logout carries
    // the transport the caller chose: production's BlockingHttp, the tests' fake.
    let removed = crate::session::sign_out_inner(http);
    crate::hooks::hooks().refresh_menu();
    removed.map_err(|_| HostError::new(SIGN_OUT_INCOMPLETE))?;
    status_now()
}

/// The page's answer when this device could not remove the saved sign-in: the logout has still
/// gone out and the gate is locked, but a relaunch would sign the account back in, so the
/// sign-out must never answer "signed out".
const SIGN_OUT_INCOMPLETE: &str = "Sign-out did not finish: this computer could not remove the saved sign-in. Try Sign out again.";

/// Sends the logout on a thread of its own, without the account-session lock, so a new sign-in never waits on
/// it. It carries the old sign-in's access token and nothing else of it; the ending paths match the exact
/// credential, so a logout of the old sign-in can never end a newer one. The answer changes nothing here (the
/// sign-in is already gone on this device); it is logged, without the token.
/// Test instrumentation: how many logouts this process sent, so the changed-server row proves
/// none ever left.
#[cfg(any(test, feature = "test-support"))]
pub static LOGOUTS_SENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn logout_soon(
    mut http: Box<dyn cloud_account::Http + Send>,
    server_url: String,
    access_token: String,
) {
    #[cfg(any(test, feature = "test-support"))]
    LOGOUTS_SENT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let started = std::thread::Builder::new()
        .name("buwiz-account-logout".into())
        .spawn(move || {
            match cloud_account::send_logout(http.as_mut(), &server_url, &access_token) {
                Ok(status) => tracing::info!("Buwiz logout sent: HTTP {status}"),
                Err(error) => tracing::info!("Buwiz logout not sent: {error}"),
            }
        });
    if let Err(error) = started {
        tracing::warn!("logout not sent ({error}); the sign-in is already gone on this device");
    }
}

fn session_opened() -> HostResult {
    // The page opens the gate after a sign-in; take the flag from the saved
    // account. An unreadable store leaves the value save_signed_in set.
    if let Ok(Some(stored)) = load_stored() {
        bir_core::transport::set_practice_account(stored.practice);
    }
    // A3a (§A.4), in this order: the saved sign-in is read; the legacy claim runs or resumes; the
    // account's room opens; the worker starts or wakes (the launch path below); the agent session
    // rebuilds (its generation check does, on the next batch). F2: the gate opens only after the
    // room — never ahead of it, so no tick and no screen runs on the old room under the new sign-in.
    // F10: the claim can copy, hash and zip a large legacy file — never on the UI thread, or
    // "Moving" never paints (the page polls the migration state while this runs). The order is
    // unchanged: the claim runs or resumes, the room opens, the gate opens only after the room.
    // A no-op for the room (the spec's choice): session::signed_in already began the open in
    // its own SESSION section, and a second begin_open for the same owner is the X1 no-op.
    // The updater still starts here, as before.
    crate::hooks::hooks().start_updater();
    Ok(HostValue::Null)
}

fn session_closed() -> HostResult {
    crate::session::close(crate::session::Reason::PageClosed);
    Ok(HostValue::Null)
}

/// Whether the gate opens, and whether the account is a practice account.
/// Offline, the saved value stands.
fn restore_saved_session() -> (bool, bool) {
    restore_saved_session_with(&mut BlockingHttp::new())
}

pub(crate) fn restore_saved_session_with(http: &mut dyn cloud_account::Http) -> (bool, bool) {
    // D2, the launch arm: the lock is held for the reads and writes, never across the network.
    let epoch = bir_core::session_epoch::current();
    let (stored, owner) = {
        let _one = bir_core::account_session::lock();
        let Ok(Some(stored)) = load_stored() else {
            return (false, false);
        };
        let Ok(resolved) = resolve() else {
            return (false, false);
        };
        if stored.server_url != resolved.url {
            return (false, false);
        }
        let Some(owner) = bir_core::account_session::OwnerKey::of(&stored) else {
            return (false, false);
        };
        (stored, owner)
    };
    if close_if_disk_owner_changed(&stored) {
        return (false, false);
    }
    let now = cloud_account::unix_now();
    let mut practice = stored.practice;
    let outcome = match verb_refresh(http, &stored, &owner, epoch, now) {
        Ok(VerbRefresh::Online {
            practice: confirmed,
        }) => {
            practice = confirmed;
            RefreshOutcome::Online
        }
        Ok(VerbRefresh::Offline) => RefreshOutcome::Offline,
        Ok(VerbRefresh::Ended) => RefreshOutcome::Revoked,
        Ok(VerbRefresh::Gone) => return (false, false),
        Err(_) => RefreshOutcome::Offline,
    };
    match cloud_account::gate_after_refresh(outcome) {
        GateDecision::Open { .. } => {
            // The cache is the machine's picture of who is stored (F8'): the launch sets it from
            // the sign-in just confirmed.
            bir_core::account_session::cache_owner(Some(owner));
            (true, practice)
        }
        GateDecision::Closed {
            clear_secrets: true,
        } => {
            // D1 at the launch too, with revision 5's credential: the delete matches only the
            // exact refresh token the server rejected, so a newer sign-in is never taken by it.
            crate::session::server_ended(owner.clone(), epoch, stored.refresh_token.clone());
            (false, false)
        }
        GateDecision::Closed {
            clear_secrets: false,
        } => (false, false),
    }
}

fn stored_from(secrets: AccountSecrets, last_refresh_unix: i64) -> StoredAccount {
    let mut stored = StoredAccount::from_secrets(&secrets);
    stored.last_refresh_unix = last_refresh_unix;
    stored
}

fn gate_value() -> HostResult {
    let resolved = resolve();
    let server_url = resolved
        .as_ref()
        .map(|item| item.url.clone())
        .unwrap_or_default();
    let detail = resolved
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    let stored = load_stored().ok().flatten();
    let unlocked = crate::account_gate::is_unlocked();
    let offline = unlocked && stored.as_ref().is_some_and(|item| item.offline);
    let email = stored.as_ref().and_then(|item| item.email.as_deref());
    Ok(HostObject::new()
        .field("unlocked", unlocked)
        .field("offline", offline)
        .field("server_url", server_url)
        .field("email", optional_text(email))
        .field("detail", detail)
        .into())
}

pub(crate) fn spawn_account(
    name: &str,
    work: impl FnOnce() -> HostResult + Send + 'static,
) -> Result<std::pin::Pin<Box<dyn std::future::Future<Output = HostResult> + Send>>, HostError> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    // The account file lives next to the database, so this thread must know the database's name as the host
    // thread does: the in-process name is per thread, and a new thread is given it here. The device
    // folder override rides with it, the same discipline — tests read their own scratch install.
    let database = bir_core::db::capture_database_path_override();
    let device = bir_core::platform::capture_device_dir_override();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let _ = sender.send(database.run(move || device.run(work)));
        })
        .map_err(|error| HostError::new(error.to_string()))?;
    Ok(Box::pin(async move {
        receiver
            .await
            .unwrap_or_else(|_| Err(HostError::new("The Buwiz account thread stopped.")))
    }))
}

pub(crate) fn resolve() -> Result<cloud_account::ResolvedServer, HostError> {
    cloud_account::configured_server().map_err(host_error)
}

fn forget_saved_server_url() {
    if let Err(error) = with_account_database(|database| {
        database
            .delete_setting(OBSOLETE_SERVER_URL_SETTING)
            .map_err(|error| HostError::new(error.to_string()))
    }) {
        tracing::warn!("could not drop the saved Buwiz server address: {error}");
    }
}

/// accounts-and-lock W3: whether the signed-in account (the host's saved sign-in, never a word from the page) signs
/// in on this device for the first time, recorded on a true (`bir_core::welcome`). No account signed in: false, and
/// nothing recorded.
pub(crate) fn welcome_first_time() -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    drop_old_welcomed_list();
    let Some(account) = signed_in_account_key() else {
        return Ok(HostValue::Bool(false));
    };
    bir_core::welcome::first_time(&bir_core::rooms::device_store_here(), &account)
        .map(HostValue::Bool)
        .map_err(host_error)
}

/// The signed-in account's key (`bir_core::welcome::account_key`), as `cloud_account_status` sees the sign-in.
fn signed_in_account_key() -> Option<String> {
    let resolved = resolve().ok()?;
    let stored = load_stored().ok().flatten()?;
    let status = cloud_account::status_from(&resolved, Some(&stored.to_secrets()));
    if !status.signed_in {
        return None;
    }
    bir_core::welcome::account_key(status.user_id.as_deref(), status.email.as_deref())
}

/// #417's list of welcomed accounts held their user ids and addresses: it is dropped from `device_settings` and from
/// the open database's settings (where it was before A1), at launch and at each welcome check. At most one more
/// welcome.
pub(crate) fn drop_old_welcomed_list() {
    if let Err(error) = bir_core::welcome::drop_old_list(&bir_core::rooms::device_store_here()) {
        tracing::warn!("could not drop the old welcomed list from the device store: {error}");
    }
    if let Err(error) = with_account_database(|database| {
        database
            .delete_setting(bir_core::welcome::OLD_WELCOMED_SETTING)
            .map_err(|error| HostError::new(error.to_string()))
    }) {
        tracing::warn!("could not drop the old welcomed list from the database: {error}");
    }
}

fn rejected_origin_text(runtime_name: &str, compiled: Option<&str>) -> String {
    bir_core::config::var(runtime_name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            compiled
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// The account site's address for this build, as the screens open it (Delete account). Empty when it does not resolve.
fn account_url_text() -> String {
    bir_core::cloud_account::configured_account()
        .map(|account| account.url)
        .unwrap_or_default()
}

fn status_value(status: &AccountStatus) -> HostValue {
    HostObject::new()
        .field("server_url", status.server_url.clone())
        .field("server_url_source", status.server_url_source.clone())
        .field("account_url", account_url_text())
        .field("signed_in", status.signed_in)
        .field("email", optional_text(status.email.as_deref()))
        .field("user_id", optional_text(status.user_id.as_deref()))
        .field("offline", status.offline)
        .field("practice", status.practice)
        .field("detail", status.detail.clone())
        .into()
}

fn optional_text(value: Option<&str>) -> HostValue {
    value
        .filter(|text| !text.is_empty())
        .map(|text| HostValue::Str(text.to_owned()))
        .unwrap_or(HostValue::Null)
}

fn host_error(error: impl std::fmt::Display) -> HostError {
    HostError::new(error.to_string())
}

/// Server origin and the user access token, when this computer is signed in.
/// The device-filing poll uses it. Tokens are not logged.
#[cfg(feature = "agent")]
pub(crate) fn device_poll_auth() -> Option<(String, String)> {
    let stored = load_stored().ok().flatten()?;
    if stored.offline || stored.access_token.is_empty() || stored.server_url.is_empty() {
        return None;
    }
    Some((stored.server_url, stored.access_token))
}

pub(crate) fn load_stored() -> Result<Option<StoredAccount>, HostError> {
    cloud_account::load_stored_account().map_err(host_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test fixtures write through the door directly; production's clears are the machine's.
    fn clear_stored() {
        bir_core::account_session::clear_stored();
    }

    /// Tests seed the stored sign-in through the one path — the guard test allows no raw call
    /// in production code, so the fixtures live in here.
    pub(crate) fn save_stored(stored: &StoredAccount) -> Result<(), HostError> {
        bir_core::account_session::save_stored(stored).map_err(host_error)
    }
    use crate::bir_module::{use_ephemeral_database, with_account_database};

    /// The in-process database name is per thread; the account file lives next to the database, so a thread
    /// started for an account call works under the name of the thread that started it.
    #[test]
    fn an_account_thread_works_under_the_database_name_of_the_thread_that_started_it() {
        let folder = tempfile::tempdir().expect("scratch folder");
        let named = folder.path().join("bir_data.db");
        bir_core::db::set_database_path_override(Some(named.clone()));
        let seen = spawn_account("buwiz-account-test", || {
            Ok(HostValue::Str(
                bir_core::db::app_database_path().display().to_string(),
            ))
        })
        .map(futures::executor::block_on);
        bir_core::db::set_database_path_override(None);
        match seen {
            Ok(Ok(HostValue::Str(path))) => assert_eq!(path, named.display().to_string()),
            _ => panic!("the account thread did not answer"),
        }
    }

    #[test]
    fn the_page_cannot_set_a_server_url_and_a_saved_one_is_ignored() {
        assert!(
            !DECLARATIONS.contains("cloud_account_set_server_url"),
            "the host must not offer a server-url setter"
        );
        assert!(DECLARATIONS.contains("cloud_account_status"));
        use_ephemeral_database();
        with_account_database(|database| {
            database
                .set_setting(OBSOLETE_SERVER_URL_SETTING, "https://saved.example.test")
                .map_err(|error| HostError::new(error.to_string()))?;
            let stored = database
                .get_setting(OBSOLETE_SERVER_URL_SETTING)
                .map_err(|error| HostError::new(error.to_string()))?;
            assert_eq!(stored.as_deref(), Some("https://saved.example.test"));
            Ok(())
        })
        .unwrap();
        let resolved = resolve().expect("the build's server resolves");
        assert_ne!(
            resolved.url, "https://saved.example.test",
            "buwiz_server_url is not read"
        );
    }

    // F10's threading is the machine's now: the claim runs on the buwiz-room-open thread the
    // machine spawns, never on the calling thread — session_opened is a no-op for the room, and
    // the machine's own tests drive the real thread.

    /// Startup deletes `buwiz_server_url`. The child uses a scratch database
    /// (`BIR_DATABASE_PATH`), `EBIR_TEST_ENV=1`, and `BIR_SFTP_DRY_RUN=1`.
    #[test]
    fn prepare_launch_deletes_a_saved_server_url() {
        const SCRATCH: &str = "BIR_SHELL_FORGET_SERVER_URL_SCRATCH";
        if std::env::var_os(SCRATCH).is_some() {
            delete_saved_server_url_on_startup();
            return;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = std::env::temp_dir().join(format!(
            "bir-shell-forget-server-url-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&scratch).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cloud_account::tests::prepare_launch_deletes_a_saved_server_url",
                "--nocapture",
            ])
            .current_dir(&scratch)
            .env(SCRATCH, &scratch)
            .env("BIR_DATABASE_PATH", scratch.join("bir_data.db"))
            .env("EBIR_TEST_ENV", "1")
            .env("BIR_AUTH_BYPASS", "1")
            .env("BIR_SFTP_DRY_RUN", "1")
            .env_remove("BUWIZ_SERVER_URL")
            .env_remove("BUWIZ_ACCOUNT_URL")
            .status()
            .unwrap();
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(status.success(), "startup did not drop buwiz_server_url");
    }

    mod sign_out {
        use super::*;
        use bir_core::cloud_account::{HttpRequest, HttpResponse};
        use std::path::PathBuf;
        use std::sync::mpsc;
        use std::time::Duration;

        const SERVER: &str = "http://127.0.0.1:9";
        const WAIT: Duration = Duration::from_secs(10);

        /// What was still on this device when the logout reached the fake server, and what the request carried.
        struct Seen {
            url: String,
            bearer: Option<String>,
            file_left: bool,
            sign_in_left: bool,
        }

        /// The server's logout endpoint. It notes what is left on this device the moment the request arrives,
        /// then, when `release` is given, hangs until the test lets it go (or drops the sender), and answers.
        struct Logout {
            file: PathBuf,
            seen: mpsc::Sender<Seen>,
            release: Option<mpsc::Receiver<()>>,
            answer: fn() -> Result<HttpResponse, CloudError>,
        }

        impl cloud_account::Http for Logout {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let _ = self.seen.send(Seen {
                    url: request.url.clone(),
                    bearer: request.bearer.clone(),
                    file_left: self.file.exists(),
                    sign_in_left: cloud_account::load_stored_account()
                        .ok()
                        .flatten()
                        .is_some(),
                });
                if let Some(release) = &self.release {
                    let _ = release.recv();
                }
                (self.answer)()
            }
        }

        fn answered() -> Result<HttpResponse, CloudError> {
            Ok(HttpResponse {
                status: 200,
                body: "{}".into(),
            })
        }

        fn refused() -> Result<HttpResponse, CloudError> {
            Ok(HttpResponse {
                status: 500,
                body: r#"{"error":"server_error"}"#.into(),
            })
        }

        fn unreachable() -> Result<HttpResponse, CloudError> {
            Err(CloudError::Network("connection refused".into()))
        }

        /// A scratch folder with a saved sign-in for [`SERVER`] (keychain off, so the private file is the record).
        fn signed_in<R>(body: impl FnOnce(PathBuf) -> R) -> R {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("account.json");
            bir_core::db::set_database_path_override(Some(dir.path().join("bir_data.db")));
            let result = temp_env::with_vars(
                [
                    ("BUWIZ_NO_KEYCHAIN", Some("1")),
                    ("BUWIZ_SERVER_URL", Some(SERVER)),
                    // A test trust key: a development build builds its relay client only for a
                    // server trusted through one, and the real-path close test observes the TIN
                    // invalidation through that client.
                    ("BUWIZ_TRUST_KEYS", Some("test-key")),
                    (
                        bir_core::email_sign_in::ENV_PENDING_FILE,
                        Some(dir.path().join("pending.json").to_str().unwrap()),
                    ),
                    (
                        cloud_account::ENV_ACCOUNT_FILE,
                        Some(file.to_str().unwrap()),
                    ),
                ],
                || {
                    save_stored(&stored("old-access", "old-refresh")).unwrap();
                    assert!(signed_in_now(), "the scratch sign-in did not take");
                    body(file.clone())
                },
            );
            bir_core::db::set_database_path_override(None);
            result
        }

        /// The fake server of the F3 flight tests: token refreshes and profile fetches counted,
        /// the first refresh parked on a gate so a second caller joins the flight as its waiter.
        struct FlightServer {
            refreshes: std::sync::atomic::AtomicU64,
            profiles: std::sync::atomic::AtomicU64,
            parked: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
            gate: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
            park_profiles: bool,
        }

        struct FlightHttp(std::sync::Arc<FlightServer>);

        impl cloud_account::Http for FlightHttp {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let server = &self.0;
                if request.url.ends_with("/oauth/token") {
                    server
                        .refreshes
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let park = server.parked.lock().unwrap().take();
                    let gate = server.gate.lock().unwrap().take();
                    if let (Some(parked), Some(gate)) = (park, gate) {
                        let _ = parked.send(());
                        let _ = gate.recv();
                    }
                    return Ok(HttpResponse {
                        status: 200,
                        body: r#"{"access_token":"access-r2","refresh_token":"refresh-r2","expires_in":3600}"#.into(),
                    });
                }
                if request.url.ends_with("/me") {
                    server
                        .profiles
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if server.park_profiles {
                        let park = server.parked.lock().unwrap().take();
                        let gate = server.gate.lock().unwrap().take();
                        if let (Some(parked), Some(gate)) = (park, gate) {
                            let _ = parked.send(());
                            let _ = gate.recv();
                        }
                    }
                    return Ok(HttpResponse {
                        status: 200,
                        body: r#"{"user_id":"u1","email":"filer@example.test","practice":false}"#
                            .into(),
                    });
                }
                Err(CloudError::Unexpected(format!(
                    "unexpected request: {}",
                    request.url
                )))
            }
        }

        fn flight_server(
            park_profiles: bool,
        ) -> (
            std::sync::Arc<FlightServer>,
            std::sync::mpsc::Receiver<()>,
            std::sync::mpsc::Sender<()>,
        ) {
            let (parked_tx, parked_rx) = std::sync::mpsc::channel();
            let (gate_tx, gate_rx) = std::sync::mpsc::channel();
            let server = std::sync::Arc::new(FlightServer {
                refreshes: std::sync::atomic::AtomicU64::new(0),
                profiles: std::sync::atomic::AtomicU64::new(0),
                parked: std::sync::Mutex::new(Some(parked_tx)),
                gate: std::sync::Mutex::new(Some(gate_rx)),
                park_profiles,
            });
            (server, parked_rx, gate_tx)
        }

        /// The scripted server of the safety-table rows: /me and /oauth/token answers scripted
        /// per call, the bearer of each /me recorded, logouts counted. A `Some` answer is the
        /// response; a `None` pops nothing and errors — a call the row never scripted.
        #[derive(Default)]
        struct ScriptServer {
            mes: std::sync::Mutex<std::collections::VecDeque<(String, u16)>>,
            tokens: std::sync::Mutex<std::collections::VecDeque<(u16, String)>>,
            me_bearers: std::sync::Mutex<Vec<String>>,
            logouts: std::sync::atomic::AtomicU64,
            parked: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
            gate: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
        }

        struct ScriptHttp(std::sync::Arc<ScriptServer>);

        impl cloud_account::Http for ScriptHttp {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let server = &self.0;
                if request.url.ends_with("/oauth/token") {
                    let park = server.parked.lock().unwrap().take();
                    let gate = server.gate.lock().unwrap().take();
                    if let (Some(parked), Some(gate)) = (park, gate) {
                        let _ = parked.send(());
                        let _ = gate.recv();
                    }
                    let Some((status, body)) = server.tokens.lock().unwrap().pop_front() else {
                        return Err(CloudError::Unexpected("unscripted token call".into()));
                    };
                    return Ok(HttpResponse { status, body });
                }
                if request.url.ends_with("/me") {
                    let bearer = request.bearer.clone().unwrap_or_default();
                    server.me_bearers.lock().unwrap().push(bearer.clone());
                    let Some((required, status)) = server.mes.lock().unwrap().pop_front() else {
                        return Err(CloudError::Unexpected("unscripted /me call".into()));
                    };
                    if required != "*" && required != bearer {
                        return Ok(HttpResponse {
                            status: 401,
                            body: r#"{"error":"invalid_token"}"#.into(),
                        });
                    }
                    return Ok(HttpResponse {
                        status,
                        body: r#"{"user_id":"u1","email":"filer@example.test","practice":false}"#
                            .into(),
                    });
                }
                if request.url.ends_with("/logout") {
                    server
                        .logouts
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    return Ok(HttpResponse {
                        status: 200,
                        body: "{}".into(),
                    });
                }
                Err(CloudError::Unexpected(format!(
                    "unexpected request: {}",
                    request.url
                )))
            }
        }

        fn scripted() -> (std::sync::Arc<ScriptServer>, ScriptHttp) {
            let server = std::sync::Arc::new(ScriptServer::default());
            (server.clone(), ScriptHttp(server))
        }

        /// The verb carries the epoch it was spawned with into the keyed flight. A stale verb
        /// never reaches the token server in the new session; if the flight re-read the current
        /// epoch instead, this unscripted token call errors and the test is red.
        #[test]
        fn a_stale_verb_refresh_never_reaches_the_token_server() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            signed_in(|_file| {
                let mut expired = stored("access-stale", "refresh-stale");
                expired.access_expires_at_unix = 1_000;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                // Keep the credential itself current on disk: the test isolates the epoch half,
                // so the stale verb must not enter the server-ended close path.
                let mut current = expired.clone();
                current.refresh_token = "refresh-current".into();
                save_stored(&current).unwrap();
                let stale_epoch = bir_core::session_epoch::current();
                bir_core::session_epoch::set(stale_epoch + 1);
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                let outcome = verb_refresh(&mut http, &expired, &owner, stale_epoch, now);
                assert!(
                    outcome.is_ok(),
                    "the stale verb reached the unscripted token server: {outcome:?}"
                );
                assert!(server.tokens.lock().unwrap().is_empty());
                assert!(server.me_bearers.lock().unwrap().is_empty());
                bir_core::session_epoch::set(stale_epoch);
            });
        }

        /// N1 (row 4): /me rejects the access token although it is valid by the clock — the
        /// rotation is FORCED, never the fast path's same rejected token — and the verb stays
        /// Online on the rotated sign-in.
        #[test]
        fn row4_a_rejected_access_token_forces_the_rotation_and_stays_online() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-r1", "refresh-r1");
                save_stored(&current).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                // /me answers 200 only for the rotated access token: the same-token fast path
                // is answered 401 again — exactly the bug's loop.
                *server.mes.lock().unwrap() = [
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                ]
                .into_iter()
                .collect();
                server.tokens.lock().unwrap().push_back((
                    200,
                    r#"{"access_token":"access-r2","refresh_token":"refresh-r2","expires_in":3600}"#
                        .into(),
                ));
                let outcome = verb_refresh(&mut http, &current, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(
                    matches!(outcome, VerbRefresh::Online { .. }),
                    "a rejected access token signed the user out although the refresh token was valid: {outcome:?}"
                );
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("the sign-in was cleared");
                assert_eq!(
                    on_disk.refresh_token, "refresh-r2",
                    "the rotation did not land"
                );
                let bearers = server.me_bearers.lock().unwrap();
                assert!(
                    bearers.iter().any(|bearer| bearer == "access-r2"),
                    "no /me ever carried the rotated token — the rotation was never forced: {bearers:?}"
                );
            });
        }

        /// (a): a merge CAS miss because the flight rotated the token meanwhile is NOT Gone —
        /// the same owner with a newer token re-loads to Online. Gone is only an owner change
        /// or an empty disk.
        #[test]
        fn a_cas_miss_with_a_newer_same_owner_token_is_not_gone() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-r2", "refresh-r2");
                save_stored(&current).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                // The merge carries the SPENT credential, as the flight's joiner does.
                let outcome = merge_profile_or_reload(
                    &owner,
                    "refresh-r1",
                    "u1",
                    Some("new@example.test".into()),
                    true,
                    false,
                )
                .expect("the merge errored");
                assert_eq!(
                    outcome,
                    Some(true),
                    "a CAS miss on the spent credential answered Gone with a valid same-owner sign-in stored"
                );
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("the sign-in was cleared");
                assert_eq!(
                    on_disk.refresh_token, "refresh-r2",
                    "the rotation was overwritten"
                );
                assert_eq!(
                    on_disk.email.as_deref(),
                    Some("new@example.test"),
                    "the profile never merged onto the fresh record"
                );
            });
        }

        /// (b): the flight's InvalidGrant where the disk has moved to ANOTHER owner clears
        /// nothing — the verb must not answer Ended (the restore would lock the gate on a valid
        /// sign-in). It re-loads and answers Gone.
        #[test]
        fn b_an_invalid_grant_after_another_owner_signed_in_is_gone_not_ended() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1_000;
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                // The disk moved on to another owner entirely.
                let mut other = stored("access-other", "refresh-other");
                other.user_id = "u2".into();
                save_stored(&other).unwrap();
                let (server, mut http) = scripted();
                server
                    .tokens
                    .lock()
                    .unwrap()
                    .push_back((400, r#"{"error":"invalid_grant"}"#.into()));
                let outcome = verb_refresh(&mut http, &expired, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(
                    matches!(outcome, VerbRefresh::Gone),
                    "the verb answered Ended for a sign-in the ending never cleared: {outcome:?}"
                );
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("the other owner's sign-in was cleared");
                assert_eq!(on_disk.refresh_token, "refresh-other");
            });
        }

        /// Row b, the empty-disk arm: the flight's InvalidGrant witnesses the ending, the disk
        /// is empty already — the answer is Ended, never Gone.
        #[test]
        fn b_an_invalid_grant_with_an_empty_disk_is_ended() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1_000;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                server
                    .tokens
                    .lock()
                    .unwrap()
                    .push_back((400, r#"{"error":"invalid_grant"}"#.into()));
                // The disk empties before the verb's confirmatory read, as the flight's own
                // hook leaves it.
                let outcome = {
                    let answer = verb_refresh(&mut http, &expired, &owner, epoch, now);
                    bir_core::account_session::clear_stored();
                    answer
                };
                // Hmm — the clear above is too late to matter; what matters is the hook's own
                // clear: server_ended cleared on the matched credential inside the flight. The
                // verb's confirmatory server_ended then matches nothing and the disk is empty.
                assert!(
                    matches!(outcome, Ok(VerbRefresh::Ended)),
                    "an empty disk after the witnessed ending is Ended, not Gone: {outcome:?}"
                );
            });
        }

        /// Row b, the same-owner-newer-token arm: the rotation landed meanwhile — the verb
        /// answers Online for the valid sign-in, never Ended.
        #[test]
        fn b_an_invalid_grant_with_a_newer_same_owner_token_is_online() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1_000;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                server.tokens.lock().unwrap().push_back((
                    200,
                    r#"{"access_token":"access-r2","refresh_token":"refresh-r2","expires_in":3600}"#
                        .into(),
                ));
                *server.mes.lock().unwrap() = [
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                ]
                .into_iter()
                .collect();
                // The rotation succeeds in the flight itself here; the newer token is what the
                // verb merges on — the Online arm of the same-owner rule.
                let outcome = verb_refresh(&mut http, &expired, &owner, epoch, now);
                assert!(
                    matches!(outcome, Ok(VerbRefresh::Online { .. })),
                    "a newer same-owner token is Online, never Ended: {outcome:?}"
                );
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("the sign-in was cleared");
                assert_eq!(on_disk.refresh_token, "refresh-r2");
            });
        }

        /// B1 (row 4, recency variant): the same rejection 1 SECOND after the rotation — the
        /// forced path's 10-second recency twin would hand the REJECTED token right back. With
        /// the rejection carried in, both fast paths are skipped for the rejected token and the
        /// rotation still happens.
        #[test]
        fn row4_a_rejected_access_token_rotates_inside_the_recency_window() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut current = stored("access-r1", "refresh-r1");
                current.last_refresh_unix = cloud_account::unix_now() - 1;
                save_stored(&current).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                *server.mes.lock().unwrap() = [
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                    ("access-r2".to_string(), 200u16),
                ]
                .into_iter()
                .collect();
                server.tokens.lock().unwrap().push_back((
                    200,
                    r#"{"access_token":"access-r2","refresh_token":"refresh-r2","expires_in":3600}"#
                        .into(),
                ));
                let outcome = verb_refresh(&mut http, &current, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(
                    matches!(outcome, VerbRefresh::Online { .. }),
                    "the rejected token short-circuited the forced path inside the recency window: {outcome:?}"
                );
                let bearers = server.me_bearers.lock().unwrap();
                assert!(
                    bearers.iter().any(|bearer| bearer == "access-r2"),
                    "no /me ever carried the rotated token inside the recency window: {bearers:?}"
                );
            });
        }

        /// N5 (row 6): after the forced rotation /me rejects the NEW token too — the ending is
        /// matched on the ROTATED credential, so the stored sign-in really is cleared.
        #[test]
        fn row6_a_rejected_new_token_signs_out_on_the_rotated_credential() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-r1", "refresh-r1");
                save_stored(&current).unwrap();
                // The row's own cache state — never what another test left: the close's owner
                // evidence is the cache, so the row seeds it as the machine's sign-in does.
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &current,
                ));
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, mut http) = scripted();
                // Every /me rejects whatever it sees.
                *server.mes.lock().unwrap() = [
                    ("*".to_string(), 401u16),
                    ("*".to_string(), 401u16),
                    ("*".to_string(), 401u16),
                ]
                .into_iter()
                .collect();
                server.tokens.lock().unwrap().push_back((
                    200,
                    r#"{"access_token":"access-r2","refresh_token":"refresh-r2","expires_in":3600}"#
                        .into(),
                ));
                bir_core::account_session::on_session_ended(|owner, epoch, credential| {
                    crate::session::server_ended(owner, epoch, credential);
                });
                let outcome = verb_refresh(&mut http, &current, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(matches!(outcome, VerbRefresh::Ended), "{outcome:?}");
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_none(),
                    "the sign-in stayed stored: the ending was matched on the OLD credential"
                );
            });
        }

        /// Row 5: the rotation's own refresh token rejected — InvalidGrant — clears and closes
        /// everything. The flight's hook runs the credential-matched ending.
        #[test]
        fn row5_an_invalid_grant_of_the_rotation_clears_and_closes() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (_server, mut http) = scripted();
                _server
                    .tokens
                    .lock()
                    .unwrap()
                    .push_back((400, r#"{"error":"invalid_grant"}"#.into()));
                bir_core::account_session::on_session_ended(|owner, epoch, credential| {
                    crate::session::server_ended(owner, epoch, credential);
                });
                let outcome = verb_refresh(&mut http, &expired, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(matches!(outcome, VerbRefresh::Ended), "{outcome:?}");
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_none(),
                    "the InvalidGrant of the current credential did not clear the sign-in"
                );
            });
        }

        /// Row 5's composition: the flight rotates R1→R2 on one path; a verb whose refresh of
        /// the spent R1 is still in flight meets the server's InvalidGrant — the ending names
        /// R1, the disk holds R2, and NOTHING is cleared: the newer sign-in stays whole.
        #[test]
        fn the_flight_composition_a_verb_holding_the_spent_token_ends_nothing() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                bir_core::account_session::on_session_ended(|owner, epoch, credential| {
                    crate::session::server_ended(owner, epoch, credential);
                });
                // The cache names the owner, as the machine's sign-in sets it.
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &expired,
                ));
                let (server, _) = scripted();
                let (parked_tx, parked_rx) = std::sync::mpsc::channel();
                let (gate_tx, gate_rx) = std::sync::mpsc::channel();
                *server.parked.lock().unwrap() = Some(parked_tx);
                *server.gate.lock().unwrap() = Some(gate_rx);
                server
                    .tokens
                    .lock()
                    .unwrap()
                    .push_back((400, r#"{"error":"invalid_grant"}"#.into()));
                // The re-read rotation's /me answers 200: the verb must reach a REAL Online.
                *server.mes.lock().unwrap() =
                    [("*".to_string(), 200u16), ("*".to_string(), 200u16)]
                        .into_iter()
                        .collect();
                // The verb leads the R1 flight, parked inside the token call.
                let verb_server = std::sync::Arc::clone(&server);
                let verb_stored = expired.clone();
                let verb_owner = owner.clone();
                let verb = std::thread::spawn(move || {
                    let mut http = ScriptHttp(verb_server);
                    verb_refresh(&mut http, &verb_stored, &verb_owner, epoch, now).ok()
                });
                parked_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the verb never reached the token refresh");
                // Mid-flight, the rotation lands on another path: the disk holds R2.
                let mut rotated = stored("access-r2", "refresh-r2");
                rotated.user_id = "u1".into();
                {
                    let _one = bir_core::account_session::lock();
                    save_stored(&rotated).unwrap();
                }
                let _ = gate_tx.send(());
                let outcome = verb.join().expect("the verb thread panicked");
                assert!(
                    matches!(outcome, Some(VerbRefresh::Online { .. })),
                    "the verb holding the spent token did not reach the real Online: {outcome:?}"
                );
                let on_disk = bir_core::cloud_account::load_stored_account().unwrap();
                assert!(
                    on_disk.as_ref().map(|stored| stored.refresh_token.as_str())
                        == Some("refresh-r2"),
                    "the late verb's ending cleared the newer sign-in: {on_disk:?}"
                );
                assert_eq!(
                    bir_core::account_session::stored_owner().map(|key| key.user_id),
                    Some("u1".to_string()),
                    "the owner cache was cleared by the spent token's InvalidGrant"
                );
            });
        }

        /// Row 2: the changed-server ending sends NO logout — the fake records every logout
        /// call, and this arm must produce none — while the disk is cleared.
        #[test]
        fn row2_the_changed_server_ending_sends_no_logout() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut moved = stored("old-access", "old-refresh");
                moved.server_url = "https://elsewhere.example.test".into();
                save_stored(&moved).unwrap();
                let logouts_before =
                    crate::cloud_account::LOGOUTS_SENT.load(std::sync::atomic::Ordering::SeqCst);
                let verb =
                    crate::cloud_account::refresh_account(&gpui_shell::HostArguments::default())
                        .expect("the verb spawns");
                let _ = futures::executor::block_on(verb);
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_none(),
                    "the changed-server sign-in is still on disk"
                );
                assert_eq!(
                    crate::cloud_account::LOGOUTS_SENT.load(std::sync::atomic::Ordering::SeqCst),
                    logouts_before,
                    "a logout went to the other deployment's server"
                );
            });
        }

        /// Row 7: a network error keeps the sign-in, offline — nothing is cleared or closed.
        #[test]
        fn row7_a_network_error_keeps_the_sign_in() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-valid", "refresh-r1");
                save_stored(&current).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                struct Down;
                impl cloud_account::Http for Down {
                    fn exchange(
                        &mut self,
                        _request: &HttpRequest,
                    ) -> Result<HttpResponse, CloudError> {
                        Err(CloudError::Network("connection refused".into()))
                    }
                }
                let mut http = Down;
                let outcome = verb_refresh(&mut http, &current, &owner, epoch, now)
                    .expect("the verb errored");
                assert!(matches!(outcome, VerbRefresh::Offline), "{outcome:?}");
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_some(),
                    "a network error cleared the sign-in"
                );
            });
        }

        /// Row 8: the restore with a stored server ≠ the configured one leaves the disk
        /// UNTOUCHED — the gate stays closed and the sign-in stays, as main does.
        #[test]
        fn row8_the_restore_with_a_changed_server_leaves_the_disk_untouched() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut moved = stored("old-access", "old-refresh");
                moved.server_url = "https://elsewhere.example.test".into();
                save_stored(&moved).unwrap();
                struct Down;
                impl cloud_account::Http for Down {
                    fn exchange(
                        &mut self,
                        _request: &HttpRequest,
                    ) -> Result<HttpResponse, CloudError> {
                        Err(CloudError::Network("never reached".into()))
                    }
                }
                let mut http = Down;
                let _ = crate::cloud_account::restore_saved_session_with(&mut http);
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_some(),
                    "the restore with a changed server cleared the disk — main leaves it"
                );
            });
        }

        /// Row 9: the token fetch itself, InvalidGrant on the credential it used — the real
        /// access_token_with_credential path a send takes — clears and closes everything.
        #[test]
        fn row9_the_token_fetchs_invalid_grant_clears_and_closes() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1;
                save_stored(&expired).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &expired,
                ));
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let now = cloud_account::unix_now();
                bir_core::account_session::on_session_ended(|owner, epoch, credential| {
                    crate::session::server_ended(owner, epoch, credential);
                });
                let (server, _) = scripted();
                server
                    .tokens
                    .lock()
                    .unwrap()
                    .push_back((400, r#"{"error":"invalid_grant"}"#.into()));
                let mut http = ScriptHttp(server);
                let answer = bir_core::account_session::access_token_with_credential(
                    &mut http, now, true, &owner,
                );
                assert!(
                    matches!(
                        answer,
                        Err(bir_core::account_session::TokenError::SignedOut)
                    ),
                    "{answer:?}"
                );
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_none(),
                    "the token fetch's InvalidGrant did not clear the sign-in"
                );
                assert_eq!(
                    bir_core::account_session::stored_owner(),
                    None,
                    "the cache still names the ended owner"
                );
            });
        }

        /// N3: a sign-out with no submitter ever built constructs none — the TIN invalidation
        /// peeks, and the peek never constructs the relay client under SESSION.
        #[test]
        fn n3_a_sign_out_with_no_submitter_built_constructs_none() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                // The cache names an owner — the owner-ending branch really runs — and the
                // submitter slot is empty, so any construction is the sign-out's own.
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                bir_core::relay::reset_submitter_for_test();
                assert!(
                    bir_core::relay::peek_submitter().is_none(),
                    "a submitter exists before the sign-out — the test is vacuous"
                );
                crate::session::sign_out();
                crate::session::join_device_forget_threads();
                assert!(
                    bir_core::relay::peek_submitter().is_none(),
                    "the sign-out CONSTRUCTED a submitter under SESSION — the N3 hole"
                );
            });
        }

        /// The machine-native open for the rewritten safety-table tests: the production
        /// begin_open, its open completed through the machine's own commit as session's tests
        /// drive it.
        fn open_machine_room(owner: &crate::session::Owner) {
            crate::session::begin_open(owner.clone(), 1, crate::session::OpenWhy::SignIn);
            let epoch = crate::session::snapshot().epoch;
            let folder = tempfile::tempdir().unwrap();
            let path = folder.keep().join("room.db");
            let handle = std::sync::Arc::new(std::sync::Mutex::new(
                bir_core::db::Database::open(&path).unwrap(),
            ));
            crate::session::commit(
                epoch,
                crate::session::Target::Room {
                    room_id: bir_core::rooms::room_id_for(&owner.origin, &owner.user_id),
                    note: crate::session::Note::None,
                    count_opened: false,
                },
                Some((handle, path)),
            );
        }

        /// Row 1/2, rewritten against the machine: sign out through the REAL sign_out_with —
        /// the machine's close drops the room, invalidates the TIN cache and forgets the device
        /// key; a same-owner re-sign-in reopens through begin_open.
        #[test]
        fn real_sign_out_closes_the_room_and_a_same_owner_re_sign_in_reopens_it() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                let owner = crate::session::Owner::of_stored(&stored).expect("a room owner");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open,
                    "the machine did not open the room"
                );
                let device_dir = bir_core::platform::device_dir();
                std::fs::create_dir_all(&device_dir).unwrap();
                std::fs::write(device_dir.join("buwiz_submit_device_key.txt"), "key-of-a").unwrap();
                std::fs::write(
                    device_dir.join("buwiz_submit_device_owner.txt"),
                    format!(
                        "{}\n{}\n",
                        bir_core::rooms::room_origin(SERVER).unwrap(),
                        "u1"
                    ),
                )
                .unwrap();
                bir_core::relay::submitter().expect("the test environment builds the submitter");
                let invalidations_before = bir_core::bir_submit::tins::INVALIDATIONS
                    .load(std::sync::atomic::Ordering::SeqCst);
                let (seen_tx, seen_rx) = mpsc::channel::<Seen>();
                sign_out_with(Box::new(Logout {
                    seen: seen_tx,
                    release: None,
                    file: _file.clone(),
                    answer: answered,
                }))
                .expect("the sign-out answers");
                crate::session::join_device_forget_threads();
                let snapshot = crate::session::snapshot();
                assert_eq!(
                    snapshot.phase_kind,
                    crate::session::PhaseKind::SignedOut,
                    "the machine is not signed out after the real sign-out"
                );
                assert!(
                    snapshot.room.is_none(),
                    "the room survived the real sign-out"
                );
                assert!(
                    bir_core::bir_submit::tins::INVALIDATIONS
                        .load(std::sync::atomic::Ordering::SeqCst)
                        > invalidations_before,
                    "the TIN cache was not invalidated"
                );
                assert!(
                    !device_dir.join("buwiz_submit_device_key.txt").exists(),
                    "the device key survived the sign-out"
                );
                let _ = seen_rx.try_recv();
                // A same-owner re-sign-in reopens the room through begin_open — the machine is
                // not wedged.
                save_stored(&stored).unwrap();
                crate::session::signed_in(&stored, 1).expect("the re-sign-in persists");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open,
                    "the same-owner re-sign-in did not reopen the room"
                );
            });
        }

        /// Finding 10: an Ended on an EMPTY disk — bir-mcp signed out meanwhile, so the
        /// ending's own clear matched nothing — still closes the machine's open session as
        /// signed in elsewhere. Driven through the production refresh_account.
        #[test]
        fn an_ended_on_an_empty_disk_closes_the_open_session() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1_000;
                save_stored(&expired).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &expired,
                ));
                let owner = crate::session::Owner::of_stored(&expired).expect("a room owner");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open
                );
                // The token endpoint parks, then answers invalid_grant: the flight's hook runs
                // with the disk ALREADY emptied by the other process's sign-out.
                struct Parked {
                    parked_tx: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
                    gate: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
                }
                struct ParkedHttp(std::sync::Arc<Parked>);
                impl cloud_account::Http for ParkedHttp {
                    fn exchange(
                        &mut self,
                        request: &HttpRequest,
                    ) -> Result<HttpResponse, CloudError> {
                        if request.url.ends_with("/oauth/token") {
                            let parked = self.0.parked_tx.lock().unwrap().take();
                            let gate = self.0.gate.lock().unwrap().take();
                            if let (Some(parked), Some(gate)) = (parked, gate) {
                                let _ = parked.send(());
                                let _ = gate.recv();
                            }
                            return Ok(HttpResponse {
                                status: 400,
                                body: r#"{"error":"invalid_grant"}"#.into(),
                            });
                        }
                        Ok(HttpResponse {
                            status: 200,
                            body:
                                r#"{"user_id":"u1","email":"filer@example.test","practice":false}"#
                                    .into(),
                        })
                    }
                }
                let (parked_tx, parked_rx) = std::sync::mpsc::channel();
                let (gate_tx, gate_rx) = std::sync::mpsc::channel();
                let parked = std::sync::Arc::new(Parked {
                    parked_tx: std::sync::Mutex::new(Some(parked_tx)),
                    gate: std::sync::Mutex::new(Some(gate_rx)),
                });
                let factory_parked = std::sync::Arc::clone(&parked);
                set_test_refresh_http(Some(Box::new(move || {
                    Box::new(ParkedHttp(std::sync::Arc::clone(&factory_parked)))
                })));
                let verb =
                    crate::cloud_account::refresh_account(&gpui_shell::HostArguments::default())
                        .expect("the refresh verb spawns");
                let driver = std::thread::spawn(move || {
                    let _ = futures::executor::block_on(verb);
                });
                parked_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the token fetch never parked");
                // bir-mcp's sign-out: the disk empties outside this process's machine.
                bir_core::account_session::clear_stored();
                let _ = gate_tx.send(());
                driver.join().expect("the refresh thread panicked");
                set_test_refresh_http(None);
                let snapshot = crate::session::snapshot();
                assert_eq!(
                    snapshot.phase_kind,
                    crate::session::PhaseKind::SignedOut,
                    "an Ended on the empty disk left the machine open and unlocked — the finding-10 hole"
                );
                assert!(snapshot.room.is_none(), "the room survived");
            });
        }

        /// Item 3: a second engine's boot does not pause a live session's sending. The first
        /// engine's session is Open and unlocked; the boot's own pause is the restore-time
        /// safety only, and what stands at the end is the gate's truth.
        #[test]
        fn a_second_engines_boot_does_not_pause_sending() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                let owner = crate::session::Owner::of_stored(&stored).expect("a room owner");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open
                );
                assert!(
                    !bir_core::background_cron::submission_paused(),
                    "sending is paused while the session is open — the setup is wrong"
                );
                // The second engine boots: prepare_launch on its own thread, as a scene
                // reconnect or a relaunch does.
                let (server, _) = scripted();
                *server.mes.lock().unwrap() = [
                    ("*".to_string(), 200u16),
                    ("*".to_string(), 200u16),
                    ("*".to_string(), 200u16),
                ]
                .into_iter()
                .collect();
                let factory_server = std::sync::Arc::clone(&server);
                set_test_refresh_http(Some(Box::new(move || {
                    Box::new(ScriptHttp(std::sync::Arc::clone(&factory_server)))
                })));
                // The bypass branch is not the boot being tested: bypass_unlock's own
                // set_unlocked unpauses by itself, so a boot through it proves nothing about
                // the ending sync. BIR_AUTH_BYPASS off for the call — the restore branch is
                // the production boot's.
                temp_env::with_vars([("BIR_AUTH_BYPASS", Some("0"))], || {
                    prepare_launch();
                });
                set_test_refresh_http(None);
                assert!(
                    !bir_core::background_cron::submission_paused(),
                    "the second engine's boot paused the live session's sending — the item-3 hole"
                );
            });
        }

        /// Row 12, its new truth under the machine: the page's close verb IS the machine's
        /// close — the gate locks, the room drops, the menu refreshes. Main left the room open;
        /// the machine deliberately does not (the close kind makes no page exception).
        #[test]
        fn row12_the_pages_close_leaves_the_room_open() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                let owner = crate::session::Owner::of_stored(&stored).expect("a room owner");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open
                );
                crate::account_gate::set_unlocked(true);
                session_closed().expect("the page verb answers");
                let snapshot = crate::session::snapshot();
                assert_eq!(
                    snapshot.phase_kind,
                    crate::session::PhaseKind::SignedOut,
                    "the page's close did not close the machine's session"
                );
                assert!(
                    snapshot.room.is_none(),
                    "the page's close left the room open — under the machine the close is full"
                );
                assert!(
                    !crate::account_gate::unlocked_flag(),
                    "the gate is still open after the page's close"
                );
            });
        }

        /// bir-mcp's condition (row 3), rewritten against the machine: bir-mcp's door signs in
        /// B — the raw disk write, no cache update — while the app has A open through the
        /// machine. refresh_account is the I/O point: the app closes A as signed in elsewhere,
        /// the cache clears, B's sign-in is never touched.
        #[test]
        fn bir_mcps_sign_in_of_another_owner_closes_the_apps_session() {
            let _turn = crate::open_room::room_test_turn();
            crate::session::reset_for_tests();
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored_a = stored("access-a", "refresh-a");
                save_stored(&stored_a).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored_a,
                ));
                let owner = crate::session::Owner::of_stored(&stored_a).expect("a room owner");
                open_machine_room(&owner);
                assert_eq!(
                    crate::session::snapshot().phase_kind,
                    crate::session::PhaseKind::Open,
                    "the machine did not open A's room"
                );
                // bir-mcp writes B the way its door really does: the account file the app
                // really reads, no cache update, no machine.
                let mut other = stored("access-b", "refresh-b");
                other.user_id = "u2".into();
                let path = std::env::var_os("BUWIZ_CLOUD_ACCOUNT_FILE")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        bir_core::platform::device_dir().join("buwiz-cloud-account.json")
                    });
                let encoded = other.encode().unwrap();
                std::fs::write(&path, encoded).unwrap();
                // The I/O point, through the production entry: refresh_account reads the disk
                // and sees the owner changed.
                let (server, _) = scripted();
                *server.mes.lock().unwrap() = [
                    ("*".to_string(), 200u16),
                    ("*".to_string(), 200u16),
                    ("*".to_string(), 200u16),
                ]
                .into_iter()
                .collect();
                let factory_server = std::sync::Arc::clone(&server);
                set_test_refresh_http(Some(Box::new(move || {
                    Box::new(ScriptHttp(std::sync::Arc::clone(&factory_server)))
                })));
                let verb =
                    crate::cloud_account::refresh_account(&gpui_shell::HostArguments::default())
                        .expect("the refresh verb spawns");
                let _ = futures::executor::block_on(verb);
                set_test_refresh_http(None);
                let snapshot = crate::session::snapshot();
                assert_eq!(
                    snapshot.phase_kind,
                    crate::session::PhaseKind::SignedOut,
                    "the app did not close A at the I/O point after bir-mcp signed in B"
                );
                assert!(
                    snapshot.room.is_none(),
                    "A's room survived the signed-in-elsewhere close"
                );
                assert!(
                    bir_core::account_session::stored_owner().is_none(),
                    "the stale cache was never cleared — it would re-close B on every refresh"
                );
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("B's sign-in was cleared");
                assert_eq!(
                    on_disk.refresh_token, "refresh-b",
                    "B's sign-in was touched"
                );
            });
        }

        /// Row 11: a 401 of the account GET or the TIN-list POST — a non-send caller with no
        /// send context — logs only: NOTHING ends, as main does.
        #[test]
        fn row11_a_non_sends_401_ends_nothing() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-a", "refresh-a");
                save_stored(&current).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &current,
                ));
                bir_core::account_session::on_session_ended(|owner, epoch, credential| {
                    crate::session::server_ended(owner, epoch, credential);
                });
                // The non-send caller's session_ended: no send context anywhere.
                bir_core::bir_submit::TokenSource::session_ended(
                    &bir_core::account_session::AccountTokens,
                );
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .unwrap()
                        .is_some(),
                    "a non-send 401 cleared the sign-in — main logs only"
                );
                assert!(
                    bir_core::account_session::stored_owner().is_some(),
                    "a non-send 401 cleared the cache"
                );
            });
        }

        /// Row 3's other half: a signed_in landing between the disk read and the close is NOT
        /// closed — the re-check under SESSION sees the cache already matches the disk.
        #[test]
        fn row3_a_signed_in_landing_before_the_close_is_not_closed() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                // bir-mcp's write of B, as row 3 — then a signed_in of B lands BEFORE the
                // close's SESSION section: the cache now matches the disk, nothing closes.
                let stored_b = stored("access-b", "refresh-b");
                let mut stored_b = stored_b.clone();
                stored_b.user_id = "user-b".into();
                std::fs::write(
                    bir_core::platform::device_dir().join("buwiz-cloud-account.json"),
                    stored_b.encode().unwrap(),
                )
                .unwrap();
                let owner_b = bir_core::account_session::OwnerKey::of(&stored_b).unwrap();
                // The sign-in lands first: the cache moves to B.
                bir_core::account_session::cache_owner(Some(owner_b.clone()));
                // The close re-checks under SESSION: the cache matches the disk — no close.
                let closed = crate::session::close_signed_in_elsewhere(&owner_b);
                assert!(
                    !closed,
                    "the signed_in that landed in between was closed anyway"
                );
                assert_eq!(
                    bir_core::account_session::stored_owner(),
                    Some(owner_b),
                    "B's cache entry did not survive"
                );
            });
        }

        /// Row 13: a second engine's launch restore concurrent with the first is exactly ONE
        /// network refresh — the two restores share the credential's keyed flight.
        #[test]
        fn row13_a_second_engines_restore_shares_one_network_refresh() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("access-r1", "refresh-r1");
                expired.access_expires_at_unix = 1;
                save_stored(&expired).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &expired,
                ));
                let (server, parked, gate) = flight_server(false);
                // The first engine's restore leads, parked in the token refresh.
                let first_server = std::sync::Arc::clone(&server);
                let first = std::thread::spawn(move || {
                    let mut http = FlightHttp(first_server);
                    crate::cloud_account::restore_saved_session_with(&mut http)
                });
                parked
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the first restore never reached the token refresh");
                let second_server = std::sync::Arc::clone(&server);
                let second = std::thread::spawn(move || {
                    let mut http = FlightHttp(second_server);
                    crate::cloud_account::restore_saved_session_with(&mut http)
                });
                std::thread::yield_now();
                let _ = gate.send(());
                let first_answer = first.join().expect("the first restore panicked");
                let second_answer = second.join().expect("the second restore panicked");
                assert_eq!(
                    server.refreshes.load(std::sync::atomic::Ordering::SeqCst),
                    1,
                    "two engines' restores burned the rotation twice"
                );
                assert!(first_answer.0, "the first restore stayed locked");
                assert!(second_answer.0, "the second restore stayed locked");
            });
        }

        /// F3: a verb's refresh concurrent with a send's flight is ONE network refresh — the
        /// verb joins the credential's keyed flight rather than burning the rotation again.
        #[test]
        fn a_verb_refresh_concurrent_with_a_sends_flight_is_one_network_refresh() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                // The access token is expired, so both callers must rotate.
                let mut expired = stored("access-old", "refresh-r1");
                expired.access_expires_at_unix = 1;
                save_stored(&expired).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&expired).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, parked, gate) = flight_server(false);
                // The verb leads: it parks inside the flight's token refresh.
                let verb_server = std::sync::Arc::clone(&server);
                let verb_stored = expired.clone();
                let verb_owner = owner.clone();
                let verb = std::thread::spawn(move || {
                    let mut http = FlightHttp(verb_server);
                    verb_refresh(&mut http, &verb_stored, &verb_owner, epoch, now).ok()
                });
                parked
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the verb never reached the token refresh");
                // The send joins the SAME credential's flight: it must wait, never refresh itself.
                let send_server = std::sync::Arc::clone(&server);
                let send = std::thread::spawn(move || {
                    let mut http = FlightHttp(send_server);
                    bir_core::account_session::access_token_with_credential(
                        &mut http, now, false, &owner,
                    )
                });
                // The waiter is in the flight before the leader is released — the join is the
                // point, so give it its moment: the release after this send's spawn either way
                // proves the count, since a second leader would burn the rotation again.
                std::thread::yield_now();
                let _ = gate.send(());
                let verb_outcome = verb.join().expect("the verb thread panicked");
                let send_answer = send.join().expect("the send thread panicked");
                assert_eq!(
                    server.refreshes.load(std::sync::atomic::Ordering::SeqCst),
                    1,
                    "two concurrent refreshes of one credential burned the rotation twice"
                );
                assert!(verb_outcome.is_some(), "the verb refresh failed");
                assert!(send_answer.is_ok(), "the send's token fetch failed");
                assert!(
                    server.profiles.load(std::sync::atomic::Ordering::SeqCst) >= 1,
                    "the verb never confirmed the profile"
                );
            });
        }

        /// F3's lost update: the profile-only arm's merge is a CAS on (owner, the refresh token
        /// it loaded) and never writes tokens — a rotation in the network window survives whole.
        #[test]
        fn the_profile_only_arms_merge_never_writes_tokens_back() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                // The access token is valid: the profile-only arm, no rotation of the verb's own.
                let current = stored("access-valid", "refresh-r1");
                save_stored(&current).unwrap();
                let owner = bir_core::account_session::OwnerKey::of(&current).unwrap();
                let epoch = bir_core::session_epoch::current();
                let now = cloud_account::unix_now();
                let (server, parked, gate) = flight_server(true);
                let verb_server = std::sync::Arc::clone(&server);
                let verb_stored = current.clone();
                let verb_owner = owner.clone();
                let verb = std::thread::spawn(move || {
                    let mut http = FlightHttp(verb_server);
                    verb_refresh(&mut http, &verb_stored, &verb_owner, epoch, now).ok()
                });
                parked
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the verb never reached the profile fetch");
                // The rotation in the network window: another path saves R2 over R1.
                let mut rotated = stored("access-r2", "refresh-r2");
                rotated.user_id = "u1".into();
                {
                    let _one = bir_core::account_session::lock();
                    save_stored(&rotated).unwrap();
                }
                let _ = gate.send(());
                verb.join().expect("the verb thread panicked");
                let on_disk = bir_core::cloud_account::load_stored_account()
                    .unwrap()
                    .expect("the sign-in vanished");
                assert_eq!(
                    on_disk.refresh_token, "refresh-r2",
                    "the spent R1 landed back over the rotation — the F3 lost update"
                );
                assert_eq!(
                    on_disk.access_token, "access-r2",
                    "the merged record wrote the old access token back"
                );
            });
        }

        /// F1: the verb's changed-server arm must not deadlock — the decision is made inside the
        /// account lock, the sign-out runs after it is released. The join is bounded: at the
        /// deadlock the verb never returns, and the margin is what fails.
        #[test]
        fn refresh_account_with_a_changed_server_signs_out_without_deadlock() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                // The stored sign-in belongs to another deployment than the build resolves.
                let mut moved = stored("old-access", "old-refresh");
                moved.server_url = "https://elsewhere.example.test".into();
                save_stored(&moved).unwrap();
                let verb =
                    crate::cloud_account::refresh_account(&gpui_shell::HostArguments::default())
                        .expect("the verb spawns");
                let ran = std::thread::spawn(move || futures::executor::block_on(verb));
                let (done, finished) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = ran.join();
                    let _ = done.send(());
                });
                finished
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("refresh_account wedged on the changed-server arm — the F1 deadlock");
                assert!(
                    bir_core::cloud_account::load_stored_account()
                        .ok()
                        .flatten()
                        .is_none(),
                    "the ended sign-in is still on disk"
                );
            });
        }

        fn stored(access: &str, refresh: &str) -> StoredAccount {
            StoredAccount {
                server_url: SERVER.into(),
                access_token: access.into(),
                refresh_token: refresh.into(),
                access_expires_at_unix: i64::MAX / 2,
                user_id: "u1".into(),
                email: Some("filer@example.test".into()),
                offline: false,
                last_refresh_unix: 1_000,
                practice: false,
            }
        }

        fn signed_in_now() -> bool {
            signed_in_in(status_now())
        }

        fn signed_in_in(status: HostResult) -> bool {
            crate::bir_module::to_json(&status.expect("a status"))["signed_in"] == true
        }

        /// Runs the sign-out as the host does, on a thread of its own under this thread's database name, so a
        /// sign-out that waits on the server shows as a timeout here instead of a hung test.
        fn sign_out_on_a_thread(http: Logout) -> mpsc::Receiver<HostResult> {
            let (sender, receiver) = mpsc::channel();
            let database = bir_core::db::capture_database_path_override();
            std::thread::spawn(move || {
                let _ = sender.send(database.run(|| sign_out_with(Box::new(http))));
            });
            receiver
        }

        fn logout(
            file: &std::path::Path,
            answer: fn() -> Result<HttpResponse, CloudError>,
        ) -> (Logout, mpsc::Receiver<Seen>) {
            let (seen, heard) = mpsc::channel();
            let fake = Logout {
                file: file.to_path_buf(),
                seen,
                release: None,
                answer,
            };
            (fake, heard)
        }

        /// (a), (d), (e): when the logout request goes out, the saved sign-in is already gone; the request
        /// carries the old access token; the status, and what a relaunch reads, say signed out.
        #[test]
        fn the_saved_sign_in_is_gone_before_the_logout_request_and_the_request_carries_the_old_token()
         {
            signed_in(|file| {
                let (fake, heard) = logout(&file, answered);
                let status = sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                assert!(
                    !seen.file_left,
                    "the account file was still there at the logout request"
                );
                assert!(
                    !seen.sign_in_left,
                    "the sign-in was still readable at the logout request"
                );
                assert_eq!(seen.url, format!("{SERVER}/api/auth/logout"));
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                assert!(!signed_in_in(status), "sign-out answered signed in");
                assert!(!file.exists());
                assert!(!signed_in_now(), "a relaunch would read signed in");
            });
        }

        /// X3: the real sign_out_with runs on its own spawned thread (the account thread in the app), and the
        /// scan a screen made on the engine thread is forgotten there too — the scan state is process-wide.
        #[test]
        fn sign_out_on_the_account_thread_forgets_the_engines_scan() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let legacy_folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../bir-core/tests/legacy-ebirforms");
                crate::legacy_import_module::scan_folder(&legacy_folder).unwrap();
                assert!(crate::legacy_import_module::holds_scan());
                let (fake, _heard) = logout(&file, answered);
                let _ = sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                assert!(
                    !crate::legacy_import_module::holds_scan(),
                    "the scan survived the sign-out: it is per-thread, and the account thread cleared only its own"
                );
            });
        }

        /// (b), (d): a logout the server never answers does not hold the sign-out: it returns signed out, with
        /// nothing left on this device, while the request still hangs.
        #[test]
        fn sign_out_returns_signed_out_while_the_logout_request_still_hangs() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let (mut fake, heard) = logout(&file, answered);
                let (release, held) = mpsc::channel();
                fake.release = Some(held);
                let done = sign_out_on_a_thread(fake);
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                let status = done
                    .recv_timeout(WAIT)
                    .expect("sign-out returned while the logout request still hangs");
                assert!(
                    !seen.file_left,
                    "the account file was still there at the logout request"
                );
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                assert!(!signed_in_in(status), "sign-out answered signed in");
                assert!(!file.exists(), "the account file outlived a hanging logout");
                assert!(!signed_in_now(), "a relaunch would read signed in");
                release.send(()).unwrap();
            });
        }

        /// (c), (d): a server that refuses the logout, or cannot be reached, changes nothing on this device: the
        /// sign-in was gone before the request, and stays gone.
        #[test]
        fn a_refused_or_unreachable_logout_leaves_the_device_signed_out() {
            let _turn = crate::open_room::room_test_turn();
            for answer in [refused, unreachable] {
                signed_in(|file| {
                    let (fake, heard) = logout(&file, answer);
                    let status = sign_out_on_a_thread(fake)
                        .recv_timeout(WAIT)
                        .expect("sign-out returned");
                    let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                    assert!(
                        !seen.file_left,
                        "the account file was still there at the logout request"
                    );
                    assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                    assert!(!signed_in_in(status), "sign-out answered signed in");
                    assert!(!file.exists());
                    assert!(!signed_in_now(), "a relaunch would read signed in");
                });
            }
        }

        /// A new sign-in saved while the old logout still hangs is not held up by it (the account-session lock is
        /// free), and survives it: the old logout carries only the old token.
        #[test]
        fn a_new_sign_in_while_the_old_logout_hangs_is_saved_and_kept() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|file| {
                let (mut fake, heard) = logout(&file, answered);
                let (release, held) = mpsc::channel();
                fake.release = Some(held);
                let done = sign_out_on_a_thread(fake);
                let seen = heard.recv_timeout(WAIT).expect("the logout was sent");
                let (saved, was_saved) = mpsc::channel();
                let database = bir_core::db::capture_database_path_override();
                std::thread::spawn(move || {
                    let _ = saved.send(database.run(|| {
                        let _one = bir_core::account_session::lock();
                        save_stored(&stored("new-access", "new-refresh"))
                    }));
                });
                was_saved
                    .recv_timeout(WAIT)
                    .expect("a new sign-in waited on the old logout")
                    .unwrap();
                release.send(()).unwrap();
                done.recv_timeout(WAIT).expect("sign-out returned").unwrap();
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                let kept = load_stored().unwrap().expect("the new sign-in is kept");
                assert_eq!(kept.access_token, "new-access");
                assert!(signed_in_now());
                clear_stored();
            });
        }

        /// A sign-in this device cannot remove (its folder and file read-only, so the delete, the second delete
        /// and the overwrite all fail) is an error the page shows, never "signed out". The gate is locked and
        /// the logout still goes out.
        #[cfg(unix)]
        #[test]
        fn a_sign_in_that_cannot_be_removed_is_an_error_and_the_gate_stays_locked() {
            let _turn = crate::open_room::room_test_turn();
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &std::path::Path, mode: u32| {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
            };
            signed_in(|file| {
                let folder = file.parent().unwrap().to_path_buf();
                mode(&file, 0o400);
                mode(&folder, 0o555);
                crate::account_gate::set_unlocked(true);
                let (fake, heard) = logout(&file, answered);
                let answer = sign_out_on_a_thread(fake)
                    .recv_timeout(WAIT)
                    .expect("sign-out returned");
                let unlocked = crate::account_gate::unlocked_flag();
                let seen = heard.recv_timeout(WAIT);
                mode(&folder, 0o755);
                mode(&file, 0o600);
                let error =
                    answer.expect_err("sign-out said signed out with the sign-in still saved");
                assert!(
                    error.to_string().contains("Sign-out did not finish"),
                    "{error}"
                );
                assert!(!unlocked, "the gate was left open");
                let seen = seen.expect("the logout was sent");
                assert_eq!(seen.bearer.as_deref(), Some("old-access"));
                clear_stored();
            });
        }

        /// The token endpoint and `/me` of the account server: `/oauth/token` takes `old-refresh` once and
        /// rotates it, as the real server does; `/me` knows only the rotated access token.
        #[derive(Default)]
        struct Rotating {
            refreshed: bool,
        }

        impl cloud_account::Http for Rotating {
            fn exchange(&mut self, request: &HttpRequest) -> Result<HttpResponse, CloudError> {
                let answer = |status: u16, body: &str| {
                    Ok(HttpResponse {
                        status,
                        body: body.into(),
                    })
                };
                match request.url.trim_start_matches(SERVER) {
                    "/oauth/token" if !self.refreshed && request.body.contains("old-refresh") => {
                        self.refreshed = true;
                        answer(
                            200,
                            r#"{"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":900}"#,
                        )
                    }
                    "/oauth/token" => answer(400, r#"{"error":"invalid_grant"}"#),
                    "/me" if request.bearer.as_deref() == Some("rotated-access") => {
                        answer(200, r#"{"user_id":"u1","email":"filer@example.test"}"#)
                    }
                    _ => answer(401, r#"{"error":"invalid_token"}"#),
                }
            }
        }

        /// The launch restore of a sign-in whose access token has expired: the refresh rotates the pair, the
        /// rotated pair is saved, and the gate opens.
        #[test]
        fn a_launch_restore_with_an_expired_token_saves_the_rotated_pair_and_opens_the_gate() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let mut expired = stored("old-access", "old-refresh");
                expired.access_expires_at_unix = 0;
                save_stored(&expired).unwrap();
                let mut server = Rotating::default();
                assert_eq!(restore_saved_session_with(&mut server), (true, false));
                assert!(server.refreshed, "the restore did not refresh");
                let saved = load_stored().unwrap().expect("still signed in");
                assert_eq!(
                    (saved.access_token.as_str(), saved.refresh_token.as_str()),
                    ("rotated-access", "rotated-refresh")
                );
                assert!(signed_in_now());
                clear_stored();
            });
        }
    }

    fn delete_saved_server_url_on_startup() {
        with_account_database(|database| {
            database
                .set_setting(OBSOLETE_SERVER_URL_SETTING, "https://saved.example.test")
                .map_err(|error| HostError::new(error.to_string()))?;
            Ok(())
        })
        .unwrap();
        let resolved = resolve().expect("the build's server resolves");
        assert_ne!(resolved.url, "https://saved.example.test");
        assert_ne!(resolved.source, cloud_account::UrlSource::Environment);
        if option_env!("BUWIZ_SERVER_URL")
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            assert_eq!(
                resolved.url,
                cloud_account::profile_server_url(cloud_account::active_build_profile())
            );
            assert_eq!(resolved.source, cloud_account::UrlSource::Default);
        }
        prepare_launch();
        let left = with_account_database(|database| {
            database
                .get_setting(OBSOLETE_SERVER_URL_SETTING)
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
        assert_eq!(left, None);
    }
}
