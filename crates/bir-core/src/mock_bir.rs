//! Local mock of the BIR SFTP endpoint, for testing a whole submission
//! without sending anything to the BIR.
//!
//! Grew out of the in-process russh loopback from PR #42's evidence harness.
//! It speaks the exchange `transport::IafSftpSession` performs against a
//! `BIR_SFTP_*` lab host: SSH with password authentication, the `sftp`
//! subsystem, `OPEN /{form folder}/{IAF basename}` with
//! `CREATE | TRUNCATE | WRITE`, `WRITE` at increasing offsets, `CLOSE`, then
//! a disconnect. `REALPATH` answers the `sftp_harness --live-connect` probe.
//! The client lists, stats and renames nothing, so neither does this server;
//! any other request is logged and refused.
//!
//! Compiled only for tests and with the non-default `mock-bir` feature (the
//! `mock_bir` binary). The shipped app never enables that feature, so no
//! listening code is linked into it. It binds loopback addresses only.
//!
//! Every stored upload is written under `{root}/uploads/{folder}/{basename}`
//! and gets a confirmation message shaped like BIR's under
//! `{root}/receipts/{BIR file name}.eml`. `{root}/mock-bir.control` (when it
//! exists) is read at every new connection and overrides the start-up
//! behaviour; see [`Behaviour::parse_control`].

use chrono::{DateTime, FixedOffset, Utc};
use russh::keys::PrivateKey;
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::server::{Auth, Msg, Session};
use russh::{Channel, ChannelId, MethodKind, MethodSet};
use russh_sftp::protocol::{
    File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Default listening port of the `mock_bir` binary.
pub const DEFAULT_PORT: u16 = 2222;
/// Obvious fake credentials. The mock checks them like a real host would.
pub const DEFAULT_USERNAME: &str = "mock-filer";
pub const DEFAULT_PASSWORD: &str = "mock-password-not-a-bir-secret";
/// File in the mock's folder that changes its behaviour without a restart.
pub const CONTROL_FILE: &str = "mock-bir.control";
/// Placeholder sender of the mock confirmation messages.
pub const MOCK_SENDER: &str = "mock-bir@localhost.invalid";

/// An injected failure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Failure {
    #[default]
    None,
    /// Reject the password even when it is right.
    RefuseLogin,
    /// Log in, then refuse the `OPEN`: no byte is stored.
    FailBeforeStore,
    /// Store the whole file, then answer the `CLOSE` with a failure.
    FailAfterStore,
}

impl Failure {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "none" | "" => Some(Self::None),
            "refuse-login" => Some(Self::RefuseLogin),
            "fail-before-store" => Some(Self::FailBeforeStore),
            "fail-after-store" => Some(Self::FailAfterStore),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RefuseLogin => "refuse-login",
            Self::FailBeforeStore => "fail-before-store",
            Self::FailAfterStore => "fail-after-store",
        }
    }
}

/// Where a configured delay is spent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DelayAt {
    /// Before answering the password (the app has not claimed the return yet).
    #[default]
    Login,
    /// Before answering the `OPEN` (claimed, nothing stored yet).
    Open,
    /// After storing the file, before answering the `CLOSE`.
    Close,
}

impl DelayAt {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "login" => Some(Self::Login),
            "open" => Some(Self::Open),
            "close" => Some(Self::Close),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Open => "open",
            Self::Close => "close",
        }
    }
}

/// What the mock does on a connection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Behaviour {
    pub failure: Failure,
    pub delay: Duration,
    pub delay_at: DelayAt,
}

impl Behaviour {
    /// Parse a control file over `base`. One `key=value` per line; `#`
    /// starts a comment. Keys: `fail` (a [`Failure`] name), `delay`
    /// (seconds), `delay_at` (`login`, `open` or `close`).
    pub fn parse_control(text: &str, base: Behaviour) -> Result<Behaviour, String> {
        let mut behaviour = base;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("not key=value: {line}"));
            };
            let value = value.trim();
            match key.trim() {
                "fail" => {
                    behaviour.failure = Failure::parse(value)
                        .ok_or_else(|| format!("unknown fail mode: {value}"))?
                }
                "delay" => {
                    let secs: u64 = value
                        .parse()
                        .map_err(|_| format!("delay is not whole seconds: {value}"))?;
                    behaviour.delay = Duration::from_secs(secs);
                }
                "delay_at" => {
                    behaviour.delay_at =
                        DelayAt::parse(value).ok_or_else(|| format!("unknown delay_at: {value}"))?
                }
                other => return Err(format!("unknown key: {other}")),
            }
        }
        Ok(behaviour)
    }

    pub fn describe(&self) -> String {
        if self.delay.is_zero() {
            format!("fail={}", self.failure.as_str())
        } else {
            format!(
                "fail={} delay={}s at {}",
                self.failure.as_str(),
                self.delay.as_secs(),
                self.delay_at.as_str()
            )
        }
    }
}

/// How to start a [`MockServer`].
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Must be a loopback address. Port 0 picks a free port.
    pub bind: SocketAddr,
    /// Uploads, receipts, the host key and the control file live here.
    pub root: PathBuf,
    pub username: String,
    pub password: String,
    pub behaviour: Behaviour,
    /// Print each event line to stdout as well as recording it.
    pub echo: bool,
}

impl MockConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT)),
            root: root.into(),
            username: DEFAULT_USERNAME.to_string(),
            password: DEFAULT_PASSWORD.to_string(),
            behaviour: Behaviour::default(),
            echo: false,
        }
    }
}

type Events = Arc<Mutex<Vec<String>>>;
type Received = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Shared {
    root: PathBuf,
    username: String,
    password: String,
    behaviour: Behaviour,
    echo: bool,
    events: Events,
    received: Received,
    next_connection: Mutex<u64>,
}

impl Shared {
    fn emit(&self, connection: u64, message: &str) {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("{now} #{connection} {message}");
        if self.echo {
            println!("[mock-bir] {line}");
        }
        if let Ok(mut events) = self.events.lock() {
            events.push(line);
        }
    }

    /// The control file over the start-up behaviour. A broken control file
    /// is reported and ignored.
    fn behaviour_now(&self, connection: u64) -> Behaviour {
        let path = self.root.join(CONTROL_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => match Behaviour::parse_control(&text, self.behaviour) {
                Ok(behaviour) => behaviour,
                Err(error) => {
                    self.emit(
                        connection,
                        &format!("ERROR control file ignored ({}): {error}", path.display()),
                    );
                    self.behaviour
                }
            },
            Err(_) => self.behaviour,
        }
    }
}

/// A running mock. Dropping it stops the listener.
pub struct MockServer {
    addr: SocketAddr,
    host_key_sha256: String,
    shared: Arc<Shared>,
    accept: tokio::task::JoinHandle<()>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

impl MockServer {
    /// Bind and start accepting. Refuses any non-loopback address.
    pub async fn start(config: MockConfig) -> std::io::Result<Self> {
        if !config.bind.ip().is_loopback() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "the mock BIR server only listens on loopback, not {}",
                    config.bind.ip()
                ),
            ));
        }
        std::fs::create_dir_all(config.root.join("uploads"))?;
        std::fs::create_dir_all(config.root.join("receipts"))?;
        let key = load_or_create_host_key(&config.root)?;
        let host_key_sha256 = key
            .public_key()
            .to_bytes()
            .map(|bytes| hex::encode(Sha256::digest(bytes)))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let listener = tokio::net::TcpListener::bind(config.bind).await?;
        let addr = listener.local_addr()?;
        let ssh_config = Arc::new(russh::server::Config {
            keys: vec![key],
            methods: MethodSet::from(&[MethodKind::Password][..]),
            auth_rejection_time: Duration::from_millis(200),
            ..Default::default()
        });
        let shared = Arc::new(Shared {
            root: config.root,
            username: config.username,
            password: config.password,
            behaviour: config.behaviour,
            echo: config.echo,
            events: Arc::new(Mutex::new(Vec::new())),
            received: Arc::new(Mutex::new(HashMap::new())),
            next_connection: Mutex::new(0),
        });
        let accept_shared = shared.clone();
        let accept = tokio::spawn(async move {
            loop {
                let (socket, peer) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(error) => {
                        accept_shared.emit(0, &format!("ERROR accept failed: {error}"));
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                };
                let connection = {
                    let mut next = accept_shared
                        .next_connection
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *next += 1;
                    *next
                };
                let behaviour = accept_shared.behaviour_now(connection);
                accept_shared.emit(
                    connection,
                    &format!("connection from {peer} ({})", behaviour.describe()),
                );
                let handler = SshSession {
                    shared: accept_shared.clone(),
                    connection,
                    behaviour,
                    channels: HashMap::new(),
                };
                let cfg = ssh_config.clone();
                let session_shared = accept_shared.clone();
                tokio::spawn(async move {
                    let result = match russh::server::run_stream(cfg, socket, handler).await {
                        Ok(running) => running.await,
                        Err(error) => Err(error),
                    };
                    match result {
                        Ok(()) | Err(russh::Error::Disconnect) => {}
                        // The client hanging up after its own disconnect.
                        Err(russh::Error::IO(error))
                            if error.kind() == std::io::ErrorKind::UnexpectedEof => {}
                        Err(error) => {
                            session_shared.emit(connection, &format!("ERROR ssh: {error}"))
                        }
                    }
                    session_shared.emit(connection, "connection closed");
                });
            }
        });
        Ok(Self {
            addr,
            host_key_sha256,
            shared,
            accept,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn root(&self) -> &Path {
        &self.shared.root
    }

    /// SHA-256 of the host key, in the form `BIR_SFTP_HOST_KEY_SHA256` takes.
    pub fn host_key_sha256(&self) -> &str {
        &self.host_key_sha256
    }

    /// Every event line so far.
    pub fn events(&self) -> Vec<String> {
        self.shared
            .events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    /// Bytes stored for a remote path as the client sent it
    /// (`/1601Cv2018/{basename}`).
    pub fn received(&self, remote_path: &str) -> Option<Vec<u8>> {
        self.shared
            .received
            .lock()
            .ok()
            .and_then(|received| received.get(remote_path).cloned())
    }

    /// Where an upload for `remote_path` lands on disk.
    pub fn upload_path(&self, remote_path: &str) -> Option<PathBuf> {
        upload_path(&self.shared.root, remote_path)
    }

    /// Where the confirmation message for an uploaded IAF basename lands.
    pub fn receipt_path(&self, iaf_basename: &str) -> PathBuf {
        self.shared
            .root
            .join("receipts")
            .join(format!("{}.eml", bir_receipt_filename(iaf_basename)))
    }
}

/// The host key persists in `{root}/host_key.seed` so a pinned fingerprint
/// survives restarts.
fn load_or_create_host_key(root: &Path) -> std::io::Result<PrivateKey> {
    let path = root.join("host_key.seed");
    let seed: [u8; 32] = match std::fs::read_to_string(&path) {
        Ok(text) => hex::decode(text.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{} is not a 32-byte hex seed", path.display()),
                )
            })?,
        Err(_) => {
            let seed: [u8; 32] = rand::random();
            std::fs::write(&path, hex::encode(seed))?;
            seed
        }
    };
    PrivateKey::new(
        KeypairData::Ed25519(Ed25519Keypair::from_seed(&seed)),
        "mock-bir",
    )
    .map_err(|error| std::io::Error::other(error.to_string()))
}

/// `{root}/uploads/...` for a remote path made of plain names only.
fn upload_path(root: &Path, remote_path: &str) -> Option<PathBuf> {
    let relative = Path::new(remote_path.trim_start_matches('/'));
    let mut path = root.join("uploads");
    let mut parts = 0;
    for component in relative.components() {
        match component {
            Component::Normal(name) if !name.to_string_lossy().contains('\\') => {
                path.push(name);
                parts += 1;
            }
            _ => return None,
        }
    }
    (parts > 0).then_some(path)
}

/// The name BIR's confirmation uses: the IAF stem without `#email#`.
pub fn bir_receipt_filename(iaf_basename: &str) -> String {
    let stem = iaf_basename.strip_suffix(".xml").unwrap_or(iaf_basename);
    let base = stem.split('#').next().unwrap_or(stem);
    format!("{base}.xml")
}

/// A confirmation message shaped like BIR's (the lines
/// `receipt::parse_bir_receipt_email` reads), stamped in Manila time.
/// It comes from [`MOCK_SENDER`], not BIR's address.
pub fn receipt_eml(iaf_basename: &str, received_at: DateTime<Utc>) -> String {
    let manila = FixedOffset::east_opt(8 * 3600).expect("UTC+08:00 is a valid offset");
    let at = received_at.with_timezone(&manila);
    let file = bir_receipt_filename(iaf_basename);
    let to = iaf_basename
        .split('#')
        .nth(1)
        .filter(|email| email.contains('@'))
        .unwrap_or("taxpayer@localhost.invalid");
    let lines = [
        format!("From: Mock BIR <{MOCK_SENDER}>"),
        format!("To: <{to}>"),
        format!("Subject: [MOCK] Confirmation of receipt {file}"),
        format!("Date: {}", at.to_rfc2822()),
        format!(
            "Message-ID: <{}@mock-bir.localhost.invalid>",
            uuid::Uuid::new_v4()
        ),
        "MIME-Version: 1.0".to_string(),
        "Content-Type: text/plain; charset=utf-8".to_string(),
        "Content-Transfer-Encoding: 8bit".to_string(),
        "X-Mock-BIR: written by the local mock; nothing was sent to the BIR".to_string(),
        String::new(),
        "This confirms receipt of your submission with the following details subject to validation by BIR:".to_string(),
        String::new(),
        format!("File name: {file}"),
        format!("Date received by BIR: {}", at.format("%-d %B %Y")),
        format!("Time received by BIR: {}", at.format("%-I:%M %p")),
        "Penalties may be imposed for any violation of the provisions of the NIRC and issuances thereof.".to_string(),
        String::new(),
        "This message was written by the local mock BIR server. Nothing was sent to the BIR.".to_string(),
        String::new(),
    ];
    lines.join("\r\n")
}

struct SshSession {
    shared: Arc<Shared>,
    connection: u64,
    behaviour: Behaviour,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

fn reject() -> Auth {
    Auth::Reject {
        proceed_with_methods: None,
        partial_success: false,
    }
}

impl russh::server::Handler for SshSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let b = self.behaviour;
        if b.delay_at == DelayAt::Login && !b.delay.is_zero() {
            self.shared.emit(
                self.connection,
                &format!(
                    "slow: waiting {}s before answering login",
                    b.delay.as_secs()
                ),
            );
            tokio::time::sleep(b.delay).await;
        }
        if b.failure == Failure::RefuseLogin {
            self.shared.emit(
                self.connection,
                &format!("ERROR login refused (injected refuse-login) user={user}"),
            );
            return Ok(reject());
        }
        if user != self.shared.username || password != self.shared.password {
            self.shared.emit(
                self.connection,
                &format!("ERROR login refused: wrong username or password user={user}"),
            );
            return Ok(reject());
        }
        self.shared
            .emit(self.connection, &format!("login ok user={user}"));
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match (name, self.channels.remove(&channel)) {
            ("sftp", Some(ch)) => {
                session.channel_success(channel)?;
                let sftp = SftpHandler {
                    shared: self.shared.clone(),
                    connection: self.connection,
                    behaviour: self.behaviour,
                    open: HashMap::new(),
                    next_handle: 0,
                };
                russh_sftp::server::run(ch.into_stream(), sftp).await;
            }
            _ => {
                self.shared.emit(
                    self.connection,
                    &format!("ERROR refused subsystem request {name:?}"),
                );
                session.channel_failure(channel)?;
            }
        }
        Ok(())
    }
}

struct OpenFile {
    remote: String,
    local: PathBuf,
    data: Vec<u8>,
}

struct SftpHandler {
    shared: Arc<Shared>,
    connection: u64,
    behaviour: Behaviour,
    open: HashMap<String, OpenFile>,
    next_handle: u64,
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

impl SftpHandler {
    fn emit(&self, message: &str) {
        self.shared.emit(self.connection, message);
    }

    async fn maybe_delay(&self, at: DelayAt, what: &str) {
        if self.behaviour.delay_at == at && !self.behaviour.delay.is_zero() {
            self.emit(&format!(
                "slow: waiting {}s before answering {what}",
                self.behaviour.delay.as_secs()
            ));
            tokio::time::sleep(self.behaviour.delay).await;
        }
    }

    /// Write the upload, record it and drop its confirmation message.
    fn store(&self, file: &OpenFile) -> std::io::Result<()> {
        if let Some(parent) = file.local.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let name = file
            .local
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let partial = file.local.with_file_name(format!(".{name}.part"));
        std::fs::write(&partial, &file.data)?;
        std::fs::rename(&partial, &file.local)?;
        if let Ok(mut received) = self.shared.received.lock() {
            received.insert(file.remote.clone(), file.data.clone());
        }
        self.emit(&format!(
            "file received {} ({} bytes, sha256 {}) -> {}",
            file.remote,
            file.data.len(),
            hex::encode(Sha256::digest(&file.data)),
            file.local.display()
        ));
        let receipt = self
            .shared
            .root
            .join("receipts")
            .join(format!("{}.eml", bir_receipt_filename(&name)));
        std::fs::write(&receipt, receipt_eml(&name, Utc::now()))?;
        self.emit(&format!("confirmation written {}", receipt.display()));
        Ok(())
    }
}

impl russh_sftp::server::Handler for SftpHandler {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        self.emit("ERROR unsupported SFTP request refused (the app's client never sends it)");
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        self.emit(&format!("sftp started (client version {version})"));
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        self.emit(&format!("realpath {path}"));
        Ok(Name {
            id,
            files: vec![File::new(
                if path == "." { "/".to_string() } else { path },
                FileAttributes::default(),
            )],
        })
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        self.emit(&format!("open {filename} ({pflags:?})"));
        self.maybe_delay(DelayAt::Open, "open").await;
        if !pflags.contains(OpenFlags::WRITE) {
            self.emit("ERROR only uploads are supported; open refused");
            return Err(StatusCode::OpUnsupported);
        }
        let Some(local) = upload_path(&self.shared.root, &filename) else {
            self.emit(&format!("ERROR refused path {filename:?}"));
            return Err(StatusCode::PermissionDenied);
        };
        if self.behaviour.failure == Failure::FailBeforeStore {
            self.emit(&format!(
                "ERROR upload refused (injected fail-before-store); nothing stored for {filename}"
            ));
            return Err(StatusCode::Failure);
        }
        self.next_handle += 1;
        let handle = format!("h{}", self.next_handle);
        self.open.insert(
            handle.clone(),
            OpenFile {
                remote: filename,
                local,
                data: Vec::new(),
            },
        );
        Ok(Handle { id, handle })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(file) = self.open.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        let start = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        let end = start + data.len();
        if file.data.len() < end {
            file.data.resize(end, 0);
        }
        file.data[start..end].copy_from_slice(&data);
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        let Some(file) = self.open.remove(&handle) else {
            return Ok(ok(id));
        };
        if let Err(error) = self.store(&file) {
            self.emit(&format!("ERROR could not store {}: {error}", file.remote));
            return Err(StatusCode::Failure);
        }
        self.maybe_delay(DelayAt::Close, "close").await;
        if self.behaviour.failure == Failure::FailAfterStore {
            self.emit(&format!(
                "ERROR close answered with a failure (injected fail-after-store); {} IS stored",
                file.remote
            ));
            return Err(StatusCode::Failure);
        }
        Ok(ok(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{
        HostKeyPolicy, SftpEndpoint, TransportError, iaf_basename, submit_iaf_with_endpoint,
    };
    use zeroize::Zeroizing;

    async fn spawn_loopback() -> (MockServer, tempfile::TempDir) {
        let root = tempfile::tempdir().expect("temp dir");
        let mut config = MockConfig::new(root.path());
        config.bind = SocketAddr::from(([127, 0, 0, 1], 0));
        config.username = "lab".into();
        config.password = "lab-not-a-bir-secret".into();
        (MockServer::start(config).await.expect("mock"), root)
    }

    fn endpoint(server: &MockServer, policy: HostKeyPolicy) -> SftpEndpoint {
        SftpEndpoint {
            host: "127.0.0.1".into(),
            port: server.addr().port(),
            username: "lab".into(),
            password: Zeroizing::new("lab-not-a-bir-secret".into()),
            remote_folder: "1601Cv2018".into(),
            host_key_policy: policy,
        }
    }

    #[tokio::test]
    async fn loopback_submit_iaf_with_endpoint_stores_fixture_basename() {
        let (server, _root) = spawn_loopback().await;
        let filename = r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml";
        let payload = b"ENCRYPTED-IAF-DUMMY-PAYLOAD-\x00\x01\x02\x03";
        submit_iaf_with_endpoint(
            "1601Cv2018",
            filename,
            payload,
            endpoint(&server, HostKeyPolicy::AcceptAnyLab),
        )
        .await
        .expect("loopback PUT");
        let remote = format!("/1601Cv2018/{}", iaf_basename(filename));
        let got = server
            .received(&remote)
            .unwrap_or_else(|| panic!("server missing {remote}, events {:?}", server.events()));
        assert_eq!(got.as_slice(), payload);
        let on_disk = std::fs::read(server.upload_path(&remote).unwrap()).unwrap();
        assert_eq!(on_disk.as_slice(), payload);
    }

    #[tokio::test]
    async fn loopback_pinned_host_key_mismatch_refuses_connect() {
        let (server, _root) = spawn_loopback().await;
        let err = submit_iaf_with_endpoint(
            "1601Cv2018",
            "file.xml",
            b"x",
            endpoint(&server, HostKeyPolicy::PinnedSha256("00".repeat(32))),
        )
        .await
        .expect_err("wrong pin must not PUT");
        assert!(matches!(err, TransportError::Ssh(_)), "{err:?}");
    }

    #[tokio::test]
    async fn the_printed_fingerprint_is_the_pin_the_client_accepts() {
        let (server, _root) = spawn_loopback().await;
        let pin = server.host_key_sha256().to_string();
        submit_iaf_with_endpoint(
            "1601Cv2018",
            "file.xml",
            b"x",
            endpoint(&server, HostKeyPolicy::PinnedSha256(pin)),
        )
        .await
        .expect("the mock's own fingerprint must pass the client's pin check");
    }

    #[tokio::test]
    async fn the_mock_refuses_a_non_loopback_address() {
        let root = tempfile::tempdir().unwrap();
        for ip in [[0, 0, 0, 0], [192, 168, 1, 10]] {
            let mut config = MockConfig::new(root.path());
            config.bind = SocketAddr::from((ip, 0));
            let error = MockServer::start(config)
                .await
                .err()
                .expect("a non-loopback bind must be refused");
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn upload_paths_stay_inside_the_uploads_folder() {
        let root = Path::new("/mock");
        assert_eq!(
            upload_path(root, "/1601Cv2018/a#b@c.d#.xml"),
            Some(PathBuf::from("/mock/uploads/1601Cv2018/a#b@c.d#.xml"))
        );
        assert_eq!(upload_path(root, "/../etc/passwd"), None);
        assert_eq!(upload_path(root, "/1601Cv2018/../../x"), None);
        assert_eq!(upload_path(root, "/"), None);
        assert_eq!(upload_path(root, r"/a\..\b"), None);
    }

    #[test]
    fn control_file_overrides_the_start_up_behaviour() {
        let base = Behaviour {
            failure: Failure::FailAfterStore,
            ..Behaviour::default()
        };
        let parsed = Behaviour::parse_control(
            "# test\nfail = refuse-login\ndelay=3\ndelay_at=close\n",
            base,
        )
        .unwrap();
        assert_eq!(parsed.failure, Failure::RefuseLogin);
        assert_eq!(parsed.delay, Duration::from_secs(3));
        assert_eq!(parsed.delay_at, DelayAt::Close);
        assert_eq!(Behaviour::parse_control("", base).unwrap(), base);
        assert!(Behaviour::parse_control("fail=sometimes", base).is_err());
        assert!(Behaviour::parse_control("speed=fast", base).is_err());
    }

    #[test]
    fn the_mock_receipt_parses_with_the_real_parser() {
        let at = DateTime::parse_from_rfc3339("2026-11-13T07:13:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let eml = receipt_eml(
            "00012345300000-1601Cv2018-102026#juana@example.com#.xml",
            at,
        );
        let parsed = mail_parser::MessageParser::default()
            .parse(eml.as_bytes())
            .expect("the mock receipt is a valid message");
        let text = parsed.body_text(0).unwrap().into_owned();
        let receipt = crate::receipt::parse_bir_receipt_email(&text, None).unwrap();
        assert_eq!(receipt.filename, "00012345300000-1601Cv2018-102026.xml");
        assert_eq!(receipt.date_received.to_string(), "2026-11-13");
        assert_eq!(receipt.time_received.to_string(), "15:13:00");
        assert!(eml.contains("To: <juana@example.com>"));
        assert!(eml.contains("Time received by BIR: 3:13 PM"));
        assert!(eml.contains(&format!("From: Mock BIR <{MOCK_SENDER}>")));
    }
}
