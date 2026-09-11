//! SFTP submission transport for eBIRForms 7.9.6.x, behind a
//! [`SubmissionTransport`] abstraction. Secret-free: no BIR host, username, or
//! password is embedded. Live endpoints are resolved at runtime from the BIR
//! dispatcher; tests/self-host supply an [`SftpEndpoint`] externally.
//!
//! Reconstructed from the installed official client (see
//! `evidence/sftp-submission/`): `ebfSFTP.exe` IL, extracted HTA/JS/VBS, a live
//! dummy-TIN dispatcher probe, and a loopback SFTP upload proof. The client
//! mechanic here (connect → password auth → accept-any host key → upload to
//! `/<formType>/<file>`) is the same one verified in
//! `evidence/sftp-submission/scripts/sftp-loopback`.

use aes::Aes256;
use aes::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use reqwest::Client;
use russh::keys::PublicKeyOrCertificate;
use serde::Deserialize;
use sha1::Sha1;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;

/// Primary connection web service (environment.js `userTypeWS.PROD.primary`).
const DISPATCHER_PRIMARY: &str = "http://birgovph.com/tinDispatcherSFTP.php";
/// Backup connection web service — `ws1` (the connection-WS backup). `ws2` is
/// the *version-check* backup and is NOT used for the dispatcher.
const DISPATCHER_BACKUP: &str = "http://ws1.birgovph.com/tinDispatcherSFTP.php";
/// Client version the dispatcher is keyed with (environment.js `currVer`).
const DISPATCHER_CLIENT_VERSION: &str = "7.9.6.0";

/// PBKDF2 passphrase `ebfSFTP.exe` uses to unwrap the dispatcher connection
/// fields. A protocol constant present in every 7.9.6 install — NOT the SFTP
/// login secret and NOT the IAF payload key.
const WRAP_PASSPHRASE: &str = "Carlo*TSSD2!018";
const PBKDF2_ROUNDS: u32 = 100_000;
const SALT_LEN: usize = 32;
const IV_LEN: usize = 16;
const KEY_LEN: usize = 32;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("dispatcher error: {0}")]
    Dispatcher(String),
    #[error("credential unwrap failed: {0}")]
    Crypto(String),
    #[error("SFTP error: {0}")]
    Sftp(String),
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("submission rejected by server")]
    Rejected,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<reqwest::Error> for TransportError {
    fn from(e: reqwest::Error) -> Self {
        Self::Dispatcher(e.to_string())
    }
}

/// A resolved SFTP endpoint. Live values come from the dispatcher; the harness
/// and self-host tests build this from `TEST_SFTP_*` env vars. No secret is ever
/// embedded in source.
#[derive(Clone)]
pub struct SftpEndpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Zeroizing<String>,
    /// Remote form-type folder (e.g. "1601Cv2018"); uploads go to "/<folder>/<file>".
    pub remote_folder: String,
}

impl SftpEndpoint {
    /// Build an endpoint from `TEST_SFTP_*` env vars (harness / self-host only).
    pub fn from_env(form_type: &str) -> Result<Self, TransportError> {
        fn req(name: &str) -> Result<String, TransportError> {
            std::env::var(name).map_err(|_| TransportError::Dispatcher(format!("missing {name}")))
        }
        let port = req("TEST_SFTP_PORT")?
            .parse::<u16>()
            .map_err(|e| TransportError::Dispatcher(format!("invalid TEST_SFTP_PORT: {e}")))?;
        Ok(Self {
            host: req("TEST_SFTP_HOST")?,
            port,
            username: req("TEST_SFTP_USER")?,
            password: Zeroizing::new(req("TEST_SFTP_PASSWORD")?),
            remote_folder: std::env::var("TEST_SFTP_FOLDER").unwrap_or_else(|_| form_type.to_string()),
        })
    }
}

/// Decrypt a dispatcher field (`server`/`username`/`password`).
///
/// Layout: `base64( salt[32] || iv[16] || AES-256-CBC/PKCS7( UTF-8 plaintext ) )`.
/// Key: PBKDF2-HMAC-SHA1(`Carlo*TSSD2!018`, salt, 100000, dkLen=32).
/// The official helper writes UTF-8 **without** a BOM; a leading BOM is stripped
/// defensively.
pub fn unwrap_dispatcher_field(cipher_text: &str) -> Result<String, TransportError> {
    let raw = data_encoding::BASE64
        .decode(cipher_text.trim().as_bytes())
        .map_err(|e| TransportError::Crypto(format!("invalid ciphertext: {e}")))?;
    if raw.len() < SALT_LEN + IV_LEN + 16 {
        return Err(TransportError::Crypto("ciphertext shorter than salt+iv+block".into()));
    }
    let (salt, rest) = raw.split_at(SALT_LEN);
    let (iv, cipher) = rest.split_at(IV_LEN);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    pbkdf2_hmac::<Sha1>(WRAP_PASSPHRASE.as_bytes(), salt, PBKDF2_ROUNDS, key.as_mut());
    let mut buf = cipher.to_vec();
    let plain = Aes256CbcDec::new(key.as_ref().into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| TransportError::Crypto(format!("decrypt failed: {e}")))?;
    let text = std::str::from_utf8(plain)
        .map_err(|e| TransportError::Crypto(format!("plaintext not UTF-8: {e}")))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// Encrypt a field for local harness round-trips only (never sent to BIR).
/// Matches the official helper: UTF-8 **without** BOM.
pub fn wrap_dispatcher_field(plain_text: &str) -> Result<String, TransportError> {
    // Deterministic-free randomness is not required for a local round-trip; use a
    // fixed non-secret salt/iv derived from process entropy via getrandom.
    let mut salt = [0u8; SALT_LEN];
    let mut iv = [0u8; IV_LEN];
    getrandom(&mut salt)?;
    getrandom(&mut iv)?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    pbkdf2_hmac::<Sha1>(WRAP_PASSPHRASE.as_bytes(), &salt, PBKDF2_ROUNDS, key.as_mut());
    let pt = plain_text.as_bytes();
    let mut buf = vec![0u8; pt.len() + 16];
    buf[..pt.len()].copy_from_slice(pt);
    let ct_len = Aes256CbcEnc::new(key.as_ref().into(), (&iv).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, pt.len())
        .map_err(|e| TransportError::Crypto(format!("encrypt failed: {e}")))?
        .len();
    let mut out = Vec::with_capacity(SALT_LEN + IV_LEN + ct_len);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&buf[..ct_len]);
    Ok(data_encoding::BASE64.encode(&out))
}

fn getrandom(buf: &mut [u8]) -> Result<(), TransportError> {
    use rand::RngExt;
    rand::rng().fill(buf);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct DispatcherResponse {
    mode: String,
    server: String,
    #[serde(rename = "SSLPort")]
    _ssl_port: Option<String>,
    port: String,
    username: String,
    password: String,
}

fn parse_dispatcher_json(body: &str) -> Result<DispatcherResponse, TransportError> {
    // Official body uses single quotes; normalize to JSON.
    let normalized = body.trim().replace('\'', "\"");
    serde_json::from_str(&normalized)
        .map_err(|e| TransportError::Dispatcher(format!("bad dispatcher JSON: {e}")))
}

async fn fetch_one(
    client: &Client,
    base: &str,
    tin9: &str,
    form_type: &str,
) -> Result<DispatcherResponse, TransportError> {
    let url = format!("{base}?t={tin9}&f={form_type}&v={DISPATCHER_CLIENT_VERSION}");
    let resp = client.get(&url).timeout(Duration::from_secs(8)).send().await?;
    if !resp.status().is_success() {
        return Err(TransportError::Dispatcher(format!("{base} HTTP {}", resp.status())));
    }
    let body = resp.text().await?;
    if !body.contains("mode") {
        return Err(TransportError::Dispatcher(format!("{base} returned no mode")));
    }
    parse_dispatcher_json(&body)
}

/// Resolve a live SFTP endpoint via `tinDispatcherSFTP.php` (primary→backup),
/// unwrapping the encrypted connection fields. Keyed on the first 9 TIN digits.
pub async fn fetch_sftp_endpoint(tin: &str, form_type: &str) -> Result<SftpEndpoint, TransportError> {
    let digits: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 9 {
        return Err(TransportError::Dispatcher("TIN needs at least 9 digits".into()));
    }
    let tin9 = &digits[..9];
    let client = Client::builder().build()?;
    let parsed = match fetch_one(&client, DISPATCHER_PRIMARY, tin9, form_type).await {
        Ok(p) if p.mode != "0" => p,
        _ => fetch_one(&client, DISPATCHER_BACKUP, tin9, form_type).await?,
    };
    if parsed.mode == "0" {
        return Err(TransportError::Dispatcher("dispatcher mode 0 (no web service)".into()));
    }
    Ok(SftpEndpoint {
        host: unwrap_dispatcher_field(&parsed.server)?,
        port: parsed
            .port
            .parse::<u16>()
            .map_err(|e| TransportError::Dispatcher(format!("bad port: {e}")))?,
        username: unwrap_dispatcher_field(&parsed.username)?,
        password: Zeroizing::new(unwrap_dispatcher_field(&parsed.password)?),
        remote_folder: form_type.to_string(),
    })
}

/// Build "/<folder>/<basename>" exactly as the shim's `"/"+folder+"/"` + PutFiles.
fn remote_path(folder: &str, filename: &str) -> String {
    let file = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);
    let folder = folder.trim_matches('/');
    if folder.is_empty() {
        format!("/{file}")
    } else {
        format!("/{folder}/{file}")
    }
}

/// Host-key handler that accepts any key — matches WinSCP
/// `SshHostKeyPolicy = GiveUpSecurityAndAcceptAny` in the official `ebfSFTP.exe`.
struct AcceptAnyHostKey;

impl russh::client::Handler for AcceptAnyHostKey {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Perform the SSH/SFTP upload (connect → password auth → open sftp → write).
/// Verified end-to-end against a loopback russh server (see evidence scripts).
async fn sftp_put(endpoint: &SftpEndpoint, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
    let remote = remote_path(&endpoint.remote_folder, filename);
    let config = Arc::new(russh::client::Config::default());
    let mut handle = russh::client::connect(config, (endpoint.host.as_str(), endpoint.port), AcceptAnyHostKey)
        .await
        .map_err(|e| TransportError::Ssh(e.to_string()))?;
    let auth = handle
        .authenticate_password(endpoint.username.clone(), endpoint.password.as_str())
        .await
        .map_err(|e| TransportError::Ssh(e.to_string()))?;
    if !auth.success() {
        return Err(TransportError::Ssh("SFTP password authentication failed".into()));
    }
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| TransportError::Ssh(e.to_string()))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| TransportError::Ssh(e.to_string()))?;
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| TransportError::Sftp(e.to_string()))?;
    let mut file = sftp
        .open_with_flags(
            remote.clone(),
            russh_sftp::protocol::OpenFlags::CREATE
                | russh_sftp::protocol::OpenFlags::TRUNCATE
                | russh_sftp::protocol::OpenFlags::WRITE,
        )
        .await
        .map_err(|e| TransportError::Sftp(e.to_string()))?;
    file.write_all(payload).await?;
    file.flush().await?;
    file.shutdown().await?;
    let _ = sftp.close().await;
    Ok(())
}

/// Upload one already-encrypted IAF payload under its form-type folder.
#[async_trait::async_trait]
pub trait SubmissionTransport: Send + Sync {
    async fn submit(&self, form_type: &str, filename: &str, payload: &[u8]) -> Result<(), TransportError>;
}

/// Production transport: resolve the endpoint from the dispatcher, then upload.
pub struct DispatcherSftpTransport {
    pub tin: String,
}

#[async_trait::async_trait]
impl SubmissionTransport for DispatcherSftpTransport {
    async fn submit(&self, form_type: &str, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        let endpoint = fetch_sftp_endpoint(&self.tin, form_type).await?;
        sftp_put(&endpoint, filename, payload).await
    }
}

/// Externally-configured transport (harness / self-host). No secrets embedded.
pub struct StaticSftpTransport {
    pub endpoint: SftpEndpoint,
}

#[async_trait::async_trait]
impl SubmissionTransport for StaticSftpTransport {
    async fn submit(&self, _form_type: &str, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        sftp_put(&self.endpoint, filename, payload).await
    }
}

/// In-memory transport for tests. Records every submit; no network.
#[derive(Default)]
pub struct MockTransport {
    pub calls: std::sync::Mutex<Vec<(String, String, usize)>>,
}

#[async_trait::async_trait]
impl SubmissionTransport for MockTransport {
    async fn submit(&self, form_type: &str, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push((form_type.to_string(), filename.to_string(), payload.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Validated non-production vector: the client's own DEV/UAT `srv` blob.
    #[test]
    fn unwraps_devuat_srv_vector() {
        let blob = "25s+rBZx/AO+YuDjzPzIBx81hOVx4fhdnOHNysmXar3RpmRnduhtuxoasmUEANldVjKUeaebvHyefVvj5aJQ/+hCjBdF+xwd7GFdWWSbqL8=";
        let host = unwrap_dispatcher_field(blob).unwrap();
        assert_eq!(host, "ftp2.birgovph.com");
        assert_eq!(host.len(), 17); // no BOM
    }

    #[test]
    fn wrap_unwrap_roundtrip_no_bom() {
        let w = wrap_dispatcher_field("sftp.example.test").unwrap();
        assert_eq!(unwrap_dispatcher_field(&w).unwrap(), "sftp.example.test");
    }

    #[test]
    fn remote_path_matches_shim_layout() {
        assert_eq!(
            remote_path(
                "1601Cv2018",
                r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#a@b.com#.xml"
            ),
            "/1601Cv2018/00000000000000-1601Cv2018-092026#a@b.com#.xml"
        );
        assert_eq!(remote_path("/", "file.xml"), "/file.xml");
    }

    #[test]
    fn parses_single_quoted_dispatcher_body() {
        let body = "{'mode':'2','server':'x','SSLPort':'990','port':'22','username':'u','password':'p'}";
        let p = parse_dispatcher_json(body).unwrap();
        assert_eq!((p.mode.as_str(), p.port.as_str()), ("2", "22"));
    }

    #[tokio::test]
    async fn mock_records_calls() {
        let t = MockTransport::default();
        t.submit("1601Cv2018", "00000000000000-1601Cv2018-092026#a@b.com#.xml", b"xx")
            .await
            .unwrap();
        let calls = t.calls.lock().unwrap();
        assert_eq!(calls[0].2, 2);
    }
}
