//! In-process russh SFTP loopback. Adapted from PR #42's evidence harness and
//! wired through production `submit_iaf_with_endpoint`.

use crate::transport::{
    HostKeyPolicy, SftpEndpoint, TransportError, iaf_basename, submit_iaf_with_endpoint,
};
use russh::keys::PrivateKey;
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{
    File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

type Received = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct LoopbackServer {
    received: Received,
}

impl russh::server::Server for LoopbackServer {
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
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.lock().await.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            let ch = self.channels.lock().await.remove(&channel).unwrap();
            session.channel_success(channel)?;
            let sftp = SftpHandler {
                received: self.received.clone(),
                open: HashMap::new(),
            };
            russh_sftp::server::run(ch.into_stream(), sftp).await;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }
}

struct SftpHandler {
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

impl russh_sftp::server::Handler for SftpHandler {
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

async fn spawn_loopback() -> (u16, Received) {
    let received: Received = Arc::new(Mutex::new(HashMap::new()));
    let kp = Ed25519Keypair::from_seed(&[0x42u8; 32]);
    let key = PrivateKey::new(KeypairData::Ed25519(kp), "loopback-test").expect("host key");
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut server = LoopbackServer {
        received: received.clone(),
    };
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
    (port, received)
}

#[tokio::test]
async fn loopback_submit_iaf_with_endpoint_stores_fixture_basename() {
    let (port, received) = spawn_loopback().await;
    let filename = r"C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml";
    let payload = b"ENCRYPTED-IAF-DUMMY-PAYLOAD-\x00\x01\x02\x03";
    let endpoint = SftpEndpoint {
        host: "127.0.0.1".into(),
        port,
        username: "lab".into(),
        password: Zeroizing::new("lab-not-a-bir-secret".into()),
        remote_folder: "1601Cv2018".into(),
        host_key_policy: HostKeyPolicy::AcceptAnyLab,
    };
    submit_iaf_with_endpoint("1601Cv2018", filename, payload, endpoint)
        .await
        .expect("loopback PUT");
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let store = received.lock().await;
    let remote = format!("/1601Cv2018/{}", iaf_basename(filename));
    let got = store
        .get(&remote)
        .unwrap_or_else(|| panic!("server missing {remote}, have {:?}", store.keys()));
    assert_eq!(got.as_slice(), payload);
}

#[tokio::test]
async fn loopback_pinned_host_key_mismatch_refuses_connect() {
    let (port, _received) = spawn_loopback().await;
    let endpoint = SftpEndpoint {
        host: "127.0.0.1".into(),
        port,
        username: "lab".into(),
        password: Zeroizing::new("lab-not-a-bir-secret".into()),
        remote_folder: "1601Cv2018".into(),
        host_key_policy: HostKeyPolicy::PinnedSha256("00".repeat(32)),
    };
    let err = submit_iaf_with_endpoint("1601Cv2018", "file.xml", b"x", endpoint)
        .await
        .expect_err("wrong pin must not PUT");
    assert!(matches!(err, TransportError::Ssh(_)), "{err:?}");
}
