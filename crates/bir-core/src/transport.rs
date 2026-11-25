//! SFTP transport for submitting encrypted returns to BIR servers.
//!
//! Official Offline eBIRForms 7.9.6.x no longer uses the historical FTP
//! gateway. The current client:
//!
//! 1. Asks `tinDispatcherSFTP.php` for encrypted SFTP connection fields.
//! 2. Decrypts those fields with AES-256-CBC / PBKDF2-HMAC-SHA1.
//! 3. Uploads the already-encrypted IAF file through WinSCP.NET SFTP.
//!
//! This module reproduces that protocol with credentials supplied at runtime.
//! It does not embed live BIR passwords.
//!
//! **No build that ships can file this way.** The dispatcher client is compiled
//! only with the `direct-bir` cargo feature, which the app never enables, and
//! without it an SFTP session opens only to a loopback address (the local mock
//! BIR). Such a build answers [`sending_unavailable`] where it would have gone
//! to the dispatcher: the queue leaves its returns alone and the app refuses to
//! queue new ones.

#[cfg(any(test, feature = "direct-bir"))]
use aes::Aes256;
#[cfg(any(test, feature = "direct-bir"))]
use aes::cipher::block_padding::Pkcs7;
#[cfg(any(test, feature = "direct-bir"))]
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
#[cfg(any(test, feature = "direct-bir"))]
use pbkdf2::pbkdf2_hmac;
#[cfg(any(test, feature = "direct-bir"))]
use rand::RngExt;
#[cfg(feature = "direct-bir")]
use reqwest::Client;
use russh::Disconnect;
use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
#[cfg(any(test, feature = "direct-bir"))]
use serde::Deserialize;
#[cfg(any(test, feature = "direct-bir"))]
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "direct-bir")]
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};
use zeroize::Zeroizing;

#[cfg(any(test, feature = "direct-bir"))]
const DISPATCHER_PRIMARY: &str = "http://birgovph.com/tinDispatcherSFTP.php";
#[cfg(any(test, feature = "direct-bir"))]
const DISPATCHER_BACKUP: &str = "http://ws2.birgovph.com/tinDispatcherSFTP.php";
#[cfg(feature = "direct-bir")]
const DISPATCHER_HTTPS_TIMEOUT: Duration = Duration::from_secs(4);
#[cfg(feature = "direct-bir")]
const DISPATCHER_CLIENT_VERSION: &str = "7.9.6.0";
/// Passphrase used by official `ebfSFTP.exe` to unwrap dispatcher ciphertext.
/// This is a protocol constant present in every 7.9.6 install, not a taxpayer secret.
/// It stays in this file: buwiz-com's `deploy/ec2/bir-env-from-forms.sh` reads it from here.
#[cfg(any(test, feature = "direct-bir"))]
const DISPATCHER_WRAP_PASSPHRASE: &str = "Carlo*TSSD2!018";
#[cfg(any(test, feature = "direct-bir"))]
const PBKDF2_ROUNDS: u32 = 100_000;
#[cfg(any(test, feature = "direct-bir"))]
const SALT_LEN: usize = 32;
#[cfg(any(test, feature = "direct-bir"))]
const IV_LEN: usize = 16;
#[cfg(any(test, feature = "direct-bir"))]
const KEY_LEN: usize = 32;

#[cfg(any(test, feature = "direct-bir"))]
type Aes256CbcEnc = cbc::Encryptor<Aes256>;
#[cfg(any(test, feature = "direct-bir"))]
type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("SFTP error: {0}")]
    Sftp(String),
    #[error("dispatcher error: {0}")]
    Dispatcher(String),
    #[error("credential unwrap failed: {0}")]
    Crypto(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("SFTP configuration error: {0}")]
    Config(String),
    #[error("submission rejected by server")]
    Rejected,
    /// The relay answered 4xx: it refused the file and kept nothing (docs/relay.md). Retrying the same file will not help.
    #[error("the relay refused the submission: {0}")]
    RelayRefused(String),
    /// The relay could not be reached, or answered 503: it did not take the file, so nothing went on. Safe to retry.
    #[error("the relay is unavailable: {0}")]
    RelayUnavailable(String),
    /// The Buwiz relay refused the return itself (a TIN the account may not send for, a file that does not match its
    /// name). Nothing reached the BIR; the same file would be refused again. The text is the server's sentence with
    /// its code in brackets.
    #[error("{0}")]
    Refused(String),
    /// The Buwiz relay tried and the BIR's way in did not take the return. Nothing reached the BIR. Safe to retry.
    #[error("{0}")]
    RelayFailed(String),
    /// The Buwiz relay did not try: Buwiz held the send, the app is signed out, or Buwiz could not be reached.
    /// Nothing reached the BIR, and it does not count as a failed try. Not before `wait_until` when there is one.
    #[error("{reason}")]
    RelayHeld {
        reason: String,
        wait_until: Option<chrono::DateTime<chrono::Utc>>,
    },
    /// The Buwiz relay cannot say whether the BIR has the return. Never sent again automatically.
    #[error("{0}")]
    RelayUnknown(String),
}

/// What a finished send says beyond "it went".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreReport {
    /// The Buwiz relay's signed receipt (a compact JWS), when the send went through it.
    pub relay_receipt: Option<String>,
    /// The relay's receipt says nothing reached the BIR: the server ran in its `dry_run` or `lab` mode.
    pub simulated: bool,
}

impl TransportError {
    /// The relay proved it did not take the file (refused or unavailable), so the claim on the return can be let go.
    /// Every other failure after the upload started may have delivered it and keeps the claim.
    pub fn relay_did_not_accept(&self) -> bool {
        !matches!(
            self.no_receipt(),
            crate::forms::queueable::NoReceipt::Unknown
        )
    }

    /// How this failure of a started send leaves the claimed return. Only what proves that nothing reached the BIR
    /// lets the claim go; everything else is an unknown outcome and keeps it.
    pub(crate) fn no_receipt(&self) -> crate::forms::queueable::NoReceipt {
        use crate::forms::queueable::NoReceipt;
        match self {
            Self::RelayRefused(_) | Self::Refused(_) => NoReceipt::Refused,
            Self::RelayUnavailable(_) => NoReceipt::Failed,
            Self::RelayFailed(_) => NoReceipt::RelayFailed,
            Self::RelayHeld { wait_until, .. } => NoReceipt::Held { until: *wait_until },
            _ => NoReceipt::Unknown,
        }
    }

    /// How a failure to open the session, before any claim, leaves the queued return: a hold is not a try; anything
    /// else is a failed try, as it always was.
    pub(crate) fn before_claim(&self) -> crate::forms::queueable::NoReceipt {
        use crate::forms::queueable::NoReceipt;
        match self {
            Self::RelayHeld { wait_until, .. } => NoReceipt::Held { until: *wait_until },
            _ => NoReceipt::Failed,
        }
    }

    /// A short name for logs.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Sftp(_) => "sftp",
            Self::Ssh(_) => "ssh",
            Self::Dispatcher(_) => "dispatcher",
            Self::Crypto(_) => "crypto",
            Self::Http(_) => "http",
            Self::Io(_) => "io",
            Self::Config(_) => "config",
            Self::Rejected => "rejected",
            Self::RelayRefused(_) => "relay_refused",
            Self::RelayUnavailable(_) => "relay_unavailable",
            Self::Refused(_) => "refused",
            Self::RelayFailed(_) => "relay_failed",
            Self::RelayHeld { .. } => "relay_held",
            Self::RelayUnknown(_) => "relay_unknown",
        }
    }
}

impl From<reqwest::Error> for TransportError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.to_string())
    }
}

impl From<russh_sftp::client::error::Error> for TransportError {
    fn from(error: russh_sftp::client::error::Error) -> Self {
        Self::Sftp(error.to_string())
    }
}

/// Runtime SFTP endpoint. Live BIR values come from the dispatcher; tests use placeholders.
#[derive(Debug, Clone)]
pub struct SftpEndpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Zeroizing<String>,
    pub remote_folder: String,
    pub host_key_policy: HostKeyPolicy,
}

impl SftpEndpoint {
    pub fn with_folder(mut self, folder: impl Into<String>) -> Self {
        self.remote_folder = folder.into();
        self
    }
}

/// SSH host-key policy. Official dispatcher matches ebfSFTP's accept-any.
/// Lab `BIR_SFTP_*` must pin a SHA-256 fingerprint or set
/// `BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyPolicy {
    AcceptAnyOfficial,
    AcceptAnyLab,
    PinnedSha256(String),
}

/// How an SFTP session was obtained. Logged without secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpEndpointSource {
    DryRun,
    Env,
    Dispatcher,
    /// The local mock relay ([`RELAY_URL_ENV`], loopback only; docs/relay.md).
    Relay,
    /// The Buwiz relay ([`crate::relay`]): the return goes to the BIR through Buwiz's server.
    Buwiz,
}

impl SftpEndpointSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Env => "env",
            Self::Dispatcher => "dispatcher",
            Self::Relay => "relay",
            Self::Buwiz => "buwiz",
        }
    }

    pub fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
    }

    /// The send goes to the local mock relay, not to BIR.
    pub fn is_relay(self) -> bool {
        matches!(self, Self::Relay)
    }
}

/// Result of resolving where to send an IAF file.
#[derive(Debug, Clone)]
pub enum ResolvedSftpTarget {
    DryRun {
        folder: String,
    },
    Live {
        endpoint: SftpEndpoint,
        source: SftpEndpointSource,
    },
    /// The mock relay at `url` (a loopback `http://` origin) takes the file for `folder` (docs/relay.md).
    Relay {
        url: String,
        folder: String,
    },
}

impl ResolvedSftpTarget {
    pub fn source(&self) -> SftpEndpointSource {
        match self {
            Self::DryRun { .. } => SftpEndpointSource::DryRun,
            Self::Live { source, .. } => *source,
            Self::Relay { .. } => SftpEndpointSource::Relay,
        }
    }
}

/// The relay the phone app sends through (docs/relay.md). Only the local mock relay exists: the URL must be a
/// plain `http://` origin on `127.0.0.1`, `::1` or `localhost`; anything else is refused, never sent to.
pub const RELAY_URL_ENV: &str = "BIR_RELAY_URL";

/// [`RELAY_URL_ENV`] without a trailing slash when it is set, checked by [`loopback_relay_url`].
pub fn relay_url() -> Option<Result<String, TransportError>> {
    env_nonempty(RELAY_URL_ENV).map(|raw| {
        require_mock_relay(cfg!(any(debug_assertions, feature = "mock-relay")))?;
        loopback_relay_url(&raw)
    })
}

fn require_mock_relay(enabled: bool) -> Result<(), TransportError> {
    if enabled {
        Ok(())
    } else {
        Err(TransportError::Config(
            "The mock relay is disabled in release builds.".into(),
        ))
    }
}

/// `raw` as a relay origin, or why it is refused: only `http://` on a loopback host, no user, path, query or
/// fragment. Pure, so every branch is tested.
pub fn loopback_relay_url(raw: &str) -> Result<String, TransportError> {
    let refuse = |why: &str| {
        Err(TransportError::Config(format!(
            "{RELAY_URL_ENV} must be the local mock relay (http://127.0.0.1:PORT): {why}"
        )))
    };
    let Ok(url) = reqwest::Url::parse(raw.trim()) else {
        return refuse("not a URL");
    };
    if url.scheme() != "http" {
        return refuse("only http:// is accepted");
    }
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if !loopback {
        return refuse("the host is not loopback");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return refuse("no credentials, query or fragment");
    }
    if !matches!(url.path(), "" | "/") {
        return refuse("no path");
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

/// `POST {url}/v1/submissions` with the encrypted IAF file as the body (docs/relay.md). `Ok` only on 202 with
/// `{"status":"accepted"}`. A 4xx answer except 408/429 is [`TransportError::RelayRefused`]; no connection or 408/429/503 is
/// [`TransportError::RelayUnavailable`] (the relay took nothing). Anything else (a timeout, a 502 from a failed hand-off,
/// a 202 that does not say accepted) leaves the outcome unknown: [`TransportError::Http`].
async fn relay_store(
    url: &str,
    form_type: &str,
    folder: &str,
    filename: &str,
    payload: &[u8],
) -> Result<(), TransportError> {
    require_mock_relay(cfg!(any(debug_assertions, feature = "mock-relay")))?;
    let name = iaf_basename(filename);
    let digest = hex::encode(Sha256::digest(payload));
    info!(
        source = "relay",
        form = form_type,
        payload_len = payload.len(),
        "Relay PUT"
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let response = client
        .post(format!("{url}/v1/submissions"))
        .header("content-type", "application/octet-stream")
        .header("x-relay-form", folder)
        .header("x-relay-filename", name)
        .header("x-relay-sha256", &digest)
        .body(payload.to_vec())
        .send()
        .await
        .map_err(|error| {
            if error.is_connect() {
                TransportError::RelayUnavailable(error.to_string())
            } else {
                TransportError::from(error)
            }
        })?;
    let status = response.status();
    let body: serde_json::Value =
        serde_json::from_slice(&response.bytes().await?).unwrap_or_default();
    if matches!(status.as_u16(), 408 | 429 | 503) {
        warn!(%status, "relay is unavailable");
        return Err(TransportError::RelayUnavailable(status.to_string()));
    }
    if status.is_client_error() {
        warn!(%status, "relay refused the submission");
        return Err(TransportError::RelayRefused(status.to_string()));
    }
    if status != reqwest::StatusCode::ACCEPTED || body["status"] != "accepted" {
        return Err(TransportError::Http(format!(
            "relay answered {status} without accepting the submission"
        )));
    }
    info!(
        form = form_type,
        payload_len = payload.len(),
        "Relay accepted the submission"
    );
    Ok(())
}

fn env_nonempty(name: &str) -> Option<String> {
    crate::config::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_flag_enabled(name: &str) -> bool {
    env_nonempty(name).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_flag_disabled(name: &str) -> bool {
    env_nonempty(name).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

/// The signed-in Buwiz account is the app-store reviewers' practice login.
/// The account layer keeps it in step with sign-in, launch and sign-out.
static PRACTICE_ACCOUNT: AtomicBool = AtomicBool::new(false);

/// Mark the signed-in account as a practice account (`false` when signed out).
pub fn set_practice_account(on: bool) {
    PRACTICE_ACCOUNT.store(on, Ordering::SeqCst);
}

/// A practice account never sends to BIR: every send is a dry run.
pub fn practice_account() -> bool {
    PRACTICE_ACCOUNT.load(Ordering::SeqCst)
}

/// Dry run from the environment, or from the Dev Mode panel (debug builds
/// only, and only while no `BIR_SFTP_*` mode variable is set). A practice
/// account is always a dry run, whatever Dev Mode says.
pub fn is_sftp_dry_run() -> bool {
    if practice_account() {
        return true;
    }
    match crate::dev_mode::panel_submission_mode() {
        Some(mode) => mode == crate::dev_mode::SubmissionMode::DryRun,
        None => sftp_dry_run_requested(),
    }
}

/// Inputs for [`dry_run_from_inputs`]. Kept separate so a release build's
/// default can be tested from a debug test binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SftpDryRunInputs {
    pub test_env: bool,
    pub dry_run: bool,
    pub live_disabled: bool,
    pub live_enabled: bool,
    pub lab_host: bool,
    pub debug: bool,
    pub practice: bool,
}

/// Dry run when a test/dry flag forces it, including over a lab host.
/// An explicit live ack or a lab host stays live. With no flags, a debug
/// build is dry and a release build proceeds to the dispatcher. A practice
/// account is dry over every live flag.
pub(crate) fn dry_run_from_inputs(inputs: SftpDryRunInputs) -> bool {
    if inputs.test_env || inputs.dry_run || inputs.live_disabled || inputs.practice {
        return true;
    }
    if inputs.live_enabled || inputs.lab_host {
        return false;
    }
    inputs.debug
}

fn sftp_dry_run_requested() -> bool {
    dry_run_from_inputs(SftpDryRunInputs {
        test_env: crate::db::test_environment(),
        dry_run: env_flag_enabled("BIR_SFTP_DRY_RUN"),
        live_disabled: env_flag_disabled("BIR_SFTP_LIVE"),
        live_enabled: env_flag_enabled("BIR_SFTP_LIVE"),
        lab_host: env_nonempty("BIR_SFTP_HOST").is_some(),
        debug: cfg!(debug_assertions),
        practice: practice_account(),
    })
}

/// `BIR_SFTP_LIVE=1` was set. Release builds no longer require this ack.
#[cfg_attr(not(test), allow(dead_code))]
fn dispatcher_live_ack_requested() -> bool {
    env_flag_enabled("BIR_SFTP_LIVE")
}

#[cfg(any(test, feature = "direct-bir"))]
fn https_dispatcher_url(http_base: &str) -> String {
    http_base.replacen("http://", "https://", 1)
}

fn parse_port(raw: &str, name: &str) -> Result<u16, TransportError> {
    raw.parse::<u16>()
        .map_err(|error| TransportError::Config(format!("invalid {name}: {error}")))
}

fn from_bir_sftp_env(form_type: &str) -> Option<Result<SftpEndpoint, TransportError>> {
    let host = env_nonempty("BIR_SFTP_HOST")?;
    let username = match env_nonempty("BIR_SFTP_USERNAME").or_else(|| env_nonempty("BIR_SFTP_USER"))
    {
        Some(username) => username,
        None => {
            return Some(Err(TransportError::Config(
                "BIR_SFTP_HOST is set but BIR_SFTP_USERNAME is missing".into(),
            )));
        }
    };
    let password = match env_nonempty("BIR_SFTP_PASSWORD") {
        Some(password) => password,
        None => {
            return Some(Err(TransportError::Config(
                "BIR_SFTP_HOST is set but BIR_SFTP_PASSWORD is missing".into(),
            )));
        }
    };
    let port = match env_nonempty("BIR_SFTP_PORT") {
        Some(raw) => match parse_port(&raw, "BIR_SFTP_PORT") {
            Ok(port) => port,
            Err(error) => return Some(Err(error)),
        },
        None => 22,
    };
    let remote_folder = env_nonempty("BIR_SFTP_FOLDER").unwrap_or_else(|| form_type.to_string());
    let host_key_policy = match lab_host_key_policy_from_env() {
        Ok(policy) => policy,
        Err(error) => return Some(Err(error)),
    };
    Some(Ok(SftpEndpoint {
        host,
        port,
        username,
        password: Zeroizing::new(password),
        remote_folder,
        host_key_policy,
    }))
}

/// Lab hosts never get accept-any silently. A `BIR_SFTP_*` override must pin
/// a SHA-256 fingerprint or set `BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
fn lab_host_key_policy_from_env() -> Result<HostKeyPolicy, TransportError> {
    match env_nonempty("BIR_SFTP_HOST_KEY_SHA256") {
        Some(pin) => Ok(HostKeyPolicy::PinnedSha256(pin.to_ascii_lowercase())),
        None if env_flag_enabled("BIR_SFTP_ACCEPT_ANY_HOST_KEY") => Ok(HostKeyPolicy::AcceptAnyLab),
        None => Err(TransportError::Config(
            "BIR_SFTP_HOST is set but lab host-key policy is missing (set BIR_SFTP_HOST_KEY_SHA256 or BIR_SFTP_ACCEPT_ANY_HOST_KEY=1)"
                .into(),
        )),
    }
}

/// Whether this build can file with the BIR itself: the `direct-bir` cargo
/// feature. No build that ships has it.
pub const fn direct_filing_built_in() -> bool {
    cfg!(feature = "direct-bir")
}

/// What a build without `direct-bir` says where it would have filed with the BIR.
pub const SENDING_UNAVAILABLE: &str =
    "This version of eBIRForms cannot send returns to the BIR yet.";

/// What a refused Queue says while [`sending_unavailable`] holds.
pub const QUEUE_UNAVAILABLE: &str = "This version of eBIRForms cannot send returns to the BIR yet. The return was not queued and stays a draft.";

/// Where a send goes, in the order [`resolve_sftp_endpoint`] decides it. Reads
/// settings only: no network, no keychain.
enum Route {
    /// The Dev Mode panel's dry run.
    PanelDryRun,
    /// The Dev Mode panel's test server (debug builds).
    PanelTestServer,
    /// [`RELAY_URL_ENV`] is set: the local mock relay, or why it is refused.
    MockRelay(Result<String, TransportError>),
    /// A test, dry-run or practice-account flag.
    DryRun,
    /// `BIR_SFTP_HOST` is set.
    LabHost,
    /// The Buwiz relay ([`buwiz_relay_requested`]).
    Buwiz,
    /// None of the above: the BIR's own dispatcher.
    Bir,
}

fn route() -> Route {
    // Dev Mode panel (debug builds, no env override). Its only modes are dry
    // run and a test server; it can never reach the production dispatcher.
    match crate::dev_mode::panel_submission_mode() {
        Some(crate::dev_mode::SubmissionMode::DryRun) => return Route::PanelDryRun,
        // A practice account falls through to the dry run below instead.
        Some(crate::dev_mode::SubmissionMode::TestServer) if !practice_account() => {
            return Route::PanelTestServer;
        }
        _ => {}
    }
    // The mock relay (a loopback URL, or a refusal) comes before the dry run: setting it is the ask to send there.
    if let Some(url) = relay_url() {
        return Route::MockRelay(url);
    }
    // The local Buwiz relay comes before the dry run for the same reason, but never before a practice account:
    // that one only simulates, and no request for it reaches a relay.
    if buwiz_relay_requested() && !practice_account() {
        return Route::Buwiz;
    }
    if sftp_dry_run_requested() {
        return Route::DryRun;
    }
    if env_nonempty("BIR_SFTP_HOST").is_some() {
        return Route::LabHost;
    }
    Route::Bir
}

/// Ask a development build to send through the Buwiz relay: `BUWIZ_RELAY_SEND=1`.
pub const RELAY_SEND_ENV: &str = "BUWIZ_RELAY_SEND";

/// Sending through the Buwiz relay was asked for. Only a development build honours [`RELAY_SEND_ENV`], and
/// [`crate::relay`] then sends only through a Buwiz server on this computer whose trust list a test key signed: no
/// build that ships sends through the relay yet, and a development build can never file through production. Like
/// the mock relay's URL, it is the ask to send there, so it wins over the dry-run flags and a test environment.
pub fn buwiz_relay_requested() -> bool {
    cfg!(debug_assertions) && env_flag_enabled(RELAY_SEND_ENV)
}

/// The send goes through the Buwiz relay ([`crate::relay`]), not over SFTP. A practice account never does.
pub(crate) fn sends_through_buwiz() -> bool {
    matches!(route(), Route::Buwiz)
}

/// This build has nowhere to send a return: nothing selects a dry run, the
/// mock relay or a lab host, and filing with the BIR itself is not built in
/// ([`direct_filing_built_in`]). The queue leaves its returns alone while this
/// holds, and the app refuses to queue new ones, with [`SENDING_UNAVAILABLE`].
pub fn sending_unavailable() -> bool {
    unavailable_for(&route(), direct_filing_built_in())
}

fn unavailable_for(route: &Route, direct: bool) -> bool {
    matches!(route, Route::Bir) && !direct
}

/// A release build stores no return as Queued while [`sending_unavailable`]
/// holds: every route that queues one (the form page, the agent) ends in a
/// database write that asks this first. A return queued now would be sent,
/// unasked, by the first later version that can send.
pub(crate) fn refuse_queue_when_unavailable() -> Result<(), crate::db::DbError> {
    if queue_refused(cfg!(debug_assertions), sending_unavailable) {
        return Err(crate::db::DbError::Other(QUEUE_UNAVAILABLE.into()));
    }
    Ok(())
}

/// Whether Queue is refused. Only a release build refuses: its environment is
/// read at launch and does not change, so the answer is one answer.
///
/// A development build never refuses here (with nowhere to send, its worker
/// leaves the queued return alone). It must not: the answer depends on the
/// process environment, tests change that environment for a moment
/// (`temp_env`) while other tests on other threads queue returns, and a
/// refusal read through another test's environment failed those at random.
/// `unavailable` is not even asked in a development build.
fn queue_refused(development: bool, unavailable: impl FnOnce() -> bool) -> bool {
    !development && unavailable()
}

/// A build without `direct-bir` opens SFTP only to a loopback address (the
/// local mock BIR): whatever `BIR_SFTP_HOST` or the Dev Mode panel names, it
/// cannot reach a BIR server. `host` must be an IP literal; a name is refused
/// rather than resolved.
fn require_loopback_unless_direct(host: &str, direct: bool) -> Result<(), TransportError> {
    let loopback = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback());
    if direct || loopback {
        Ok(())
    } else {
        Err(TransportError::Config(
            "This build sends over SFTP only to a local test server on 127.0.0.1 or ::1.".into(),
        ))
    }
}

/// Pick dry-run, a complete `BIR_SFTP_*` lab override, or the official dispatcher
/// (`direct-bir` builds only; any other build answers [`SENDING_UNAVAILABLE`]).
pub async fn resolve_sftp_endpoint(
    form_type: &str,
    #[cfg_attr(not(feature = "direct-bir"), allow(unused_variables))] tin: &str,
) -> Result<ResolvedSftpTarget, TransportError> {
    match route() {
        Route::PanelDryRun => {
            let folder = env_nonempty("BIR_SFTP_FOLDER").unwrap_or_else(|| form_type.to_string());
            info!(
                source = "dry-run",
                folder = folder.as_str(),
                "SFTP dry-run (Dev Mode panel): no network"
            );
            Ok(ResolvedSftpTarget::DryRun { folder })
        }
        Route::PanelTestServer => {
            let endpoint = crate::dev_mode::test_server_endpoint(form_type)?;
            info!(
                source = "env",
                host = endpoint.host.as_str(),
                port = endpoint.port,
                folder = endpoint.remote_folder.as_str(),
                username_len = endpoint.username.len(),
                password_len = endpoint.password.len(),
                "Using Dev Mode test SFTP server"
            );
            Ok(ResolvedSftpTarget::Live {
                endpoint,
                source: SftpEndpointSource::Env,
            })
        }
        Route::MockRelay(url) => {
            let url = url?;
            let folder = env_nonempty("BIR_SFTP_FOLDER").unwrap_or_else(|| form_type.to_string());
            info!(
                source = "relay",
                form = form_type,
                "Using the local mock relay"
            );
            Ok(ResolvedSftpTarget::Relay { url, folder })
        }
        Route::DryRun => {
            let folder = env_nonempty("BIR_SFTP_FOLDER").unwrap_or_else(|| form_type.to_string());
            info!(
                source = "dry-run",
                folder = folder.as_str(),
                "SFTP dry-run: no network"
            );
            Ok(ResolvedSftpTarget::DryRun { folder })
        }
        Route::LabHost => {
            let endpoint = from_bir_sftp_env(form_type).ok_or_else(|| {
                TransportError::Config("BIR_SFTP_HOST was unset while the send was resolved".into())
            })??;
            info!(
                source = "env",
                host = endpoint.host.as_str(),
                port = endpoint.port,
                folder = endpoint.remote_folder.as_str(),
                username_len = endpoint.username.len(),
                password_len = endpoint.password.len(),
                "Using BIR_SFTP_* override"
            );
            Ok(ResolvedSftpTarget::Live {
                endpoint,
                source: SftpEndpointSource::Env,
            })
        }
        // Debug with no flags already resolved to a dry run. A release build with
        // no flags, and any build with `BIR_SFTP_LIVE=1`, reaches this arm.
        #[cfg(feature = "direct-bir")]
        Route::Bir => {
            let endpoint = fetch_sftp_endpoint(tin, form_type).await?;
            info!(
                source = "dispatcher",
                host = endpoint.host.as_str(),
                port = endpoint.port,
                folder = endpoint.remote_folder.as_str(),
                username_len = endpoint.username.len(),
                password_len = endpoint.password.len(),
                "Using tinDispatcherSFTP.php"
            );
            Ok(ResolvedSftpTarget::Live {
                endpoint,
                source: SftpEndpointSource::Dispatcher,
            })
        }
        #[cfg(not(feature = "direct-bir"))]
        Route::Bir => Err(TransportError::Config(SENDING_UNAVAILABLE.into())),
        // The queue asks `sends_through_buwiz` first and never resolves an SFTP target for this route.
        Route::Buwiz => Err(TransportError::Config(
            "This send goes through the Buwiz relay, not over SFTP.".into(),
        )),
    }
}

#[cfg(any(test, feature = "direct-bir"))]
#[cfg_attr(not(feature = "direct-bir"), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct DispatcherResponse {
    mode: String,
    server: String,
    /// Present in the official body; the SFTP path never uses it.
    #[serde(rename = "SSLPort")]
    #[allow(dead_code)]
    ssl_port: Option<String>,
    port: String,
    username: String,
    password: String,
}

/// Decrypt dispatcher ciphertext using the official ebfSFTP algorithm.
///
/// Layout: `Base64(salt[32] || iv[16] || AES-256-CBC(PKCS7(UTF-8 plaintext)))`
/// Key: PBKDF2-HMAC-SHA1(password=`Carlo*TSSD2!018`, salt, rounds=100000, dkLen=32).
/// StreamWriter in the official helper prepends a UTF-8 BOM.
#[cfg(any(test, feature = "direct-bir"))]
pub fn unwrap_dispatcher_field(cipher_text: &str) -> Result<String, TransportError> {
    let raw = data_encoding::BASE64
        .decode(cipher_text.trim().as_bytes())
        .map_err(|error| {
            TransportError::Crypto(format!("invalid dispatcher ciphertext: {error}"))
        })?;
    if raw.len() < SALT_LEN + IV_LEN + 16 {
        return Err(TransportError::Crypto(
            "dispatcher ciphertext is shorter than salt+iv+one block".into(),
        ));
    }
    let (salt, rest) = raw.split_at(SALT_LEN);
    let (iv, cipher) = rest.split_at(IV_LEN);
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha1>(
        DISPATCHER_WRAP_PASSPHRASE.as_bytes(),
        salt,
        PBKDF2_ROUNDS,
        &mut key,
    );
    let mut buf = cipher.to_vec();
    let plain = Aes256CbcDec::new((&key).into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|error| TransportError::Crypto(format!("dispatcher decrypt failed: {error}")))?;
    let text = std::str::from_utf8(plain).map_err(|error| {
        TransportError::Crypto(format!("dispatcher plaintext was not UTF-8: {error}"))
    })?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// Encrypt a dispatcher field for local harness round-trips. Not used against BIR.
#[cfg(any(test, feature = "direct-bir"))]
pub fn wrap_dispatcher_field(plain_text: &str) -> Result<String, TransportError> {
    let mut salt = [0u8; SALT_LEN];
    let mut iv = [0u8; IV_LEN];
    let mut rng = rand::rng();
    rng.fill(&mut salt);
    rng.fill(&mut iv);
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha1>(
        DISPATCHER_WRAP_PASSPHRASE.as_bytes(),
        &salt,
        PBKDF2_ROUNDS,
        &mut key,
    );
    let mut payload = Vec::from("\u{feff}");
    payload.extend_from_slice(plain_text.as_bytes());
    let mut buf = vec![0u8; payload.len() + 16];
    buf[..payload.len()].copy_from_slice(&payload);
    let cipher_len = Aes256CbcEnc::new((&key).into(), (&iv).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, payload.len())
        .map_err(|error| TransportError::Crypto(format!("dispatcher encrypt failed: {error}")))?
        .len();
    let mut out = Vec::with_capacity(SALT_LEN + IV_LEN + cipher_len);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&buf[..cipher_len]);
    Ok(data_encoding::BASE64.encode(&out))
}

#[cfg(any(test, feature = "direct-bir"))]
fn parse_dispatcher_json(body: &str) -> Result<DispatcherResponse, TransportError> {
    let normalized = body.trim().replace('\'', "\"");
    serde_json::from_str(&normalized).map_err(|error| {
        TransportError::Dispatcher(format!("could not parse dispatcher JSON: {error}"))
    })
}

#[cfg(feature = "direct-bir")]
async fn fetch_dispatcher(
    client: &Client,
    base: &str,
    tin: &str,
    form_type: &str,
) -> Result<DispatcherResponse, TransportError> {
    fetch_dispatcher_timed(client, base, tin, form_type, Duration::from_secs(8)).await
}

#[cfg(feature = "direct-bir")]
async fn fetch_dispatcher_timed(
    client: &Client,
    base: &str,
    tin: &str,
    form_type: &str,
    timeout: Duration,
) -> Result<DispatcherResponse, TransportError> {
    let url = format!("{base}?t={tin}&f={form_type}&v={DISPATCHER_CLIENT_VERSION}");
    let response = client.get(&url).timeout(timeout).send().await?;
    if !response.status().is_success() {
        return Err(TransportError::Dispatcher(format!(
            "{base} returned HTTP {}",
            response.status()
        )));
    }
    let body = response.text().await?;
    if !body.contains("mode") {
        return Err(TransportError::Dispatcher(format!(
            "{base} returned no mode field"
        )));
    }
    parse_dispatcher_json(&body)
}

/// Prefer HTTPS; official ebfSFTP still uses HTTP, so fall back.
#[cfg(feature = "direct-bir")]
async fn fetch_dispatcher_prefer_https(
    client: &Client,
    http_base: &str,
    tin: &str,
    form_type: &str,
) -> Result<DispatcherResponse, TransportError> {
    let https_base = https_dispatcher_url(http_base);
    match fetch_dispatcher_timed(
        client,
        &https_base,
        tin,
        form_type,
        DISPATCHER_HTTPS_TIMEOUT,
    )
    .await
    {
        Ok(parsed) => {
            info!(
                scheme = "https",
                base = https_base.as_str(),
                "dispatcher HTTPS ok"
            );
            Ok(parsed)
        }
        Err(error) => {
            tracing::warn!(
                scheme = "https",
                base = https_base.as_str(),
                error = error.to_string(),
                "HTTPS dispatcher failed; falling back to official plaintext HTTP (dispatcher fields are MITM-recoverable on this path)"
            );
            fetch_dispatcher(client, http_base, tin, form_type).await
        }
    }
}

/// Ask the official dispatcher for SFTP connection fields and unwrap them.
#[cfg(feature = "direct-bir")]
pub async fn fetch_sftp_endpoint(
    tin: &str,
    form_type: &str,
) -> Result<SftpEndpoint, TransportError> {
    let digits: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 9 {
        return Err(TransportError::Dispatcher(
            "dispatcher TIN must include at least the 9-digit prefix".into(),
        ));
    }
    let dispatcher_tin = &digits[..9];
    let client = Client::builder()
        .danger_accept_invalid_certs(false)
        .build()?;
    let parsed =
        match fetch_dispatcher_prefer_https(&client, DISPATCHER_PRIMARY, dispatcher_tin, form_type)
            .await
        {
            Ok(parsed) if parsed.mode != "0" => parsed,
            Ok(_) | Err(_) => {
                fetch_dispatcher_prefer_https(&client, DISPATCHER_BACKUP, dispatcher_tin, form_type)
                    .await?
            }
        };
    if parsed.mode == "0" {
        return Err(TransportError::Dispatcher(
            "dispatcher returned mode 0 (no web service)".into(),
        ));
    }
    let host = unwrap_dispatcher_field(&parsed.server)?;
    let username = unwrap_dispatcher_field(&parsed.username)?;
    let password = unwrap_dispatcher_field(&parsed.password)?;
    let port = parsed
        .port
        .parse::<u16>()
        .map_err(|error| TransportError::Dispatcher(format!("invalid SFTP port: {error}")))?;
    Ok(SftpEndpoint {
        host,
        port,
        username,
        password: Zeroizing::new(password),
        remote_folder: form_type.to_string(),
        host_key_policy: HostKeyPolicy::AcceptAnyOfficial,
    })
}

/// Basename for an IAF remote path. Splits on `/` and `\\` so a Windows
/// absolute dummy path still yields the file name on Unix CI.
pub fn iaf_basename(filename: &str) -> &str {
    filename
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(filename)
}

fn remote_path(folder: &str, filename: &str) -> String {
    let file = iaf_basename(filename);
    let folder = folder.trim_matches('/');
    if folder.is_empty() {
        format!("/{file}")
    } else {
        format!("/{folder}/{file}")
    }
}

fn host_key_fingerprint_sha256(key: &PublicKeyOrCertificate) -> Option<String> {
    let encoded = match key {
        PublicKeyOrCertificate::PublicKey { key, .. } => key.to_bytes().ok()?,
        PublicKeyOrCertificate::Certificate(cert) => cert.to_bytes().ok()?,
    };
    Some(hex::encode(Sha256::digest(encoded)))
}

struct ConfiguredHostKey {
    policy: HostKeyPolicy,
}

impl client::Handler for ConfiguredHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        match &self.policy {
            HostKeyPolicy::AcceptAnyOfficial | HostKeyPolicy::AcceptAnyLab => Ok(true),
            HostKeyPolicy::PinnedSha256(expected) => {
                Ok(host_key_fingerprint_sha256(server_public_key)
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(expected.trim())))
            }
        }
    }
}

/// In-process fake PUT. Not a BIR filing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunPut {
    pub form_type: String,
    pub filename: String,
    pub remote_path: String,
    pub payload_len: usize,
    pub payload_sha256: String,
}

/// Prepared SFTP session that has not yet uploaded the IAF file.
pub(crate) struct IafSftpSession {
    handle: Option<Handle<ConfiguredHostKey>>,
    remote_folder: String,
    source: SftpEndpointSource,
    dry_run_puts: Option<std::sync::Arc<std::sync::Mutex<Vec<DryRunPut>>>>,
    /// The relay origin and form code when `source` is [`SftpEndpointSource::Relay`].
    relay: Option<(String, String)>,
}

impl IafSftpSession {
    pub fn source(&self) -> SftpEndpointSource {
        self.source
    }
    pub async fn store(mut self, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        if let Some((url, form_type)) = &self.relay {
            return relay_store(url, form_type, &self.remote_folder, filename, payload).await;
        }
        let path = remote_path(&self.remote_folder, filename);
        if self.source.is_dry_run() {
            let put = DryRunPut {
                form_type: self.remote_folder.clone(),
                filename: iaf_basename(filename).to_string(),
                remote_path: path.clone(),
                payload_len: payload.len(),
                payload_sha256: hex::encode(Sha256::digest(payload)),
            };
            info!(
                source = "dry-run",
                remote_path = put.remote_path.as_str(),
                payload_len = put.payload_len,
                payload_sha256 = put.payload_sha256.as_str(),
                "SFTP dry-run PUT (no network)"
            );
            if let Some(log) = &self.dry_run_puts {
                log.lock()
                    .expect("dry-run PUT log should not be poisoned")
                    .push(put);
            }
            return Ok(());
        }
        let handle = self
            .handle
            .take()
            .ok_or_else(|| TransportError::Sftp("SFTP session was already consumed".into()))?;
        info!(
            source = self.source.as_str(),
            "Transmitting payload: {}", path
        );
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|error| TransportError::Ssh(error.to_string()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|error| TransportError::Ssh(error.to_string()))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|error| TransportError::Sftp(error.to_string()))?;
        let mut file = sftp
            .open_with_flags(
                path.clone(),
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await?;
        file.write_all(payload).await?;
        file.flush().await?;
        file.shutdown().await?;
        let _ = sftp.close().await;
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
        info!("Transmission complete: {}", path);
        Ok(())
    }

    /// Authenticate and open SFTP without uploading. Used by the live-connect probe.
    pub async fn probe(mut self) -> Result<String, TransportError> {
        if self.source.is_dry_run() || self.relay.is_some() {
            return Ok("/".to_string());
        }
        let handle = self
            .handle
            .take()
            .ok_or_else(|| TransportError::Sftp("SFTP session was already consumed".into()))?;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|error| TransportError::Ssh(error.to_string()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|error| TransportError::Ssh(error.to_string()))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|error| TransportError::Sftp(error.to_string()))?;
        let cwd = sftp
            .canonicalize(".")
            .await
            .map_err(|error| TransportError::Sftp(error.to_string()))?;
        let _ = sftp.close().await;
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
        Ok(cwd)
    }
}

impl Drop for IafSftpSession {
    fn drop(&mut self) {
        let _ = self.handle.take();
    }
}

/// Open SFTP through authentication and folder resolution. Connect / login does not upload.
pub(crate) async fn open_iaf_session(
    form_type: &str,
    tin: &str,
) -> Result<IafSftpSession, TransportError> {
    match resolve_sftp_endpoint(form_type, tin).await? {
        ResolvedSftpTarget::DryRun { folder } => Ok(IafSftpSession {
            handle: None,
            remote_folder: folder,
            source: SftpEndpointSource::DryRun,
            dry_run_puts: None,
            relay: None,
        }),
        ResolvedSftpTarget::Live { endpoint, source } => {
            open_iaf_session_with_source(form_type, endpoint, source).await
        }
        ResolvedSftpTarget::Relay { url, folder } => {
            relay_health(&url).await?;
            Ok(relay_session(url, folder, form_type))
        }
    }
}

/// Service identity for the local development relay.
pub const MOCK_RELAY_SERVICE: &str = "buwiz-mock-relay";

/// Checks the versioned mock relay identity, with upgrade guidance for legacy relays.
pub fn check_mock_relay_health(
    body: &serde_json::Value,
    address: SocketAddr,
) -> Result<(), String> {
    if body["service"] == MOCK_RELAY_SERVICE
        && body["version"] == 1
        && body["ok"] == true
        && body["mock"] == true
    {
        Ok(())
    } else if body.as_object().is_some_and(|fields| fields.len() == 2)
        && body["ok"] == true
        && body["mock"] == true
    {
        Err(format!(
            "The mock relay is out of date. Restart it with `just mock-relay --bind {} --port {}`, then restart the app.",
            address.ip(),
            address.port()
        ))
    } else {
        Err("Health response did not identify the mock relay".into())
    }
}

/// `GET {url}/v1/health` before a return is claimed: a relay that is down fails here, where the worker can retry
/// later, instead of after the claim.
async fn relay_health(url: &str) -> Result<(), TransportError> {
    require_mock_relay(cfg!(any(debug_assertions, feature = "mock-relay")))?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let response = client
        .get(format!("{url}/v1/health"))
        .send()
        .await
        .map_err(|error| TransportError::RelayUnavailable(error.to_string()))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(TransportError::RelayUnavailable(format!(
            "health answered {}",
            response.status()
        )));
    }
    let address = response.remote_addr().ok_or_else(|| {
        TransportError::RelayUnavailable("The relay connection address is unavailable".into())
    })?;
    let body = response
        .bytes()
        .await
        .map_err(|error| TransportError::RelayUnavailable(error.to_string()))?;
    let body = serde_json::from_slice(&body).map_err(|_| {
        TransportError::RelayUnavailable("Invalid mock relay health response".into())
    })?;
    check_mock_relay_health(&body, address).map_err(TransportError::RelayUnavailable)?;
    Ok(())
}

fn relay_session(url: String, folder: String, form_type: &str) -> IafSftpSession {
    IafSftpSession {
        handle: None,
        remote_folder: folder,
        source: SftpEndpointSource::Relay,
        dry_run_puts: None,
        relay: Some((url, form_type.to_owned())),
    }
}

async fn open_iaf_session_with_source(
    form_type: &str,
    endpoint: SftpEndpoint,
    source: SftpEndpointSource,
) -> Result<IafSftpSession, TransportError> {
    require_loopback_unless_direct(&endpoint.host, direct_filing_built_in())?;
    info!(
        "Connecting to BIR SFTP gateway {}:{} folder={}",
        endpoint.host, endpoint.port, form_type
    );
    let config = client::Config {
        nodelay: true,
        ..Default::default()
    };
    let mut handle = client::connect(
        Arc::new(config),
        (endpoint.host.as_str(), endpoint.port),
        ConfiguredHostKey {
            policy: endpoint.host_key_policy.clone(),
        },
    )
    .await
    .map_err(|error| TransportError::Ssh(error.to_string()))?;
    let auth = handle
        .authenticate_password(endpoint.username.clone(), endpoint.password.as_str())
        .await
        .map_err(|error| TransportError::Ssh(error.to_string()))?;
    if !auth.success() {
        return Err(TransportError::Ssh(
            "SFTP password authentication failed".into(),
        ));
    }
    Ok(IafSftpSession {
        handle: Some(handle),
        remote_folder: if endpoint.remote_folder.is_empty() {
            form_type.to_string()
        } else {
            endpoint.remote_folder
        },
        source,
        dry_run_puts: None,
        relay: None,
    })
}

/// Uploads an encrypted IAF file to the BIR SFTP server.
///
/// `form_type` must match the subfolder on the BIR server (e.g. "2551Qv2018").
/// `filename` is the IAF filename; `payload` is the encrypted IAF bytes.
///
/// This raw irreversible boundary is crate-internal. External callers must not
/// bypass the reviewed Final Copy, queue-admission, and claim workflows.
/// Queue workers call [`open_iaf_session`], then claim, then
/// [`IafSftpSession::store`] so a connect timeout does not freeze a claim.
pub(crate) async fn submit_iaf(
    form_type: &str,
    tin: &str,
    filename: &str,
    payload: &[u8],
) -> Result<(), TransportError> {
    let session = open_iaf_session(form_type, tin).await?;
    session.store(filename, payload).await
}

/// Probe using an already-resolved target, including in-process dry-run.
///
/// Public only for the `sftp_harness` binary. It authenticates and lists the
/// working directory; it never uploads. Application code must go through the
/// queue worker, never call this.
pub async fn probe_sftp_target(
    form_type: &str,
    target: ResolvedSftpTarget,
) -> Result<String, TransportError> {
    match target {
        ResolvedSftpTarget::DryRun { folder } => {
            let folder = folder.trim_matches('/');
            if folder.is_empty() {
                Ok("/".to_string())
            } else {
                Ok(format!("/{folder}"))
            }
        }
        ResolvedSftpTarget::Relay { url, .. } => {
            relay_health(&url).await?;
            Ok(format!("relay {url}"))
        }
        ResolvedSftpTarget::Live { endpoint, source } => {
            let session = open_iaf_session_with_source(form_type, endpoint, source).await?;
            session.probe().await
        }
    }
}

/// Direct upload against a caller-supplied endpoint. Crate-internal: only the
/// loopback test exercises it. The logged provenance follows the host-key
/// policy, so a dispatcher endpoint is never mislabelled as `env`.
#[cfg(test)]
pub(crate) async fn submit_iaf_with_endpoint(
    form_type: &str,
    filename: &str,
    payload: &[u8],
    endpoint: SftpEndpoint,
) -> Result<(), TransportError> {
    let source = match endpoint.host_key_policy {
        HostKeyPolicy::AcceptAnyOfficial => SftpEndpointSource::Dispatcher,
        HostKeyPolicy::AcceptAnyLab | HostKeyPolicy::PinnedSha256(_) => SftpEndpointSource::Env,
    };
    let session = open_iaf_session_with_source(form_type, endpoint, source).await?;
    session.store(filename, payload).await
}

/// Upload using an already-resolved target, including in-process dry-run.
///
/// Public only for the `sftp_harness` binary. This is the same irreversible
/// boundary as [`submit_iaf`]: it bypasses Final Copy, queue admission, claim
/// and lease. Never call it from application, agent, or cron code.
pub async fn submit_sftp_target(
    form_type: &str,
    filename: &str,
    payload: &[u8],
    target: ResolvedSftpTarget,
) -> Result<(), TransportError> {
    match target {
        ResolvedSftpTarget::DryRun { folder } => {
            let session = IafSftpSession {
                handle: None,
                remote_folder: folder,
                source: SftpEndpointSource::DryRun,
                dry_run_puts: None,
                relay: None,
            };
            session.store(filename, payload).await
        }
        ResolvedSftpTarget::Relay { url, folder } => {
            relay_session(url, folder, form_type)
                .store(filename, payload)
                .await
        }
        ResolvedSftpTarget::Live { endpoint, source } => {
            let session = open_iaf_session_with_source(form_type, endpoint, source).await?;
            session.store(filename, payload).await
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_dev_or_explicit_mock_builds_allow_the_relay() {
        assert!(require_mock_relay(true).is_ok());
        assert!(matches!(
            require_mock_relay(false),
            Err(TransportError::Config(_))
        ));
    }

    async fn http_reply(status: u16, body: &str, probe: bool) -> Result<(), TransportError> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        http_reply_from(listener, status, body, probe).await
    }

    async fn http_reply_from(
        listener: std::net::TcpListener,
        status: u16,
        body: &str,
        probe: bool,
    ) -> Result<(), TransportError> {
        use std::io::{Read, Write};
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_owned();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return false;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("relay test accept failed: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 8192];
            let count = socket.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.starts_with(if probe {
                "GET /v1/health "
            } else {
                "POST /v1/submissions "
            }));
            write!(
                socket,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            true
        });
        let result = if probe {
            probe_sftp_target(
                "1601C",
                ResolvedSftpTarget::Relay {
                    url,
                    folder: "1601C".into(),
                },
            )
            .await
            .map(|_| ())
        } else {
            relay_store(
                &url,
                "1601C",
                "1601C",
                "000123453-1601C-2026#juana@example.com#.xml",
                b"",
            )
            .await
        };
        assert_eq!(
            server.join().unwrap(),
            cfg!(any(debug_assertions, feature = "mock-relay"))
        );
        result
    }

    #[cfg(not(any(debug_assertions, feature = "mock-relay")))]
    #[tokio::test]
    async fn release_refuses_relay_health_and_upload_without_connecting() {
        for probe in [true, false] {
            assert!(matches!(
                http_reply(200, "{}", probe).await,
                Err(TransportError::Config(_))
            ));
        }
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn relay_response_classification() {
        for status in [408, 429, 503] {
            assert!(matches!(
                http_reply(status, "{}", false).await,
                Err(TransportError::RelayUnavailable(_))
            ));
        }
        assert!(matches!(
            http_reply(400, "{}", false).await,
            Err(TransportError::RelayRefused(_))
        ));
        assert!(
            http_reply(202, r#"{"status":"accepted"}"#, false)
                .await
                .is_ok()
        );
        assert!(matches!(
            http_reply(202, "", false).await,
            Err(TransportError::Http(_))
        ));
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn relay_logs_form_and_length_without_iaf_names_or_reflected_pii() {
        use std::io::Write;
        use std::sync::Mutex;
        use tracing::instrument::WithSubscriber;

        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let captured = Capture(Arc::new(Mutex::new(Vec::new())));
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish();
        async {
            assert!(
                http_reply(
                    202,
                    r#"{"status":"accepted","receipt_id":"juana@example.com"}"#,
                    false
                )
                .await
                .is_ok()
            );
            for status in [400, 408, 429, 503, 502, 202] {
                let error = http_reply(
                    status,
                    r#"{"error":"000123453-1601C-202601#juana@example.com#.xml"}"#,
                    false,
                )
                .await
                .unwrap_err();
                warn!(%error, "the worker can log the returned error safely");
            }
        }
        .with_subscriber(subscriber)
        .await;
        let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(
            logs.contains("1601C") && logs.contains("payload_len=0"),
            "{logs}"
        );
        for private in ["000123453", "juana@example.com", ".xml"] {
            assert!(!logs.contains(private), "{logs}");
        }
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn relay_probe_checks_health() {
        assert!(
            http_reply(
                200,
                r#"{"service":"buwiz-mock-relay","version":1,"ok":true,"mock":true}"#,
                true
            )
            .await
            .is_ok()
        );
        for body in [
            "{}",
            "invalid",
            "",
            r#"{"ok":true,"mock":true,"version":0}"#,
            r#"{"ok":"true","mock":true}"#,
            r#"{"service":"another-service","version":1,"ok":true,"mock":true}"#,
        ] {
            let Err(TransportError::RelayUnavailable(why)) = http_reply(200, body, true).await
            else {
                panic!("invalid health must be unavailable");
            };
            assert!(!why.contains("out of date"), "{why}");
        }
        assert!(matches!(
            http_reply(503, "{}", true).await,
            Err(TransportError::RelayUnavailable(_))
        ));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        assert!(matches!(
            probe_sftp_target(
                "1601C",
                ResolvedSftpTarget::Relay {
                    url,
                    folder: "1601C".into()
                }
            )
            .await,
            Err(TransportError::RelayUnavailable(_))
        ));
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn legacy_relay_advice_preserves_the_checked_bind_and_port() {
        for host in ["127.0.0.1", "::1"] {
            let listener = match std::net::TcpListener::bind((host, 0)) {
                Ok(listener) => listener,
                Err(error) if host == "::1" => {
                    eprintln!("IPv6 loopback is unavailable: {error}");
                    continue;
                }
                Err(error) => panic!("IPv4 loopback is unavailable: {error}"),
            };
            let address = listener.local_addr().unwrap();
            let Err(TransportError::RelayUnavailable(why)) =
                http_reply_from(listener, 200, r#"{"ok":true,"mock":true}"#, true).await
            else {
                panic!("legacy health must be unavailable");
            };
            assert_eq!(
                why,
                format!(
                    "The mock relay is out of date. Restart it with `just mock-relay --bind {} --port {}`, then restart the app.",
                    address.ip(),
                    address.port()
                )
            );
        }
    }

    #[test]
    fn only_a_loopback_http_origin_is_a_relay() {
        assert_eq!(
            loopback_relay_url("http://127.0.0.1:8787/").unwrap(),
            "http://127.0.0.1:8787"
        );
        assert_eq!(
            loopback_relay_url("http://[::1]:8787").unwrap(),
            "http://[::1]:8787"
        );
        assert_eq!(
            loopback_relay_url("http://localhost:8787").unwrap(),
            "http://localhost:8787"
        );
        for refused in [
            "https://127.0.0.1:8787",
            "http://relay.buwiz.com",
            "http://10.0.0.2:8787",
            "http://127.0.0.1:8787/v1",
            "http://user:pw@127.0.0.1:8787",
            "http://127.0.0.1:8787?x=1",
            "not a url",
        ] {
            assert!(
                matches!(loopback_relay_url(refused), Err(TransportError::Config(_))),
                "{refused}"
            );
        }
    }

    use super::*;

    #[test]
    fn dispatcher_wrap_roundtrip_strips_bom() {
        let wrapped = wrap_dispatcher_field("ebf2.bir.gov.ph").expect("wrap");
        let plain = unwrap_dispatcher_field(&wrapped).expect("unwrap");
        assert_eq!(plain, "ebf2.bir.gov.ph");
    }

    #[test]
    fn dispatcher_json_accepts_single_quoted_official_body() {
        let body = "{'mode':'2', 'server':'abc', 'SSLPort':'990', 'port':'22', 'username':'u', 'password':'p'}";
        let parsed = parse_dispatcher_json(body).expect("parse");
        assert_eq!(parsed.mode, "2");
        assert_eq!(parsed.port, "22");
        assert_eq!(parsed.ssl_port.as_deref(), Some("990"));
    }

    #[test]
    fn remote_path_joins_form_folder_and_basename() {
        assert_eq!(
            remote_path(
                "1601Cv2018",
                r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#a@b.com#.xml"
            ),
            "/1601Cv2018/00000000000000-1601Cv2018-092026#a@b.com#.xml"
        );
        assert_eq!(
            remote_path(
                "1601Cv2018",
                "/tmp/00000000000000-1601Cv2018-092026#a@b.com#.xml"
            ),
            "/1601Cv2018/00000000000000-1601Cv2018-092026#a@b.com#.xml"
        );
        assert_eq!(remote_path("/", "file.xml"), "/file.xml");
        assert_eq!(iaf_basename(r"C:\dir\file.xml"), "file.xml");
        assert_eq!(iaf_basename("/unix/dir/file.xml"), "file.xml");
    }

    #[test]
    fn iaf_filename_keeps_fourteen_digit_prefix() {
        let name = crate::naming::iaf_filename(
            &crate::naming::Tin {
                segment1: "000".into(),
                segment2: "000".into(),
                segment3: "000".into(),
                branch: "00000".into(),
            },
            "1601Cv2018",
            "092026",
            "test@mail.com",
        );
        assert_eq!(name, "00000000000000-1601Cv2018-092026#test@mail.com#.xml");
        assert_eq!(name.split('-').next().unwrap().len(), 14);
    }

    const SFTP_ENV_KEYS: [&str; 11] = [
        "EBIR_TEST_ENV",
        "BIR_SFTP_DRY_RUN",
        "BIR_SFTP_LIVE",
        "BIR_SFTP_HOST",
        "BIR_SFTP_PORT",
        "BIR_SFTP_USERNAME",
        "BIR_SFTP_USER",
        "BIR_SFTP_PASSWORD",
        "BIR_SFTP_FOLDER",
        "BIR_SFTP_HOST_KEY_SHA256",
        "BIR_SFTP_ACCEPT_ANY_HOST_KEY",
    ];

    fn isolated_sftp_env<T>(
        extra: impl IntoIterator<Item = (&'static str, Option<&'static str>)>,
        f: impl FnOnce() -> T,
    ) -> T {
        let mut vars: Vec<(&str, Option<&str>)> =
            SFTP_ENV_KEYS.iter().map(|key| (*key, None)).collect();
        vars.extend(extra);
        temp_env::with_vars(vars, f)
    }

    async fn isolated_sftp_env_async<T>(
        extra: impl IntoIterator<Item = (&'static str, Option<&'static str>)>,
        f: impl std::future::Future<Output = T>,
    ) -> T {
        let mut vars: Vec<(&str, Option<&str>)> =
            SFTP_ENV_KEYS.iter().map(|key| (*key, None)).collect();
        vars.extend(extra);
        temp_env::async_with_vars(vars, f).await
    }

    #[test]
    fn bir_sftp_env_defaults_port_22_and_form_folder() {
        isolated_sftp_env(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", Some("1")),
            ],
            || {
                let endpoint = from_bir_sftp_env("1601Cv2018").unwrap().unwrap();
                assert_eq!(endpoint.host, "127.0.0.1");
                assert_eq!(endpoint.port, 22);
                assert_eq!(endpoint.username, "lab");
                assert_eq!(endpoint.remote_folder, "1601Cv2018");
            },
        );
    }

    #[test]
    fn bir_sftp_env_honors_port_and_folder() {
        isolated_sftp_env(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_PORT", Some("2222")),
                ("BIR_SFTP_USER", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_FOLDER", Some("labdir")),
                ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", Some("1")),
            ],
            || {
                let endpoint = from_bir_sftp_env("1601Cv2018").unwrap().unwrap();
                assert_eq!(endpoint.port, 2222);
                assert_eq!(endpoint.username, "lab");
                assert_eq!(endpoint.remote_folder, "labdir");
            },
        );
    }

    #[test]
    fn bir_sftp_host_without_username_is_config_error() {
        isolated_sftp_env(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
            ],
            || {
                let err = from_bir_sftp_env("1601Cv2018").unwrap().unwrap_err();
                assert!(matches!(err, TransportError::Config(_)));
            },
        );
    }

    #[test]
    fn empty_bir_sftp_host_is_unset() {
        isolated_sftp_env([("BIR_SFTP_HOST", Some("   "))], || {
            assert!(from_bir_sftp_env("1601Cv2018").is_none());
        });
    }

    #[test]
    fn bir_sftp_live_zero_is_dry_run() {
        isolated_sftp_env([("BIR_SFTP_LIVE", Some("0"))], || {
            assert!(sftp_dry_run_requested());
            assert!(!dispatcher_live_ack_requested());
        });
    }

    #[test]
    fn https_dispatcher_url_flips_scheme_only() {
        assert_eq!(
            https_dispatcher_url(DISPATCHER_PRIMARY),
            "https://birgovph.com/tinDispatcherSFTP.php"
        );
        assert_eq!(
            https_dispatcher_url(DISPATCHER_BACKUP),
            "https://ws2.birgovph.com/tinDispatcherSFTP.php"
        );
    }

    #[tokio::test]
    async fn dispatcher_without_live_ack_is_dry_run_in_debug() {
        // This test binary is a debug build. Empty env is a dry run.
        // A release build with the same empty env is live (see
        // `release_default_without_flags_is_live`).
        isolated_sftp_env_async([], async {
            let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                .await
                .unwrap();
            assert_eq!(target.source(), SftpEndpointSource::DryRun);
        })
        .await;
    }

    #[test]
    fn release_default_without_flags_is_live() {
        let none = SftpDryRunInputs {
            test_env: false,
            dry_run: false,
            live_disabled: false,
            live_enabled: false,
            lab_host: false,
            debug: false,
            practice: false,
        };
        assert!(!dry_run_from_inputs(none));
        assert!(dry_run_from_inputs(SftpDryRunInputs {
            debug: true,
            ..none
        }));
        assert!(dry_run_from_inputs(SftpDryRunInputs {
            test_env: true,
            lab_host: true,
            debug: false,
            ..none
        }));
        assert!(!dry_run_from_inputs(SftpDryRunInputs {
            lab_host: true,
            debug: true,
            ..none
        }));
        assert!(!dry_run_from_inputs(SftpDryRunInputs {
            live_enabled: true,
            debug: true,
            ..none
        }));
        assert!(dry_run_from_inputs(SftpDryRunInputs {
            live_disabled: true,
            live_enabled: true,
            debug: false,
            ..none
        }));
    }

    #[test]
    fn a_practice_account_is_a_dry_run_over_a_live_ack_and_a_lab_host() {
        let live = SftpDryRunInputs {
            test_env: false,
            dry_run: false,
            live_disabled: false,
            live_enabled: true,
            lab_host: true,
            debug: false,
            practice: false,
        };
        assert!(!dry_run_from_inputs(live));
        assert!(dry_run_from_inputs(SftpDryRunInputs {
            practice: true,
            ..live
        }));
    }

    /// Resets the process-wide flag even when an assertion fails.
    struct PracticeReset;

    impl Drop for PracticeReset {
        fn drop(&mut self) {
            set_practice_account(false);
        }
    }

    // Runs under the temp_env lock (as every test that expects a live target
    // does), so no other test sees the flag set.
    #[tokio::test]
    async fn the_practice_flag_forces_a_dry_run_and_resetting_restores_it() {
        isolated_sftp_env_async(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", Some("1")),
            ],
            async {
                let _reset = PracticeReset;
                assert!(!is_sftp_dry_run(), "a lab host is live without the flag");
                set_practice_account(true);
                assert!(practice_account());
                assert!(is_sftp_dry_run());
                let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                    .await
                    .unwrap();
                assert_eq!(target.source(), SftpEndpointSource::DryRun);
                set_practice_account(false);
                assert!(!is_sftp_dry_run());
            },
        )
        .await;
    }

    #[test]
    fn dry_run_store_records_path_and_hash_without_network() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = IafSftpSession {
            handle: None,
            remote_folder: "1601Cv2018".into(),
            source: SftpEndpointSource::DryRun,
            dry_run_puts: Some(log.clone()),
            relay: None,
        };
        let payload = b"dummy-encrypted-iaf";
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(session.store("00000000000000-1601Cv2018-092026#a@b.com#.xml", payload))
            .unwrap();
        let puts = log.lock().unwrap().clone();
        assert_eq!(puts.len(), 1);
        assert_eq!(
            puts[0].remote_path,
            "/1601Cv2018/00000000000000-1601Cv2018-092026#a@b.com#.xml"
        );
        assert_eq!(puts[0].payload_len, payload.len());
        assert_eq!(puts[0].payload_sha256, hex::encode(Sha256::digest(payload)));
    }

    #[test]
    fn only_the_bir_route_without_direct_filing_has_nowhere_to_send() {
        for route in [
            Route::PanelDryRun,
            Route::PanelTestServer,
            Route::MockRelay(Ok("http://127.0.0.1:8787".into())),
            Route::MockRelay(Err(TransportError::Config("refused".into()))),
            Route::DryRun,
            Route::LabHost,
            Route::Buwiz,
        ] {
            assert!(!unavailable_for(&route, false));
            assert!(!unavailable_for(&route, true));
        }
        assert!(unavailable_for(&Route::Bir, false));
        assert!(!unavailable_for(&Route::Bir, true));
    }

    #[test]
    fn sftp_without_direct_filing_is_loopback_only() {
        for host in ["127.0.0.1", "::1", "[::1]", " 127.0.0.1 ", "127.8.9.10"] {
            assert!(
                require_loopback_unless_direct(host, false).is_ok(),
                "{host}"
            );
        }
        for host in [
            "192.0.2.1",
            "10.0.2.2",
            "0.0.0.0",
            "localhost",
            "ebf2.example",
            "127.0.0.1.example",
            "",
        ] {
            assert!(
                matches!(
                    require_loopback_unless_direct(host, false),
                    Err(TransportError::Config(_))
                ),
                "{host}"
            );
            assert!(require_loopback_unless_direct(host, true).is_ok(), "{host}");
        }
    }

    /// A lab host that is not this computer resolves (the settings are complete)
    /// but the session never connects: refused before any socket opens.
    #[cfg(not(feature = "direct-bir"))]
    #[tokio::test]
    async fn a_lab_host_off_this_computer_is_refused_before_connecting() {
        let endpoint = SftpEndpoint {
            host: "192.0.2.1".into(),
            port: 22,
            username: "lab".into(),
            password: Zeroizing::new("secret".into()),
            remote_folder: "1601Cv2018".into(),
            host_key_policy: HostKeyPolicy::AcceptAnyLab,
        };
        let refused =
            open_iaf_session_with_source("1601Cv2018", endpoint, SftpEndpointSource::Env).await;
        assert!(matches!(refused, Err(TransportError::Config(_))));
    }

    /// The careless launch of a build that ships: not a test environment, a live
    /// acknowledgement, nothing else. It has nowhere to send, says so without
    /// touching the network, and refuses to queue.
    #[cfg(not(feature = "direct-bir"))]
    #[tokio::test]
    async fn a_live_ack_without_direct_filing_sends_nowhere() {
        isolated_sftp_env_async(
            [("EBIR_TEST_ENV", None), ("BIR_SFTP_LIVE", Some("1"))],
            async {
                assert!(!is_sftp_dry_run(), "the setup must ask for a live send");
                assert!(!direct_filing_built_in());
                assert!(sending_unavailable());
                let refused = resolve_sftp_endpoint("1601Cv2018", "000000000").await;
                assert!(
                    matches!(&refused, Err(TransportError::Config(why)) if why == SENDING_UNAVAILABLE),
                    "{refused:?}"
                );
                // A release build refuses Queue here; this development build does not ask.
                assert!(queue_refused(false, sending_unavailable));
                assert_eq!(
                    refuse_queue_when_unavailable().is_ok(),
                    cfg!(debug_assertions)
                );
            },
        )
        .await;
    }

    /// Queue is refused only by a release build with nowhere to send. A development build never refuses and never
    /// reads the environment for it: another test may be changing that environment on another thread.
    #[test]
    fn only_a_release_build_with_nowhere_to_send_refuses_queue() {
        assert!(queue_refused(false, || true));
        assert!(!queue_refused(false, || false));
        assert!(!queue_refused(true, || panic!(
            "a development build does not ask"
        )));
    }

    /// A dry run, the mock relay and a lab host are somewhere to send: the queue runs and Queue is allowed.
    #[test]
    fn a_dry_run_a_mock_relay_and_a_lab_host_are_somewhere_to_send() {
        for vars in [
            [("EBIR_TEST_ENV", None), ("BIR_SFTP_DRY_RUN", Some("1"))],
            [("EBIR_TEST_ENV", Some("1")), ("BIR_SFTP_LIVE", Some("1"))],
            [
                ("EBIR_TEST_ENV", None),
                ("BIR_RELAY_URL", Some("http://127.0.0.1:8787")),
            ],
            [
                ("EBIR_TEST_ENV", None),
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
            ],
        ] {
            isolated_sftp_env(vars, || {
                assert!(!sending_unavailable(), "{vars:?}");
                assert!(!queue_refused(false, sending_unavailable), "{vars:?}");
            });
        }
    }

    /// The local Buwiz relay is asked for like the mock relay: it wins over the dry-run flags and a test environment
    /// (a development build only), and never over a practice account.
    #[test]
    fn the_local_buwiz_relay_wins_over_a_dry_run_but_never_over_a_practice_account() {
        struct ResetPractice;
        impl Drop for ResetPractice {
            fn drop(&mut self) {
                set_practice_account(false);
            }
        }
        isolated_sftp_env(
            [
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_SFTP_DRY_RUN", Some("1")),
                (RELAY_SEND_ENV, Some("1")),
            ],
            || {
                assert_eq!(buwiz_relay_requested(), cfg!(debug_assertions));
                assert_eq!(sends_through_buwiz(), cfg!(debug_assertions));
                assert!(!sending_unavailable());
                let _reset = ResetPractice;
                set_practice_account(true);
                assert!(!sends_through_buwiz(), "a practice account only simulates");
                assert!(matches!(route(), Route::DryRun));
            },
        );
        isolated_sftp_env([(RELAY_SEND_ENV, None)], || {
            assert!(!sends_through_buwiz());
        });
    }

    #[tokio::test]
    async fn dry_run_wins_over_env_and_skips_dispatcher() {
        isolated_sftp_env_async(
            [
                ("BIR_SFTP_DRY_RUN", Some("1")),
                ("BIR_SFTP_HOST", Some("should-not-use.example")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
            ],
            async {
                let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                    .await
                    .unwrap();
                assert!(matches!(target, ResolvedSftpTarget::DryRun { .. }));
                assert_eq!(target.source(), SftpEndpointSource::DryRun);
            },
        )
        .await;
    }

    #[tokio::test]
    async fn bir_sftp_live_zero_skips_dispatcher() {
        isolated_sftp_env_async([("BIR_SFTP_LIVE", Some("0"))], async {
            let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                .await
                .unwrap();
            assert_eq!(target.source(), SftpEndpointSource::DryRun);
        })
        .await;
    }

    #[tokio::test]
    async fn test_env_forces_dry_run_over_a_lab_host() {
        isolated_sftp_env_async(
            [
                ("EBIR_TEST_ENV", Some("1")),
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", Some("1")),
            ],
            async {
                let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                    .await
                    .unwrap();
                assert_eq!(target.source(), SftpEndpointSource::DryRun);
            },
        )
        .await;
    }

    #[tokio::test]
    async fn bir_sftp_env_resolves_live_env_target() {
        isolated_sftp_env_async(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_ACCEPT_ANY_HOST_KEY", Some("1")),
            ],
            async {
                let target = resolve_sftp_endpoint("1601Cv2018", "000000000")
                    .await
                    .unwrap();
                match target {
                    ResolvedSftpTarget::Live { endpoint, source } => {
                        assert_eq!(source, SftpEndpointSource::Env);
                        assert_eq!(endpoint.host, "127.0.0.1");
                    }
                    other => panic!("expected live env target, got {other:?}"),
                }
            },
        )
        .await;
    }

    #[test]
    fn bir_sftp_host_without_host_key_policy_is_config_error() {
        isolated_sftp_env(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
            ],
            || {
                let err = from_bir_sftp_env("1601Cv2018").unwrap().unwrap_err();
                assert!(matches!(err, TransportError::Config(_)));
            },
        );
    }

    #[test]
    fn bir_sftp_env_accepts_pinned_host_key() {
        isolated_sftp_env(
            [
                ("BIR_SFTP_HOST", Some("127.0.0.1")),
                ("BIR_SFTP_USERNAME", Some("lab")),
                ("BIR_SFTP_PASSWORD", Some("secret")),
                ("BIR_SFTP_HOST_KEY_SHA256", Some("abc123")),
            ],
            || {
                let endpoint = from_bir_sftp_env("1601Cv2018").unwrap().unwrap();
                assert_eq!(
                    endpoint.host_key_policy,
                    HostKeyPolicy::PinnedSha256("abc123".into())
                );
            },
        );
    }
}
