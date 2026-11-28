//! Sending a return to the BIR through the Buwiz relay.
//!
//! The client is `crates/bir-submit`: it seals the plain return for Buwiz's
//! sender, pipes it through this computer to the BIR's own hosts (only the
//! ones on Buwiz's signed trust list) and comes back with a signed receipt.
//! This module is the queue's side of it: one [`Submitter`] per process, with
//! the app's one sign-in ([`crate::account_session::AccountTokens`]), and the
//! translation of the client's answers into the queue's outcomes
//! ([`send_outcome`]).
//!
//! What the queue does with each outcome is in
//! [`crate::forms::queueable::NoReceipt`]. The rules that must hold here:
//! a return the BIR may have is never an outcome that lets the queue send it
//! again, and a hold is never counted as a failed try.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bir_submit::wire::{TinRefusal, is_final_reason};
use bir_submit::{SendError, Sent, SubmitConfig, Submitter};

use crate::transport::{StoreReport, TransportError};

/// The longest one send may take before its outcome is called unknown. The
/// relay pipe itself may run 9 minutes.
const SEND_LIMIT: Duration = Duration::from_secs(600);

/// How much of a refusal's sentence is kept on the return (its code is
/// always kept whole).
const REFUSAL_CHARS: usize = 300;

/// The process's submitter, with the server it was made for.
static SUBMITTER: Mutex<Option<(String, Arc<Submitter>)>> = Mutex::new(None);

/// An http(s) origin on this computer.
fn on_this_computer(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = parsed
        .host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Whether a build may send through the Buwiz server at `url`. A development
/// build sends only through a server on this computer: it can never file a
/// real return through production, whatever its environment says.
fn server_allowed(url: &str, development_build: bool) -> Result<(), TransportError> {
    if development_build && !on_this_computer(url) {
        return Err(TransportError::Config(
            "A development build sends only through a Buwiz server on this computer (BUWIZ_SERVER_URL=http://localhost:PORT)."
                .into(),
        ));
    }
    Ok(())
}

/// Whether a build may send on a trust list signed by `trust_keys`. A development build never trusts the key that
/// signs production's list (the one compiled in, which names the BIR's real hosts): it needs a test key in
/// `BUWIZ_TRUST_KEYS`, so even a server on this computer that serves production's list is refused.
fn trust_keys_allowed(
    trust_keys: &[String],
    development_build: bool,
) -> Result<(), TransportError> {
    let production = |key: &String| bir_submit::trust::TRUST_KEYS.contains(&key.as_str());
    if development_build && (trust_keys.is_empty() || trust_keys.iter().any(production)) {
        return Err(TransportError::Config(
            "A development build sends only on a trust list signed by a test key (BUWIZ_TRUST_KEYS), never on production's."
                .into(),
        ));
    }
    Ok(())
}

/// The trust list's pinned keys for this process: `BUWIZ_TRUST_KEYS` as [`crate::config::var`] reads it (the
/// process environment, or an Android development build's baked list), under `bir_submit`'s own rule: it replaces
/// the production key only in a debug or `lab` build.
fn trust_keys() -> Vec<String> {
    bir_submit::trust::pinned_keys_from(
        crate::config::var(bir_submit::trust::KEYS_ENV)
            .ok()
            .as_deref(),
    )
}

/// The Buwiz server this build may send through, or why it may not: the configured server, which for a
/// development build must be on this computer and trusted through a test key. No request is made.
pub fn server() -> Result<String, TransportError> {
    let server = crate::cloud_account::configured_server()
        .map_err(|error| TransportError::Config(error.to_string()))?;
    server_allowed(&server.url, cfg!(debug_assertions))?;
    trust_keys_allowed(&trust_keys(), cfg!(debug_assertions))?;
    Ok(server.url)
}

/// Where the device key and id live when the keychain is not used: the device folder, as the saved sign-in
/// is (`platform::device_dir`), whichever room is open. A scratch database (a development, test or agent run) gets
/// a scratch device, never a file in the app's real data folder.
pub(crate) fn secrets_dir() -> std::path::PathBuf {
    crate::platform::device_dir()
}

thread_local! {
    /// The key material of the key the gate created ON THIS THREAD, if it just did (F6): the
    /// registration's discard deletes only a key it knows is its own — never whatever the disk
    /// happens to hold after another registration interleaved, which a disk read back could
    /// not tell apart.
    static CREATED_ON_THIS_THREAD: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

fn note_key_created_here(raw: String) {
    CREATED_ON_THIS_THREAD.with(|slot| *slot.borrow_mut() = Some(raw));
}

fn take_key_created_here() -> Option<String> {
    CREATED_ON_THIS_THREAD.with(|slot| slot.borrow_mut().take())
}

/// The creation gate of F7, installed once: bir-submit's lazy first key is created under
/// DEVICE_KEY, and the owner record is written in the same step from the owner cache — no key
/// is ever created outside the funnel or, while anyone is signed in, without its owner. The
/// cache is the only owner source allowed here: the disk read would take the account lock,
/// which is never taken under DEVICE_KEY. A creation with nobody cached — no send should ever
/// reach one — leaves the key unowned for the adoption at the next sign-in's first commit.
pub fn install_device_key_gate() {
    bir_submit::client::set_device_key_gate(std::sync::Arc::new(|create| {
        let _device_key = DEVICE_KEY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = create()?;
        // The creation marker: inside DEVICE_KEY on the creating thread, the stored key IS the
        // one just made — a discard on this thread can only ever mean this one.
        if let Some(raw) = stored_device_key_raw() {
            note_key_created_here(raw);
        }
        let owner = crate::account_session::SEND_CONTEXT
            .try_with(|context| context.owner.clone())
            .ok()
            .or_else(crate::account_session::stored_owner);
        if let Some(owner) = owner {
            write_device_owner(&owner);
        } else {
            tracing::warn!(
                "a device key was created with nobody signed in: unowned until adoption"
            );
        }
        Ok(key)
    }));
}

/// The already-constructed submitter for the configured server, if one exists — a PEEK that
/// never constructs (N3): under SESSION a construction would read the install id, the platform
/// store and the DEVICE_KEY gate, so the owner-ending close invalidates only what exists.
pub fn peek_submitter() -> Option<Arc<Submitter>> {
    let server = crate::cloud_account::configured_server().ok()?;
    let slot = SUBMITTER.lock().ok()?;
    match slot.as_ref() {
        Some((url, submitter)) if *url == server.url => Some(submitter.clone()),
        _ => None,
    }
}

/// The client for the configured Buwiz server, made once per server.
pub fn submitter() -> Result<Arc<Submitter>, TransportError> {
    install_device_key_gate();
    let server = crate::cloud_account::configured_server()
        .map_err(|error| TransportError::Config(error.to_string()))?;
    server_allowed(&server.url, cfg!(debug_assertions))?;
    let mut slot = SUBMITTER
        .lock()
        .map_err(|_| TransportError::Config("the relay client is unavailable".into()))?;
    if let Some((url, submitter)) = slot.as_ref()
        && *url == server.url
    {
        return Ok(submitter.clone());
    }
    let mut config = SubmitConfig::new(&server.url)
        .map_err(|error| TransportError::Config(error.to_string()))?;
    config.install_id = Some(crate::install_id::install_id());
    config.platform = Some(crate::install_id::platform().to_owned());
    config.trust_keys = trust_keys();
    trust_keys_allowed(&config.trust_keys, cfg!(debug_assertions))?;
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    let submitter = Arc::new(Submitter::new(
        config,
        store,
        Arc::new(crate::account_session::AccountTokens),
    ));
    submitter.set_client("buwiz-forms", env!("CARGO_PKG_VERSION"));
    *slot = Some((server.url, submitter.clone()));
    Ok(submitter)
}

/// Drop the cached submitter — the tests' reset, since the slot is process-wide and a test
/// proving no construction needs a clean start.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_submitter_for_test() {
    if let Ok(mut slot) = SUBMITTER.lock() {
        *slot = None;
    }
}

/// Forget this device's key on the Buwiz server's side of things: a new
/// sign-in registers a new key. The account layer calls it on sign-in and
/// sign-out. Nothing to forget when the client cannot be made.
pub fn forget_device() {
    // N3: never CONSTRUCT a client to forget with — a construction reads the install id, the
    // platform store and the device-key gate, and the close paths run this on the sign-out's
    // own thread. An existing client clears its TIN cache too; with none, the store deletion
    // stands alone (no cache exists to clear).
    if let Some(submitter) = peek_submitter() {
        if let Err(error) = submitter.forget_device() {
            tracing::warn!("could not forget the relay device key: {error}");
        }
        return;
    }
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    if let Err(error) = store.delete(bir_submit::DEVICE_KEY) {
        tracing::warn!("could not forget the device key: {error}");
    }
    if let Err(error) = store.delete(bir_submit::DEVICE_ID) {
        tracing::warn!("could not forget the device id: {error}");
    }
}

// ---------- I6: the device key's owner, recorded (F2') ----------

/// The one lock of the device key's lifecycle (F2'b): the forget's check-and-delete and the
/// registration's replace-and-write both run under it, so they can never interleave. NEVER
/// nested inside SESSION — the session machine calls in only after its own lock is released.
static DEVICE_KEY: Mutex<()> = Mutex::new(());

/// Where the key's owner is recorded: a small file beside the key in the secrets folder,
/// written in the same step as the key itself. Two lines: the normalised origin, the user id.
fn device_owner_path() -> std::path::PathBuf {
    secrets_dir().join("buwiz_submit_device_owner.txt")
}

/// The owner recorded beside the device key, or None for a key from before the recording
/// (an existing install's — the adoption case) or no key at all.
pub fn device_key_owner() -> Option<crate::account_session::OwnerKey> {
    let text = std::fs::read_to_string(device_owner_path()).ok()?;
    let mut lines = text.lines();
    let origin = lines.next()?.trim().to_string();
    let user_id = lines.next()?.trim().to_string();
    if origin.is_empty() || user_id.is_empty() {
        return None;
    }
    Some(crate::account_session::OwnerKey { origin, user_id })
}

/// Write the owner beside the key, in the key's own step. The caller holds DEVICE_KEY.
fn write_device_owner(owner: &crate::account_session::OwnerKey) {
    let text = format!(
        "{}
{}
",
        owner.origin, owner.user_id
    );
    if let Err(error) = std::fs::write(device_owner_path(), text) {
        tracing::warn!("could not record the device key's owner: {error}");
    }
}

/// The caller holds DEVICE_KEY.
fn clear_device_owner() {
    match std::fs::remove_file(device_owner_path()) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!("could not remove the device key's owner record: {error}"),
    }
}

/// The forget's half of I6, under DEVICE_KEY: forget the key only while it is still the ending
/// owner's — recorded as his, or unrecorded (a pre-recording install's key, which is the current
/// sign-in's until adoption). A key already recorded for a NEWER owner is never touched: someone
/// got there first, and it is not this ending's to take.
pub fn forget_device_key_for(owner: &crate::account_session::OwnerKey) {
    match device_key_owner() {
        Some(recorded) if recorded != *owner => {
            tracing::info!("device key forget skipped: the key is a newer owner's already");
            return;
        }
        _ => {}
    }
    let _device_key = DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // The owner is re-read inside the lock: a registration in the gap may have re-recorded it.
    match device_key_owner() {
        Some(recorded) if recorded != *owner => {
            tracing::info!("device key forget skipped: the key is a newer owner's already");
            return;
        }
        _ => {}
    }
    forget_device_key_local();
    clear_device_owner();
}

/// The close's forget (F7): the epoch is re-checked INSIDE DEVICE_KEY, so a key registered after
/// the epoch moved — the newer session's — is never taken by the old session's forget, however
/// the wait ordered them. Deletes only its own owner's key: a key recorded for a newer owner is
/// never touched.
/// C5: a forget's check-and-delete inside DEVICE_KEY. The caller's closure runs with the lock
/// held; the epoch is re-checked inside it, so a key registered after the epoch moved — the
/// newer session's — is never taken however the wait ordered them.
pub(crate) fn with_device_key_lock<R>(inside: impl FnOnce() -> R) -> R {
    let _device_key = DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    inside()
}

pub fn forget_device_key_at_epoch(owner: &crate::account_session::OwnerKey, epoch: u64) {
    let _device_key = DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if crate::session_epoch::current() != epoch {
        tracing::info!("device key forget skipped: the epoch moved on while the lock was waited");
        return;
    }
    match device_key_owner() {
        Some(recorded) if recorded != *owner => {
            tracing::info!("device key forget skipped: the key is a newer owner's already");
            return;
        }
        _ => {}
    }
    forget_device_key_local();
    clear_device_owner();
}

/// The key and id deleted from the device store directly — never through the submitter's
/// construction, so the forget cannot be skipped for want of a server config (the deletion is
/// local in bir-submit too; nothing here reaches the network).
/// The public half of the key in the device store, if one is stored and parseable.
#[cfg(any(test, feature = "test-support"))]
fn stored_device_public_key() -> Option<String> {
    stored_device_key_raw()
        .and_then(|raw| bir_submit::device::DeviceKey::from_stored(&raw))
        .map(|key| key.public_key())
}

/// The stored key material verbatim — the identity a discard compares (F6: the public key IS
/// the key, so the raw compares identically, parseable or not), so it deletes only its own
/// registration's key, never a newer one.
fn stored_device_key_raw() -> Option<String> {
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    store.get(bir_submit::DEVICE_KEY).ok().flatten()
}

fn forget_device_key_local() {
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    if let Err(error) = store.delete(bir_submit::DEVICE_KEY) {
        tracing::warn!("could not forget the device key: {error}");
    }
    if let Err(error) = store.delete(bir_submit::DEVICE_ID) {
        tracing::warn!("could not forget the device id: {error}");
    }
    forget_device();
}

/// The registration's half of I6, under DEVICE_KEY: a key recorded for ANOTHER owner, or one
/// with no owner recorded, is replaced — the old key and its server registration go, a new key
/// is made and registered, and key plus owner are written in this one step. So a failed, skipped
/// or crashed forget never hands A's key to B. A key already this owner's is kept (signing in
/// again as the same owner breaks no sends).
pub fn register_device_key_for(
    owner: &crate::account_session::OwnerKey,
    epoch: u64,
) -> Registration {
    register_device_key_with(owner, epoch, register_after_sign_in)
}

/// The registration's wall-clock bound: the DEVICE_KEY lock is held across the register step's
/// network, so that step must be bounded. bir-submit's reqwest client carries a 30 s per-request
/// timeout (client.rs), and register_device is the one POST — the network is already inside this
/// bound. The elapsed check here is the belt: a register step that comes back past the bound,
/// however it ended, is discarded — no key, no owner record.
const REGISTER_BOUND: std::time::Duration = std::time::Duration::from_secs(30);

/// The tests' shorter bound, so `a_registration_past_its_timeout_writes_nothing` does not wait
/// 30 s. Zero means every registration is past it.
#[cfg(any(test, feature = "test-support"))]
static REGISTER_BOUND_OVERRIDE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(u64::MAX);

/// Override the registration's wall-clock bound (milliseconds) for a test.
#[cfg(any(test, feature = "test-support"))]
pub fn set_register_bound_for_test(milliseconds: u64) {
    REGISTER_BOUND_OVERRIDE.store(milliseconds, std::sync::atomic::Ordering::SeqCst);
}

fn register_bound() -> std::time::Duration {
    #[cfg(any(test, feature = "test-support"))]
    {
        let over = REGISTER_BOUND_OVERRIDE.load(std::sync::atomic::Ordering::SeqCst);
        if over != u64::MAX {
            return std::time::Duration::from_millis(over);
        }
    }
    REGISTER_BOUND
}

/// The registration's decision logic, with the register step injected: the tests drive it with
/// a registration that needs no server, so the replace-and-write is proven file by file.
///
/// The DEVICE_KEY lock IS held across the register step's network: bir-submit writes the key
/// inside it, and the write of key plus owner record must stay one step, or a forget can
/// interleave and hand the new key no owner — or the old key a new one. DEVICE_KEY is never
/// nested inside SESSION, so the only other waiter is a forget; the network is bounded (see
/// [`REGISTER_BOUND`]), so the wait is too.
fn register_device_key_with(
    owner: &crate::account_session::OwnerKey,
    epoch: u64,
    register: impl FnOnce() -> Registration,
) -> Registration {
    // F6: DEVICE_KEY is never held across the network. Step 1, a short locked section: the
    // pre-clean — a key recorded for ANOTHER owner, or one with no owner, is forgotten before
    // this owner's registration, so a failed or crashed forget never hands A's key to B.
    {
        let _device_key = DEVICE_KEY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if device_key_owner().as_ref() != Some(owner) {
            forget_device_key_local();
            clear_device_owner();
        }
    }
    // Step 2: the token fetch and the register with NO LOCK of any kind. The InvalidGrant hook
    // the fetch may run therefore never fires under DEVICE_KEY either — it takes the account
    // lock and SESSION freely, and a forget waiting its turn is never deadlocked against it.
    let _ = take_key_created_here();
    let started = std::time::Instant::now();
    let registration = register();
    let elapsed = started.elapsed();
    if !matches!(
        registration,
        Registration::Registered | Registration::AlreadyDone
    ) {
        return registration;
    }
    // The key THIS registration created, if it created one — the gate's own record on this
    // thread, immune to whatever another registration wrote in between. A registration that
    // reused an existing key created nothing, and its discard deletes nothing either: the key
    // was never its own.
    let key_created_here = take_key_created_here();
    // Step 3, under DEVICE_KEY: the write gate (the page, revision 5's C) — the cache is still
    // this owner AND the session is still at the registration's epoch AND the register step
    // came back inside its bound. Only then is the owner record written. Otherwise the
    // registration is discarded: no owner record, and the key deleted ONLY if it is still this
    // registration's own — a newer registration's key and record are never taken.
    let _device_key = DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache_is_owner = crate::account_session::stored_owner().as_ref() == Some(owner);
    let epoch_current = crate::session_epoch::current() == epoch;
    let inside_bound = elapsed < register_bound();
    if cache_is_owner && epoch_current && inside_bound {
        write_device_owner(owner);
    } else {
        if key_created_here.is_some() && stored_device_key_raw() == key_created_here {
            forget_device_key_local();
            clear_device_owner();
        }
        tracing::info!(
            "device registration discarded: the session moved on or the register step overran (cache {cache_is_owner}, epoch {epoch_current}, elapsed {elapsed:?})"
        );
    }
    registration
}

/// The test door of [`register_device_key_with`]: a registration without a server.
#[cfg(any(test, feature = "test-support"))]
pub fn register_device_key_for_test(
    owner: &crate::account_session::OwnerKey,
    epoch: u64,
    register: impl FnOnce() -> Registration,
) -> Registration {
    register_device_key_with(owner, epoch, register)
}

/// Test instrumentation: the DEVICE_KEY lock itself, so the funnel test can hold it and prove
/// the forget and the registration both wait on it.
#[cfg(any(test, feature = "test-support"))]
pub fn device_key_lock_for_test() -> std::sync::MutexGuard<'static, ()> {
    DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The test door of the lazy creation: a first key exactly as a send's client creates it —
/// through the installed gate — in this thread's device folder.
#[cfg(any(test, feature = "test-support"))]
pub fn lazy_first_key_for_test() -> bool {
    install_device_key_gate();
    let store =
        bir_submit::secrets::platform_store(secrets_dir(), !crate::platform::keychain_disabled());
    let before = stored_device_public_key();
    let created = bir_submit::client::device_key_or_create(store.as_ref())
        .map(|key| key.public_key())
        .ok();
    before.is_none() && created.is_some()
}

/// Adoption (F2'): an existing install's key has no owner recorded. The stored sign-in's owner
/// adopts it — the record is written, the key keeps working — at the first commit after the
/// update, under DEVICE_KEY, and only while the owner cache is that owner. Never replaced while
/// the same owner is signed in, so nobody's sends break on the update. Answers whether the
/// adoption ran.
pub fn adopt_device_key(owner: &crate::account_session::OwnerKey) -> bool {
    let _device_key = DEVICE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if device_key_owner().is_some() {
        return false;
    }
    if crate::account_session::stored_owner().as_ref() != Some(owner) {
        return false;
    }
    write_device_owner(owner);
    true
}

/// What [`register_after_sign_in`] did. Never an error the sign-in sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Registration {
    /// A practice account never sends, so it registers no device.
    Practice,
    /// No client for this build or server (a development build without a test trust key, say): the first send
    /// registers, or says why it cannot.
    NoClient(String),
    Registered,
    /// This sign-in already has its device (`409 device_registered`).
    AlreadyDone,
    /// It did not go through (offline, refused): logged; the first send registers as it always did.
    Failed(String),
}

/// Adds this device to the account right after a sign-in's tokens are saved (after [`forget_device`]): once, with the
/// install id and the platform. A practice account registers none. Blocks the calling thread for the one request, so the
/// host calls it off the sign-in's path; nothing here can fail the sign-in.
pub fn register_after_sign_in() -> Registration {
    let outcome = register_with(crate::transport::practice_account(), submitter);
    match &outcome {
        Registration::Practice => {
            tracing::info!("device registration: a practice account adds no device")
        }
        Registration::NoClient(why) => {
            tracing::info!("device registration: not now ({why}); the first send registers")
        }
        Registration::Registered => tracing::info!("device registration: this device was added"),
        Registration::AlreadyDone => {
            tracing::info!("device registration: already done for this sign-in")
        }
        Registration::Failed(why) => {
            tracing::warn!("device registration failed ({why}); the first send registers")
        }
    }
    outcome
}

fn register_with(
    practice: bool,
    client: impl FnOnce() -> Result<Arc<Submitter>, TransportError>,
) -> Registration {
    if practice {
        return Registration::Practice;
    }
    let submitter = match client() {
        Ok(submitter) => submitter,
        Err(error) => return Registration::NoClient(error.to_string()),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return Registration::Failed(error.to_string()),
    };
    match runtime.block_on(submitter.register_device()) {
        Ok(bir_submit::Registered::New) => Registration::Registered,
        Ok(bir_submit::Registered::AlreadyDone) => Registration::AlreadyDone,
        Err(error) => Registration::Failed(error.to_string()),
    }
}

/// The saved sign-in is for the server the client talks to. A token is never offered to another server: signed
/// out, or signed in elsewhere, the return is held until the person signs in.
fn signed_in_to(server_url: &str) -> Result<(), TransportError> {
    let saved = crate::cloud_account::load_stored_account().ok().flatten();
    // §5.4 early check: inside a send's scope the sign-in must still be the SEND'S owner — the
    // account having changed mid-send holds the return (not a try, it stays Queued).
    if let Ok(context) = crate::account_session::SEND_CONTEXT.try_with(|context| context.clone()) {
        let still_the_sends = saved
            .as_ref()
            .and_then(crate::account_session::OwnerKey::of)
            .is_some_and(|owner| owner == context.owner);
        if !still_the_sends {
            return Err(TransportError::RelayHeld {
                reason: "The account changed while this return was being sent. It sends when its account is back."
                    .into(),
                wait_until: None,
            });
        }
    }
    held_unless_signed_in_to(
        saved.as_ref().map(|saved| saved.server_url.as_str()),
        server_url,
    )
}

fn held_unless_signed_in_to(saved: Option<&str>, server_url: &str) -> Result<(), TransportError> {
    let reason = match saved {
        Some(saved) if saved.trim_end_matches('/') == server_url.trim_end_matches('/') => {
            return Ok(());
        }
        Some(_) => "The saved Buwiz sign-in is for another server. Sign in again to send returns.",
        None => "Sign in to send returns.",
    };
    Err(TransportError::RelayHeld {
        reason: reason.into(),
        wait_until: None,
    })
}

/// A relay send that is ready to go: the trust list verified, this device
/// registered, the sealing key checked. Nothing about the return has left
/// this computer yet.
pub(crate) struct RelaySession {
    submitter: Arc<Submitter>,
}

/// Make the relay ready, before the queue claims the return. Everything here
/// talks to Buwiz only, so a failure is never a failed try of the return: it
/// is held ([`TransportError::RelayHeld`]) and goes again later.
pub(crate) async fn open() -> Result<RelaySession, TransportError> {
    let submitter = submitter()?;
    signed_in_to(&submitter.config().server_url)?;
    submitter
        .ready()
        .await
        .map_err(|error| TransportError::RelayHeld {
            wait_until: error.wait_until(),
            reason: error.to_string(),
        })?;
    Ok(RelaySession { submitter })
}

impl RelaySession {
    /// Send the plain return `xml` under its eBIRForms file name. `note`
    /// keeps the client's pending send on the claimed return before anything
    /// could reach the BIR ([`bir_submit::Submitter::send_noting`]).
    pub(crate) async fn store(
        self,
        filename: &str,
        xml: &[u8],
        note: bir_submit::Note<'_>,
    ) -> Result<StoreReport, TransportError> {
        let name = crate::transport::iaf_basename(filename);
        // This device carries the upload: a phone stays awake and asks for time until the send ends, however it ends.
        let _in_flight = crate::send_in_flight::begin();
        match tokio::time::timeout(SEND_LIMIT, self.submitter.send_noting(name, xml, note)).await {
            Ok(sent) => send_outcome(sent),
            Err(_) => Err(TransportError::RelayUnknown(
                "Sending took longer than 10 minutes and was stopped. The BIR may have received this return."
                    .into(),
            )),
        }
    }
}

/// What the Buwiz server says of a send this app noted and did not see end
/// ([`settle`]).
#[derive(Debug)]
pub(crate) enum Settlement {
    /// A receipt that verified: finish the return as a send that went.
    Sent(StoreReport),
    /// No receipt: the outcome the send itself would have had.
    Failed(TransportError),
    /// Nothing to go on yet (the server cannot be asked or the submission is still under way).
    NotYet,
    /// The owner signed out or changed: keep the record indefinitely. It is never evidence that
    /// the send is gone.
    Keep,
    /// The server does not know the submission, or the record is unreadable.
    Gone,
}

/// Ask the Buwiz server how the noted send `pending` (the stored
/// `bir_submit::PendingSend` JSON) ended. Only through the client this
/// build may send through ([`submitter`]: a development build's rules hold),
/// and only with a sign-in for that server: otherwise nothing is asked.
#[cfg(test)]
static AFTER_SETTLE_DISK_CHECK: Mutex<Option<Box<dyn FnOnce() + Send>>> = Mutex::new(None);

pub(crate) async fn settle(pending: &str) -> Settlement {
    let Ok(pending) = serde_json::from_str::<bir_submit::PendingSend>(pending) else {
        return Settlement::Gone;
    };
    let saved = crate::cloud_account::load_stored_account().ok().flatten();
    match saved.as_ref().map(|saved| saved.server_url.as_str()) {
        Some(saved) if saved.trim_end_matches('/') == pending.server.trim_end_matches('/') => {}
        Some(_) => {
            // The account moved to another server: the send is kept, whatever the server. The
            // NotYet this returned before REOPENED the disk-before-epoch window — a switch's
            // close writes the disk before the epoch, so the new server's sign-in could age a
            // mid-move send into needs-attention and drop a filing. This server can no more
            // answer for the send than a signed-out one can; the keep is never evidence the
            // send is gone.
            return Settlement::Keep;
        }
        None => {
            // The disk answer is not evidence that the send is gone. Sign-out writes the disk before
            // the epoch and room close finish, so a noted send keeps its record until its own account
            // can be asked again.
            return Settlement::Keep;
        }
    }
    // C6: the server matching is not enough — the USER must be the send's own too. A same-server
    // sign-in of another account would ask after this send with a token that is not its owner's,
    // and the server's honest "unknown" would read a live filing as Gone. The send's owner is the
    // task-local context the tick was spawned in; with none (a caller outside a send) the server
    // check above is all there is, as before.
    let context_user = crate::account_session::SEND_CONTEXT
        .try_with(|context| context.owner.user_id.clone())
        .ok();
    if let (Some(context_user), Some(saved)) = (context_user, saved.as_ref())
        && saved.user_id != context_user
    {
        tracing::info!("settle keeps the send: the disk sign-in is another user's");
        return Settlement::Keep;
    }
    #[cfg(test)]
    AFTER_SETTLE_DISK_CHECK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .map(|hook| hook());
    let Ok(submitter) = submitter() else {
        return Settlement::NotYet;
    };
    settlement(submitter.settle(&pending).await)
}

/// The client's answer as the queue's, through [`send_outcome`]. Pure.
pub(crate) fn settlement(settled: bir_submit::Settled) -> Settlement {
    match settled {
        bir_submit::Settled::Sent(sent) => match send_outcome(Ok(*sent)) {
            Ok(report) => Settlement::Sent(report),
            Err(error) => Settlement::Failed(error),
        },
        bir_submit::Settled::Failed(error) => Settlement::Failed(send_failure(error)),
        bir_submit::Settled::NotYet => Settlement::NotYet,
        bir_submit::Settled::SignedOut => Settlement::Keep,
        bir_submit::Settled::Gone => Settlement::Gone,
    }
}

/// The client's answer as the queue's outcome. Pure, so every case is
/// tested.
///
/// - A receipt: sent. `simulated` when the server did not send to the BIR
///   (its `dry_run` or `lab` mode).
/// - Not sent with a reason no retry can change (a TIN refusal, a file that
///   does not match its name): refused, back to Draft with that reason.
/// - Not sent otherwise, or a counted failure of the BIR's way in: a failed
///   try, retried later.
/// - Not a try: held.
/// - Maybe sent: unknown. Never anything that lets the return go again.
pub(crate) fn send_outcome(sent: Result<Sent, SendError>) -> Result<StoreReport, TransportError> {
    match sent {
        Ok(sent) => Ok(StoreReport {
            simulated: sent.receipt.is_test() || sent.receipt.mode != "live",
            relay_receipt: Some(sent.receipt_jws),
        }),
        Err(error) => Err(send_failure(error)),
    }
}

/// The relay's `result` for a test TIN, shared by the three links of "a relay dry_run result never reads as filed"
/// (`bir-submit/tests/fixtures/relay-result-dry-run.json`): status `dry_run`, and the claims of its dry-run receipt for
/// `<xml/>` on ticket `jti-1`. bir-submit's `a_relay_dry_run_result_with_its_signed_receipt_is_a_verified_test_send`
/// signs those claims with its test key and proves the client's send verifies them and hands them over unchanged.
#[cfg(any(test, feature = "test-support"))]
pub const DRY_RUN_RESULT_FIXTURE: &str =
    include_str!("../../bir-submit/tests/fixtures/relay-result-dry-run.json");

/// What the queue keeps of [`DRY_RUN_RESULT_FIXTURE`]: the client's [`Sent`] of it through [`send_outcome`], exactly
/// as a send through the Buwiz relay hands it to the worker. Its receipt is the fixture's claims in compact form; the
/// queue keeps it as it comes and checks no signature (the client did, link 1).
#[cfg(any(test, feature = "test-support"))]
pub fn dry_run_result_fixture_report() -> StoreReport {
    let result: serde_json::Value =
        serde_json::from_str(DRY_RUN_RESULT_FIXTURE).expect("the fixture is JSON");
    let claims = &result["receipt_claims"];
    let part = |value: &serde_json::Value| {
        data_encoding::BASE64URL_NOPAD.encode(value.to_string().as_bytes())
    };
    let header = serde_json::json!({"alg": "EdDSA", "typ": bir_submit::receipt::TYP});
    let sent = Sent {
        submission_id: "s1".into(),
        receipt: serde_json::from_value(claims.clone()).expect("receipt claims"),
        receipt_jws: format!("{}.{}.verified-by-the-client", part(&header), part(claims)),
    };
    send_outcome(Ok(sent)).expect("a receipt is a send that went")
}

/// [`send_outcome`] of a send that ended without a receipt.
fn send_failure(error: SendError) -> TransportError {
    match error {
        SendError::NotSent(reason) if is_final_reason(&reason) => {
            TransportError::Refused(TinRefusal::capped(&reason, REFUSAL_CHARS))
        }
        SendError::NotSent(reason) | SendError::Counted { reason, .. } => {
            TransportError::RelayFailed(reason)
        }
        SendError::NotATry { reason, wait_until } => {
            TransportError::RelayHeld { reason, wait_until }
        }
        SendError::MaybeSent(reason) => TransportError::RelayUnknown(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::queueable::NoReceipt;
    use bir_submit::Blame;

    /// A fixed access token (the mock's), as the host's sign-in would give it.
    struct Fixed(String);
    impl bir_submit::TokenSource for Fixed {
        fn access_token(
            &self,
            _force_refresh: bool,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<String, bir_submit::ClientError>>
                    + Send
                    + '_,
            >,
        > {
            let token = self.0.clone();
            Box::pin(async move { Ok(token) })
        }
        fn login(&self) -> Option<String> {
            Some("filer@example.test".into())
        }
        fn session_ended(&self) {}
    }

    #[test]
    fn a_practice_account_registers_no_device_and_no_client_is_not_a_failure() {
        let asked = std::cell::Cell::new(false);
        let outcome = register_with(true, || {
            asked.set(true);
            Err(TransportError::Config("unused".into()))
        });
        assert_eq!(outcome, Registration::Practice);
        assert!(!asked.get(), "no client is made for a practice account");
        assert_eq!(
            register_with(false, || Err(TransportError::Config(
                "no test trust key".into()
            ))),
            Registration::NoClient(TransportError::Config("no test trust key".into()).to_string())
        );
        // Through the real entry point in a test run: a debug build has no test trust key here, so no client; nothing
        // is sent and nothing fails.
        if !crate::transport::practice_account() {
            assert!(matches!(
                register_after_sign_in(),
                Registration::NoClient(_) | Registration::Failed(_)
            ));
        }
    }

    /// Against the in-repo mock: after an email sign-in the device is added once with the install id; a second time
    /// is already done (the kept id, or the server's 409); a dead server is a logged failure.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn after_a_sign_in_the_device_is_added_once_and_409_is_done() {
        use crate::email_sign_in::{Intent, new_secret, start, verify};
        use crate::mock_account::{MockAccount, MockOptions};
        let server =
            MockAccount::start_with("127.0.0.1:0".parse().unwrap(), MockOptions::default())
                .await
                .unwrap();
        let url = server.url();
        let install = "install-id-of-this-test-0001";
        let outcome = std::thread::spawn(move || {
            let mut http = crate::cloud_account::BlockingHttp::new();
            let secret = new_secret();
            let started = start(
                &mut http,
                &url,
                "filer@example.test",
                Intent::SignIn,
                &secret,
                "macos",
                install,
                None,
            )
            .unwrap();
            let tokens = verify(
                &mut http,
                &url,
                &started.request_id,
                &secret,
                crate::mock_account::DEFAULT_CODE,
            )
            .unwrap();
            let client = |store: Arc<bir_submit::secrets::MemoryStore>| {
                let mut config = SubmitConfig::new(&url).unwrap();
                config.install_id = Some(install.into());
                config.platform = Some("macos".into());
                use bir_submit::secrets::SecretStore;
                struct Shared(Arc<bir_submit::secrets::MemoryStore>);
                impl bir_submit::secrets::SecretStore for Shared {
                    fn get(
                        &self,
                        name: &str,
                    ) -> Result<Option<String>, bir_submit::secrets::StoreError>
                    {
                        self.0.get(name)
                    }
                    fn set(
                        &self,
                        name: &str,
                        value: &str,
                    ) -> Result<(), bir_submit::secrets::StoreError> {
                        self.0.set(name, value)
                    }
                    fn delete(&self, name: &str) -> Result<(), bir_submit::secrets::StoreError> {
                        self.0.delete(name)
                    }
                }
                Arc::new(Submitter::new(
                    config,
                    Box::new(Shared(store)),
                    Arc::new(Fixed(tokens.access_token.clone())),
                ))
            };
            let store = Arc::new(bir_submit::secrets::MemoryStore::default());
            let first = register_with(false, || Ok(client(store.clone())));
            let again = register_with(false, || Ok(client(store.clone())));
            // A fresh store (the id forgotten) on the same sign-in: the server's 409 is "already done".
            let fresh = register_with(false, || {
                Ok(client(
                    Arc::new(bir_submit::secrets::MemoryStore::default()),
                ))
            });
            (first, again, fresh)
        })
        .join()
        .unwrap();
        assert_eq!(
            outcome,
            (
                Registration::Registered,
                Registration::AlreadyDone,
                Registration::AlreadyDone
            )
        );
        assert_eq!(server.registered_devices(), 1);
        let (_, platform, _) = server.registered(install).unwrap();
        assert_eq!(platform, "macos");
        drop(server);
        let dead = std::thread::spawn(|| {
            register_with(false, || {
                let config = SubmitConfig::new("http://127.0.0.1:9").unwrap();
                Ok(Arc::new(Submitter::new(
                    config,
                    Box::new(bir_submit::secrets::MemoryStore::default()),
                    Arc::new(Fixed("token".into())),
                )))
            })
        })
        .join()
        .unwrap();
        assert!(matches!(dead, Registration::Failed(_)), "{dead:?}");
    }

    /// F2 through the production settle entry point: when the disk cannot prove the pending
    /// send's sign-in, the answer is Keep, never a NotYet that the queue can age into Clear.
    #[tokio::test(flavor = "current_thread")]
    async fn settle_keeps_a_noted_send_when_the_disk_sign_in_does_not_match() {
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("buwiz-cloud-account.json");
        temp_env::async_with_vars(
            [(
                crate::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            )],
            async {
                let pending = serde_json::to_string(&bir_submit::PendingSend {
                    server: "http://localhost:3008".into(),
                    submission_id: "s1".into(),
                    jti: "jti-1".into(),
                    xml_sha256: "0".repeat(64),
                    opening: true,
                    noted_at: chrono::Utc::now(),
                })
                .unwrap();
                assert!(
                    matches!(settle(&pending).await, Settlement::Keep),
                    "a disk-signed-out settle was left to age into Clear"
                );
            },
        )
        .await;
    }

    #[test]
    fn a_development_build_sends_only_through_a_server_on_this_computer() {
        for url in [
            "http://localhost:3008",
            "http://127.0.0.1:3008",
            "http://[::1]:3008",
            "https://localhost",
        ] {
            assert!(server_allowed(url, true).is_ok(), "{url}");
        }
        for url in [
            "https://api.buwiz.com",
            "https://localhost.example",
            "http://192.0.2.1:3008",
            "not a url",
        ] {
            assert!(
                matches!(server_allowed(url, true), Err(TransportError::Config(_))),
                "{url}"
            );
        }
        assert!(server_allowed("https://api.buwiz.com", false).is_ok());
    }

    #[test]
    fn a_token_is_only_offered_to_the_server_the_sign_in_is_for() {
        assert!(
            held_unless_signed_in_to(Some("http://localhost:3008"), "http://localhost:3008/")
                .is_ok()
        );
        for saved in [None, Some("https://api.buwiz.com")] {
            let held = held_unless_signed_in_to(saved, "http://localhost:3008").unwrap_err();
            assert_eq!(
                held.before_claim(),
                NoReceipt::Held { until: None },
                "{saved:?}"
            );
        }
    }

    #[test]
    fn a_development_build_never_trusts_the_production_trust_key() {
        let production: Vec<String> = bir_submit::trust::TRUST_KEYS
            .iter()
            .map(|key| (*key).to_owned())
            .collect();
        let test_key = vec!["dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk".to_owned()];
        let mixed = [production.clone(), test_key.clone()].concat();
        assert!(trust_keys_allowed(&test_key, true).is_ok());
        for refused in [&production, &mixed, &Vec::new()] {
            assert!(
                matches!(
                    trust_keys_allowed(refused, true),
                    Err(TransportError::Config(_))
                ),
                "{refused:?}"
            );
        }
        assert!(trust_keys_allowed(&production, false).is_ok());
    }

    /// The server and the trust keys are read through `config::var` (an Android development build's baked list),
    /// and that path keeps both rules: on this computer, and a test key, never production's. Runs in a child
    /// process, so its overrides never reach another test.
    #[test]
    fn configured_server_and_trust_keys_keep_the_development_rules() {
        const CHILD: &str = "BIR_RELAY_CONFIG_CHILD";
        const TEST: &str =
            "relay::tests::configured_server_and_trust_keys_keep_the_development_rules";
        let test_key = "dGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXktdGVzdC1rZXk";
        let production = bir_submit::trust::TRUST_KEYS[0];
        if std::env::var_os(CHILD).is_none() {
            // The environment names production's server and key: the overrides below must win over it.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .env("BUWIZ_SERVER_URL", "https://api.buwiz.com")
                .env("BUWIZ_TRUST_KEYS", production)
                .env_remove("BUWIZ_ACCOUNT_URL")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        use crate::config::force;
        force("BUWIZ_SERVER_URL", Some("http://localhost:3008"));
        force("BUWIZ_TRUST_KEYS", Some(test_key));
        assert_eq!(
            crate::cloud_account::configured_server().unwrap().url,
            "http://localhost:3008"
        );
        let expected = if cfg!(debug_assertions) {
            vec![test_key.to_owned()]
        } else {
            vec![production.to_owned()]
        };
        assert_eq!(trust_keys(), expected);
        if !cfg!(debug_assertions) {
            return;
        }
        assert_eq!(server().unwrap(), "http://localhost:3008");
        for keys in [Some(production), None] {
            force("BUWIZ_TRUST_KEYS", keys);
            let refused = server().unwrap_err().to_string();
            assert!(refused.contains("test key"), "{keys:?}: {refused}");
        }
        force("BUWIZ_TRUST_KEYS", Some(test_key));
        for url in ["https://api.buwiz.com", "https://192.0.2.1:3008"] {
            force("BUWIZ_SERVER_URL", Some(url));
            let refused = server().unwrap_err().to_string();
            assert!(refused.contains("on this computer"), "{url}: {refused}");
        }
        // Plain http to another host is refused before that, as an address: never a sign-in in the clear.
        force("BUWIZ_SERVER_URL", Some("http://192.0.2.1:3008"));
        let refused = server().unwrap_err().to_string();
        assert!(refused.contains("https://"), "{refused}");
    }

    fn receipt(mode: &str, status: &str) -> Sent {
        Sent {
            submission_id: "s1".into(),
            receipt_jws: "header.payload.signature".into(),
            receipt: bir_submit::receipt::Receipt {
                sub: "u1".into(),
                tin: "123456789000".into(),
                form_type: "1601Cv2018".into(),
                period: "052024".into(),
                filename: "f.xml".into(),
                xml_sha256: "00".into(),
                payload_sha256: "00".into(),
                remote_path: "/1601Cv2018/f.xml".into(),
                mode: mode.into(),
                status: status.into(),
                iat: 0,
                jti: None,
            },
        }
    }

    #[test]
    fn only_a_live_receipt_is_a_real_send() {
        let live = send_outcome(Ok(receipt("live", "sent"))).unwrap();
        assert!(!live.simulated);
        assert_eq!(
            live.relay_receipt.as_deref(),
            Some("header.payload.signature")
        );
        for (mode, status) in [
            ("dry_run", "dry_run"),
            ("lab", "sent"),
            ("dry_run", "sent"),
            ("", "sent"),
        ] {
            let report = send_outcome(Ok(receipt(mode, status))).unwrap();
            assert!(report.simulated, "{mode} {status}");
            assert!(report.relay_receipt.is_some());
        }
    }

    #[test]
    fn a_refusal_goes_back_to_draft_with_its_code() {
        for code in [
            "tin_not_active",
            "tin_unverified",
            "tin_pending",
            "tin_needs_fix",
            "tin_paused",
            "tin_test_only",
            "account_banned",
            "account_practice",
            "return_mismatch",
        ] {
            let reason = format!("The server's sentence. ({code})");
            let error = send_outcome(Err(SendError::NotSent(reason.clone()))).unwrap_err();
            assert!(
                matches!(&error, TransportError::Refused(kept) if kept == &reason),
                "{code}: {error:?}"
            );
            assert_eq!(error.no_receipt(), NoReceipt::Refused, "{code}");
        }
    }

    #[test]
    fn a_long_refusal_is_cut_and_keeps_its_code() {
        let reason = format!("{} (tin_needs_fix)", "x".repeat(1000));
        let TransportError::Refused(kept) =
            send_outcome(Err(SendError::NotSent(reason))).unwrap_err()
        else {
            panic!("a refusal");
        };
        assert!(kept.ends_with(" (tin_needs_fix)"), "{kept}");
        assert!(kept.chars().count() < 330, "{}", kept.chars().count());
    }

    #[test]
    fn a_failure_of_the_birs_way_in_is_a_failed_try_never_draft() {
        for failed in [
            SendError::NotSent("the server did not send the return".into()),
            SendError::Counted {
                reason: "BIR's upload server did not answer".into(),
                blame: Blame::Upload,
            },
        ] {
            let error = send_outcome(Err(failed)).unwrap_err();
            assert_eq!(error.no_receipt(), NoReceipt::RelayFailed, "{error:?}");
        }
    }

    #[test]
    fn a_hold_is_not_a_try_and_keeps_the_servers_time() {
        let until = chrono::Utc::now() + chrono::Duration::minutes(40);
        let error = send_outcome(Err(SendError::NotATry {
            reason: "BIR is not accepting returns right now. (bir_offline)".into(),
            wait_until: Some(until),
        }))
        .unwrap_err();
        assert_eq!(error.no_receipt(), NoReceipt::Held { until: Some(until) });
        assert_eq!(error.before_claim(), NoReceipt::Held { until: Some(until) });
        assert_eq!(
            error.to_string(),
            "BIR is not accepting returns right now. (bir_offline)"
        );
    }

    #[test]
    fn maybe_sent_is_unknown_and_never_lets_the_return_go_again() {
        // Even when its text ends in a refusal code: the BIR may have it.
        for reason in [
            "The relay ended without a result.",
            "already submitted (tin_not_active)",
        ] {
            let error = send_outcome(Err(SendError::MaybeSent(reason.into()))).unwrap_err();
            assert!(
                matches!(error, TransportError::RelayUnknown(_)),
                "{error:?}"
            );
            assert_eq!(error.no_receipt(), NoReceipt::Unknown);
            assert!(!error.relay_did_not_accept());
        }
    }

    /// §5.4 early check: inside a send scoped to owner A, `signed_in_to` holds the return once
    /// the stored sign-in is B — RelayHeld, never a try. Positive control: still A, the check passes.
    #[test]
    fn a_send_scope_holds_the_return_when_the_account_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.json");
        temp_env::with_var(
            crate::cloud_account::ENV_ACCOUNT_FILE,
            Some(path.to_str().unwrap()),
            || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async {
                    let stored = crate::cloud_account::StoredAccount {
                        server_url: "http://localhost:3008".into(),
                        access_token: "access-a".into(),
                        refresh_token: "refresh-a".into(),
                        access_expires_at_unix: 4_000_000_000,
                        user_id: "user-1".into(),
                        email: None,
                        offline: false,
                        last_refresh_unix: 0,
                        practice: true,
                    };
                    crate::cloud_account::save_stored_account(&stored).unwrap();
                    let owner = crate::account_session::OwnerKey::of(&stored).unwrap();
                    let context = crate::account_session::SendContext::new(
                        owner,
                        crate::session_epoch::current(),
                    );
                    crate::account_session::SEND_CONTEXT
                        .scope(context, async {
                            assert!(
                                signed_in_to("http://localhost:3008").is_ok(),
                                "positive control: still the send's account"
                            );
                            let mut other = stored.clone();
                            other.user_id = "user-2".into();
                            crate::cloud_account::save_stored_account(&other).unwrap();
                            match signed_in_to("http://localhost:3008") {
                                Err(TransportError::RelayHeld { .. }) => {}
                                other_answer => panic!(
                                    "the switched account holds the return: {other_answer:?}"
                                ),
                            }
                        })
                        .await;
                });
            },
        );
    }

    /// C3: the pure map — a SignedOut answer of the client is Keep, never Gone: the server's
    /// sign-in state is not evidence the send is gone. Positive control: Gone still maps Gone.
    #[test]
    fn a_signed_out_settle_maps_to_keep_never_gone() {
        assert!(
            matches!(settlement(bir_submit::Settled::SignedOut), Settlement::Keep),
            "SignedOut must keep the send"
        );
        assert!(
            matches!(settlement(bir_submit::Settled::Gone), Settlement::Gone),
            "positive control: Gone still maps Gone"
        );
        assert!(
            matches!(settlement(bir_submit::Settled::NotYet), Settlement::NotYet),
            "positive control: NotYet still maps NotYet"
        );
    }

    /// C6: a same-server sign-in of ANOTHER user does not answer for the send — without the
    /// user compare the settle would ask the server with a token that is not the owner's, and
    /// the honest "unknown" would read a live filing as Gone. Positive control: the send's own
    /// user on disk passes the gate (and the settle then ends NotYet for want of a relay
    /// here, never Keep-for-mismatch).
    #[tokio::test(flavor = "current_thread")]
    async fn settle_keeps_the_send_when_the_disk_user_is_not_the_sends() {
        use crate::account_session::{OwnerKey, SEND_CONTEXT, SendContext};
        let _epoch_turn = crate::session_epoch::test_turn();
        let dir = tempfile::tempdir().unwrap();
        let account_path = dir.path().join("buwiz-cloud-account.json");
        temp_env::async_with_vars(
            [(
                crate::cloud_account::ENV_ACCOUNT_FILE,
                Some(account_path.to_str().unwrap().to_owned()),
            )],
            async {
                let mut other = stored_for("http://localhost:3008");
                other.user_id = "user-b".into();
                crate::account_session::save_stored(&other).unwrap();
                let pending = pending_for("http://localhost:3008", chrono::Utc::now());
                let owner_a = OwnerKey {
                    origin: "localhost:3008".into(),
                    user_id: "user-a".into(),
                };
                let epoch = crate::session_epoch::current();
                let answer = SEND_CONTEXT
                    .scope(SendContext::new(owner_a, epoch), async {
                        settle(&pending).await
                    })
                    .await;
                assert!(
                    matches!(answer, Settlement::Keep),
                    "another user's same-server sign-in must keep the send: {answer:?}"
                );
                // positive control: the send's OWN user on disk passes the gate — the settle
                // is then NotYet for want of a relay here, never the mismatch's Keep
                let mut own = stored_for("http://localhost:3008");
                own.user_id = "user-a".into();
                crate::account_session::save_stored(&own).unwrap();
                let answer = SEND_CONTEXT
                    .scope(
                        SendContext::new(
                            OwnerKey {
                                origin: "localhost:3008".into(),
                                user_id: "user-a".into(),
                            },
                            epoch,
                        ),
                        async { settle(&pending).await },
                    )
                    .await;
                assert!(
                    !matches!(answer, Settlement::Keep),
                    "positive control: the send's own user passes the user gate: {answer:?}"
                );
            },
        )
        .await;
    }

    fn stored_for(server: &str) -> crate::cloud_account::StoredAccount {
        crate::cloud_account::StoredAccount {
            server_url: server.into(),
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            access_expires_at_unix: i64::MAX,
            user_id: "user".into(),
            email: Some("filer@example.test".into()),
            offline: true,
            last_refresh_unix: 0,
            practice: true,
        }
    }

    fn pending_for(server: &str, noted_at: chrono::DateTime<chrono::Utc>) -> String {
        serde_json::to_string(&crate::bir_submit::PendingSend {
            server: server.into(),
            submission_id: "s1".into(),
            jti: "jti".into(),
            xml_sha256: "0".repeat(64),
            opening: false,
            noted_at,
        })
        .unwrap()
    }

    #[test]
    fn settle_disk_check_keeps_a_signed_out_send_before_any_server_ask() {
        let _epoch_turn = crate::session_epoch::test_turn();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("account.json");
        temp_env::with_var(
            crate::cloud_account::ENV_ACCOUNT_FILE,
            Some(path.as_os_str()),
            || {
                let pending = pending_for("http://server-a.example", chrono::Utc::now());
                let continued = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let mark = continued.clone();
                *AFTER_SETTLE_DISK_CHECK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(move || {
                    mark.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }));
                let context = crate::account_session::SendContext::new(
                    crate::account_session::OwnerKey {
                        origin: "server-a.example".into(),
                        user_id: "user".into(),
                    },
                    crate::session_epoch::current(),
                );
                let answer = tokio::runtime::Runtime::new().unwrap().block_on(
                    crate::account_session::SEND_CONTEXT.scope(context, settle(&pending)),
                );
                assert!(matches!(answer, Settlement::Keep));
                assert_eq!(continued.load(std::sync::atomic::Ordering::SeqCst), 0);

                // Positive control: when the disk holds the same server's account, the disk check
                // passes and the settle reaches the path that would ask the server.
                crate::account_session::save_stored(&stored_for("http://server-a.example"))
                    .unwrap();
                let mark = continued.clone();
                *AFTER_SETTLE_DISK_CHECK
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Box::new(move || {
                    mark.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }));
                let context = crate::account_session::SendContext::new(
                    crate::account_session::OwnerKey {
                        origin: "server-a.example".into(),
                        user_id: "user".into(),
                    },
                    crate::session_epoch::current(),
                );
                let _answer = tokio::runtime::Runtime::new().unwrap().block_on(
                    crate::account_session::SEND_CONTEXT.scope(context, settle(&pending)),
                );
                assert_eq!(continued.load(std::sync::atomic::Ordering::SeqCst), 1);
            },
        );
    }

    /// P1: the window is CLOSED — a send for a server the account left is kept, whatever the
    /// server, exactly as a signed-out send is: the close writes the disk before the epoch, so
    /// the another-server read can be a move still under way, and ageing it out would drop a
    /// filing. (This test asserted the old NotYet; the correction of semantics is the fix.)
    #[test]
    fn a_pending_send_for_a_server_the_account_left_is_kept() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("account.json");
        temp_env::with_var(
            crate::cloud_account::ENV_ACCOUNT_FILE,
            Some(path.as_os_str()),
            || {
                crate::account_session::save_stored(&stored_for("http://server-b.example"))
                    .unwrap();
                let pending = pending_for(
                    "http://server-a.example",
                    chrono::Utc::now() - chrono::Duration::hours(25),
                );
                assert!(matches!(
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(settle(&pending)),
                    Settlement::Keep
                ));
            },
        );
    }
}
