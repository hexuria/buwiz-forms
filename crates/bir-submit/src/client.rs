//! The client of buwiz-server: the device key, tickets, sealed v2
//! submissions, the relay pipe and receipts, with the host's sign-in
//! ([`TokenSource`]).

use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::device::{self, DeviceKey, HEADER_DEVICE, hex};
use crate::pipe::{self, Ended};
use crate::receipt::Receipt;
use crate::seal::{self, SealingKey};
use crate::secrets::SecretStore;
use crate::trust::{self, TrustList};
use crate::wire::{
    Blame, FailureClass, TinRefusal, capped_wait_until, is_final_reason, is_hold_code,
    is_limit_code,
};

/// The production Buwiz server, for a host that has no URL of its own.
pub const DEFAULT_SERVER_URL: &str = "https://api.buwiz.com";
/// The header that names the app and its version on every request to the
/// Buwiz server: `buwiz-forms/0.2.1`, `mini/0.5.0`.
pub const CLIENT_HEADER: &str = "buwiz-client";
pub const DEVICE_KEY: &str = "device_key";
pub const DEVICE_ID: &str = "device_id";

/// The gate a host installs around the lazy device-key creation (bir-core's relay does): the
/// creation closure runs INSIDE the host's device-key lock, so no key is ever created outside
/// the funnel the registration and the forget already use — and the host records the key's
/// owner in the same step. With no gate installed the creation runs ungated, as before (tests
/// of bir-submit alone).
pub type DeviceKeyGate = std::sync::Arc<
    dyn Fn(&mut dyn FnMut() -> Result<DeviceKey, ClientError>) -> Result<DeviceKey, ClientError>
        + Send
        + Sync,
>;

static DEVICE_KEY_GATE: std::sync::RwLock<Option<DeviceKeyGate>> = std::sync::RwLock::new(None);

/// Install the creation gate. The first install wins; later calls are ignored (the host wires
/// it once at startup).
pub fn set_device_key_gate(gate: DeviceKeyGate) {
    let mut slot = DEVICE_KEY_GATE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if slot.is_none() {
        *slot = Some(gate);
    }
}

fn device_key_gate() -> Option<DeviceKeyGate> {
    DEVICE_KEY_GATE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// The lazy first key, as every client creates it: the stored key when there is one, else a
/// generated one — created through the host's gate when one is installed (F7): under the
/// device-key lock, with the owner recorded in the same step, so no key is ever created
/// without an owner record, and a forget can never delete a key whose owner record it would
/// have skipped, nor miss one it should take.
pub fn device_key_or_create(
    store: &dyn crate::secrets::SecretStore,
) -> Result<DeviceKey, ClientError> {
    if let Some(key) = store
        .get(DEVICE_KEY)?
        .as_deref()
        .and_then(DeviceKey::from_stored)
    {
        return Ok(key);
    }
    let mut create = || {
        let key = DeviceKey::generate();
        store.set(DEVICE_KEY, &key.to_stored())?;
        Ok(key)
    };
    if let Some(gate) = device_key_gate() {
        return gate(&mut create);
    }
    create()
}

/// The app's one Buwiz sign-in. This crate never signs in, refreshes or stores a token.
pub trait TokenSource: Send + Sync {
    /// An access token for the signed-in account; `force_refresh` after the server answered 401 `invalid_token`.
    /// `Err(ClientError::SignedOut)` when no one is signed in.
    fn access_token(
        &self,
        force_refresh: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, ClientError>> + Send + '_>>;
    /// Who is signed in, as the cache key of the account — the user id, never the email (PLAN §A.5:
    /// the TIN-list cache is keyed by user id); empty when unknown, `None` when no one is. No request.
    fn login(&self) -> Option<String>;
    /// The server ended the sign-in (`403 device_revoked`): the host signs the user out.
    fn session_ended(&self);
    /// Whether device-key deletion still belongs to the session that made this call. Hosts with
    /// per-session sends override it; the default keeps single-session clients unchanged.
    fn device_key_is_current(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug)]
pub struct SubmitConfig {
    pub server_url: String,
    /// Shown on the account site's device list.
    pub device_name: String,
    /// Public keys (base64url) the signed trust list must verify with:
    /// [`trust::pinned_keys`] unless a test or lab sets its own in code, or
    /// the host sets [`trust::pinned_keys_from`] with the value it read.
    pub trust_keys: Vec<String>,
    /// This install's id, sent with the registration (the host's own; never
    /// a raw hardware value).
    pub install_id: Option<String>,
    /// `ios`, `ipados`, `android`, `macos`, `windows` or `linux`, sent with
    /// the registration.
    pub platform: Option<String>,
}

/// An https URL, or an http one whose host is this machine.
pub(crate) fn is_private_url(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

impl SubmitConfig {
    /// The host's own server URL (its [`DEFAULT_SERVER_URL`] unless it says
    /// otherwise). Only https, or http to this machine: tokens never cross
    /// a network in plain text. Any other URL is an error.
    pub fn new(server_url: &str) -> Result<Self, ClientError> {
        let url = server_url.trim();
        if !is_private_url(url) {
            return Err(ClientError::Other(format!(
                "The Buwiz server URL must be https, or http to this machine: {url}"
            )));
        }
        Ok(Self {
            server_url: url.trim_end_matches('/').to_owned(),
            device_name: format!("eBIRForms on {}", std::env::consts::OS),
            trust_keys: trust::pinned_keys(),
            install_id: None,
            platform: None,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Sign in to send returns.")]
    SignedOut,
    /// The server answered `403 device_revoked`: the device, and the sign-in
    /// it belongs to, were removed on the account site. The host was told
    /// ([`TokenSource::session_ended`]). Not a try: nothing reached the BIR.
    #[error("This device was removed from your Buwiz account. Sign in again to send returns.")]
    DeviceRevoked,
    #[error("could not reach the Buwiz server: {0}")]
    Network(String),
    /// The server's sentence, with its code in brackets so the app can
    /// tell `tin_not_active` and the like apart.
    #[error("{message} ({code})")]
    Server {
        status: u16,
        code: String,
        message: String,
    },
    /// A second registration on this sign-in was refused (`409
    /// device_registered`) while the app holds no registered key: it lost
    /// its device key (the key file or Keychain item is gone). Only a new
    /// sign-in can register one. Not a try, and never tried again by itself.
    #[error("This device's key is missing. Sign out and sign in again to send returns.")]
    DeviceKeyMissing,
    /// `409 install_in_use`: this install's device belongs to another live
    /// sign-in. This client sends its install id from the start, so it should
    /// never see it. Not a try, never tried again by itself.
    #[error("Sign out and sign in again on this device.")]
    InstallInUse,
    /// The account credential or its store failed. This is account evidence,
    /// never evidence about the submission being settled.
    #[error("{0}")]
    Account(String),
    #[error("{0}")]
    Other(String),
    /// The server says this was not a try and nothing reached the BIR: a
    /// hold (`503` with `bir_offline`, `bir_busy`, `relay_paused` or
    /// `restarting`), or a fault on Buwiz's side (`class: "ours"`). Its
    /// text keeps the code in brackets, as [`ClientError::Server`]'s does.
    /// `wait_until` is the body's, or `Retry-After` from now.
    #[error("{message} ({code})")]
    NotATry {
        status: u16,
        code: String,
        message: String,
        wait_until: Option<DateTime<Utc>>,
    },
}

impl ClientError {
    /// The TIN refusal the server's code names, if any.
    pub fn tin_refusal(&self) -> Option<TinRefusal> {
        match self {
            ClientError::Server { code, .. } => TinRefusal::from_code(code),
            _ => None,
        }
    }

    /// Whether this error says nothing reached the BIR and no try was made:
    /// a hold or class `ours` ([`ClientError::NotATry`]), being signed out
    /// (or the device removed), or not reaching the Buwiz server at all.
    pub fn is_not_a_try(&self) -> bool {
        matches!(
            self,
            ClientError::NotATry { .. }
                | ClientError::SignedOut
                | ClientError::DeviceRevoked
                | ClientError::DeviceKeyMissing
                | ClientError::InstallInUse
                | ClientError::Network(_)
        )
    }

    /// The server's `wait_until` on a hold.
    pub fn wait_until(&self) -> Option<DateTime<Utc>> {
        match self {
            ClientError::NotATry { wait_until, .. } => *wait_until,
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.to_string())
    }
}

impl From<crate::secrets::StoreError> for ClientError {
    fn from(error: crate::secrets::StoreError) -> Self {
        Self::Other(error.to_string())
    }
}

/// How a send ended when it did not end in a receipt.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// Nothing reached BIR; sending again is safe.
    #[error("{0}")]
    NotSent(String),
    /// BIR may have the return. Never send it again automatically.
    #[error("{0}")]
    MaybeSent(String),
    /// Nothing reached BIR and it was not a try: Buwiz held the ticket, the
    /// relay failed on Buwiz's side (class `ours`), the app is signed out or
    /// could not reach Buwiz. Sending again is safe; the queue does not
    /// count it. Not before `wait_until` when the server gave one.
    #[error("{reason}")]
    NotATry {
        reason: String,
        wait_until: Option<DateTime<Utc>>,
    },
    /// Something touched the BIR's way in and it did not get through; sending
    /// again is safe. It counts as a try, and `blame` says where it failed:
    /// the whole queue backs off for a way in that is down, this return
    /// alone for the rest.
    #[error("{reason}")]
    Counted { reason: String, blame: Blame },
}

impl SendError {
    /// The TIN refusal a not-sent reason ends with
    /// ([`TinRefusal::from_reason`]). Never one for [`SendError::MaybeSent`]:
    /// BIR may have that return whatever its text says.
    pub fn refusal(&self) -> Option<TinRefusal> {
        match self {
            SendError::NotSent(reason) => TinRefusal::from_reason(reason),
            SendError::MaybeSent(_) | SendError::NotATry { .. } | SendError::Counted { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sent {
    pub submission_id: String,
    pub receipt_jws: String,
    pub receipt: Receipt,
}

/// What the host keeps about a send in progress, so a later process can ask
/// the server how it ended ([`Submitter::settle`]) when this one dies first.
/// It holds no token, no key and no part of the return: the submission's id,
/// the ticket's id and the hash of the XML the receipt must name.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingSend {
    /// The Buwiz server the submission was made on ([`SubmitConfig::server_url`]).
    pub server: String,
    pub submission_id: String,
    /// The ticket's id, which the receipt must carry.
    pub jti: String,
    /// Lowercase hex SHA-256 of the plain XML that was sealed.
    pub xml_sha256: String,
    /// The app was about to open, or had opened, a connection toward the BIR.
    pub opening: bool,
    /// When the submission was made (the first note).
    pub noted_at: DateTime<Utc>,
}

/// The host's note of a send's progress ([`Submitter::send_noting`]). An
/// error stops the send before anything more leaves this computer.
pub type Note<'a> = &'a (dyn Fn(&PendingSend) -> Result<(), String> + Send + Sync);

/// What the server says of a [`PendingSend`] ([`Submitter::settle`]).
#[derive(Clone, Debug)]
pub enum Settled {
    /// It reached the BIR (or the server's dry run): its receipt verified
    /// against the trust list, the XML hash and the ticket id.
    Sent(Box<Sent>),
    /// It ended without a receipt, as the send itself would have said.
    Failed(SendError),
    /// The server could not be asked (offline, an error, another server) or
    /// the submission is not final yet: ask again later.
    NotYet,
    /// The token owner signed out or another owner is signed in: keep the
    /// waiting record; it is never evidence that the send is gone.
    SignedOut,
    /// The server does not know the submission (404).
    Gone,
}

/// The statuses a submission ends in. Anything else is still under way.
fn is_final_status(status: &str) -> bool {
    matches!(
        status,
        "sent" | "dry_run" | "failed" | "expired" | "unknown"
    )
}

/// The parts of an eBIRForms file name, `{TIN}-{form}-{period}#{email}#.xml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileName {
    pub tin: String,
    pub form: String,
    pub period: String,
    pub email: String,
}

impl FileName {
    pub fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_suffix("#.xml")?;
        let (head, email) = stem.split_once('#')?;
        let mut parts = head.splitn(3, '-');
        let (tin, form, period) = (parts.next()?, parts.next()?, parts.next()?);
        let digits = matches!(tin.len(), 12 | 14) && tin.bytes().all(|byte| byte.is_ascii_digit());
        (digits && !form.is_empty() && !period.is_empty() && email.contains('@')).then(|| Self {
            tin: tin.to_owned(),
            form: form.to_owned(),
            period: period.to_owned(),
            email: email.to_owned(),
        })
    }
}

/// The longest the relay WebSocket may take to open.
const RELAY_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What [`Submitter::register_device`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registered {
    /// This sign-in's device was registered now.
    New,
    /// It already was (a kept id, or `409 device_registered`).
    AlreadyDone,
}

/// One app's connection to buwiz-server. Cheap to share behind an `Arc`.
pub struct Submitter {
    config: SubmitConfig,
    http: reqwest::Client,
    store: Box<dyn SecretStore>,
    /// The host's sign-in.
    tokens: Arc<dyn TokenSource>,
    /// Serializes device registration.
    session_lock: tokio::sync::Mutex<()>,
    /// The account's TIN list ([`crate::tins`]).
    pub(crate) tins: crate::tins::TinsCache,
    /// The [`CLIENT_HEADER`] value.
    client: RwLock<String>,
    /// Per idempotency key: when the server's last open submission can no
    /// longer hold the key (its ticket's expiry plus [`OPEN_GRACE`]). A send
    /// before then would be answered `already_submitted`, which reads as
    /// maybe sent, so none is made.
    open_until: Mutex<std::collections::HashMap<String, DateTime<Utc>>>,
}

/// How long past its ticket's expiry the server may still hold a try open.
const OPEN_GRACE: chrono::Duration = chrono::Duration::seconds(30);
/// The server's ticket lifetime (300 s) plus the grace, for an answer that
/// names no expiry.
const OPEN_DEFAULT: chrono::Duration = chrono::Duration::seconds(330);
/// No answer holds a key longer than this, whatever expiry it names.
const OPEN_MOST: chrono::Duration = chrono::Duration::seconds(900);

/// What `send_return` learned about the server's open submission, for
/// `keep_floor`.
#[derive(Default)]
struct OpenTry {
    /// The floor once a ticket was obtained.
    until: Option<DateTime<Utc>>,
    /// The server's own status (`failed`, `expired`) says the submission is
    /// closed.
    closed: bool,
}

/// The ticket answer's `expires_at` (unix seconds) plus the grace, or now
/// plus 330 s when it is absent or unreadable.
fn open_floor(ticket: &Value, now: DateTime<Utc>) -> DateTime<Utc> {
    let expiry = ticket["expires_at"]
        .as_i64()
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .map(|expiry| expiry.min(now + OPEN_MOST - OPEN_GRACE) + OPEN_GRACE);
    expiry.unwrap_or(now + OPEN_DEFAULT)
}

/// The app name the header carries until the app says its own:
/// `buwiz-forms` and this crate's version.
fn default_client() -> String {
    format!(
        "buwiz-forms/{}",
        header_word(env!("CARGO_PKG_VERSION"), "0")
    )
}

/// A header-safe name or version: ASCII letters, digits and `._-+`, at most
/// 32 characters. Anything else (a space, a control character, a non-ASCII
/// letter) becomes `-`; a word with nothing left is `fallback`. Never an
/// error: the app is named whatever it was called.
fn header_word(word: &str, fallback: &str) -> String {
    let clean: String = word
        .trim()
        .chars()
        .take(32)
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-+".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    if clean.chars().all(|c| c == '-') {
        fallback.to_owned()
    } else {
        clean
    }
}

impl Submitter {
    pub fn new(
        config: SubmitConfig,
        store: Box<dyn SecretStore>,
        tokens: Arc<dyn TokenSource>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(format!(
                "buwiz-forms/{} ({})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS
            ))
            .build()
            .expect("an HTTP client");
        Self {
            http,
            store,
            tokens,
            session_lock: tokio::sync::Mutex::new(()),
            tins: crate::tins::TinsCache::default(),
            client: RwLock::new(default_client()),
            open_until: Mutex::new(std::collections::HashMap::new()),
            config,
        }
    }

    pub fn config(&self) -> &SubmitConfig {
        &self.config
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.config.server_url)
    }

    /// Name the app in the [`CLIENT_HEADER`] of every later request:
    /// `set_client("buwiz-forms", "0.2.1")`. A name or version with characters
    /// a header cannot carry is cleaned (each such character becomes `-`),
    /// never refused.
    pub fn set_client(&self, app: &str, version: &str) {
        if let Ok(mut client) = self.client.write() {
            *client = format!("{}/{}", header_word(app, "app"), header_word(version, "0"));
        }
    }

    /// The [`CLIENT_HEADER`] value requests carry now.
    pub fn client(&self) -> String {
        self.client
            .read()
            .map(|client| client.clone())
            .unwrap_or_else(|_| default_client())
    }

    /// A request to the Buwiz server at `path`, with the [`CLIENT_HEADER`].
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.url(path))
            .header(CLIENT_HEADER, self.client())
    }

    /// Who is signed in (the email, empty when unknown); `None` when no one
    /// is. Asks the host ([`TokenSource::login`]), which makes no request.
    pub(crate) fn login(&self) -> Option<String> {
        self.tokens.login()
    }

    /// `GET path` with the bearer token alone and a 5 second limit, for
    /// [`crate::tins`]: once more with a refreshed token when the server says
    /// the token expired.
    pub(crate) async fn bearer_get(&self, path: &str) -> Result<Value, ClientError> {
        self.bearer_send(reqwest::Method::GET, path, None).await
    }

    /// [`Self::bearer_get`] with any method and an optional JSON body. Asked
    /// once more only after `401 invalid_token`, which the server answers
    /// before it does anything.
    pub(crate) async fn bearer_send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, ClientError> {
        let mut refreshed = false;
        loop {
            let access_token = self.tokens.access_token(refreshed).await?;
            let mut request = self
                .request(method.clone(), path)
                .bearer_auth(&access_token)
                .timeout(std::time::Duration::from_secs(5));
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await?;
            match self.ok_json(response).await {
                Err(ClientError::Server {
                    status: 401, code, ..
                }) if code == "invalid_token" && !refreshed => {
                    refreshed = true;
                }
                other => return other,
            }
        }
    }

    async fn authorized_get(&self, path: &str) -> Result<Value, ClientError> {
        let access_token = self.tokens.access_token(false).await?;
        let response = self
            .request(reqwest::Method::GET, path)
            .bearer_auth(&access_token)
            .send()
            .await?;
        self.ok_json(response).await
    }

    // -------------------------------------------------------- device key

    fn device_key(&self) -> Result<DeviceKey, ClientError> {
        device_key_or_create(self.store.as_ref())
    }

    /// Forgets this device's key and id, and the TIN list: a new sign-in
    /// registers a new key. The host calls it on sign-in and sign-out.
    pub fn forget_device(&self) -> Result<(), ClientError> {
        self.tins.clear();
        self.store.delete(DEVICE_KEY)?;
        self.store.delete(DEVICE_ID)?;
        Ok(())
    }

    /// Registers this device right after a sign-in (the host calls it once
    /// the tokens are saved): [`Registered::New`], or
    /// [`Registered::AlreadyDone`] when this sign-in already has its device
    /// (`409 device_registered`, or an id is kept). An error is the host's to
    /// log: the first send registers as it always did.
    pub async fn register_device(&self) -> Result<Registered, ClientError> {
        let _guard = self.session_lock.lock().await;
        if self.store.get(DEVICE_ID)?.is_some() {
            return Ok(Registered::AlreadyDone);
        }
        match self.post_registration().await {
            Ok(_) => Ok(Registered::New),
            Err(ClientError::Server {
                status: 409, code, ..
            }) if code == "device_registered" => Ok(Registered::AlreadyDone),
            Err(ClientError::Server {
                status: 409, code, ..
            }) if code == "install_in_use" => Err(ClientError::InstallInUse),
            Err(error) => Err(error),
        }
    }

    /// Registers the device key once per sign-in; answers its id. A `409
    /// device_registered` here means the sign-in has a device whose key this
    /// app no longer holds: [`ClientError::DeviceKeyMissing`], once.
    async fn ensure_device(&self) -> Result<String, ClientError> {
        if let Some(id) = self.store.get(DEVICE_ID)? {
            return Ok(id);
        }
        let _guard = self.session_lock.lock().await;
        if let Some(id) = self.store.get(DEVICE_ID)? {
            return Ok(id);
        }
        match self.post_registration().await {
            Err(ClientError::Server {
                status: 409, code, ..
            }) if code == "device_registered" => Err(ClientError::DeviceKeyMissing),
            Err(ClientError::Server {
                status: 409, code, ..
            }) if code == "install_in_use" => Err(ClientError::InstallInUse),
            other => other,
        }
    }

    /// `POST /api/devices` with this device's key, name, install id and
    /// platform, signed with the key; keeps and answers the id.
    async fn post_registration(&self) -> Result<String, ClientError> {
        let access_token = self.tokens.access_token(false).await?;
        let key = self.device_key()?;
        let mut fields = json!({ "public_key": key.public_key(), "name": self.config.device_name });
        if let Some(install_id) = &self.config.install_id {
            fields["install_id"] = json!(install_id);
        }
        if let Some(platform) = &self.config.platform {
            fields["platform"] = json!(platform);
        }
        let body = serde_json::to_vec(&fields).expect("json");
        let path = "/api/devices";
        let mut request = self
            .request(reqwest::Method::POST, path)
            .bearer_auth(&access_token)
            .header("content-type", "application/json")
            .body(body.clone());
        for (name, value) in key.sign("POST", path, &body, device::now(), &device::nonce()) {
            request = request.header(name, value);
        }
        let registered: Value = self.ok_json(request.send().await?).await?;
        let id = registered["id"]
            .as_str()
            .ok_or_else(|| ClientError::Other("no device id".into()))?
            .to_owned();
        self.store.set(DEVICE_ID, &id)?;
        Ok(id)
    }

    /// A request signed with the device key. Retries once with a refreshed
    /// token when the access token has just expired, and once with a newly
    /// registered key when the server does not know this device (it belongs
    /// to another sign-in).
    pub async fn signed(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Value, ClientError> {
        let body = serde_json::to_vec(body).expect("json");
        let mut refreshed = false;
        let mut force_refresh = false;
        let mut registered_again = false;
        loop {
            let device_id = self.ensure_device().await?;
            let key = self.device_key()?;
            let access_token = self
                .tokens
                .access_token(std::mem::take(&mut force_refresh))
                .await?;
            let method_value = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|e| ClientError::Other(e.to_string()))?;
            let mut request = self
                .request(method_value, path)
                .bearer_auth(&access_token)
                .header(HEADER_DEVICE, &device_id)
                .header("content-type", "application/json")
                .body(body.clone());
            for (name, value) in key.sign(method, path, &body, device::now(), &device::nonce()) {
                request = request.header(name, value);
            }
            match self.ok_json(request.send().await?).await {
                Err(ClientError::Server {
                    status: 401, code, ..
                }) if code == "invalid_token" && !refreshed => {
                    refreshed = true;
                    force_refresh = true;
                }
                Err(ClientError::Server {
                    status: 401, code, ..
                }) if code == "unknown_device" && !registered_again => {
                    if self.tokens.device_key_is_current() {
                        self.forget_device()?;
                    }
                    registered_again = true;
                }
                Err(ClientError::Server { code, .. }) if code == "device_revoked" => {
                    if self.tokens.device_key_is_current() {
                        self.forget_device()?;
                    }
                    self.tokens.session_ended();
                    return Err(ClientError::DeviceRevoked);
                }
                other => return other,
            }
        }
    }

    /// The server's relay trust list, checked against the pinned keys.
    /// Without a valid one there is no send.
    pub async fn trust_list(&self) -> Result<TrustList, ClientError> {
        let response = self
            .request(reqwest::Method::GET, "/.well-known/buwiz-trust.json")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ClientError::Other(format!(
                "Can't send: the server has no signed trust list ({}).",
                response.status()
            )));
        }
        let text = response.text().await?;
        let jws = text.trim().trim_matches('"');
        trust::verify(&self.config.trust_keys, jws, device::now())
            .map_err(|error| ClientError::Other(format!("Can't send: {error}.")))
    }

    /// The Sender's sealing key: the one `/.well-known/buwiz-hpke.json`
    /// names as current, taken only if `list` carries it.
    pub async fn sealing_key(&self, list: &TrustList) -> Result<SealingKey, ClientError> {
        let document: Value = self
            .ok_json(
                self.request(reqwest::Method::GET, "/.well-known/buwiz-hpke.json")
                    .send()
                    .await?,
            )
            .await?;
        let offered: SealingKey = serde_json::from_value(document["keys"][0].clone())
            .map_err(|_| ClientError::Other("the server published no sealing key".into()))?;
        list.sealing_key(&offered)
            .map_err(|error| ClientError::Other(format!("Can't send: {error}.")))
    }

    /// Checks a receipt against the receipt keys of the signed trust list
    /// only; any other key is refused.
    pub async fn verify_receipt(
        &self,
        jws: &str,
        xml_sha256: &str,
        jti: Option<&str>,
    ) -> Result<Receipt, ClientError> {
        let list = self.trust_list().await?;
        list.verify_receipt(jws, xml_sha256, jti)
            .map_err(|error| ClientError::Other(error.to_string()))
    }

    /// Everything a send needs before the return itself: a live sign-in,
    /// a registered device, a valid trust list and a listed sealing key.
    /// A TIN refusal on the way (a banned account) makes the next TIN list
    /// read ask the server, as one on a send does.
    pub async fn ready(&self) -> Result<(), ClientError> {
        let ready = self.check_ready().await;
        if let Err(error) = &ready
            && error.tin_refusal().is_some()
        {
            self.tins.invalidate();
        }
        ready
    }

    async fn check_ready(&self) -> Result<(), ClientError> {
        let list = self.trust_list().await?;
        self.ensure_device().await?;
        self.sealing_key(&list).await?;
        Ok(())
    }

    // --------------------------------------------------------------- send

    /// Sends one return: ticket, sealed envelope, relay, verified receipt.
    /// A refusal for the TIN makes the next TIN list read ask the server.
    pub async fn send(&self, filename: &str, xml: &[u8]) -> Result<Sent, SendError> {
        self.send_noting(filename, xml, &|_| Ok(())).await
    }

    /// [`Self::send`], telling the host its progress through `note`, and
    /// waiting for it: once the server made the submission and before the
    /// pipe opens (`opening: false`), and again before the first connection
    /// toward the BIR is attempted (`opening: true`). A note that fails ends
    /// the send there, not sent: nothing more leaves this computer.
    pub async fn send_noting(
        &self,
        filename: &str,
        xml: &[u8],
        note: Note<'_>,
    ) -> Result<Sent, SendError> {
        let sent = self.send_return(filename, xml, note).await;
        if let Err(error) = &sent
            && error.refusal().is_some()
        {
            self.tins.invalidate();
        }
        sent
    }

    async fn send_return(
        &self,
        filename: &str,
        xml: &[u8],
        note: Note<'_>,
    ) -> Result<Sent, SendError> {
        let name = FileName::parse(filename)
            .ok_or_else(|| SendError::NotSent(format!("not an eBIRForms file name: {filename}")))?;
        let xml_sha256 = hex(&Sha256::digest(xml));
        let idempotency_key = hex(&Sha256::digest(
            format!("buwiz-send\0{filename}\0{xml_sha256}").as_bytes(),
        ))[..40]
            .to_owned();
        let now = Utc::now();
        if let Some(until) = self.open_floor_for(&idempotency_key, now) {
            let minutes = ((until - now).num_seconds().max(0) + 59) / 60;
            return Err(SendError::NotATry {
                reason: format!(
                    "The last try for this return is still open on the server. It tries again in about {} min.",
                    minutes.max(1)
                ),
                wait_until: Some(until),
            });
        }
        let mut open = OpenTry::default();
        let sent = self
            .send_open(&name, xml, &xml_sha256, &idempotency_key, &mut open, note)
            .await;
        self.keep_floor(&idempotency_key, sent, open)
    }

    /// The floor still in the future for `key`, if any.
    fn open_floor_for(&self, key: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let mut floors = self
            .open_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        floors.retain(|_, until| *until > now);
        floors.get(key).copied()
    }

    /// Keeps or clears the floor for `key` by how the send ended, and lifts
    /// a [`SendError::NotATry`]'s `wait_until` to it. Once a ticket was
    /// obtained, only a success, a maybe-sent or a server status of
    /// `failed` or `expired` leaves the key free to try again at once.
    fn keep_floor(
        &self,
        key: &str,
        sent: Result<Sent, SendError>,
        open: OpenTry,
    ) -> Result<Sent, SendError> {
        let mut floors = self
            .open_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let free = open.closed || matches!(sent, Ok(_) | Err(SendError::MaybeSent(_)));
        let Some(until) = open.until.filter(|_| !free) else {
            if open.until.is_some() || free {
                floors.remove(key);
            }
            return sent;
        };
        floors.insert(key.to_owned(), until);
        match sent {
            Err(SendError::NotATry { reason, wait_until }) => Err(SendError::NotATry {
                reason,
                wait_until: Some(wait_until.map_or(until, |own| own.max(until))),
            }),
            other => other,
        }
    }

    async fn send_open(
        &self,
        name: &FileName,
        xml: &[u8],
        xml_sha256: &str,
        idempotency_key: &str,
        open: &mut OpenTry,
        note: Note<'_>,
    ) -> Result<Sent, SendError> {
        let not_sent = |error: ClientError| {
            if error.is_not_a_try() {
                SendError::NotATry {
                    wait_until: error.wait_until(),
                    reason: error.to_string(),
                }
            } else {
                SendError::NotSent(error.to_string())
            }
        };
        // No valid trust list or listed sealing key, no ticket: nothing is
        // spent, and nothing sealed, for a send that can't be trusted.
        let list = self.trust_list().await.map_err(not_sent)?;
        let sealing_key = self.sealing_key(&list).await.map_err(not_sent)?;

        let ticket = match self
            .signed(
                "POST",
                "/api/tickets",
                &json!({
                    "tin": name.tin, "form": name.form, "period": name.period,
                    "payload_sha256": xml_sha256, "email": name.email, "idempotency_key": idempotency_key,
                }),
            )
            .await
        {
            Ok(ticket) => ticket,
            Err(ClientError::Server { code, message, .. }) if code == "already_submitted" => {
                return Err(SendError::MaybeSent(message));
            }
            Err(error) => return Err(not_sent(error)),
        };
        let (Some(jws), Some(jti)) = (ticket["ticket"].as_str(), ticket["jti"].as_str()) else {
            return Err(SendError::NotSent(
                "the server answered without a ticket".into(),
            ));
        };
        open.until = Some(open_floor(&ticket, Utc::now()));
        let envelope = seal::seal(&sealing_key, jti, xml)
            .map_err(|error| SendError::NotSent(error.to_string()))?;
        let submission = match self
            .signed(
                "POST",
                "/api/v2/submissions",
                &json!({ "ticket": jws, "envelope": envelope }),
            )
            .await
        {
            Ok(submission) => submission,
            Err(error) => return Err(not_sent(error)),
        };
        let (Some(id), Some(relay_path)) =
            (submission["id"].as_str(), submission["relay_path"].as_str())
        else {
            return Err(SendError::NotSent(
                "the server answered without a submission".into(),
            ));
        };

        // Kept by the host before the pipe opens, so a later process can ask
        // how the send ended if this one dies.
        let pending = PendingSend {
            server: self.config.server_url.clone(),
            submission_id: id.to_owned(),
            jti: jti.to_owned(),
            xml_sha256: xml_sha256.to_owned(),
            opening: false,
            noted_at: Utc::now(),
        };
        note(&pending).map_err(|error| {
            SendError::NotSent(format!(
                "The send could not be recorded on this computer, so nothing was relayed: {error}"
            ))
        })?;
        let opening = PendingSend {
            opening: true,
            ..pending
        };
        let before_first_open = || match note(&opening) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!("relay: the send could not be recorded: {error}");
                false
            }
        };
        let ended = self
            .relay_with(&list, relay_path, &before_first_open)
            .await
            .map_err(not_sent)?;
        let mut relay_class = None;
        let mut hold_until = None;
        let mut held = false;
        let touched;
        let (ending, status, error, receipt_jws) = match ended {
            Ended::Stopped => {
                return Err(SendError::NotSent(
                    "The send could not be recorded on this computer, so nothing was opened toward the BIR."
                        .into(),
                ));
            }
            Ended::Result {
                status,
                error,
                receipt,
                class,
                hold,
                opened,
                path_down,
            } => {
                open.closed = matches!(status.as_str(), "failed" | "expired");
                relay_class = class.as_deref().map(FailureClass::from_wire);
                hold_until = hold.as_ref().and_then(|hold| hold.wait_until);
                held = hold.is_some();
                touched = Touched {
                    opened,
                    path_down,
                    dropped: false,
                };
                (Ending::Result, status, error, receipt)
            }
            Ended::Dropped { opened, path_down } => {
                touched = Touched {
                    opened,
                    path_down,
                    dropped: true,
                };
                let ending = if opened {
                    Ending::DroppedAfterOpening
                } else {
                    Ending::DroppedBeforeOpening
                };
                match self.authorized_get(&format!("/api/submissions/{id}")).await {
                    Ok(view) => (
                        ending,
                        {
                            let status = view["status"].as_str().unwrap_or("unknown");
                            open.closed = matches!(status, "failed" | "expired");
                            status.to_owned()
                        },
                        view["error"].as_str().map(str::to_owned),
                        view["receipt_jws"].as_str().map(str::to_owned),
                    ),
                    Err(_) if !opened => (
                        ending,
                        "failed".to_owned(),
                        Some("the connection ended before sending".into()),
                        None,
                    ),
                    Err(_) => (
                        ending,
                        "unknown".to_owned(),
                        Some("the connection ended during the send".into()),
                        None,
                    ),
                }
            }
        };
        let (receipt_jws, receipt) = outcome(ending, &status, error, receipt_jws, |receipt_jws| {
            list.verify_receipt(receipt_jws, xml_sha256, Some(jti))
        })
        .map_err(|failure| relay_failure(failure, relay_class, held, hold_until, touched))?;
        Ok(Sent {
            submission_id: id.to_owned(),
            receipt_jws,
            receipt,
        })
    }

    /// Opens the relay WebSocket for a submission and forwards, only to the
    /// hosts of the verified trust list, until the server's result.
    pub async fn relay(&self, relay_path: &str) -> Result<Ended, ClientError> {
        let list = self.trust_list().await?;
        self.relay_with(&list, relay_path, &|| true).await
    }

    async fn relay_with(
        &self,
        list: &TrustList,
        relay_path: &str,
        before_first_open: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<Ended, ClientError> {
        let device_id = self.ensure_device().await?;
        let key = self.device_key()?;
        let access_token = self.tokens.access_token(false).await?;
        let url = self.url(relay_path).replacen("http", "ws", 1);
        let mut request = url
            .into_client_request()
            .map_err(|error| ClientError::Other(error.to_string()))?;
        let headers = request.headers_mut();
        let mut put = |name: &'static str, value: String| -> Result<(), ClientError> {
            let value = value
                .parse()
                .map_err(|_| ClientError::Other(format!("bad {name} header")))?;
            headers.insert(name, value);
            Ok(())
        };
        put("authorization", format!("Bearer {access_token}"))?;
        put(CLIENT_HEADER, self.client())?;
        put(HEADER_DEVICE, device_id)?;
        for (name, value) in key.sign("GET", relay_path, b"", device::now(), &device::nonce()) {
            put(name, value)?;
        }
        let (socket, _) = tokio::time::timeout(
            RELAY_CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| ClientError::Network("the relay did not answer in time".into()))?
        .map_err(|error| match error {
            tokio_tungstenite::tungstenite::Error::Http(response) => ClientError::Server {
                status: response.status().as_u16(),
                code: "relay_refused".into(),
                message: format!("the server refused the relay ({})", response.status()),
            },
            other => ClientError::Network(other.to_string()),
        })?;
        Ok(pipe::serve_noting(socket, list, pipe::Limits::default(), before_first_open).await)
    }

    /// How a send this app noted ([`Self::send_noting`]) ended, asked of the
    /// server after the process that made it is gone. A final status is
    /// decided exactly as a dropped relay's read-back is ([`outcome`], then
    /// [`relay_failure`]), with the ending `pending.opening` says; a receipt
    /// counts only when it verifies against the trust list, `pending`'s XML
    /// hash and ticket id. A status that is not final yet, or a server that
    /// cannot be asked, is [`Settled::NotYet`].
    pub async fn settle(&self, pending: &PendingSend) -> Settled {
        if pending.server.trim_end_matches('/') != self.config.server_url {
            return Settled::NotYet;
        }
        let view = match self
            .bearer_get(&format!("/api/submissions/{}", pending.submission_id))
            .await
        {
            Ok(view) => view,
            Err(ClientError::Server { status: 404, .. }) => return Settled::Gone,
            Err(ClientError::SignedOut) => return Settled::SignedOut,
            // A token-store failure or a second `invalid_token` is account evidence, never
            // evidence that the submission is gone: the queue keeps the noted send. Other local
            // read failures (an unreadable answer, a failed task join) say nothing about either.
            Err(ClientError::Account(_)) => return Settled::SignedOut,
            Err(ClientError::Other(_)) => return Settled::NotYet,
            Err(ClientError::Server {
                status: 401,
                ref code,
                ..
            }) if code == "invalid_token" => {
                return Settled::SignedOut;
            }
            Err(_) => return Settled::NotYet,
        };
        let Some(status) = view["status"]
            .as_str()
            .filter(|status| is_final_status(status))
        else {
            return Settled::NotYet;
        };
        let receipt_jws = view["receipt_jws"].as_str().map(str::to_owned);
        let list = if receipt_jws.is_some() && matches!(status, "sent" | "dry_run") {
            match self.trust_list().await {
                Ok(list) => Some(list),
                Err(_) => return Settled::NotYet,
            }
        } else {
            None
        };
        let ending = if pending.opening {
            Ending::DroppedAfterOpening
        } else {
            Ending::DroppedBeforeOpening
        };
        let decided = outcome(
            ending,
            status,
            view["error"].as_str().map(str::to_owned),
            receipt_jws,
            |receipt_jws| match &list {
                Some(list) => list
                    .verify_receipt(receipt_jws, &pending.xml_sha256, Some(&pending.jti))
                    .map_err(|error| error.to_string()),
                None => Err("no trust list".to_owned()),
            },
        );
        match decided {
            Ok((receipt_jws, receipt)) => Settled::Sent(Box::new(Sent {
                submission_id: pending.submission_id.clone(),
                receipt_jws,
                receipt,
            })),
            Err(failure) => Settled::Failed(relay_failure(
                failure,
                None,
                false,
                None,
                Touched {
                    opened: pending.opening,
                    path_down: false,
                    dropped: true,
                },
            )),
        }
    }
}

/// How the relay ended, for [`outcome`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ending {
    /// The server delivered its `result` over the socket.
    Result,
    /// The socket closed before any connection toward BIR was made; the
    /// status was read back from the server.
    DroppedBeforeOpening,
    /// The socket closed after the relay had opened a connection toward
    /// BIR; the status was read back from the server.
    DroppedAfterOpening,
}

/// What a send's final status means. `verify` checks a receipt and runs
/// only for `sent` or `dry_run` with one; a verified receipt counts only
/// when its signed claims say what the status says ([`claims_disagree`]).
/// Once the relay had opened toward BIR and dropped, a status read back at
/// that moment may not record the upload yet, so only an explicit `failed`
/// is not sent.
fn outcome<E: std::fmt::Display>(
    ending: Ending,
    status: &str,
    error: Option<String>,
    receipt_jws: Option<String>,
    verify: impl FnOnce(&str) -> Result<Receipt, E>,
) -> Result<(String, Receipt), SendError> {
    match (status, receipt_jws) {
        ("sent" | "dry_run", Some(receipt_jws)) => match verify(&receipt_jws) {
            Ok(receipt) => match claims_disagree(status, &receipt) {
                None => Ok((receipt_jws, receipt)),
                Some(why) => Err(SendError::MaybeSent(format!(
                    "the server's answer and its signed receipt disagree ({why}); BIR may have received this return; check for its confirmation email before sending again"
                ))),
            },
            Err(error) => Err(SendError::MaybeSent(format!(
                "the server reports the return sent, but its receipt does not check out ({error}); check for BIR's confirmation email"
            ))),
        },
        ("expired" | "ready", _) if ending == Ending::DroppedAfterOpening => Err(SendError::MaybeSent(format!(
            "the connection ended after the relay opened toward BIR, and the server still reports the return {status}; BIR may have received this return; check for its confirmation email before sending again"
        ))),
        ("failed" | "expired" | "ready", _) => {
            Err(SendError::NotSent(error.unwrap_or_else(|| "the server did not send the return".into())))
        }
        _ => Err(SendError::MaybeSent(error.unwrap_or_else(|| {
            "BIR may have received this return; check for its confirmation email before sending again".into()
        }))),
    }
}

/// Why the server's final `status` and a verified receipt's signed claims
/// do not tell of the same send, or `None` when they agree:
///
/// | status    | claims `status` | claims `mode`   |
/// |-----------|-----------------|-----------------|
/// | `sent`    | `sent`          | `live` or `lab` |
/// | `dry_run` | `dry_run`       | `dry_run` or `lab` |
///
/// Anything else disagrees: a `sent` whose claims say `dry_run` (in either
/// claim), a `dry_run` whose claims say `sent` or `live`, and a claim this
/// client does not know. A disagreement is never a send nor a test send: the
/// person is asked ([`SendError::MaybeSent`]).
fn claims_disagree(status: &str, receipt: &Receipt) -> Option<String> {
    let modes: &[&str] = match status {
        "sent" => &["live", "lab"],
        "dry_run" => &["dry_run", "lab"],
        _ => &[],
    };
    if receipt.status == status && modes.contains(&receipt.mode.as_str()) {
        return None;
    }
    Some(format!(
        "the server says {status}, the receipt says status {} in mode {}",
        receipt.status, receipt.mode
    ))
}

/// What the app saw of its own connections toward the BIR during a send.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Touched {
    /// Some connection opened: the BIR was reached, whatever the class says.
    opened: bool,
    /// Every connection the app attempted failed on the way in.
    path_down: bool,
    /// The relay's socket ended without a result.
    dropped: bool,
}

/// The BIR's front desk (the dispatcher) failed to answer: the try never got
/// past it.
fn front_desk_failed(reason: &str) -> bool {
    let reason = reason.to_ascii_lowercase();
    [
        "dispatcher did not answer",
        "dispatcher request failed",
        "dispatcher answered",
    ]
    .iter()
    .any(|text| reason.contains(text))
}

/// The BIR has no upload service for this TIN (the dispatcher's mode 0).
pub fn no_service_reason(reason: &str) -> bool {
    reason
        .to_ascii_lowercase()
        .contains("no upload server for this tin")
}

/// A refused login after the upload stream opened: the relay's own words
/// ("SFTP login was refused"), not any text that mentions a login.
fn login_refused(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("login was refused")
}

/// What a relay that ended without a receipt means for the queue.
///
/// - A maybe-sent outcome stays maybe sent, whatever else is said.
/// - A refusal code (`tin_*`, `account_banned`) or `return_mismatch` always
///   wins: not sent, final, whatever class the relay gave.
/// - A hold, or class `ours` when NO stream opened, is not a try: nothing
///   touched the BIR. If any stream opened it is a counted failure whatever
///   its class, and nothing decides that from `class` or `bir`.
/// - A counted failure is blamed on the way in (every open the app attempted
///   was refused, or the front desk failed) or on the return (no upload
///   service, a refused login), else on the upload.
fn relay_failure(
    failure: SendError,
    class: Option<FailureClass>,
    held: bool,
    wait_until: Option<DateTime<Utc>>,
    touched: Touched,
) -> SendError {
    let SendError::NotSent(reason) = failure else {
        return failure;
    };
    if is_final_reason(&reason) {
        return SendError::NotSent(reason);
    }
    let wait_until = wait_until.map(|until| capped_wait_until(until, Utc::now()));
    if held {
        return SendError::NotATry { reason, wait_until };
    }
    if no_service_reason(&reason) {
        return SendError::Counted {
            reason,
            blame: Blame::NoService,
        };
    }
    let blame = if touched.path_down || (!touched.opened && front_desk_failed(&reason)) {
        Blame::Path
    } else if !touched.opened && touched.dropped {
        // The pipe to Buwiz broke before any connection toward the BIR
        // opened: nothing touched the BIR.
        return SendError::NotATry { reason, wait_until };
    } else if touched.opened && login_refused(&reason) {
        Blame::Return
    } else if touched.opened || class != Some(FailureClass::Ours) {
        Blame::Upload
    } else {
        return SendError::NotATry { reason, wait_until };
    };
    SendError::Counted { reason, blame }
}

impl Submitter {
    /// The JSON body of a 2xx answer, or the server's `{"error":{…}}`. A
    /// `bir` object in the body (the server's BIR sign) is ignored.
    async fn ok_json<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ClientError> {
        let status = response.status();
        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|error| ClientError::Other(format!("unreadable answer: {error}")));
        }
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<i64>().ok());
        let body: Value = response.json().await.unwrap_or_default();
        Err(server_error(
            status.as_u16(),
            retry_after,
            &body,
            Utc::now(),
        ))
    }
}

/// The error a non-2xx answer with `body` stands for. A hold code or class
/// `ours` is [`ClientError::NotATry`], with the body's `wait_until`, else
/// `Retry-After` (the header, then the body's `retry_after`) from `now`.
fn server_error(
    status: u16,
    retry_after: Option<i64>,
    body: &Value,
    now: DateTime<Utc>,
) -> ClientError {
    let (code, message) = error_code_and_message(status, body);
    let error = &body["error"];
    let class = error
        .get("class")
        .or_else(|| body.get("class"))
        .and_then(Value::as_str);
    let ours = class.map(FailureClass::from_wire) == Some(FailureClass::Ours);
    // A refusal code always wins over class: a TIN refusal or a mismatch is
    // final, whoever says it failed.
    let final_code = TinRefusal::from_code(&code).is_some() || code == crate::wire::RETURN_MISMATCH;
    if final_code || !(is_hold_code(&code) || is_limit_code(&code) || ours) {
        return ClientError::Server {
            status,
            code,
            message,
        };
    }
    let wait_until = error
        .get("wait_until")
        .and_then(Value::as_str)
        .and_then(crate::wire::parse_time)
        .or_else(|| {
            retry_after
                .or_else(|| error.get("retry_after").and_then(Value::as_i64))
                .filter(|seconds| *seconds >= 0)
                .map(|seconds| {
                    now + chrono::Duration::seconds(seconds.min(crate::wire::MAX_SERVER_WAIT_S))
                })
        })
        .map(|until| capped_wait_until(until, now));
    ClientError::NotATry {
        status,
        code,
        message,
        wait_until,
    }
}

fn error_code_and_message(status: u16, body: &Value) -> (String, String) {
    match &body["error"] {
        Value::Object(error) => (
            error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_owned(),
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the server refused")
                .to_owned(),
        ),
        Value::String(code) => (
            code.clone(),
            body["error_description"]
                .as_str()
                .unwrap_or(code)
                .to_owned(),
        ),
        _ => ("error".to_owned(), format!("the server answered {status}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_split_into_ticket_fields() {
        let name =
            FileName::parse("00012345300000-1701Qv2018-2026Q1#juana@example.com#.xml").unwrap();
        assert_eq!(name.tin, "00012345300000");
        assert_eq!(
            (name.form.as_str(), name.period.as_str()),
            ("1701Qv2018", "2026Q1")
        );
        assert_eq!(name.email, "juana@example.com");
        assert_eq!(
            FileName::parse("00012345300000-1701Qv2018-2026Q1.xml"),
            None
        );
        assert_eq!(FileName::parse("0001-1701Qv2018-2026Q1#a@b.c#.xml"), None);
    }

    #[test]
    fn the_server_url_must_be_https_or_this_machine() {
        let url = |text: &str| SubmitConfig::new(text).map(|config| config.server_url);
        assert_eq!(url(DEFAULT_SERVER_URL).unwrap(), DEFAULT_SERVER_URL);
        assert_eq!(
            url("http://localhost:3008/").unwrap(),
            "http://localhost:3008"
        );
        assert_eq!(url(" http://127.0.0.1:9 ").unwrap(), "http://127.0.0.1:9");
        assert_eq!(url("http://[::1]:3008").unwrap(), "http://[::1]:3008");
        assert_eq!(url("https://lab.example/").unwrap(), "https://lab.example");
        for refused in [
            "http://buwiz.example",
            "http://localhost.example:3008",
            "ftp://localhost",
            "",
        ] {
            assert!(
                matches!(url(refused), Err(ClientError::Other(_))),
                "{refused:?}"
            );
        }
        let config = SubmitConfig::new(DEFAULT_SERVER_URL).unwrap();
        assert_eq!(
            config.device_name,
            format!("eBIRForms on {}", std::env::consts::OS)
        );
        assert_eq!(config.trust_keys, trust::pinned_keys());
    }

    /// A receipt whose signed claims say `mode` and `status`.
    fn signed(mode: &str, status: &str) -> Receipt {
        Receipt {
            sub: "u1".into(),
            tin: "000000000000".into(),
            form_type: "1601Cv2018".into(),
            period: "102026".into(),
            filename: "f.xml".into(),
            xml_sha256: "00".into(),
            payload_sha256: "00".into(),
            remote_path: "/1601Cv2018/f.xml".into(),
            mode: mode.into(),
            status: status.into(),
            iat: 1,
            jti: None,
        }
    }

    /// What `outcome` answers, as a word: sent, not sent or maybe sent. A
    /// receipt named `mode/status` verifies with those claims (`good` is
    /// `live/sent`, `dry` is `dry_run/dry_run`); any other is forged.
    fn decided(ending: Ending, status: &str, receipt: Option<&str>) -> &'static str {
        let verify = |jws: &str| match jws {
            "good" => Ok(signed("live", "sent")),
            "dry" => Ok(signed("dry_run", "dry_run")),
            named => match named.split_once('/') {
                Some((mode, status)) => Ok(signed(mode, status)),
                None => Err("bad signature"),
            },
        };
        match outcome(ending, status, None, receipt.map(str::to_owned), verify) {
            Ok(_) => "sent",
            Err(SendError::NotSent(_)) => "not sent",
            Err(SendError::MaybeSent(_)) => "maybe sent",
            Err(SendError::NotATry { .. }) => "not a try",
            Err(SendError::Counted { .. }) => "counted",
        }
    }

    /// RELAY-PROTOCOL.md §6: only `failed` says nothing reached BIR. Once
    /// the relay had opened toward BIR and dropped, a `ready` or `expired`
    /// read back may only mean the server has not recorded the upload yet.
    #[test]
    fn every_final_status_maps_to_one_outcome() {
        use Ending::{
            DroppedAfterOpening as Opened, DroppedBeforeOpening as Before, Result as Res,
        };
        const MAYBE: [&str; 3] = ["maybe sent", "maybe sent", "maybe sent"];
        const SENT: [&str; 3] = ["sent", "sent", "sent"];
        let rows: [(&str, Option<&str>, [&str; 3]); 24] = [
            ("sent", Some("good"), SENT),
            ("sent", Some("lab/sent"), SENT),
            ("dry_run", Some("dry"), SENT),
            ("dry_run", Some("lab/dry_run"), SENT),
            ("sent", Some("forged"), MAYBE),
            ("dry_run", Some("forged"), MAYBE),
            ("sent", None, MAYBE),
            ("dry_run", None, MAYBE),
            // The status and the signed claims disagree: neither a send nor a test send.
            ("dry_run", Some("good"), MAYBE),
            ("dry_run", Some("live/dry_run"), MAYBE),
            ("dry_run", Some("dry_run/sent"), MAYBE),
            ("dry_run", Some("lab/sent"), MAYBE),
            ("sent", Some("dry"), MAYBE),
            ("sent", Some("live/dry_run"), MAYBE),
            ("sent", Some("dry_run/sent"), MAYBE),
            ("sent", Some("lab/dry_run"), MAYBE),
            // A claim this client does not know.
            ("sent", Some("/sent"), MAYBE),
            ("sent", Some("staging/sent"), MAYBE),
            ("sent", Some("live/queued"), MAYBE),
            ("failed", None, ["not sent", "not sent", "not sent"]),
            ("expired", None, ["not sent", "not sent", "maybe sent"]),
            ("ready", None, ["not sent", "not sent", "maybe sent"]),
            ("unknown", None, ["maybe sent", "maybe sent", "maybe sent"]),
            (
                "uploading",
                None,
                ["maybe sent", "maybe sent", "maybe sent"],
            ),
        ];
        for (status, receipt, want) in rows {
            for (ending, want) in [Res, Before, Opened].into_iter().zip(want) {
                assert_eq!(
                    decided(ending, status, receipt),
                    want,
                    "{ending:?}, {status}, receipt {receipt:?}"
                );
            }
        }
    }

    /// After the relay opened and dropped, a `ready` or `expired` tells the
    /// person BIR may have the return, not the server's own error.
    #[test]
    fn a_dropped_relay_that_opened_says_bir_may_have_the_return() {
        for status in ["ready", "expired"] {
            let answer = outcome(
                Ending::DroppedAfterOpening,
                status,
                Some("the ticket expired".into()),
                None,
                |_: &str| Ok::<Receipt, String>(signed("live", "sent")),
            );
            let Err(SendError::MaybeSent(message)) = answer else {
                panic!("{status}: {answer:?}");
            };
            assert!(
                message.contains("BIR may have received this return"),
                "{message}"
            );
            assert!(message.contains("confirmation email"), "{message}");
        }
    }

    /// The server's error and the verifier's text pass through as before.
    #[test]
    fn outcome_messages_are_unchanged_where_the_outcome_is() {
        let message = |answer: Result<(String, Receipt), SendError>| match answer {
            Err(SendError::NotSent(message) | SendError::MaybeSent(message)) => message,
            Err(SendError::NotATry { reason, .. } | SendError::Counted { reason, .. }) => reason,
            Ok(_) => panic!("sent"),
        };
        let ok = |_: &str| Ok::<Receipt, String>(signed("live", "sent"));
        for ending in [
            Ending::Result,
            Ending::DroppedBeforeOpening,
            Ending::DroppedAfterOpening,
        ] {
            assert_eq!(
                message(outcome(
                    ending,
                    "failed",
                    Some("no ticket".into()),
                    None,
                    ok
                )),
                "no ticket"
            );
            assert_eq!(
                message(outcome(ending, "failed", None, None, ok)),
                "the server did not send the return"
            );
            assert_eq!(
                message(outcome(
                    ending,
                    "unknown",
                    Some("the connection ended during the send".into()),
                    None,
                    ok
                )),
                "the connection ended during the send"
            );
            assert_eq!(
                message(outcome(ending, "unknown", None, None, ok)),
                "BIR may have received this return; check for its confirmation email before sending again"
            );
            assert_eq!(
                message(outcome(
                    ending,
                    "sent",
                    None,
                    Some("x".into()),
                    |_: &str| Err::<Receipt, _>("bad signature")
                )),
                "the server reports the return sent, but its receipt does not check out (bad signature); check for BIR's confirmation email"
            );
        }
        for ending in [Ending::Result, Ending::DroppedBeforeOpening] {
            assert_eq!(
                message(outcome(
                    ending,
                    "expired",
                    Some("the ticket expired".into()),
                    None,
                    ok
                )),
                "the ticket expired"
            );
            assert_eq!(
                message(outcome(ending, "ready", None, None, ok)),
                "the server did not send the return"
            );
        }
    }

    const TIN_CODES: [&str; 8] = [
        "tin_not_active",
        "tin_unverified",
        "tin_pending",
        "tin_needs_fix",
        "tin_paused",
        "tin_test_only",
        "account_banned",
        "account_practice",
    ];

    /// A ticket refusal reaches the worker as `ClientError::Server`'s text,
    /// which keeps the code for the one decoder.
    #[test]
    fn a_ticket_refusal_keeps_its_code_in_the_text() {
        for code in TIN_CODES {
            let error = ClientError::Server {
                status: 403,
                code: code.into(),
                message: "Some sentence (with brackets) the server chose.".into(),
            };
            let text = error.to_string();
            assert!(text.ends_with(&format!("({code})")), "{text}");
            let refusal = TinRefusal::from_reason(&text).expect(code);
            assert_eq!(refusal.code(), code);
            assert_eq!(error.tin_refusal(), Some(refusal));
            let not_sent = SendError::NotSent(format!("Not sent (nothing reached BIR): {text}"));
            assert_eq!(not_sent.refusal(), Some(refusal));
        }
        let other = ClientError::Server {
            status: 409,
            code: "already_submitted".into(),
            message: "x".into(),
        };
        assert_eq!(other.tin_refusal(), None);
        assert_eq!(
            ClientError::Network("tin_not_active".into()).tin_refusal(),
            None
        );
    }

    /// The relay may end `failed` with a TIN code when the TIN stopped
    /// being sendable after the ticket: nothing reached BIR, the same
    /// refusal. A maybe-sent text with a code is never one.
    #[test]
    fn a_relay_failed_with_a_tin_code_is_a_refusal() {
        let ok = |_: &str| Ok::<Receipt, String>(signed("live", "sent"));
        for ending in [
            Ending::Result,
            Ending::DroppedBeforeOpening,
            Ending::DroppedAfterOpening,
        ] {
            let error = Some(
                "Sending is paused for this TIN. Write to support@buwiz.com. (tin_paused)"
                    .to_owned(),
            );
            let Err(failure) = outcome(ending, "failed", error, None, ok) else {
                panic!("a failed relay is not sent");
            };
            assert!(matches!(failure, SendError::NotSent(_)), "{failure:?}");
            assert_eq!(failure.refusal(), Some(TinRefusal::Paused));
        }
        let maybe =
            SendError::MaybeSent("the connection ended during the send (tin_paused)".into());
        assert_eq!(maybe.refusal(), None);
        assert_eq!(
            SendError::NotSent("the relay closed early (relay_closed)".into()).refusal(),
            None
        );
    }

    /// A good receipt comes back with its JWS; `verify` runs only for one.
    #[test]
    fn outcome_verifies_only_a_sent_receipt() {
        let answer = outcome(
            Ending::Result,
            "dry_run",
            None,
            Some("jws".into()),
            |jws: &str| {
                assert_eq!(jws, "jws");
                Ok::<_, String>(signed("dry_run", "dry_run"))
            },
        );
        assert_eq!(
            answer.unwrap(),
            ("jws".to_owned(), signed("dry_run", "dry_run"))
        );
        let answer = outcome(
            Ending::Result,
            "failed",
            None,
            Some("jws".into()),
            |_: &str| -> Result<Receipt, String> { panic!("verified a failed send's receipt") },
        );
        assert!(matches!(answer, Err(SendError::NotSent(_))));
    }

    /// A verified receipt whose claims say another send than the status asks
    /// the person, and says what disagrees.
    #[test]
    fn a_status_its_receipt_disagrees_with_says_what_disagrees() {
        for (status, mode, claimed) in [("dry_run", "live", "sent"), ("sent", "dry_run", "dry_run")]
        {
            let answer = outcome(
                Ending::Result,
                status,
                None,
                Some("jws".into()),
                |_: &str| Ok::<_, String>(signed(mode, claimed)),
            );
            let Err(SendError::MaybeSent(message)) = answer else {
                panic!("{status} {mode} {claimed}: {answer:?}");
            };
            assert_eq!(
                message,
                format!(
                    "the server's answer and its signed receipt disagree (the server says {status}, the receipt says status {claimed} in mode {mode}); BIR may have received this return; check for its confirmation email before sending again"
                )
            );
        }
    }

    // ------------------------------------------ the host's sign-in

    /// A signed call asks the host for a token once; after a 401
    /// `invalid_token` once more with `force_refresh`, and only once.
    #[tokio::test]
    async fn the_token_is_asked_again_with_force_refresh_after_invalid_token() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, seen) = serve(|request| match request.header("authorization") {
            Some("Bearer a2") => Answer::json(200, &json!({"ok": true})),
            _ => Answer::json(
                401,
                &json!({"error": {"code": "invalid_token", "message": "expired"}}),
            ),
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        assert_eq!(
            client
                .signed("POST", "/api/tickets", &json!({}))
                .await
                .unwrap(),
            json!({"ok": true})
        );
        assert_eq!(tokens.asked(), [false, true]);
        assert_eq!(seen.lock().unwrap().len(), 2);

        let (url, seen) = serve(|_| {
            Answer::json(
                401,
                &json!({"error": {"code": "invalid_token", "message": "expired"}}),
            )
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ClientError::Server { status: 401, code, .. } if code == "invalid_token"),
            "{error:?}"
        );
        assert_eq!(tokens.asked(), [false, true], "refreshed once, not again");
        assert_eq!(seen.lock().unwrap().len(), 2);

        // A plain call is asked once, never forced.
        let (url, _) = serve(|_| Answer::json(200, &json!({"ok": true})));
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap();
        assert_eq!(tokens.asked(), [false]);
    }

    /// The host's `SignedOut` is the call's, and nothing is asked of the
    /// server; a send signed out is not a try.
    #[tokio::test]
    async fn the_hosts_signed_out_surfaces_as_signed_out() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, seen) = serve(|_| Answer::json(200, &json!({"id": "d2"})));
        let (client, _) = submitter_tokens(&url, false, Vec::new());
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::SignedOut), "{error:?}");
        assert!(matches!(
            client.bearer_get("/api/tax-profiles").await,
            Err(ClientError::SignedOut)
        ));
        assert!(seen.lock().unwrap().is_empty());
        assert!(error.is_not_a_try());
    }

    /// `403 device_revoked`: the device is forgotten, the host told the
    /// sign-in ended, and the sentence is the one before.
    #[tokio::test]
    async fn a_revoked_device_is_forgotten_and_the_session_ended() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, _) = serve(|_| {
            Answer::json(
                403,
                &json!({"error": {"code": "device_revoked", "message": "revoked"}}),
            )
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        client.device_key().unwrap();
        assert!(client.store.get(DEVICE_KEY).unwrap().is_some());
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "This device was removed from your Buwiz account. Sign in again to send returns."
        );
        assert!(matches!(error, ClientError::DeviceRevoked), "{error:?}");
        assert!(error.is_not_a_try(), "nothing reached the BIR");
        assert_eq!(tokens.ended(), 1);
        assert_eq!(client.store.get(DEVICE_KEY).unwrap(), None);
        assert_eq!(client.store.get(DEVICE_ID).unwrap(), None);
        assert_eq!(client.login(), None, "the host signed out");
    }

    /// F8: an ended send's `device_revoked` must not delete the newer session's key. The client
    /// asks its token source whether the device epoch is still current before forgetting anything.
    #[tokio::test]
    async fn a_revoked_device_of_an_old_epoch_is_not_forgotten() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, _) = serve(|_| {
            Answer::json(
                403,
                &json!({"error": {"code": "device_revoked", "message": "revoked"}}),
            )
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        client.device_key().unwrap();
        tokens.make_device_epoch_stale();
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::DeviceRevoked), "{error:?}");
        assert!(
            client.store.get(DEVICE_KEY).unwrap().is_some(),
            "the newer session's device key was deleted"
        );
        assert_eq!(client.store.get(DEVICE_ID).unwrap().as_deref(), Some("d"));
    }

    /// `401 unknown_device` (the device is another sign-in's): the device
    /// is forgotten, a new key registered once and the call made once more.
    #[tokio::test]
    async fn an_unknown_device_registers_a_new_key_and_tries_once_more() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let unknown = || {
            Answer::json(
                401,
                &json!({"error": {"code": "unknown_device", "message": "who?"}}),
            )
        };
        let (url, seen) =
            serve(
                move |request| match (request.path.as_str(), request.header(HEADER_DEVICE)) {
                    ("/api/devices", _) => Answer::json(200, &json!({"id": "d2"})),
                    ("/api/tickets", Some("d2")) => Answer::json(200, &json!({"ok": true})),
                    _ => unknown(),
                },
            );
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        let old_key = client.device_key().unwrap().public_key();
        assert_eq!(
            client
                .signed("POST", "/api/tickets", &json!({}))
                .await
                .unwrap(),
            json!({"ok": true})
        );
        let paths: Vec<(String, Option<String>)> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|request| {
                (
                    request.path.clone(),
                    request.header(HEADER_DEVICE).map(str::to_owned),
                )
            })
            .collect();
        assert_eq!(
            paths,
            [
                ("/api/tickets".to_owned(), Some("d".to_owned())),
                ("/api/devices".to_owned(), None),
                ("/api/tickets".to_owned(), Some("d2".to_owned())),
            ]
        );
        assert_eq!(client.store.get(DEVICE_ID).unwrap().as_deref(), Some("d2"));
        assert_ne!(
            client.device_key().unwrap().public_key(),
            old_key,
            "a new key"
        );
        assert_eq!(tokens.ended(), 0, "not a sign-out");

        // A second `unknown_device` is the answer: registered once, not again.
        let (url, seen) = serve(move |request| match request.path.as_str() {
            "/api/devices" => Answer::json(200, &json!({"id": "d2"})),
            _ => unknown(),
        });
        let (client, _) = submitter_tokens(&url, true, Vec::new());
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ClientError::Server { status: 401, code, .. } if code == "unknown_device"),
            "{error:?}"
        );
        let paths: Vec<String> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.path.clone())
            .collect();
        assert_eq!(paths, ["/api/tickets", "/api/devices", "/api/tickets"]);
    }

    // ----------------------------------------------- registration after a sign-in

    /// Right after a sign-in: one registration with the install id and the
    /// platform; a second is already done, by the kept id or by `409`.
    #[tokio::test]
    async fn a_new_sign_in_registers_its_device_once_with_the_install_id() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url2, seen2) = serve(|request| match request.path.as_str() {
            "/api/devices" => Answer::json(200, &json!({"id": "d7"})),
            _ => Answer::json(404, &json!({})),
        });
        let (mut client, _) = submitter_tokens(&url2, true, Vec::new());
        client.store.delete(DEVICE_ID).unwrap();
        client.config.install_id = Some("install-id-of-this-test-0001".into());
        client.config.platform = Some("macos".into());
        assert_eq!(client.register_device().await.unwrap(), Registered::New);
        assert_eq!(client.store.get(DEVICE_ID).unwrap().as_deref(), Some("d7"));
        assert_eq!(
            client.register_device().await.unwrap(),
            Registered::AlreadyDone
        );
        let sent = seen2.lock().unwrap();
        assert_eq!(sent.len(), 1, "one registration");
        let body: Value = serde_json::from_str(&sent[0].body).unwrap();
        assert_eq!(body["install_id"], "install-id-of-this-test-0001");
        assert_eq!(body["platform"], "macos");
        assert_eq!(body["name"], "test");
        for later in ["model", "os_version", "computer_name"] {
            assert!(body.get(later).is_none(), "{later} is a later change");
        }
    }

    #[tokio::test]
    async fn a_409_after_the_sign_in_is_already_done_and_other_failures_are_errors() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, _) = serve(|_| {
            Answer::json(
                409,
                &json!({"error": "device_registered", "message": "already"}),
            )
        });
        let (client, _) = submitter_tokens(&url, true, Vec::new());
        client.store.delete(DEVICE_ID).unwrap();
        assert_eq!(
            client.register_device().await.unwrap(),
            Registered::AlreadyDone
        );
        let (url, _) = serve(|_| Answer::json(500, &json!({"error": "oops"})));
        let (client, _) = submitter_tokens(&url, true, Vec::new());
        client.store.delete(DEVICE_ID).unwrap();
        assert!(client.register_device().await.is_err());
    }

    /// `409 install_in_use`: a plain sentence where the send is held, after
    /// the sign-in and on a send alike; one request each, no loop.
    #[tokio::test]
    async fn an_install_in_use_says_sign_in_again_and_never_loops() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, seen) = serve(|request| match request.path.as_str() {
            "/api/devices" => Answer::json(
                409,
                &json!({"error": "install_in_use", "message": "in use"}),
            ),
            _ => Answer::json(404, &json!({})),
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        client.store.delete(DEVICE_ID).unwrap();
        let after_sign_in = client.register_device().await.unwrap_err();
        assert!(
            matches!(after_sign_in, ClientError::InstallInUse),
            "{after_sign_in:?}"
        );
        let on_send = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert_eq!(
            on_send.to_string(),
            "Sign out and sign in again on this device."
        );
        assert!(on_send.is_not_a_try());
        assert_eq!(
            seen.lock().unwrap().len(),
            2,
            "one registration each, no loop"
        );
        assert_eq!(tokens.ended(), 0);
    }

    /// The device key is gone while still signed in: the server does not
    /// F8: an ended send's `unknown_device` must not delete the newer session's key either.
    #[tokio::test]
    async fn an_unknown_device_of_an_old_epoch_is_not_forgotten() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, _) = serve(|_| {
            Answer::json(
                401,
                &json!({"error": {"code": "unknown_device", "message": "who?"}}),
            )
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        client.device_key().unwrap();
        tokens.make_device_epoch_stale();
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ClientError::Server { status: 401, code, .. } if code == "unknown_device"),
            "{error:?}"
        );
        assert!(
            client.store.get(DEVICE_KEY).unwrap().is_some(),
            "the newer session's device key was deleted"
        );
        assert_eq!(client.store.get(DEVICE_ID).unwrap().as_deref(), Some("d"));
    }

    /// know the new key (`401 unknown_device`), the one registration again is
    /// refused (`409`): one plain sentence, held, never a loop.
    #[tokio::test]
    async fn a_lost_device_key_says_so_once_and_never_loops() {
        use crate::testkit::{Answer, serve, submitter_tokens};
        let (url, seen) = serve(|request| match request.path.as_str() {
            "/api/devices" => Answer::json(
                409,
                &json!({"error": "device_registered", "message": "already"}),
            ),
            _ => Answer::json(
                401,
                &json!({"error": {"code": "unknown_device", "message": "who?"}}),
            ),
        });
        let (client, tokens) = submitter_tokens(&url, true, Vec::new());
        let error = client
            .signed("POST", "/api/tickets", &json!({}))
            .await
            .unwrap_err();
        assert!(matches!(error, ClientError::DeviceKeyMissing), "{error:?}");
        assert_eq!(
            error.to_string(),
            "This device's key is missing. Sign out and sign in again to send returns."
        );
        assert!(error.is_not_a_try());
        let paths: Vec<String> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.path.clone())
            .collect();
        assert_eq!(paths, ["/api/tickets", "/api/devices"]);
        assert_eq!(tokens.ended(), 0, "not signed out by itself");
        // The next send asks once more and says the same; still no loop.
        assert!(matches!(
            client.signed("POST", "/api/tickets", &json!({})).await,
            Err(ClientError::DeviceKeyMissing)
        ));
        assert_eq!(
            seen.lock().unwrap().len(),
            3,
            "one registration try per send"
        );
    }

    // ----------------------------------------------- the version header

    #[tokio::test]
    async fn every_request_names_the_app_and_its_version() {
        use crate::testkit::{Answer, serve, submitter};
        let (url, seen) = serve(|_| {
            Answer::json(
                404,
                &json!({"error": {"code": "not_found", "message": "no such route"}}),
            )
        });
        let client = submitter(&url, true, Vec::new());
        let desktop = format!("buwiz-forms/{}", env!("CARGO_PKG_VERSION"));
        assert_eq!(client.client(), desktop);
        let _ = client.trust_list().await;
        let _ = client.bearer_get("/api/tax-profiles").await;
        client.set_client("mini", "0.5.0");
        let _ = client.signed("POST", "/api/tickets", &json!({})).await;
        client.set_client("mini app", "0.5.0");
        assert_eq!(
            client.client(),
            "mini-app/0.5.0",
            "a space is cleaned, not refused"
        );
        client.set_client("mini", "0.5.0\r\nx-evil: 1");
        assert_eq!(
            client.client(),
            "mini/0.5.0--x-evil--1",
            "no control character reaches a header"
        );
        client.set_client("", "");
        assert_eq!(client.client(), "app/0");
        client.set_client("mini", "0.5.0");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(
            seen.iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            ["GET", "GET", "POST"]
        );
        let agent = format!(
            "buwiz-forms/{} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        );
        assert!(
            seen.iter()
                .all(|request| request.header("user-agent") == Some(agent.as_str()))
        );
        let labels: Vec<(&str, Option<&str>)> = seen
            .iter()
            .map(|request| (request.path.as_str(), request.header(CLIENT_HEADER)))
            .collect();
        assert_eq!(
            labels,
            [
                ("/.well-known/buwiz-trust.json", Some(desktop.as_str())),
                ("/api/tax-profiles", Some(desktop.as_str())),
                ("/api/tickets", Some("mini/0.5.0")),
            ]
        );
    }

    // ------------------------------------------------------- the holds

    fn hold(status: u16, retry_after: Option<i64>, fixture: &str) -> ClientError {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-11T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        server_error(
            status,
            retry_after,
            &crate::testkit::fixture_json(fixture),
            now,
        )
    }

    /// The public contract (`docs/bir-uptime.md` in `buwiz-com/buwiz-com`): every hold is not a try, keeps its code in its
    /// text, and carries `wait_until` (the body's, else `Retry-After`).
    #[test]
    fn the_holds_of_api_5_2_and_10_are_not_tries() {
        let at = |text: &str| Some(crate::wire::parse_time(text).unwrap());
        let offline = hold(503, Some(841), "contract-hold-bir-offline.json");
        let ClientError::NotATry {
            status,
            code,
            wait_until,
            ..
        } = &offline
        else {
            panic!("{offline:?}")
        };
        assert_eq!((*status, code.as_str()), (503, "bir_offline"));
        assert_eq!(
            *wait_until,
            at("2026-11-11T09:13:41Z"),
            "the body's exact time"
        );
        assert!(offline.to_string().ends_with("(bir_offline)"));
        assert!(offline.is_not_a_try());
        assert_eq!(offline.tin_refusal(), None);

        let busy = hold(503, Some(600), "contract-hold-bir-busy.json");
        assert_eq!(
            busy.wait_until(),
            at("2026-11-11T09:10:00Z"),
            "Retry-After from now"
        );
        assert!(busy.is_not_a_try());

        let paused = hold(503, None, "contract-error-relay-paused.json");
        assert!(
            matches!(&paused, ClientError::NotATry { code, wait_until: None, .. } if code == "relay_paused")
        );

        let restarting = server_error(
            503,
            Some(30),
            &json!({"error": {"code": "restarting", "message": "Buwiz is restarting."}}),
            Utc::now(),
        );
        assert!(restarting.is_not_a_try());
        assert!(restarting.wait_until().is_some());

        let internal = hold(500, None, "contract-error-internal-ours.json");
        assert!(
            matches!(&internal, ClientError::NotATry { code, .. } if code == "internal"),
            "class ours"
        );

        // Everything else stays a server error, as before.
        for (status, body) in [
            (
                403,
                json!({"error": {"code": "tin_not_active", "message": "x"}}),
            ),
            (
                503,
                json!({"error": {"code": "a_code_from_the_future", "message": "x"}}),
            ),
            (
                500,
                json!({"error": {"code": "internal", "class": "bir", "message": "x"}}),
            ),
            (
                400,
                json!({"error": "invalid_grant", "error_description": "x"}),
            ),
        ] {
            let error = server_error(status, Some(10), &body, Utc::now());
            assert!(matches!(error, ClientError::Server { .. }), "{body}");
            assert!(!error.is_not_a_try());
        }
        assert!(ClientError::SignedOut.is_not_a_try());
        assert!(ClientError::Network("refused".into()).is_not_a_try());
        assert!(!ClientError::Other("Can't send: no list.".into()).is_not_a_try());
        // A Retry-After that makes no sense is no time.
        let silly = server_error(
            503,
            Some(-5),
            &json!({"error": {"code": "bir_busy"}}),
            Utc::now(),
        );
        assert_eq!(silly.wait_until(), None);
    }

    /// A relay refusal carrying class `ours` still goes by its code: a TIN
    /// refusal or a mismatch is final, and 429 limits are not tries but
    /// The server's own body for a practice account (`403 account_practice`,
    /// buwiz-com: an account with a fixed sign-in code never sends, whatever
    /// its TINs): a refusal with the server's sentence and its code, never a
    /// hold and never "maybe sent".
    #[test]
    fn a_practice_account_is_refused_with_the_servers_sentence() {
        let body: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/contract-error-account-practice.json"
        ))
        .unwrap();
        let error = server_error(403, None, &body, Utc::now());
        assert!(
            matches!(&error, ClientError::Server { status: 403, code, .. } if code == "account_practice"),
            "{error:?}"
        );
        assert_eq!(error.tin_refusal(), Some(TinRefusal::AccountPractice));
        assert!(!error.is_not_a_try(), "a refusal, not a hold");
        let reason = error.to_string();
        assert_eq!(
            reason,
            "This is a practice account: you can prepare returns, but Buwiz never sends them to the BIR. (account_practice)"
        );
        // As the queue stores it: the one decoder reads the code back, and it is final.
        assert_eq!(
            SendError::NotSent(reason.clone()).refusal(),
            Some(TinRefusal::AccountPractice)
        );
        assert!(crate::wire::is_final_reason(&reason));
    }

    /// honour `Retry-After` (up to 7 days).
    #[test]
    fn a_refusal_code_wins_over_class_ours_and_a_429_is_not_a_try() {
        for code in [
            "tin_not_active",
            "tin_needs_fix",
            "account_banned",
            "account_practice",
            "return_mismatch",
        ] {
            let body = json!({"error": {"code": code, "class": "ours", "message": "x"}});
            for status in [403, 409, 500, 503] {
                let error = server_error(status, Some(60), &body, Utc::now());
                assert!(
                    matches!(error, ClientError::Server { .. }),
                    "{code} {status}: {error:?}"
                );
            }
        }
        let now = Utc::now();
        for code in ["rate_limited", "quota_exceeded", "try_anyway_limited"] {
            let body = json!({"error": {"code": code, "message": "x"}});
            let error = server_error(429, Some(600), &body, now);
            assert!(error.is_not_a_try(), "{code}");
            assert_eq!(
                error.wait_until(),
                Some(now + chrono::Duration::seconds(600))
            );
        }
        let body = json!({"error": {"code": "rate_limited", "message": "x"}});
        let year = server_error(429, Some(365 * 86_400), &body, now);
        assert_eq!(
            year.wait_until(),
            Some(now + chrono::Duration::days(7)),
            "beyond 7 days reads as 7 days"
        );
        let far = json!({"error": {"code": "bir_busy", "wait_until": "2099-01-01T00:00:00Z", "message": "x"}});
        assert_eq!(
            server_error(503, None, &far, now).wait_until(),
            Some(now + chrono::Duration::days(7))
        );
    }

    /// Nothing decides a resend from `class`: a result is not a try only
    /// when no stream opened.
    #[test]
    fn a_relay_failure_is_not_a_try_only_when_nothing_opened() {
        let wait = Some(Utc::now());
        let failed = |text: &str| SendError::NotSent(text.into());
        let none = Touched::default();
        let opened = Touched {
            opened: true,
            ..none
        };
        let down = Touched {
            path_down: true,
            ..none
        };
        let dropped = Touched {
            dropped: true,
            ..none
        };
        let ours = Some(FailureClass::Ours);

        // Class ours, nothing opened: not a try. A hold is, whatever opened.
        assert_eq!(
            relay_failure(failed("held"), ours, false, wait, none),
            SendError::NotATry {
                reason: "held".into(),
                wait_until: wait
            }
        );
        assert!(matches!(
            relay_failure(failed("held"), None, true, wait, opened),
            SendError::NotATry { .. }
        ));
        // Class ours, a stream opened: a counted failure.
        assert_eq!(
            relay_failure(failed("write failed"), ours, false, wait, opened),
            SendError::Counted {
                reason: "write failed".into(),
                blame: Blame::Upload
            }
        );
        // Other classes, nothing opened and nothing known: the old reading.
        for class in [
            None,
            Some(FailureClass::Bir),
            Some(FailureClass::UserNetwork),
        ] {
            assert_eq!(
                relay_failure(failed("x"), class, false, None, none),
                SendError::Counted {
                    reason: "x".into(),
                    blame: Blame::Upload
                }
            );
        }
        // The way in down, whatever the class says: the path.
        for class in [ours, None] {
            assert_eq!(
                relay_failure(failed("could not reach BIR"), class, false, None, down),
                SendError::Counted {
                    reason: "could not reach BIR".into(),
                    blame: Blame::Path
                }
            );
        }
        let desk = "BIR's dispatcher did not answer: tls://x: dispatcher request failed";
        assert!(matches!(
            relay_failure(failed(desk), None, false, None, none),
            SendError::Counted {
                blame: Blame::Path,
                ..
            }
        ));
        // A refused login after the stream opened is the return's own.
        assert!(matches!(
            relay_failure(failed("the login was refused"), None, false, None, opened),
            SendError::Counted {
                blame: Blame::Return,
                ..
            }
        ));
        // No upload service for the TIN.
        let none_for_tin = "dispatcher has no upload server for this TIN (mode 0)";
        assert!(matches!(
            relay_failure(failed(none_for_tin), None, false, None, none),
            SendError::Counted {
                blame: Blame::NoService,
                ..
            }
        ));
        // A pipe that broke before anything opened touched nothing.
        assert!(matches!(
            relay_failure(
                failed("the connection ended before sending"),
                None,
                false,
                None,
                dropped
            ),
            SendError::NotATry { .. }
        ));
        // A refusal code or a mismatch wins over class and over a hold.
        for text in [
            "no (tin_not_active)",
            "no (account_banned)",
            "no (return_mismatch)",
        ] {
            for (class, held) in [(ours, false), (ours, true), (None, false)] {
                assert_eq!(
                    relay_failure(failed(text), class, held, wait, none),
                    failed(text),
                    "{text}"
                );
            }
        }
        // Maybe sent stays maybe sent, whatever the class.
        let maybe = SendError::MaybeSent("the connection ended during the send".into());
        assert_eq!(relay_failure(maybe.clone(), ours, true, wait, none), maybe);
        assert_eq!(
            SendError::NotATry {
                reason: "x (tin_not_active)".into(),
                wait_until: None
            }
            .refusal(),
            None
        );
    }

    /// The relay's real failure texts, each pinned to its blame. They come
    /// from `src/bir/dispatcher.rs`, `src/bir/sftp.rs` and `src/bir/mod.rs`
    /// in `buwiz-com/buwiz-com`. `None` is not a try. The upload server's
    /// texts are written without their leading protocol name ("SFTP login
    /// was refused" is "login was refused" here): the repo's secrets check
    /// keeps that word out of code, and the matching never reads it.
    #[test]
    fn every_relay_failure_text_has_its_blame() {
        let none = Touched::default();
        let opened = Touched {
            opened: true,
            ..none
        };
        let ours = Some(FailureClass::Ours);
        let rows: [(&str, Touched, Option<FailureClass>, Option<Blame>); 14] = [
            // The front desk did not answer: the way in.
            (
                "dispatcher request failed: connection reset",
                none,
                None,
                Some(Blame::Path),
            ),
            ("dispatcher answered 502", none, None, Some(Blame::Path)),
            (
                "BIR's dispatcher did not answer: tls://example.test:1: timed out",
                none,
                None,
                Some(Blame::Path),
            ),
            // It answered, and the answer was the fault: not the way in.
            (
                "dispatcher answer was not understood: bad frame",
                none,
                ours,
                None,
            ),
            (
                "dispatcher answer was not understood: bad frame",
                none,
                None,
                Some(Blame::Upload),
            ),
            (
                "the dispatcher named an unknown upload server: example.test:22",
                none,
                ours,
                None,
            ),
            (
                "the dispatcher named an unknown upload server: example.test:22",
                none,
                None,
                Some(Blame::Upload),
            ),
            (
                "dispatcher has no upload server for this TIN (mode 0)",
                none,
                None,
                Some(Blame::NoService),
            ),
            // After a stream opened.
            ("login was refused", opened, None, Some(Blame::Return)),
            (
                "SSH failed: connection reset",
                opened,
                None,
                Some(Blame::Upload),
            ),
            (
                "failed before the upload started: no such file",
                opened,
                None,
                Some(Blame::Upload),
            ),
            (
                "failed during the upload: broken pipe",
                opened,
                None,
                Some(Blame::Upload),
            ),
            (
                "timed out before the upload started",
                opened,
                None,
                Some(Blame::Upload),
            ),
            (
                "the server's host key is not pinned (SHA256:abc)",
                opened,
                ours,
                Some(Blame::Upload),
            ),
        ];
        for (text, touched, class, want) in rows {
            let got = relay_failure(SendError::NotSent(text.into()), class, false, None, touched);
            match (want, got) {
                (Some(blame), SendError::Counted { blame: got, .. }) => {
                    assert_eq!(got, blame, "{text}")
                }
                (None, SendError::NotATry { .. }) => {}
                (want, got) => panic!("{text}: wanted {want:?}, got {got:?}"),
            }
        }
        for text in [
            "dispatcher answer was not understood: x",
            "the dispatcher named an unknown upload server: x",
            "login was refused",
            "SSH failed: x",
        ] {
            assert!(
                !front_desk_failed(text),
                "{text} is not the way in being down"
            );
        }
        // A transient BIR error that merely says "login" is not the return's.
        for text in [
            "BIR login page is down",
            "authentication service busy",
            "permission denied by a proxy",
        ] {
            let got = relay_failure(SendError::NotSent(text.into()), None, false, None, opened);
            assert!(
                matches!(
                    got,
                    SendError::Counted {
                        blame: Blame::Upload,
                        ..
                    }
                ),
                "{text}: {got:?}"
            );
        }
        assert!(no_service_reason(
            "dispatcher has no upload server for this TIN (mode 0)"
        ));
        assert!(login_refused("The Login Was Refused"));
    }

    /// End to end on loopback: a held ticket comes back as not a try with
    /// the hold's time; the hold's sign (`bir` in the body) is ignored.
    #[tokio::test]
    async fn a_held_ticket_is_not_sent_and_not_a_try() {
        use crate::testkit::{Answer, fixture_json, serve, submitter, trust_documents};
        let (public, jws, sealing) = trust_documents(&["127.0.0.1:9"]);
        let (url, seen) = serve(move |request| match request.path.as_str() {
            "/.well-known/buwiz-trust.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: jws.clone(),
            },
            "/.well-known/buwiz-hpke.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: sealing.clone(),
            },
            "/api/tickets" => Answer::json(503, &fixture_json("contract-hold-bir-offline.json"))
                .with("retry-after", "841")
                .with("buwiz-bir", "v=1, seq=4100, state=offline"),
            _ => Answer::json(
                404,
                &json!({"error": {"code": "not_found", "message": "x"}}),
            ),
        });
        let client = submitter(&url, true, vec![public]);
        let sent = client
            .send(
                "00000000000000-1601Cv2018-102026#juana@example.test#.xml",
                b"<xml/>",
            )
            .await;
        let Err(SendError::NotATry { reason, wait_until }) = sent else {
            panic!("{sent:?}")
        };
        assert!(reason.ends_with("(bir_offline)"), "{reason}");
        assert_eq!(wait_until, crate::wire::parse_time("2026-11-11T09:13:41Z"));
        let seen = seen.lock().unwrap();
        assert!(seen.iter().any(|request| request.path == "/api/tickets"));
        assert!(
            !seen
                .iter()
                .any(|request| request.path == "/api/v2/submissions"),
            "no submission after a hold"
        );
        assert!(
            seen.iter()
                .all(|request| request.header(CLIENT_HEADER).is_some())
        );
    }

    /// Nothing reachable at all: not a try, never a counted failure.
    #[tokio::test]
    async fn an_unreachable_server_is_not_a_try() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let client = crate::testkit::submitter(&url, true, Vec::new());
        let sent = client
            .send(
                "00000000000000-1601Cv2018-102026#juana@example.test#.xml",
                b"<xml/>",
            )
            .await;
        assert!(
            matches!(
                sent,
                Err(SendError::NotATry {
                    wait_until: None,
                    ..
                })
            ),
            "{sent:?}"
        );
    }

    // ------------------------------------------- an open submission's floor

    use crate::testkit::{Answer, Seen, serve, submitter, trust_documents};

    const RETURN: &str = "00000000000000-1601Cv2018-102026#juana@example.test#.xml";

    /// A loopback server that gives a ticket (expiring 300 s out), then
    /// answers the submission, the relay and the read-back as told.
    fn filing(
        submission: Answer,
        relay: Answer,
        read_back: Answer,
    ) -> (Submitter, std::sync::Arc<Mutex<Vec<Seen>>>, i64) {
        filing_to(&["127.0.0.1:9"], submission, relay, read_back)
    }

    /// [`filing`] with a trust list of `hosts`.
    fn filing_to(
        hosts: &[&str],
        submission: Answer,
        relay: Answer,
        read_back: Answer,
    ) -> (Submitter, std::sync::Arc<Mutex<Vec<Seen>>>, i64) {
        let (public, jws, sealing) = trust_documents(hosts);
        let expires_at = Utc::now().timestamp() + 300;
        let (url, seen) = serve(move |request| match request.path.as_str() {
            "/.well-known/buwiz-trust.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: jws.clone(),
            },
            "/.well-known/buwiz-hpke.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: sealing.clone(),
            },
            "/api/tickets" => Answer::json(
                200,
                &json!({"ticket": "a.b.c", "jti": "jti-1", "expires_at": expires_at}),
            ),
            "/api/v2/submissions" => submission.clone(),
            "/relay/s1" => relay.clone(),
            "/api/submissions/s1" => read_back.clone(),
            _ => Answer::json(
                404,
                &json!({"error": {"code": "not_found", "message": "x"}}),
            ),
        });
        (submitter(&url, true, vec![public]), seen, expires_at)
    }

    fn created() -> Answer {
        Answer::json(200, &json!({"id": "s1", "relay_path": "/relay/s1"}))
    }

    fn count(seen: &std::sync::Arc<Mutex<Vec<Seen>>>) -> usize {
        seen.lock().unwrap().len()
    }

    fn tickets(seen: &std::sync::Arc<Mutex<Vec<Seen>>>) -> usize {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|request| request.path == "/api/tickets")
            .count()
    }

    /// The floor: the ticket's expiry plus 30 s.
    fn floor(expires_at: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(expires_at + 30, 0).unwrap()
    }

    /// The first send ended not sent while the server may hold the
    /// submission open: it waits for the ticket's expiry plus 30 s, and an
    /// immediate second send makes no request at all.
    async fn waits_for_the_floor(
        client: &Submitter,
        seen: &std::sync::Arc<Mutex<Vec<Seen>>>,
        expires_at: i64,
    ) {
        let before = count(seen);
        let second = client.send(RETURN, b"<xml/>").await;
        let Err(SendError::NotATry { reason, wait_until }) = second else {
            panic!("{second:?}")
        };
        assert!(reason.contains("still open on the server"), "{reason}");
        assert!(wait_until.unwrap() >= floor(expires_at));
        assert_eq!(count(seen), before, "no request while the last try is open");
    }

    #[tokio::test]
    async fn a_submission_post_that_dies_in_transport_waits_for_the_ticket_to_expire() {
        let (client, seen, expires_at) =
            filing(Answer::hang_up(), Answer::hang_up(), Answer::hang_up());
        let first = client.send(RETURN, b"<xml/>").await;
        let Err(SendError::NotATry { wait_until, .. }) = first else {
            panic!("{first:?}")
        };
        assert!(
            wait_until.unwrap() >= floor(expires_at),
            "the queue stores the floor"
        );
        assert!(
            seen.lock()
                .unwrap()
                .iter()
                .any(|request| request.path == "/api/v2/submissions")
        );
        waits_for_the_floor(&client, &seen, expires_at).await;
        assert_eq!(tickets(&seen), 1);
    }

    #[tokio::test]
    async fn an_unreadable_submission_answer_waits_for_the_ticket_to_expire() {
        let garbled = Answer {
            status: 200,
            headers: Vec::new(),
            body: "not json".into(),
        };
        let (client, seen, expires_at) = filing(garbled, Answer::hang_up(), Answer::hang_up());
        let first = client.send(RETURN, b"<xml/>").await;
        assert!(matches!(first, Err(SendError::NotSent(_))), "{first:?}");
        waits_for_the_floor(&client, &seen, expires_at).await;
    }

    #[tokio::test]
    async fn a_relay_that_cannot_be_opened_waits_for_the_ticket_to_expire() {
        // Hung up: the app could not open the socket.
        let (client, seen, expires_at) = filing(created(), Answer::hang_up(), Answer::hang_up());
        let first = client.send(RETURN, b"<xml/>").await;
        let Err(SendError::NotATry { wait_until, .. }) = first else {
            panic!("{first:?}")
        };
        assert!(wait_until.unwrap() >= floor(expires_at));
        waits_for_the_floor(&client, &seen, expires_at).await;
        // Refused with an HTTP answer: not sent, and the same floor.
        let refused = Answer::json(
            403,
            &json!({"error": {"code": "forbidden", "message": "x"}}),
        );
        let (client, seen, expires_at) = filing(created(), refused, Answer::hang_up());
        let first = client.send(RETURN, b"<xml/>").await;
        assert!(matches!(first, Err(SendError::NotSent(_))), "{first:?}");
        waits_for_the_floor(&client, &seen, expires_at).await;
    }

    #[tokio::test]
    async fn a_relay_that_dropped_before_opening_with_the_submission_ready_keeps_the_floor() {
        let ready = Answer::json(200, &json!({"status": "ready"}));
        let (client, seen, expires_at) = filing(created(), Answer::relay(&[]), ready);
        let first = client.send(RETURN, b"<xml/>").await;
        let Err(SendError::NotATry { wait_until, .. }) = first else {
            panic!("{first:?}")
        };
        assert!(wait_until.unwrap() >= floor(expires_at));
        waits_for_the_floor(&client, &seen, expires_at).await;
        // Its status cannot be read back: the same.
        let (client, seen, expires_at) = filing(created(), Answer::relay(&[]), Answer::hang_up());
        let first = client.send(RETURN, b"<xml/>").await;
        assert!(matches!(first, Err(SendError::NotATry { .. })), "{first:?}");
        waits_for_the_floor(&client, &seen, expires_at).await;
    }

    #[tokio::test]
    async fn a_failed_or_expired_submission_leaves_no_floor() {
        let results = [
            Answer::relay(&[
                r#"{"type":"result","status":"failed","error":"the BIR's mailbox did not answer","receipt":null,"class":"bir"}"#,
            ]),
            Answer::relay(&[
                r#"{"type":"result","status":"expired","error":"the ticket expired","receipt":null}"#,
            ]),
            Answer::relay(&[
                r#"{"type":"result","status":"failed","error":"held","receipt":null,"class":"ours","hold":{"code":"bir_offline"}}"#,
            ]),
        ];
        for relay in results {
            let (client, seen, _) = filing(created(), relay, Answer::hang_up());
            let first = client.send(RETURN, b"<xml/>").await;
            assert!(first.is_err(), "{first:?}");
            let second = client.send(RETURN, b"<xml/>").await;
            assert!(
                !matches!(&second, Err(SendError::NotATry { reason, .. }) if reason.contains("still open")),
                "{second:?}"
            );
            assert_eq!(tickets(&seen), 2, "the second send asked for a ticket");
        }
        // A dropped relay whose status reads back failed.
        for status in ["failed", "expired"] {
            let (client, seen, _) = filing(
                created(),
                Answer::relay(&[]),
                Answer::json(200, &json!({"status": status})),
            );
            let _ = client.send(RETURN, b"<xml/>").await;
            let _ = client.send(RETURN, b"<xml/>").await;
            assert_eq!(tickets(&seen), 2, "{status}");
        }
    }

    #[tokio::test]
    async fn already_submitted_is_still_maybe_sent() {
        let (public, jws, sealing) = trust_documents(&["127.0.0.1:9"]);
        let (url, _) = serve(move |request| match request.path.as_str() {
            "/.well-known/buwiz-trust.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: jws.clone(),
            },
            "/.well-known/buwiz-hpke.json" => Answer {
                status: 200,
                headers: Vec::new(),
                body: sealing.clone(),
            },
            _ => Answer::json(
                409,
                &json!({"error": {"code": "already_submitted", "message": "Already submitted."}}),
            ),
        });
        let client = submitter(&url, true, vec![public]);
        let sent = client.send(RETURN, b"<xml/>").await;
        assert!(
            matches!(&sent, Err(SendError::MaybeSent(message)) if message == "Already submitted."),
            "{sent:?}"
        );
    }

    /// A maybe-sent or a success clears the floor; a floor with no ticket
    /// behind it never exists.
    #[test]
    fn a_maybe_sent_clears_the_floor() {
        let client = submitter("http://127.0.0.1:9", false, Vec::new());
        let later = Utc::now() + chrono::Duration::seconds(300);
        let open = || OpenTry {
            until: Some(later),
            closed: false,
        };
        let kept = client.keep_floor("k", Err(SendError::NotSent("x".into())), open());
        assert!(matches!(kept, Err(SendError::NotSent(_))));
        assert_eq!(client.open_floor_for("k", Utc::now()), Some(later));
        let maybe = client.keep_floor("k", Err(SendError::MaybeSent("x".into())), open());
        assert!(matches!(maybe, Err(SendError::MaybeSent(_))));
        assert_eq!(client.open_floor_for("k", Utc::now()), None);
        client
            .keep_floor("k", Err(SendError::NotSent("x".into())), open())
            .ok();
        client
            .keep_floor(
                "k",
                Err(SendError::NotSent("y".into())),
                OpenTry {
                    until: Some(later),
                    closed: true,
                },
            )
            .ok();
        assert_eq!(
            client.open_floor_for("k", Utc::now()),
            None,
            "a closed submission frees the key"
        );
        // Before a ticket there is nothing to hold.
        client
            .keep_floor(
                "k",
                Err(SendError::NotATry {
                    reason: "x".into(),
                    wait_until: None,
                }),
                OpenTry::default(),
            )
            .ok();
        assert_eq!(client.open_floor_for("k", Utc::now()), None);
    }

    #[test]
    fn the_floor_is_the_expiry_plus_30_s_else_330_s_and_never_beyond_15_minutes() {
        let now = Utc::now();
        let at = |seconds: i64| (now + chrono::Duration::seconds(seconds)).timestamp();
        assert_eq!(
            open_floor(&json!({"expires_at": at(300)}), now).timestamp(),
            at(330)
        );
        for odd in [
            json!({}),
            json!({"expires_at": "soon"}),
            json!({"expires_at": null}),
        ] {
            assert_eq!(open_floor(&odd, now), now + chrono::Duration::seconds(330));
        }
        assert_eq!(
            open_floor(&json!({"expires_at": at(86_400)}), now).timestamp(),
            at(900)
        );
    }

    // ------------------------------------------- the host's notes and settle

    /// The SHA-256 of the test return `<xml/>`.
    fn xml_hash() -> String {
        hex(&Sha256::digest(b"<xml/>"))
    }

    /// A loopback "BIR host" the trust list carries, and how many
    /// connections it took.
    fn bir_host() -> (String, std::net::TcpListener) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        (
            format!("127.0.0.1:{}", listener.local_addr().unwrap().port()),
            listener,
        )
    }

    fn connections(listener: &std::net::TcpListener) -> usize {
        std::iter::from_fn(|| listener.accept().ok()).count()
    }

    /// A relay that opens one stream toward `host`, then ends with a receipt.
    fn relay_to(host: &str) -> Answer {
        let (name, port) = host.split_once(':').unwrap();
        let open = format!(r#"{{"type":"open","stream":1,"host":"{name}","port":{port}}}"#);
        let receipt = crate::testkit::receipt_jws(&xml_hash(), "jti-1", "live");
        let result =
            format!(r#"{{"type":"result","status":"sent","error":null,"receipt":"{receipt}"}}"#);
        Answer::relay(&[&open, &result])
    }

    /// The host's notes, in order, and a switch that makes the note with
    /// `opening == fail_at` fail.
    #[allow(clippy::type_complexity)]
    fn notes(
        fail_at: Option<bool>,
    ) -> (
        std::sync::Arc<Mutex<Vec<PendingSend>>>,
        impl Fn(&PendingSend) -> Result<(), String> + Send + Sync,
    ) {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let note = move |pending: &PendingSend| {
            log.lock().unwrap().push(pending.clone());
            if fail_at == Some(pending.opening) {
                Err("the disk is full".to_owned())
            } else {
                Ok(())
            }
        };
        (seen, note)
    }

    #[tokio::test]
    async fn a_send_is_noted_before_the_pipe_opens_and_before_the_first_connection_toward_the_bir()
    {
        let (host, listener) = bir_host();
        let (client, seen, _) = filing_to(&[&host], created(), relay_to(&host), Answer::hang_up());
        let (noted, note) = notes(None);
        let sent = client.send_noting(RETURN, b"<xml/>", &note).await;
        assert!(sent.is_ok(), "{sent:?}");
        let noted = noted.lock().unwrap().clone();
        assert_eq!(noted.len(), 2, "{noted:?}");
        assert_eq!(
            noted[0],
            PendingSend {
                server: client.config().server_url.clone(),
                submission_id: "s1".into(),
                jti: "jti-1".into(),
                xml_sha256: xml_hash(),
                opening: false,
                noted_at: noted[0].noted_at,
            }
        );
        assert_eq!(
            noted[1],
            PendingSend {
                opening: true,
                ..noted[0].clone()
            },
            "the same record, about to open toward the BIR"
        );
        assert_eq!(connections(&listener), 1);
        assert!(
            seen.lock()
                .unwrap()
                .iter()
                .any(|request| request.path == "/relay/s1")
        );
    }

    #[tokio::test]
    async fn a_failed_first_note_stops_the_send_before_the_pipe() {
        let (host, listener) = bir_host();
        let (client, seen, _) = filing_to(&[&host], created(), relay_to(&host), Answer::hang_up());
        let (noted, note) = notes(Some(false));
        let sent = client.send_noting(RETURN, b"<xml/>", &note).await;
        let Err(SendError::NotSent(reason)) = &sent else {
            panic!("{sent:?}")
        };
        assert!(reason.contains("nothing was relayed"), "{reason}");
        assert_eq!(noted.lock().unwrap().len(), 1);
        assert!(
            !seen
                .lock()
                .unwrap()
                .iter()
                .any(|request| request.path == "/relay/s1")
        );
        assert_eq!(connections(&listener), 0);
    }

    /// Link 1 of 3 of "a relay dry_run result never reads as filed" (link 2:
    /// bir-core `background_cron::tests::a_relay_dry_run_result_is_stored_as_a_test_send_on_every_path`;
    /// link 3: bir-app-api `screen_form_module::tests::a_relay_dry_run_result_never_reads_as_filed`).
    /// The shared fixture `relay-result-dry-run.json` is the relay's `result`
    /// for a test TIN: status `dry_run`, and the claims of its dry-run receipt
    /// for `<xml/>` on ticket `jti-1`. The test key signs those claims here
    /// (no signed token is kept in the repository), and over a loopback relay
    /// the real send verifies the receipt and hands it over as a test send:
    /// exactly the fixture's claims, which is what the queue reads.
    #[tokio::test]
    async fn a_relay_dry_run_result_with_its_signed_receipt_is_a_verified_test_send() {
        let fixture = crate::testkit::fixture_json("relay-result-dry-run.json");
        assert_eq!(fixture["type"], "result");
        assert_eq!(fixture["status"], "dry_run");
        let claims = &fixture["receipt_claims"];
        assert_eq!(
            claims["xml_sha256"],
            xml_hash(),
            "the claims are for <xml/>"
        );
        let receipt = crate::testkit::sign_receipt_claims(&[7; 32], claims);
        let mut result = fixture.clone();
        let frame = result.as_object_mut().unwrap();
        frame.remove("receipt_claims");
        frame.insert("receipt".into(), json!(receipt));
        let (host, listener) = bir_host();
        let (name, port) = host.split_once(':').unwrap();
        let open = format!(r#"{{"type":"open","stream":1,"host":"{name}","port":{port}}}"#);
        let (client, _, _) = filing_to(
            &[&host],
            created(),
            Answer::relay(&[&open, &result.to_string()]),
            Answer::hang_up(),
        );
        let (_, note) = notes(None);
        let sent = client
            .send_noting(RETURN, b"<xml/>", &note)
            .await
            .expect("a verified receipt");
        assert_eq!(sent.receipt_jws, receipt);
        assert_eq!(
            sent.receipt,
            serde_json::from_value::<Receipt>(claims.clone()).unwrap(),
            "the fixture's claims, verified"
        );
        assert!(sent.receipt.is_test(), "nothing reached the BIR");
        assert_eq!(connections(&listener), 1);
    }

    #[tokio::test]
    async fn a_failed_opening_note_opens_nothing_toward_the_bir() {
        let (host, listener) = bir_host();
        let (client, _, _) = filing_to(&[&host], created(), relay_to(&host), Answer::hang_up());
        let (noted, note) = notes(Some(true));
        let sent = client.send_noting(RETURN, b"<xml/>", &note).await;
        let Err(SendError::NotSent(reason)) = &sent else {
            panic!("{sent:?}")
        };
        assert!(
            reason.contains("nothing was opened toward the BIR"),
            "{reason}"
        );
        assert_eq!(noted.lock().unwrap().len(), 2);
        assert_eq!(connections(&listener), 0, "no connection toward the BIR");
    }

    /// A pending send of `<xml/>` on ticket `jti-1`, noted `age_minutes` ago.
    fn pending(client: &Submitter, opening: bool) -> PendingSend {
        PendingSend {
            server: client.config().server_url.clone(),
            submission_id: "s1".into(),
            jti: "jti-1".into(),
            xml_sha256: xml_hash(),
            opening,
            noted_at: Utc::now(),
        }
    }

    /// A server whose `GET /api/submissions/s1` answers `read_back`.
    fn settling(read_back: Answer) -> (Submitter, std::sync::Arc<Mutex<Vec<Seen>>>) {
        let (client, seen, _) = filing(created(), Answer::hang_up(), read_back);
        (client, seen)
    }

    fn view(body: Value) -> Answer {
        Answer::json(200, &body)
    }

    /// What `settle` answers, as a word.
    fn settled_word(settled: &Settled) -> &'static str {
        match settled {
            Settled::Sent(_) => "sent",
            Settled::Failed(SendError::NotSent(_)) => "not sent",
            Settled::Failed(SendError::NotATry { .. }) => "not a try",
            Settled::Failed(SendError::Counted { .. }) => "counted",
            Settled::Failed(SendError::MaybeSent(_)) => "maybe sent",
            Settled::NotYet => "not yet",
            Settled::SignedOut => "signed out",
            Settled::Gone => "gone",
        }
    }

    async fn settle_word(read_back: Answer, opening: bool) -> &'static str {
        let (client, _) = settling(read_back);
        settled_word(&client.settle(&pending(&client, opening)).await)
    }

    /// Every status the server ends a submission in, both ways the app may
    /// have left it, through the one decision table; anything not final is
    /// asked again later.
    #[tokio::test]
    async fn settle_decides_every_final_status_and_waits_for_the_rest() {
        let good = crate::testkit::receipt_jws(&xml_hash(), "jti-1", "live");
        let dry = crate::testkit::receipt_jws(&xml_hash(), "jti-1", "dry_run");
        let forged = crate::testkit::receipt_signed_by(&[9; 32], &xml_hash(), "jti-1", "live");
        let other_xml = crate::testkit::receipt_jws(&"0".repeat(64), "jti-1", "live");
        let other_ticket = crate::testkit::receipt_jws(&xml_hash(), "jti-2", "live");
        let rows: Vec<(Value, [&str; 2])> = vec![
            (
                json!({"status": "sent", "receipt_jws": good}),
                ["sent", "sent"],
            ),
            (
                json!({"status": "dry_run", "receipt_jws": dry}),
                ["sent", "sent"],
            ),
            // The status and the signed claims disagree.
            (
                json!({"status": "dry_run", "receipt_jws": good}),
                ["maybe sent", "maybe sent"],
            ),
            (
                json!({"status": "sent", "receipt_jws": dry}),
                ["maybe sent", "maybe sent"],
            ),
            (
                json!({"status": "sent", "receipt_jws": forged}),
                ["maybe sent", "maybe sent"],
            ),
            (
                json!({"status": "sent", "receipt_jws": other_xml}),
                ["maybe sent", "maybe sent"],
            ),
            (
                json!({"status": "sent", "receipt_jws": other_ticket}),
                ["maybe sent", "maybe sent"],
            ),
            (json!({"status": "sent"}), ["maybe sent", "maybe sent"]),
            (
                json!({"status": "failed", "error": "the connection ended before sending"}),
                ["not a try", "counted"],
            ),
            (
                json!({"status": "expired", "error": "the ticket expired"}),
                ["not a try", "maybe sent"],
            ),
            (json!({"status": "unknown"}), ["maybe sent", "maybe sent"]),
            // The server's two states that are not final: stored with the pipe not opened yet, and relay under way.
            (json!({"status": "ready"}), ["not yet", "not yet"]),
            (json!({"status": "sending"}), ["not yet", "not yet"]),
            // A name this client does not know, or none at all, is asked again, never settled.
            (
                json!({"status": "some-later-state"}),
                ["not yet", "not yet"],
            ),
            (json!({"id": "s1"}), ["not yet", "not yet"]),
        ];
        for (body, want) in rows {
            for (opening, want) in [false, true].into_iter().zip(want) {
                assert_eq!(
                    settle_word(view(body.clone()), opening).await,
                    want,
                    "{body}, opening {opening}"
                );
            }
        }
    }

    /// A verified receipt comes back whole, for the queue to keep.
    #[tokio::test]
    async fn a_settled_send_carries_its_verified_receipt() {
        let good = crate::testkit::receipt_jws(&xml_hash(), "jti-1", "live");
        let (client, _) = settling(view(json!({"status": "sent", "receipt_jws": good})));
        let Settled::Sent(sent) = client.settle(&pending(&client, true)).await else {
            panic!("sent")
        };
        assert_eq!(sent.submission_id, "s1");
        assert_eq!(sent.receipt_jws, good);
        assert_eq!(sent.receipt.jti.as_deref(), Some("jti-1"));
        assert_eq!(sent.receipt.mode, "live");
    }

    /// `failed` is safe to send again whatever the app had opened; `expired`
    /// before opening is not even a try.
    #[tokio::test]
    async fn a_failed_or_expired_settle_is_safe_to_send_again() {
        for (status, opening) in [("failed", false), ("failed", true), ("expired", false)] {
            let (client, _) = settling(view(json!({"status": status})));
            let Settled::Failed(error) = client.settle(&pending(&client, opening)).await else {
                panic!("{status} {opening}")
            };
            assert!(
                matches!(error, SendError::NotATry { .. } | SendError::Counted { .. }),
                "{status} {opening}: {error:?}"
            );
        }
        // A refusal code in the server's error is final, as in the send.
        let (client, _) = settling(view(
            json!({"status": "failed", "error": "Sending is paused for this TIN. (tin_paused)"}),
        ));
        let Settled::Failed(error) = client.settle(&pending(&client, true)).await else {
            panic!("failed")
        };
        assert_eq!(error.refusal(), Some(TinRefusal::Paused));
    }

    #[tokio::test]
    async fn settle_waits_when_the_server_cannot_be_asked_and_knows_a_404() {
        let not_found = Answer::json(
            404,
            &json!({"error": {"code": "not_found", "message": "x"}}),
        );
        assert_eq!(settle_word(not_found, true).await, "gone");
        let broken = Answer::json(500, &json!({"error": {"code": "internal", "message": "x"}}));
        assert_eq!(settle_word(broken, true).await, "not yet");
        let invalid = Answer::json(
            401,
            &json!({"error": {"code": "invalid_token", "message": "expired"}}),
        );
        assert_eq!(
            settle_word(invalid, true).await,
            "signed out",
            "a second invalid_token keeps the noted send"
        );
        let held = Answer::json(
            503,
            &json!({"error": {"code": "restarting", "message": "x"}}),
        );
        assert_eq!(settle_word(held, false).await, "not yet");
        assert_eq!(settle_word(Answer::hang_up(), true).await, "not yet");

        // Offline: nothing listens.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let client = submitter(&url, true, Vec::new());
        assert!(matches!(
            client.settle(&pending(&client, true)).await,
            Settled::NotYet
        ));

        // Signed out: no request at all, and never evidence that the send is gone.
        let (url, seen) = serve(|_| Answer::json(200, &json!({"status": "failed"})));
        let client = submitter(&url, false, Vec::new());
        assert!(matches!(
            client.settle(&pending(&client, false)).await,
            Settled::SignedOut
        ));
        assert!(seen.lock().unwrap().is_empty());

        // A record from another server: never asked of this one.
        let (url, seen) = serve(|_| Answer::json(200, &json!({"status": "failed"})));
        let client = submitter(&url, true, Vec::new());
        let elsewhere = PendingSend {
            server: "https://api.buwiz.example".into(),
            ..pending(&client, false)
        };
        assert!(matches!(client.settle(&elsewhere).await, Settled::NotYet));
        assert!(seen.lock().unwrap().is_empty());
    }

    /// Other local read failures — a captive portal answering invalid JSON, or token work that
    /// cannot be joined — say nothing about the account or the submission, so the queue asks again.
    #[tokio::test]
    async fn settle_retries_local_failures_that_are_not_account_evidence() {
        let unreadable = Answer {
            status: 200,
            headers: Vec::new(),
            body: "<html>captive portal</html>".into(),
        };
        assert_eq!(settle_word(unreadable, true).await, "not yet");

        struct JoinFailure;
        impl TokenSource for JoinFailure {
            fn access_token(
                &self,
                _force_refresh: bool,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, ClientError>> + Send + '_>,
            > {
                Box::pin(async { Err(ClientError::Other("join failed".into())) })
            }
            fn login(&self) -> Option<String> {
                Some("user".into())
            }
            fn session_ended(&self) {}
        }
        let client = Submitter::new(
            SubmitConfig::new("http://127.0.0.1:9").unwrap(),
            Box::new(crate::secrets::MemoryStore::default()),
            std::sync::Arc::new(JoinFailure),
        );
        let pending = PendingSend {
            server: client.config().server_url.clone(),
            submission_id: "s1".into(),
            jti: "jti-1".into(),
            xml_sha256: xml_hash(),
            opening: true,
            noted_at: Utc::now(),
        };
        assert!(matches!(client.settle(&pending).await, Settled::NotYet));
    }

    /// F3: a token-store failure is account evidence, never evidence that a noted send is gone.
    #[tokio::test]
    async fn settle_keeps_a_send_when_the_token_store_fails() {
        struct BrokenTokens;
        impl TokenSource for BrokenTokens {
            fn access_token(
                &self,
                _force_refresh: bool,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, ClientError>> + Send + '_>,
            > {
                Box::pin(async { Err(ClientError::Account("the account store failed".into())) })
            }
            fn login(&self) -> Option<String> {
                Some("user".into())
            }
            fn session_ended(&self) {}
        }
        let client = Submitter::new(
            SubmitConfig::new("http://127.0.0.1:9").unwrap(),
            Box::new(crate::secrets::MemoryStore::default()),
            std::sync::Arc::new(BrokenTokens),
        );
        let pending = PendingSend {
            server: client.config().server_url.clone(),
            submission_id: "s1".into(),
            jti: "jti-1".into(),
            xml_sha256: xml_hash(),
            opening: true,
            noted_at: Utc::now(),
        };
        assert!(matches!(client.settle(&pending).await, Settled::SignedOut));
    }

    #[test]
    fn a_pending_send_holds_no_secret_and_reads_back() {
        let record = PendingSend {
            server: "https://api.buwiz.com".into(),
            submission_id: "s1".into(),
            jti: "jti-1".into(),
            xml_sha256: xml_hash(),
            opening: true,
            noted_at: DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&json).unwrap(),
            json!({
                "server": "https://api.buwiz.com", "submission_id": "s1", "jti": "jti-1",
                "xml_sha256": xml_hash(), "opening": true, "noted_at": "2027-01-15T08:00:00Z",
            })
        );
        assert_eq!(serde_json::from_str::<PendingSend>(&json).unwrap(), record);
    }
}
