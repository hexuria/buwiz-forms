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
use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;
use russh::Disconnect;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use serde::Deserialize;
use sha1::Sha1;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::info;
use zeroize::Zeroizing;

const DISPATCHER_PRIMARY: &str =
    "http://birgovph.com/tinDispatcherSFTP.php";
const DISPATCHER_BACKUP: &str =
    "http://ws2.birgovph.com/tinDispatcherSFTP.php";
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
}

impl SftpEndpoint {
    pub fn from_env() -> Result<Self, TransportError> {
        fn required(name: &str) -> Result<String, TransportError> {
            std::env::var(name).map_err(|_| {
                TransportError::Dispatcher(format!("missing {name} for SFTP harness/config"))
            })
        }
        let port = required("TEST_SFTP_PORT")?
            .parse::<u16>()
            .map_err(|error| TransportError::Dispatcher(format!("invalid TEST_SFTP_PORT: {error}")))?;
        Ok(Self {
            host: required("TEST_SFTP_HOST")?,
            port,
            username: required("TEST_SFTP_USER")?,
            password: Zeroizing::new(required("TEST_SFTP_PASSWORD")?),
            remote_folder: std::env::var("TEST_SFTP_FOLDER").unwrap_or_else(|_| "/".to_string()),
        })
    }

    pub fn with_folder(mut self, folder: impl Into<String>) -> Self {
        self.remote_folder = folder.into();
        self
    }
}

#[derive(Debug, Deserialize)]
struct DispatcherResponse {
    mode: String,
    server: String,
    #[serde(rename = "SSLPort")]
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
        .map_err(|error| TransportError::Crypto(format!("invalid dispatcher ciphertext: {error}")))?;
    if raw.len() < SALT_LEN + IV_LEN + 16 {
        return Err(TransportError::Crypto(
            "dispatcher ciphertext is shorter than salt+iv+one block".into(),
        ));
    }
    let (salt, rest) = raw.split_at(SALT_LEN);
    let (iv, cipher) = rest.split_at(IV_LEN);
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha1>(DISPATCHER_WRAP_PASSPHRASE.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    let mut buf = cipher.to_vec();
    let plain = Aes256CbcDec::new((&key).into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|error| TransportError::Crypto(format!("dispatcher decrypt failed: {error}")))?;
    let text = std::str::from_utf8(plain)
        .map_err(|error| TransportError::Crypto(format!("dispatcher plaintext was not UTF-8: {error}")))?;
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
    pbkdf2_hmac::<Sha1>(DISPATCHER_WRAP_PASSPHRASE.as_bytes(), &salt, PBKDF2_ROUNDS, &mut key);
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
    let url = format!(
        "{base}?t={tin}&f={form_type}&v={DISPATCHER_CLIENT_VERSION}"
    );
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(8))
        .send()
        .await?;
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

/// Ask the official dispatcher for SFTP connection fields and unwrap them.
pub async fn fetch_sftp_endpoint(tin: &str, form_type: &str) -> Result<SftpEndpoint, TransportError> {
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
    let parsed = match fetch_dispatcher(&client, DISPATCHER_PRIMARY, dispatcher_tin, form_type).await {
        Ok(parsed) if parsed.mode != "0" => parsed,
        Ok(_) | Err(_) => fetch_dispatcher(&client, DISPATCHER_BACKUP, dispatcher_tin, form_type).await?,
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
    let _ssl_port = parsed.ssl_port;
    Ok(SftpEndpoint {
        host,
        port,
        username,
        password: Zeroizing::new(password),
        remote_folder: form_type.to_string(),
    })
}

fn remote_path(folder: &str, filename: &str) -> String {
    let file = Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(filename);
    let folder = folder.trim_matches('/');
    if folder.is_empty() {
        format!("/{file}")
    } else {
        format!("/{folder}/{file}")
    }
}

struct AcceptAnyHostKey;

impl client::Handler for AcceptAnyHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // Official ebfSFTP.exe sets SshHostKeyPolicy = GiveUpSecurityAndAcceptAny.
        Ok(true)
    }
}

/// Prepared SFTP session that has not yet uploaded the IAF file.
pub(crate) struct IafSftpSession {
    handle: Option<Handle<AcceptAnyHostKey>>,
    remote_folder: String,
}

impl IafSftpSession {
    pub async fn store(mut self, filename: &str, payload: &[u8]) -> Result<(), TransportError> {
        let handle = self
            .handle
            .take()
            .ok_or_else(|| TransportError::Sftp("SFTP session was already consumed".into()))?;
        let path = remote_path(&self.remote_folder, filename);
        info!("Transmitting payload: {}", path);
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
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
        info!("Transmission complete: {}", path);
        Ok(())
    }

    /// Authenticate and open SFTP without uploading. Used by the live-connect probe.
    pub async fn probe(mut self) -> Result<String, TransportError> {
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
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
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
    let endpoint = if std::env::var("TEST_SFTP_HOST").is_ok() {
        SftpEndpoint::from_env()?.with_folder(form_type)
    } else {
        fetch_sftp_endpoint(tin, form_type).await?
    };
    open_iaf_session_with_endpoint(form_type, endpoint).await
}

/// Open SFTP using an already-resolved endpoint. Used by the local harness.
pub(crate) async fn open_iaf_session_with_endpoint(
    form_type: &str,
    endpoint: SftpEndpoint,
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
        AcceptAnyHostKey,
    )
    .await
    .map_err(|error| TransportError::Ssh(error.to_string()))?;
    let auth = handle
        .authenticate_password(endpoint.username.clone(), endpoint.password.as_str())
        .await
        .map_err(|error| TransportError::Ssh(error.to_string()))?;
    if !auth.success() {
        return Err(TransportError::Ssh("SFTP password authentication failed".into()));
    }
    Ok(IafSftpSession {
        handle: Some(handle),
        remote_folder: if endpoint.remote_folder.is_empty() {
            form_type.to_string()
        } else {
            endpoint.remote_folder
        },
    })
}

/// Uploads an encrypted IAF file to the BIR SFTP server.
pub(crate) async fn submit_iaf(
    form_type: &str,
    tin: &str,
    filename: &str,
    payload: &[u8],
) -> Result<(), TransportError> {
    let session = open_iaf_session(form_type, tin).await?;
    session.store(filename, payload).await
}

/// Authenticate and open SFTP without uploading. Used by the live-connect probe.
pub async fn probe_iaf_endpoint(
    form_type: &str,
    endpoint: SftpEndpoint,
) -> Result<String, TransportError> {
    let session = open_iaf_session_with_endpoint(form_type, endpoint).await?;
    session.probe().await
}

/// Direct upload helper for the local harness. Never talks to BIR unless the
/// supplied endpoint does.
pub async fn submit_iaf_with_endpoint(
    form_type: &str,
    filename: &str,
    payload: &[u8],
    endpoint: SftpEndpoint,
) -> Result<(), TransportError> {
    let session = open_iaf_session_with_endpoint(form_type, endpoint).await?;
    session.store(filename, payload).await
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
            remote_path("1601Cv2018", r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#a@b.com#.xml"),
            "/1601Cv2018/00000000000000-1601Cv2018-092026#a@b.com#.xml"
        );
        assert_eq!(remote_path("/", "file.xml"), "/file.xml");
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
}
