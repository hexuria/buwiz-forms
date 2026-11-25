//! Test helpers: a loopback HTTP server that records every request with its
//! headers, a signed trust list, a fake host sign-in ([`FakeTokens`]) and a
//! signed-in submitter. Nothing here leaves 127.0.0.1.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::client::{ClientError, SubmitConfig, Submitter, TokenSource};
use crate::secrets::{MemoryStore, SecretStore};

/// One request the fake server saw.
#[derive(Clone, Debug)]
pub(crate) struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// An answer: status, extra headers, body.
#[derive(Clone)]
pub(crate) struct Answer {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

impl Answer {
    pub fn json(status: u16, body: &Value) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_string(),
        }
    }

    /// Hangs up without answering: the request was read and recorded.
    pub fn hang_up() -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: String::new(),
        }
    }

    /// Accepts a WebSocket upgrade, sends `frames` as text frames, then
    /// closes the connection.
    pub fn relay(frames: &[&str]) -> Self {
        Self {
            status: 101,
            headers: Vec::new(),
            body: frames.join("\n"),
        }
    }

    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// A loopback server answering each request with `answer`.
pub(crate) fn serve(
    answer: impl Fn(&Seen) -> Answer + Send + 'static,
) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap_or_default();
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let mut headers = Vec::new();
            let mut length = 0;
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                    break;
                }
                if let Some((name, value)) = header.trim().split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                    headers.push((name.trim().to_owned(), value.trim().to_owned()));
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let request = Seen {
                method,
                path,
                headers,
            };
            let reply = answer(&request);
            let key = request.header("sec-websocket-key").map(str::to_owned);
            log.lock().unwrap().push(request);
            if reply.status == 0 {
                continue;
            }
            if reply.status == 101 {
                let accept = key
                    .map(|key| {
                        tokio_tungstenite::tungstenite::handshake::derive_accept_key(key.as_bytes())
                    })
                    .unwrap_or_default();
                let _ = write!(
                    stream,
                    "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: {accept}\r\n\r\n"
                );
                for frame in reply.body.split('\n').filter(|frame| !frame.is_empty()) {
                    let bytes = frame.as_bytes();
                    let mut out = vec![0x81];
                    if bytes.len() < 126 {
                        out.push(bytes.len() as u8);
                    } else {
                        out.push(126);
                        out.extend((bytes.len() as u16).to_be_bytes());
                    }
                    out.extend(bytes);
                    let _ = stream.write_all(&out);
                }
                let _ = stream.flush();
                continue;
            }
            let mut head = format!(
                "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                reply.status,
                reply.body.len()
            );
            for (name, value) in reply.headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            let _ = write!(stream, "{head}\r\n{}", reply.body);
        }
    });
    (format!("http://{addr}"), seen)
}

/// The host's sign-in, as a test drives it: the login, whether tokens are
/// given, and every `access_token` call's `force_refresh`. Its token is `a`,
/// or `a2` when forced.
#[derive(Default)]
pub(crate) struct FakeTokens {
    login: Mutex<Option<String>>,
    refused: std::sync::atomic::AtomicBool,
    asked: Mutex<Vec<bool>>,
    ended: std::sync::atomic::AtomicUsize,
}

impl FakeTokens {
    pub fn new(login: Option<&str>) -> Arc<Self> {
        let tokens = Self::default();
        tokens.sign_in(login);
        Arc::new(tokens)
    }

    /// Who is signed in now (`None`: signed out).
    pub fn sign_in(&self, login: Option<&str>) {
        *self.login.lock().unwrap() = login.map(str::to_owned);
    }

    /// Still a login, but every token request answers `SignedOut` (the
    /// host's refresh failed).
    pub fn refuse_tokens(&self) {
        self.refused
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Each `access_token` call's `force_refresh`, in order.
    pub fn asked(&self) -> Vec<bool> {
        self.asked.lock().unwrap().clone()
    }

    /// How many times the server ended the sign-in.
    pub fn ended(&self) -> usize {
        self.ended.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl TokenSource for FakeTokens {
    fn access_token(
        &self,
        force_refresh: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, ClientError>> + Send + '_>>
    {
        Box::pin(async move {
            self.asked.lock().unwrap().push(force_refresh);
            if self.login.lock().unwrap().is_none()
                || self.refused.load(std::sync::atomic::Ordering::SeqCst)
            {
                return Err(ClientError::SignedOut);
            }
            Ok(if force_refresh { "a2" } else { "a" }.to_owned())
        })
    }

    fn login(&self) -> Option<String> {
        self.login.lock().unwrap().clone()
    }

    fn session_ended(&self) {
        self.ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.sign_in(None);
    }
}

/// A submitter against `url`, signed in with a registered device (`d`) when
/// `signed_in`, pinning `trust_keys`.
pub(crate) fn submitter(url: &str, signed_in: bool, trust_keys: Vec<String>) -> Submitter {
    submitter_tokens(url, signed_in, trust_keys).0
}

/// [`submitter`], with its host sign-in.
pub(crate) fn submitter_tokens(
    url: &str,
    signed_in: bool,
    trust_keys: Vec<String>,
) -> (Submitter, Arc<FakeTokens>) {
    let store = MemoryStore::default();
    if signed_in {
        store.set("device_id", "d").unwrap();
    }
    let config = SubmitConfig {
        server_url: url.to_owned(),
        device_name: "test".into(),
        trust_keys,
    };
    let tokens = FakeTokens::new(signed_in.then_some("juana@example.test"));
    (
        Submitter::new(config, Box::new(store), tokens.clone()),
        tokens,
    )
}

/// The pinned key, a trust list it signed for `hosts`, and the sealing key
/// document the list names.
pub(crate) fn trust_documents(hosts: &[&str]) -> (String, String, String) {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};

    let key = SigningKey::from_bytes(&[7; 32]);
    let public = URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes());
    let kid = crate::device::hex(&Sha256::digest(key.verifying_key().as_bytes())[..8]);
    let sealing = json!({ "kid": "s1", "kem": crate::seal::KEM, "public_key": URL_SAFE_NO_PAD.encode([1; 32]) });
    let claims = json!({
        "hosts": hosts,
        "sealing_keys": [sealing],
        "receipt_keys": [{ "kty": "OKP", "crv": "Ed25519", "kid": kid, "x": public }],
        "iat": 1,
        "exp": 4_102_444_800_i64,
    });
    let header = json!({ "alg": "EdDSA", "kid": kid, "typ": crate::trust::TYP });
    let input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap()),
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    );
    let jws = format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(input.as_bytes()).to_bytes())
    );
    (public, jws, json!({ "keys": [sealing] }).to_string())
}

/// A fixture from `tests/fixtures`, a sample of the public contract (`docs/bir-uptime.md` in `buwiz-com/buwiz-com`).
pub(crate) fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub(crate) fn fixture_json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).expect("fixture JSON")
}

/// A receipt the key of [`trust_documents`] signed, for XML with
/// `xml_sha256` and the ticket `jti`, in `mode` (`live`, `dry_run`).
pub(crate) fn receipt_jws(xml_sha256: &str, jti: &str, mode: &str) -> String {
    receipt_signed_by(&[7; 32], xml_sha256, jti, mode)
}

/// [`receipt_jws`], signed by the key with `seed`: any seed but `[7; 32]` is
/// a key the trust list does not carry.
pub(crate) fn receipt_signed_by(
    seed: &[u8; 32],
    xml_sha256: &str,
    jti: &str,
    mode: &str,
) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};

    let listed = SigningKey::from_bytes(&[7; 32]);
    let kid = crate::device::hex(&Sha256::digest(listed.verifying_key().as_bytes())[..8]);
    let key = SigningKey::from_bytes(seed);
    let status = if mode == "dry_run" { "dry_run" } else { "sent" };
    let claims = json!({
        "sub": "u1", "tin": "000000000000", "form_type": "1601Cv2018", "period": "102026",
        "filename": "f.xml", "xml_sha256": xml_sha256, "payload_sha256": "00",
        "remote_path": "/1601Cv2018/f.xml", "mode": mode, "status": status, "iat": 1, "jti": jti,
    });
    let header = json!({ "alg": "EdDSA", "kid": kid, "typ": crate::receipt::TYP });
    let input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap()),
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
    );
    format!(
        "{input}.{}",
        URL_SAFE_NO_PAD.encode(key.sign(input.as_bytes()).to_bytes())
    )
}
