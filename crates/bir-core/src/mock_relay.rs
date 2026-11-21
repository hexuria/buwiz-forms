//! Local mock of the Buwiz relay (docs/relay.md): the phone app's way out, loopback only.
//!
//! `POST /v1/submissions` takes one encrypted IAF file, checks it, keeps it under `inbox/`, writes a receipt under
//! `receipts/`, and, when a mock BIR server's `app.env` is given ([`MockBirTarget`]), hands the file on to that
//! server over SFTP. `GET /v1/health` says it is up. There is no other route and no way to name another host: the
//! hand-off target is read from the mock BIR server's own file and must be loopback.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zeroize::Zeroizing;

use crate::transport::{HostKeyPolicy, ResolvedSftpTarget, SftpEndpoint, SftpEndpointSource};

/// The default port of `just mock-relay`.
pub const DEFAULT_PORT: u16 = 8787;
/// Largest body accepted (an IAF file is a few kilobytes).
const MAX_BODY: usize = 8 * 1024 * 1024;
/// Largest request head accepted.
const MAX_HEAD: usize = 16 * 1024;

/// What the relay does with an accepted file besides keeping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RelayFailure {
    /// Accept (202), and hand on to the mock BIR server when there is one.
    #[default]
    None,
    /// Refuse every submission (422): the app sees a rejection.
    Reject,
    /// Fail every submission (503): the app sees a transport error and may retry.
    Unavailable,
}

impl RelayFailure {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "none" => Some(Self::None),
            "reject" => Some(Self::Reject),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

/// The mock BIR server the relay hands files on to, from the `app.env` it writes while it runs.
#[derive(Debug, Clone)]
pub struct MockBirTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub host_key_sha256: String,
}

impl MockBirTarget {
    /// Reads `app.env` (`BIR_SFTP_HOST`, `_PORT`, `_USERNAME`, `_PASSWORD`, `_HOST_KEY_SHA256`). Refuses a host that
    /// is not `127.0.0.1` or `::1`.
    pub fn from_app_env(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let value = |key: &str| {
            text.lines()
                .filter_map(|line| line.split_once('='))
                .find(|(name, _)| name.trim() == key)
                .map(|(_, value)| value.trim().to_string())
                .ok_or_else(|| format!("{} has no {key}", path.display()))
        };
        let host = value("BIR_SFTP_HOST")?;
        if host != "127.0.0.1" && host != "::1" {
            return Err(format!(
                "the mock BIR host {host} is not loopback; refusing to hand anything on"
            ));
        }
        Ok(Self {
            host,
            port: value("BIR_SFTP_PORT")?
                .parse()
                .map_err(|_| "BIR_SFTP_PORT is not a port".to_string())?,
            username: value("BIR_SFTP_USERNAME")?,
            password: value("BIR_SFTP_PASSWORD")?,
            host_key_sha256: value("BIR_SFTP_HOST_KEY_SHA256")?,
        })
    }

    fn target(&self, folder: &str) -> ResolvedSftpTarget {
        ResolvedSftpTarget::Live {
            endpoint: SftpEndpoint {
                host: self.host.clone(),
                port: self.port,
                username: self.username.clone(),
                password: Zeroizing::new(self.password.clone()),
                remote_folder: folder.to_string(),
                host_key_policy: HostKeyPolicy::PinnedSha256(self.host_key_sha256.clone()),
            },
            source: SftpEndpointSource::Env,
        }
    }
}

/// How the relay runs.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Where `inbox/` and `receipts/` go.
    pub root: PathBuf,
    pub failure: RelayFailure,
    /// The mock BIR server to hand files on to; `None` keeps them in the inbox only.
    pub mock_bir: Option<MockBirTarget>,
}

/// A running relay.
pub struct MockRelay {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl MockRelay {
    /// Listens on `addr`, which must be loopback.
    pub async fn start(addr: SocketAddr, config: RelayConfig) -> std::io::Result<Self> {
        if !addr.ip().is_loopback() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the mock relay listens on loopback only",
            ));
        }
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let config = Arc::new(config);
        let task = tokio::spawn(async move {
            while let Ok((stream, peer)) = listener.accept().await {
                let config = config.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve(stream, &config).await {
                        println!("[mock-relay] {peer}: {error}");
                    }
                });
            }
        });
        Ok(Self { addr, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The origin to put in `BIR_RELAY_URL`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for MockRelay {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

async fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut buffer = Vec::new();
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > MAX_HEAD {
            return Err("request head too large".into());
        }
        let mut chunk = [0u8; 4096];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed before the request head ended".into());
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| "request head is not UTF-8")?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default().to_string();
    let path = first.next().unwrap_or_default().to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect();
    let length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.parse::<usize>().map_err(|_| "bad content-length"))
        .transpose()?
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err("body too large".into());
    }
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let mut chunk = vec![0u8; (length - body.len()).min(64 * 1024)];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("connection closed before the body ended".into());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    body: serde_json::Value,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        502 => "Bad Gateway",
        _ => "Service Unavailable",
    };
    let body = body.to_string();
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream
        .write_all(body.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream.shutdown().await.map_err(|error| error.to_string())
}

/// A form folder: letters, digits, `.`, `_`, `-`; never a path.
fn plain_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && !value.starts_with('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// An IAF file name (`{TIN}-{form}-{period}#{email}#.xml`): printable ASCII with no path separator, not starting with a dot.
fn file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.starts_with('.')
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '/' | '\\' | ':'))
}

async fn serve(mut stream: TcpStream, config: &RelayConfig) -> Result<(), String> {
    let request = match read_request(&mut stream).await {
        Ok(request) => request,
        // The app's send guard only checks that the port answers: it connects and closes without a request.
        Err(error) if error.starts_with("connection closed before the request head") => {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let (status, body) = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/health") => (
            200,
            serde_json::json!({
                "service": crate::transport::MOCK_RELAY_SERVICE,
                "version": 1,
                "ok": true,
                "mock": true
            }),
        ),
        ("POST", "/v1/submissions") => submit(&request, config).await,
        _ => (
            404,
            serde_json::json!({ "status": "rejected", "error": "no such route" }),
        ),
    };
    println!("[mock-relay] HTTP {status} ({} bytes)", request.body.len());
    respond(&mut stream, status, body).await
}

async fn submit(request: &Request, config: &RelayConfig) -> (u16, serde_json::Value) {
    let reject = |status: u16, error: &str| {
        (
            status,
            serde_json::json!({ "status": "rejected", "error": error }),
        )
    };
    let form = request.header("x-relay-form").unwrap_or_default();
    let filename = request.header("x-relay-filename").unwrap_or_default();
    let claimed = request
        .header("x-relay-sha256")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !plain_name(form) || !file_name(filename) {
        return reject(
            400,
            "x-relay-form must be a plain name and x-relay-filename a file name, never a path",
        );
    }
    if request.body.is_empty() {
        return reject(400, "empty body");
    }
    let sha256 = hex::encode(Sha256::digest(&request.body));
    if claimed != sha256 {
        return reject(400, "x-relay-sha256 does not match the body");
    }
    match config.failure {
        RelayFailure::Reject => return reject(422, "the mock relay is set to reject"),
        RelayFailure::Unavailable => {
            return (
                503,
                serde_json::json!({ "status": "failed", "error": "the mock relay is set to be unavailable" }),
            );
        }
        RelayFailure::None => {}
    }
    let inbox = config.root.join("inbox").join(form);
    if let Err(error) = std::fs::create_dir_all(&inbox)
        .and_then(|()| std::fs::write(inbox.join(filename), &request.body))
    {
        return (
            503,
            serde_json::json!({ "status": "failed", "error": format!("could not keep the file: {error}") }),
        );
    }
    let forwarded = match &config.mock_bir {
        None => "none",
        Some(mock) => match crate::transport::submit_sftp_target(
            form,
            filename,
            &request.body,
            mock.target(form),
        )
        .await
        {
            Ok(()) => "mock-bir",
            Err(error) => {
                return (
                    502,
                    serde_json::json!({ "status": "failed", "error": format!("mock BIR hand-off failed: {error}") }),
                );
            }
        },
    };
    let receipt_id = format!("relay-{}", &sha256[..16]);
    let receipt = serde_json::json!({
        "status": "accepted",
        "receipt_id": receipt_id,
        "form": form,
        "filename": filename,
        "sha256": sha256,
        "bytes": request.body.len(),
        "forwarded": forwarded,
        "mock": true,
    });
    let receipts = config.root.join("receipts");
    let _ = std::fs::create_dir_all(&receipts).and_then(|()| {
        std::fs::write(
            receipts.join(format!("{receipt_id}.json")),
            receipt.to_string(),
        )
    });
    (202, receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    use crate::transport::{TransportError, submit_sftp_target};

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    async fn relay(failure: RelayFailure) -> (MockRelay, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config = RelayConfig {
            root: dir.path().to_path_buf(),
            failure,
            mock_bir: None,
        };
        (
            MockRelay::start("127.0.0.1:0".parse().unwrap(), config)
                .await
                .unwrap(),
            dir,
        )
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    fn target(relay: &MockRelay) -> ResolvedSftpTarget {
        ResolvedSftpTarget::Relay {
            url: relay.url(),
            folder: "1701Qv2018".into(),
        }
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn an_accepted_file_is_kept_byte_for_byte_with_a_receipt() {
        let (relay, dir) = relay(RelayFailure::None).await;
        let name = "000123453000-1701Qv2018-122025#juana.santos@example.com#.xml";
        submit_sftp_target(
            "1701Qv2018",
            &format!("/tmp/{name}"),
            b"iaf-bytes",
            target(&relay),
        )
        .await
        .unwrap();
        let kept = std::fs::read(dir.path().join("inbox/1701Qv2018").join(name)).unwrap();
        assert_eq!(kept, b"iaf-bytes");
        assert_eq!(
            std::fs::read_dir(dir.path().join("receipts"))
                .unwrap()
                .count(),
            1
        );
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    #[tokio::test]
    async fn a_rejecting_relay_is_a_refusal_and_an_unavailable_or_absent_one_is_unavailable() {
        let (relay, _dir) = relay(RelayFailure::Reject).await;
        let error = submit_sftp_target("1701Qv2018", "a.xml", b"x", target(&relay))
            .await
            .unwrap_err();
        assert!(
            matches!(error, TransportError::RelayRefused(_)),
            "{error:?}"
        );
        assert!(error.relay_did_not_accept());
        let (relay, _dir) = relay_unavailable().await;
        let error = submit_sftp_target("1701Qv2018", "a.xml", b"x", target(&relay))
            .await
            .unwrap_err();
        assert!(
            matches!(error, TransportError::RelayUnavailable(_)),
            "{error:?}"
        );
        assert!(error.relay_did_not_accept());
        // Nothing listening: the connection is refused, so nothing was sent.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let absent = crate::transport::ResolvedSftpTarget::Relay {
            url: format!("http://127.0.0.1:{port}"),
            folder: "1701Qv2018".into(),
        };
        let error = submit_sftp_target("1701Qv2018", "a.xml", b"x", absent)
            .await
            .unwrap_err();
        assert!(
            matches!(error, TransportError::RelayUnavailable(_)),
            "{error:?}"
        );
    }

    #[cfg(any(debug_assertions, feature = "mock-relay"))]
    async fn relay_unavailable() -> (MockRelay, tempfile::TempDir) {
        relay(RelayFailure::Unavailable).await
    }

    #[test]
    fn names_that_are_paths_are_refused() {
        assert!(plain_name("1701Qv2018"));
        assert!(!plain_name("../etc"));
        assert!(!plain_name("a/b"));
        assert!(!plain_name(".hidden"));
        assert!(!plain_name(""));
        assert!(file_name(
            "000123453000-1601Cv2018-102026#juana.santos@example.com#.xml"
        ));
        assert!(!file_name("../x.xml"));
        assert!(!file_name("a/b.xml"));
        assert!(!file_name("a b.xml"));
    }

    #[test]
    fn the_hand_off_target_must_be_loopback() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.env");
        let env = |host: &str| {
            format!(
                "BIR_SFTP_HOST={host}\nBIR_SFTP_PORT=2222\nBIR_SFTP_USERNAME=u\nBIR_SFTP_PASSWORD=p\nBIR_SFTP_HOST_KEY_SHA256=abc\n"
            )
        };
        std::fs::write(&file, env("127.0.0.1")).unwrap();
        assert_eq!(MockBirTarget::from_app_env(&file).unwrap().port, 2222);
        std::fs::write(&file, env("203.0.113.5")).unwrap();
        assert!(MockBirTarget::from_app_env(&file).is_err());
    }
}
