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
    bir_core::account_session::on_session_ended(crate::session::server_ended);
    bir_core::background_cron::set_submission_paused(true);
    forget_saved_server_url();
    drop_old_welcomed_list();
    if crate::account_gate::auth_bypass_enabled() {
        bir_core::transport::set_practice_account(false);
        crate::account_gate::set_unlocked(true);
        return;
    }
    let (unlocked, practice) = restore_saved_session();
    // Set before the gate opens, so no send can start ahead of it.
    bir_core::transport::set_practice_account(unlocked && practice);
    crate::account_gate::set_unlocked(unlocked);
    if unlocked && let Ok(Some(stored)) = load_stored() {
        // The launch resume (§A.4): a claim cut mid-move finishes here, before anything opens the
        // room. R1's order is the room open's own guard.
        crate::room_session::open_session_room(&stored);
    }
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
    bir_core::transport::set_practice_account(practice);
    // F10/F8': the persist and the cache are the machine's one step. Commit 3 adds the begin_open
    // inside it; register_device_soon stays here until commit moves it into commit.
    crate::session::signed_in(&stored_from(secrets, now)).map_err(host_error)?;
    bir_core::relay::forget_device();
    register_device_soon();
    Ok(())
}

/// Adds this device to the account (bir-core `relay::register_after_sign_in`) on a thread of its own, so signing in never
/// waits for it: a failure is logged there and the first send registers as it always did; a practice account adds none.
fn register_device_soon() {
    let database = bir_core::db::capture_database_path_override();
    let started = std::thread::Builder::new()
        .name("buwiz-device-register".into())
        .spawn(move || {
            database.run(bir_core::relay::register_after_sign_in);
        });
    if let Err(error) = started {
        tracing::warn!("device registration not started ({error}); the first send registers");
    }
}

/// Test seam: the Http refresh_account uses. A test parks the network by installing a factory
/// whose answer blocks on a harness gate; production never sets it, and the blocking default is
/// the real client.
#[cfg(test)]
static TEST_REFRESH_HTTP: std::sync::Mutex<
    Option<Box<dyn Fn() -> Box<dyn cloud_account::Http + Send> + Send>>,
> = std::sync::Mutex::new(None);

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
                let saved = {
                    let _one = bir_core::account_session::lock();
                    bir_core::account_session::merge_profile_if(
                        owner,
                        &stored.refresh_token,
                        &user_id,
                        email,
                        practice,
                        false,
                    )
                    .map_err(host_error)?
                };
                return Ok(if saved {
                    VerbRefresh::Online { practice }
                } else {
                    VerbRefresh::Gone
                });
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
    match bir_core::account_session::access_token_with_credential(http, now, force, owner) {
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
                Ok((user_id, email, practice)) => {
                    let saved = {
                        let _one = bir_core::account_session::lock();
                        bir_core::account_session::merge_profile_if(
                            owner,
                            &fresh.refresh_token,
                            &user_id,
                            email,
                            practice,
                            false,
                        )
                        .map_err(host_error)?
                    };
                    Ok(if saved {
                        VerbRefresh::Online { practice }
                    } else {
                        VerbRefresh::Gone
                    })
                }
                Err(CloudError::Network(_)) => Ok(VerbRefresh::Offline),
                Err(CloudError::InvalidGrant) => {
                    // The just-refreshed access token rejected at once: the sign-in is ended.
                    // N5: the credential is the rotation's OWN — the refresh token now stored —
                    // never the pre-rotation one, which matches nothing and would answer Ended
                    // while the sign-in stays stored.
                    crate::session::server_ended(owner.clone(), epoch, credential);
                    Ok(VerbRefresh::Ended)
                }
                Err(error) => Err(host_error(error)),
            }
        }
        Err(bir_core::account_session::TokenError::SignedOut) => {
            // The flight's InvalidGrant already ran the ending hook exactly once for the
            // credential; this call is the no-op confirmation when it did, and the ending
            // itself when the verb is the only witness.
            crate::session::server_ended(owner.clone(), epoch, stored.refresh_token.clone());
            Ok(VerbRefresh::Ended)
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
            crate::session::end_changed_server();
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
            VerbRefresh::Offline | VerbRefresh::Ended | VerbRefresh::Gone => {}
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
/// it. It carries the old sign-in's access token, which cannot end a newer sign-in, and nothing else of it. The
/// answer changes nothing here (the sign-in is already gone on this device); it is logged, without the token.
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
    let _room_open = spawn_account("buwiz-room-open", || {
        if let Ok(Some(stored)) = load_stored() {
            crate::room_session::open_session_room(&stored);
        }
        crate::account_gate::set_unlocked(true);
        crate::bir_module::open_database_at_launch();
        crate::hooks::hooks().refresh_menu();
        Ok(HostValue::Null)
    });
    crate::hooks::hooks().start_updater();
    Ok(HostValue::Null)
}

fn session_closed() -> HostResult {
    crate::account_gate::set_unlocked(false);
    bir_core::transport::set_practice_account(false);
    crate::hooks::hooks().refresh_menu();
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

    /// F10: session_opened never runs the claim on the calling thread — it returns while the
    /// room work is still blocked, and the work finishes on the spawned thread.
    #[test]
    fn session_opened_returns_while_the_room_work_is_still_running() {
        use_ephemeral_database();
        let release = crate::room_session::block_next_open_session_for_test();
        let opened = std::thread::spawn(session_opened);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            opened.is_finished(),
            "session_opened is still blocked in the room work: the claim runs on the calling thread"
        );
        let _ = release.send(());
        let _ = opened.join();
        crate::account_gate::set_unlocked(false);
    }

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

        impl ScriptServer {
            /// (required bearer or "*" for any, status). 401 answers any other bearer too when
            /// the required one is not met — the row's rejection is the point.
            fn me(&self, answers: &[(&str, u16)]) -> std::sync::Arc<ScriptServer> {
                let server = std::sync::Arc::new(ScriptServer::default());
                *server.mes.lock().unwrap() = answers
                    .iter()
                    .map(|(bearer, status)| (bearer.to_string(), *status))
                    .collect();
                server
            }
        }

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

        /// N5 (row 6): after the forced rotation /me rejects the NEW token too — the ending is
        /// matched on the ROTATED credential, so the stored sign-in really is cleared.
        #[test]
        fn row6_a_rejected_new_token_signs_out_on_the_rotated_credential() {
            let _turn = crate::open_room::room_test_turn();
            signed_in(|_file| {
                let current = stored("access-r1", "refresh-r1");
                save_stored(&current).unwrap();
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
                bir_core::account_session::on_session_ended(crate::session::server_ended);
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
                bir_core::account_session::on_session_ended(crate::session::server_ended);
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
                bir_core::account_session::on_session_ended(crate::session::server_ended);
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
                verb.join().expect("the verb thread panicked");
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
                futures::executor::block_on(verb);
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
                bir_core::account_session::on_session_ended(crate::session::server_ended);
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

        /// Row 12: the page's close verb locks the gate and refreshes the menu but does NOT
        /// close the room — as main, until commit 3.
        #[test]
        fn row12_the_pages_close_leaves_the_room_open() {
            let _turn = crate::open_room::room_test_turn();
            crate::open_room::reset_for_tests();
            crate::room_session::reset_for_tests();
            bir_core::db::set_open_room_database(None);
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                let opened = crate::room_session::open_session_room(&stored);
                assert!(
                    crate::open_room::current().is_some(),
                    "the real path opened no room ({opened:?})"
                );
                crate::account_gate::set_unlocked(true);
                session_closed().expect("the page verb answers");
                assert!(
                    crate::open_room::current().is_some(),
                    "the page's close closed the room — main does not"
                );
                assert!(
                    !crate::account_gate::unlocked_flag(),
                    "the gate is still open after the page's close"
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
                bir_core::account_session::on_session_ended(crate::session::server_ended);
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

        /// F2 (the standing rule): the REAL sign-out path closes the room, invalidates the TIN
        /// cache and forgets the device key, and a same-owner re-sign-in reopens the room. Until
        /// commit 3 wires the machine, the close verbs run the old slot's close themselves.
        #[test]
        fn real_sign_out_closes_the_room_and_a_same_owner_re_sign_in_reopens_it() {
            let _turn = crate::open_room::room_test_turn();
            crate::open_room::reset_for_tests();
            crate::room_session::reset_for_tests();
            bir_core::db::set_open_room_database(None);
            let device = tempfile::tempdir().unwrap();
            bir_core::platform::set_device_dir_override(Some(device.path().to_path_buf()));
            signed_in(|_file| {
                let stored = stored("access-a", "refresh-a");
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                // The room opens through today's real path: session_opened's own entry.
                let opened = crate::room_session::open_session_room(&stored);
                assert!(
                    crate::open_room::current().is_some(),
                    "the real path opened no room ({opened:?})"
                );
                // A device key owned by the account, as a registration leaves it — in the device
                // folder as this environment resolves it, the same path the forget deletes from.
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
                assert!(
                    bir_core::relay::device_key_owner().is_some(),
                    "the seeded owner record does not read back"
                );
                // The submitter exists, as it does on any real path that has sent or listed
                // TINs — N3 invalidates only an existing one's cache, never constructs one.
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
                // The device-key forget is the backgrounded half of the close: join it before
                // judging, as the machine's own tests do.
                crate::session::join_device_forget_threads();
                assert!(
                    crate::open_room::current().is_none(),
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
                // A same-owner re-sign-in reopens the room — the slot is not wedged.
                save_stored(&stored).unwrap();
                bir_core::account_session::cache_owner(bir_core::account_session::OwnerKey::of(
                    &stored,
                ));
                let reopened = crate::room_session::open_session_room(&stored);
                assert!(
                    bir_core::db::open_room_id().is_some(),
                    "the re-sign-in did not reopen the room ({reopened:?})"
                );
            });
            bir_core::platform::set_device_dir_override(None);
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
                sign_out_on_a_thread(fake)
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
