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

use aes::Aes256;
use aes::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use rand::RngExt;
use reqwest::Client;
use russh::Disconnect;
use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use serde::Deserialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::info;
use zeroize::Zeroizing;

const DISPATCHER_PRIMARY: &str = "http://birgovph.com/tinDispatcherSFTP.php";
const DISPATCHER_BACKUP: &str = "http://ws2.birgovph.com/tinDispatcherSFTP.php";
const DISPATCHER_HTTPS_TIMEOUT: Duration = Duration::from_secs(4);
const DISPATCHER_CLIENT_VERSION: &str = "7.9.6.0";
/// Passphrase used by official `ebfSFTP.exe` to unwrap dispatcher ciphertext.
/// This is a protocol constant present in every 7.9.6 install, not a taxpayer secret.
const DISPATCHER_WRAP_PASSPHRASE: &str = "Carlo*TSSD2!018";
const PBKDF2_ROUNDS: u32 = 100_000;
const SALT_LEN: usize = 32;
const IV_LEN: usize = 16;
const KEY_LEN: usize = 32;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
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
}

impl SftpEndpointSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Env => "env",
            Self::Dispatcher => "dispatcher",
        }
    }

    pub fn is_dry_run(self) -> bool {
        matches!(self, Self::DryRun)
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
}

impl ResolvedSftpTarget {
    pub fn source(&self) -> SftpEndpointSource {
        match self {
            Self::DryRun { .. } => SftpEndpointSource::DryRun,
            Self::Live { source, .. } => *source,
        }
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
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

pub fn is_sftp_dry_run() -> bool {
    sftp_dry_run_requested()
}

fn sftp_dry_run_requested() -> bool {
    env_flag_enabled("BIR_SFTP_DRY_RUN") || env_flag_disabled("BIR_SFTP_LIVE")
}

/// Production dispatcher is opt-in. Unset `BIR_SFTP_LIVE` refuses it.
fn dispatcher_live_ack_requested() -> bool {
    env_flag_enabled("BIR_SFTP_LIVE")
}

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

/// Pick dry-run, a complete `BIR_SFTP_*` lab override, or the official dispatcher.
pub async fn resolve_sftp_endpoint(
    form_type: &str,
    tin: &str,
) -> Result<ResolvedSftpTarget, TransportError> {
    if sftp_dry_run_requested() {
        let folder = env_nonempty("BIR_SFTP_FOLDER").unwrap_or_else(|| form_type.to_string());
        info!(
            source = "dry-run",
            folder = folder.as_str(),
            "SFTP dry-run: no network"
        );
        return Ok(ResolvedSftpTarget::DryRun { folder });
    }
    if let Some(endpoint) = from_bir_sftp_env(form_type) {
        let endpoint = endpoint?;
        info!(
            source = "env",
            host = endpoint.host.as_str(),
            port = endpoint.port,
            folder = endpoint.remote_folder.as_str(),
            username_len = endpoint.username.len(),
            password_len = endpoint.password.len(),
            "Using BIR_SFTP_* override"
        );
        return Ok(ResolvedSftpTarget::Live {
            endpoint,
            source: SftpEndpointSource::Env,
        });
    }
    if !dispatcher_live_ack_requested() {
        return Err(TransportError::Config(
            "refusing production tinDispatcherSFTP.php without BIR_SFTP_LIVE=1 (set BIR_SFTP_DRY_RUN=1, complete BIR_SFTP_* for a lab host, or BIR_SFTP_LIVE=1)"
                .into(),
        ));
    }
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

fn parse_dispatcher_json(body: &str) -> Result<DispatcherResponse, TransportError> {
    let normalized = body.trim().replace('\'', "\"");
    serde_json::from_str(&normalized).map_err(|error| {
        TransportError::Dispatcher(format!("could not parse dispatcher JSON: {error}"))
    })
}

async fn fetch_dispatcher(
    client: &Client,
    base: &str,
    tin: &str,
    form_type: &str,
) -> Result<DispatcherResponse, TransportError> {
    fetch_dispatcher_timed(client, base, tin, form_type, Duration::from_secs(8)).await
}

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
}

impl IafSftpSession {
    pub fn source(&self) -> SftpEndpointSource {
        self.source
    }
    pub async fn store(mut self, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
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
        if self.source.is_dry_run() {
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
        }),
        ResolvedSftpTarget::Live { endpoint, source } => {
            open_iaf_session_with_source(form_type, endpoint, source).await
        }
    }
}

async fn open_iaf_session_with_source(
    form_type: &str,
    endpoint: SftpEndpoint,
    source: SftpEndpointSource,
) -> Result<IafSftpSession, TransportError> {
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
            };
            session.store(filename, payload).await
        }
        ResolvedSftpTarget::Live { endpoint, source } => {
            let session = open_iaf_session_with_source(form_type, endpoint, source).await?;
            session.store(filename, payload).await
        }
    }
}

#[cfg(test)]
mod tests {
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

    const SFTP_ENV_KEYS: [&str; 10] = [
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
    async fn dispatcher_without_live_ack_is_config_error() {
        isolated_sftp_env_async([], async {
            let err = resolve_sftp_endpoint("1601Cv2018", "000000000")
                .await
                .unwrap_err();
            match err {
                TransportError::Config(message) => {
                    assert!(message.contains("BIR_SFTP_LIVE=1"), "{message}");
                }
                other => panic!("expected config refuse, got {other:?}"),
            }
        })
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
                    ResolvedSftpTarget::DryRun { .. } => panic!("expected live env target"),
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
