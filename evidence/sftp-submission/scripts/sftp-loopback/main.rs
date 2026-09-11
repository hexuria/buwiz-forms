//! Loopback SFTP proof: an in-process russh SFTP server + the same client
//! pattern Buwiz uses (accept-any host key, password auth, russh-sftp upload),
//! uploading a dummy IAF over 127.0.0.1. Proves connect + auth + upload work
//! end-to-end. No BIR, no real data, no network egress.
//!
//! NOT RUN in the investigation VM: `ring` needs a C compiler (`clang`, absent)
//! and the box had no spare disk; there is also no reachable SSH server there.
//! Runs on any machine with clang + the workspace toolchain. Signatures target
//! russh 0.63 / russh-sftp 3.0 and may need a nudge from the compiler on exact
//! struct field names (e.g. Status/Name/Handle) — verify against those versions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use russh::keys::{Algorithm, PrivateKey};
use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version};

type Received = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Server {
    received: Received,
}

impl russh::server::Server for Server {
    type Handler = SshSession;
    fn new_client(&mut self, _peer: Option<std::net::SocketAddr>) -> SshSession {
        SshSession {
            received: self.received.clone(),
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

struct SshSession {
    received: Received,
    channels: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
}

impl russh::server::Handler for SshSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channels.lock().await.insert(channel.id(), channel);
        Ok(true)
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            let channel = self.channels.lock().await.remove(&channel_id).unwrap();
            session.channel_success(channel_id)?;
            let sftp = SftpSession {
                received: self.received.clone(),
                open: HashMap::new(),
            };
            russh_sftp::server::run(channel.into_stream(), sftp).await;
        } else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }
}

struct SftpSession {
    received: Received,
    open: HashMap<String, (String, Vec<u8>)>,
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

#[async_trait::async_trait]
impl russh_sftp::server::Handler for SftpSession {
    type Error = StatusCode;
    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
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
        _pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let handle = format!("h{id}");
        self.open.insert(handle.clone(), (filename, Vec::new()));
        Ok(Handle { id, handle })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        _offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        if let Some((_, buf)) = self.open.get_mut(&handle) {
            buf.extend_from_slice(&data);
        }
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        if let Some((path, buf)) = self.open.remove(&handle) {
            self.received.lock().await.insert(path, buf);
        }
        Ok(ok(id))
    }
}

struct AcceptAnyHostKey;

impl russh::client::Handler for AcceptAnyHostKey {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true) // == WinSCP GiveUpSecurityAndAcceptAny
    }
}

fn iaf_filename() -> String {
    // 14-digit prefix (9 TIN + 5 branch), matching the live format.
    format!(
        "{}{}{}{}-{}-{}#{}#.xml",
        "000", "000", "000", "00000", "1601Cv2018", "092026", "test@example.com"
    )
}

#[tokio::main]
async fn main() {
    let received: Received = Arc::new(Mutex::new(HashMap::new()));
    let key = PrivateKey::random(&mut rand_core::OsRng, Algorithm::Ed25519).unwrap();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut server = Server { received: received.clone() };
    tokio::spawn(async move {
        loop {
            let (socket, peer) = listener.accept().await.unwrap();
            let handler = server.new_client(Some(peer));
            let cfg = config.clone();
            tokio::spawn(async move {
                let _ = russh::server::run_stream(cfg, socket, handler).await;
            });
        }
    });

    let payload = b"ENCRYPTED-IAF-DUMMY-PAYLOAD-\x00\x01\x02\x03".to_vec();
    let filename = iaf_filename();
    let folder = "1601Cv2018";
    let remote = format!("/{folder}/{filename}");

    let cconfig = Arc::new(russh::client::Config::default());
    let mut handle = russh::client::connect(cconfig, ("127.0.0.1", port), AcceptAnyHostKey)
        .await
        .expect("connect");
    let auth = handle
        .authenticate_password("uploadOnly", "dummy-not-a-real-secret")
        .await
        .expect("auth call");
    assert!(auth.success(), "password auth must succeed");
    println!("auth=ok (connected to 127.0.0.1:{port}, accept-any host key)");

    let channel = handle.channel_open_session().await.expect("channel");
    channel.request_subsystem(true, "sftp").await.expect("subsystem");
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .expect("sftp session");

    let mut file = sftp
        .open_with_flags(
            remote.clone(),
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .expect("open");
    file.write_all(&payload).await.expect("write");
    file.flush().await.expect("flush");
    file.shutdown().await.expect("shutdown");
    let _ = sftp.close().await;
    println!("uploaded {} bytes to {}", payload.len(), remote);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let store = received.lock().await;
    let got = store.get(&remote).expect("server must have stored the file");
    assert_eq!(got, &payload, "server bytes must equal uploaded payload");
    println!(
        "PASS: connect + password auth + accept-any host key + upload verified; \
         server holds {} bytes at {}",
        got.len(),
        remote
    );
}
